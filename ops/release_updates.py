"""Cached GitHub Release discovery. No Job DB, downloads or deployment side effects."""
import copy
import json
import os
from pathlib import Path
import re
import threading
import time
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, build_opener, HTTPRedirectHandler

DEFAULT_REPOSITORY = 'LoomOps/Loom'
TOKEN_FILE = '/etc/loom/releases/github-token'
MAX_BYTES = 4 * 1024 * 1024
VERSION = re.compile(r'v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(alpha|beta|rc)\.(0|[1-9]\d*))?(?:\+[0-9A-Za-z.-]+)?$')
CURRENT = None


def version(value):
    match = VERSION.fullmatch(value) if isinstance(value, str) else None
    if not match:
        return None
    major, minor, patch, stage, number = match.groups()
    return (int(major), int(minor), int(patch), {'alpha': 0, 'beta': 1, 'rc': 2, None: 3}[stage], int(number or 0))


def settings(build, environ=None):
    env = os.environ if environ is None else environ
    repository = env.get('LOOM_RELEASE_REPOSITORY', DEFAULT_REPOSITORY)
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,99}/[A-Za-z0-9][A-Za-z0-9_.-]{0,99}', repository):
        raise ValueError('Invalid release repository')
    installed = build.get('image_build_id') or build.get('build_id', 'unversioned')
    parsed = version(installed)
    channel = env.get('LOOM_RELEASE_CHANNEL') or ('stable' if parsed and parsed[3] == 3 else 'alpha')
    if channel not in ('stable', 'alpha', 'beta', 'rc'):
        raise ValueError('Invalid release channel')
    interval = int(env.get('LOOM_RELEASE_INTERVAL', '21600'))
    if not 300 <= interval <= 604800:
        raise ValueError('Release interval must be 300–604800 seconds')
    enabled = env.get('LOOM_RELEASE_CHECK_ENABLED', '1')
    if enabled not in ('0', '1'):
        raise ValueError('Release check flag must be 0 or 1')
    return {'repository': repository, 'channel': channel, 'interval': interval,
            'enabled': enabled == '1', 'installed': installed}


def select_release(rows, channel, repository):
    rank = {'alpha': 0, 'beta': 1, 'rc': 2, 'stable': 3}[channel]
    candidates = []
    for row in rows:
        if not isinstance(row, dict) or row.get('draft') or not row.get('published_at'):
            continue
        tag = row.get('tag_name', '')
        parsed = version(tag)
        if not parsed or parsed[3] < rank or (channel == 'stable' and row.get('prerelease')):
            continue
        # Construct a repository-bound link; never expose an arbitrary upstream URL.
        value={'tag': tag, 'url': 'https://github.com/' + repository + '/releases/tag/' + quote(tag, safe=''),
               'published_at': row['published_at']}
        if any(isinstance(a,dict) and a.get('name')=='upgrade.json' for a in row.get('assets',[])):
            value['upgrade']={'descriptor_url':'https://github.com/'+repository+'/releases/download/'+quote(tag,safe='')+'/upgrade.json',
                              'instructions_url':'https://github.com/'+repository+'/blob/'+quote(tag,safe='')+'/docs/UPGRADE.md'}
        candidates.append((parsed,value))
    return max(candidates, key=lambda pair: pair[0])[1] if candidates else None


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None  # Never forward a private repository credential to another origin.


class CheckError(Exception):
    def __init__(self, code, retry_at=0):
        self.code, self.retry_at = code, retry_at


def fetch_page(repository, page, token, cached, now, opener=None):
    headers = {'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'Loom-release-check'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    if cached.get('etag'):
        headers['If-None-Match'] = cached['etag']
    request = Request(f'https://api.github.com/repos/{repository}/releases?per_page=100&page={page}', headers=headers)
    try:
        with (opener or build_opener(NoRedirect())).open(request, timeout=10) as response:
            body = response.read(MAX_BYTES + 1)
            if len(body) > MAX_BYTES:
                raise CheckError('response_too_large')
            rows = json.loads(body)
            if not isinstance(rows, list):
                raise CheckError('invalid_response')
            return {'etag': response.headers.get('ETag'), 'rows': rows,
                    'next': bool(re.search(r'rel="next"', response.headers.get('Link', '')))}
    except HTTPError as error:
        if error.code == 304 and 'rows' in cached:
            return cached
        if error.code == 429 or (error.code == 403 and (error.headers.get('X-RateLimit-Remaining') == '0' or error.headers.get('Retry-After'))):
            try:
                retry = max(now + 60, now + float(error.headers.get('Retry-After', '0')),
                            float(error.headers.get('X-RateLimit-Reset', '0')))
            except ValueError:
                retry = now + 300
            raise CheckError('rate_limited', retry) from None
        raise CheckError({401: 'credential_rejected', 403: 'access_denied', 404: 'repository_unavailable',
                          301: 'repository_moved', 302: 'repository_moved', 307: 'repository_moved'}.get(error.code, 'github_unavailable')) from None
    except (URLError, TimeoutError, OSError):
        raise CheckError('network_unavailable') from None
    except (ValueError, TypeError):
        raise CheckError('invalid_response') from None


class Checker:
    def __init__(self, config, cache, token_file=TOKEN_FILE, fetch=fetch_page, clock=time.time):
        self.config, self.cache, self.token_file = config, Path(cache), Path(token_file)
        self.fetch, self.clock = fetch, clock
        self.lock = threading.Lock()
        self.wake, self.stop = threading.Event(), threading.Event()
        self.thread = None
        self.busy = self.pending = False
        self.pages = {}
        self.state = {'checked_at': None, 'last_success_at': None, 'next_check_at': 0, 'retry_at': 0,
                      'latest': None, 'error': None, 'failures': 0}
        try:
            saved = json.loads(self.cache.read_text())
            if saved['repository'] == config['repository'] and saved['channel'] == config['channel']:
                # Only our format is eligible for conditional requests after restart.
                state = saved['state']; pages = saved['pages']
                if (set(state) == set(self.state) and isinstance(pages, dict)
                        and all(isinstance(state[k], (int, float)) for k in ('next_check_at', 'retry_at', 'failures'))
                        and all(state[k] is None or isinstance(state[k], (int, float)) for k in ('checked_at','last_success_at'))
                        and (state['latest'] is None or isinstance(state['latest'],dict) and version(state['latest'].get('tag')))
                        and all(isinstance(p, dict) and isinstance(p.get('rows'), list) for p in pages.values())):
                    self.state = state; self.pages = pages
        except (OSError, ValueError, KeyError, TypeError):
            pass

    def start(self):
        if self.config['enabled'] and self.thread is None:
            self.thread = threading.Thread(target=self._loop, name='loom-release-check', daemon=True)
            self.thread.start()

    def close(self):
        self.stop.set(); self.wake.set()
        if self.thread:
            self.thread.join(timeout=11)

    def status(self):
        with self.lock:
            state = copy.deepcopy(self.state)
            checking = self.busy or self.pending
        latest = state['latest']
        installed = version(self.config['installed'])
        available = (version(latest['tag']) > installed) if latest and installed else None
        if not self.config['enabled']:
            result = 'disabled'
        elif checking:
            result = 'checking'
        elif state['error']:
            result = 'error'
        elif state['last_success_at'] is None:
            result = 'not_checked'
        elif not latest:
            result = 'no_release'
        elif not installed:
            result = 'unversioned'
        else:
            result = 'update_available' if available else 'up_to_date'
        return {'schema': 'loom-release-check/v1', **self.config, **{k: v for k, v in state.items() if k != 'failures'},
                'status': result, 'checking': checking, 'update_available': available,
                'stale': bool(state['last_success_at'] is not None and (state['error'] or self.clock() > state['next_check_at'])),
                'automatic_install': False}

    def request(self):
        with self.lock:
            now = self.clock()
            reason = ('disabled' if not self.config['enabled'] else 'not_running' if self.thread is None
                      else 'checking' if self.busy or self.pending else 'cooldown'
                      if now < max(self.state['retry_at'], (self.state['checked_at'] or 0) + 60) else None)
            if reason is None:
                self.pending = True; self.wake.set()
        return {'accepted': reason is None, 'reason': reason or 'queued', 'updates': self.status()}

    def _loop(self):
        while not self.stop.is_set():
            with self.lock:
                due = self.pending or self.clock() >= self.state['next_check_at']
            if due:
                self.check()
            self.wake.wait(30); self.wake.clear()

    def check(self):
        with self.lock:
            if self.busy or not self.config['enabled']:
                return
            self.busy = True; self.pending = False
            old_pages = self.pages.copy()
        now = self.clock()
        error = None; retry = 0; latest = None; pages = {}
        try:
            try:
                token = self.token_file.read_text().strip() if self.token_file.exists() else ''
                if token and (len(token) > 4096 or any(c.isspace() for c in token)):
                    raise CheckError('credential_invalid')
            except OSError:
                raise CheckError('credential_unreadable') from None
            rows = []
            for page in range(1, 11):
                if self.stop.is_set():
                    raise CheckError('interrupted')
                item = self.fetch(self.config['repository'], page, token, old_pages.get(str(page), {}), now)
                pages[str(page)] = item; rows.extend(item['rows'])
                if not item['next']:
                    break
            else:
                raise CheckError('release_list_incomplete')
            latest = select_release(rows, self.config['channel'], self.config['repository'])
        except CheckError as failure:
            error, retry = failure.code, failure.retry_at
        except Exception:
            error = 'invalid_response'  # No upstream text or credential in status/logs.
        with self.lock:
            self.state['checked_at'] = now
            self.state['error'] = error
            self.state['failures'] = self.state['failures'] + 1 if error else 0
            self.state['retry_at'] = max(retry, now + min(3600, 60 * 2 ** min(self.state['failures'], 6))) if error else now + 60
            self.state['next_check_at'] = max(self.state['retry_at'], now + self.config['interval'])
            if not error:
                self.state['latest'] = latest; self.state['last_success_at'] = now; self.pages = pages
            saved = {'repository': self.config['repository'], 'channel': self.config['channel'], 'state': self.state.copy(), 'pages': self.pages}
        try:
            self.cache.parent.mkdir(parents=True, exist_ok=True)
            pending = self.cache.with_suffix('.tmp')
            with open(pending, 'w', opener=lambda path, flags: os.open(path, flags | os.O_NOFOLLOW, 0o600)) as stream:
                json.dump(saved, stream)
            pending.replace(self.cache)
        except OSError:
            with self.lock:
                self.state['error'] = 'cache_unwritable'
        finally:
            with self.lock:
                self.busy = False


def status():
    return CURRENT.status() if CURRENT else {'status': 'not_running', 'automatic_install': False}


def request():
    return CURRENT.request() if CURRENT else {'accepted': False, 'reason': 'not_running', 'updates': status()}


def start(build, root):
    global CURRENT
    try:
        config = settings(build)
    except (ValueError,TypeError):
        CURRENT = Checker(settings(build, {}), Path(root) / 'release-check-invalid.json')
        CURRENT.state['error'] = 'configuration_invalid'
        return CURRENT
    CURRENT = Checker(config, Path(root) / 'release-check.json')
    CURRENT.start()
    return CURRENT
