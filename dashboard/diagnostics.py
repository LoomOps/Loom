"""Private bounded local diagnostics. No request payloads, messages or network export."""
import fcntl
import json
import os
from pathlib import Path
import re
import stat
import threading
import time
import traceback
import uuid

SCHEMA = 'loom-diagnostic/v1'
MAX_BYTES = 64 * 1024 * 1024
BACKUPS = 15
_seen = {}
_guard = threading.Lock()


def retention():
    """Invalid overrides fall back without disabling incident collection."""
    def integer(name, default, minimum, scale=1):
        try:
            value=int(os.environ[name])
            return value*scale if value>=minimum else default
        except (KeyError, ValueError):return default
    return (integer('LOOM_DIAGNOSTICS_SEGMENT_MIB',MAX_BYTES,1,1024*1024),
            integer('LOOM_DIAGNOSTICS_BACKUPS',BACKUPS,0))


def label(value):
    return value if isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_.:-]{1,96}', value) else 'unknown'


def record(component, operation, error=None, *, code='unexpected_error', directory=None, build='unknown'):
    """Fail open, including read-only/full disks and busy cross-process writers."""
    try:
        root = Path(directory or os.environ.get('LOOM_DIAGNOSTICS_DIR', '/loom/ops/diagnostics'))
        if not root.is_absolute() or any(p.is_symlink() for p in (root, *root.parents)):
            return None
        root.mkdir(parents=True, mode=0o700, exist_ok=True)
        info = root.stat()
        if info.st_uid != os.getuid() or info.st_mode & 0o077:
            return None
        if build=='unknown':
            try:build=json.loads(Path(__file__).with_name('build-info.json').read_text()).get('build_id','unknown')
            except (OSError,ValueError):pass
        event = {'schema': SCHEMA, 'id': uuid.uuid4().hex, 'at': time.time(),
                 'component': label(component), 'operation': label(operation), 'code': label(code),
                 'build': label(build), 'exception': label(type(error).__name__) if error else None,
                 'errno': error.errno if isinstance(error, OSError) else None,
                 'frames': [{'file': label(Path(f.filename).name), 'function': label(f.name), 'line': f.lineno}
                            for f in traceback.extract_tb(error.__traceback__)[-12:]] if error else []}
        # No exception message, locals, source lines, URLs, headers, IDs or arbitrary context.
        key = (event['component'], event['operation'], event['code'], event['exception'], event['errno'])
        now = time.monotonic()
        with _guard:
            if now - _seen.get(key, -100) < 30:
                return None
            if len(_seen) >= 256:
                _seen.clear()
            fd = os.open(root / '.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'a') as lock:
                lock_info=os.fstat(lock.fileno())
                if lock_info.st_uid != os.getuid() or not stat.S_ISREG(lock_info.st_mode) or lock_info.st_nlink!=1:return None
                os.fchmod(lock.fileno(),0o600)
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                target = root / 'events.jsonl'
                if target.is_symlink():return None
                payload = (json.dumps(event, separators=(',', ':')) + '\n').encode()
                max_bytes,backups=retention()
                if target.exists() and target.stat().st_size + len(payload) > max_bytes:
                    if backups==0:target.unlink()
                    for i in range(backups, 0, -1):
                        older = root / f'events.{i}.jsonl'
                        previous = root / f'events.{i-1}.jsonl' if i > 1 else target
                        if older.is_symlink() or previous.is_symlink():return None
                        if i == backups:older.unlink(missing_ok=True)
                        if previous.exists():os.replace(previous, older)
                fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, 'ab') as out:
                    st = os.fstat(out.fileno())
                    if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or st.st_nlink != 1:return None
                    os.fchmod(out.fileno(), 0o600)
                    out.write(payload)
                _seen[key] = now
        return event['id']
    except Exception:
        return None


def install_hooks(component, *, directory=None, build='unknown'):
    """Retain normal exception behavior; add private evidence for uncaught failures."""
    import sys
    old = sys.excepthook
    def uncaught(kind, error, tb):
        record(component, 'uncaught', error, directory=directory, build=build)
        old(kind, error, tb)
    sys.excepthook = uncaught
    prior = threading.excepthook
    def threaded(args):
        record(component, 'thread', args.exc_value, directory=directory, build=build)
        prior(args)
    threading.excepthook = threaded


class ServerMixin:
    def handle_error(self, request, client_address):
        import sys
        record('http-server', 'handler', sys.exc_info()[1])
        super().handle_error(request, client_address)
