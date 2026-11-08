"""Machine-scoped access requests. No Jobs, public data relay or implicit approval."""
import fcntl
import hashlib
import ipaddress
import json
import re
import sqlite3
import threading
import time
import uuid
from contextlib import contextmanager
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import quote, urlencode
from urllib.request import Request, build_opener
from sso import NoRedirect, validate_url


class RemoteError(Exception):
    def __init__(self, status):
        self.status = status
        super().__init__('tailscale_api_' + str(status))


class TailnetAPI:
    """Credential stays server-side. Redirects and API response bodies never leak."""
    def __init__(self, credential_file):
        self.path = Path(credential_file)
        self.access = None
        self.until = 0

    def wire(self, method, url, data=None, headers=None):
        try:
            with build_opener(NoRedirect).open(Request(url, data=data, headers=headers or {}, method=method), timeout=15) as r:
                raw = r.read(4*1024*1024+1)
                if len(raw) > 4*1024*1024: raise RemoteError(502)
                return (json.loads(raw) if raw else {}, dict(r.headers))
        except HTTPError as e:
            raise RemoteError(e.code) from None

    def token(self):
        c = json.loads(self.path.read_text())
        if c.get('type') == 'api': return c['token']
        if c.get('type') != 'oauth': raise ValueError('invalid_management_credential')
        if not self.access or self.until <= time.time():
            r, _ = self.wire('POST', 'https://api.tailscale.com/api/v2/oauth/token',
                urlencode({'grant_type':'client_credentials','client_id':c['client_id'],
                           'client_secret':c['client_secret']}).encode(),
                {'Content-Type':'application/x-www-form-urlencoded'})
            self.access = r['access_token']; self.until = time.time()+int(r['expires_in'])-60
        return self.access

    def call(self, method, path, value=None, headers=None):
        return self.wire(method, 'https://api.tailscale.com/api/v2'+path,
            json.dumps(value).encode() if value is not None else None,
            {'Authorization':'Bearer '+self.token(), 'Accept':'application/json',
             'Content-Type':'application/json', **(headers or {})})


def validate(config):
    if config.get('enrollment_mode','sharing') != 'sharing':raise ValueError('Only machine Sharing enrollment is supported')
    for name in ('sharing_hourly_limit','sharing_pending_limit'):
        if name in config and (type(config[name]) is not int or not 1<=config[name]<=10000):raise ValueError('invalid_sharing_limit')
    for k in ('origin','dashboard_origin'): validate_url(config[k], origin=True)
    if config['origin'] == config['dashboard_origin']: raise ValueError('public_and_private_origins_must_differ')
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}',config['machine_id']): raise ValueError('invalid_machine_id')
    if not config.get('tailnet') or config['tailnet'] == '-': raise ValueError('explicit_tailnet_required')
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}',config['device_id']): raise ValueError('invalid_device_id')
    if not config.get('admins') or any(set(a) != {'issuer','subject'} or not a['subject'] for a in config['admins']):
        raise ValueError('explicit_admin_subjects_required')
    ports=config.get('ports',[])
    if not ports or any(type(p) is not int or not 1 <= p <= 65535 for p in ports): raise ValueError('invalid_ports')
    for address in config['addresses']: ipaddress.ip_address(address)
    if not config['addresses']: raise ValueError('machine_addresses_required')
    if any(not re.fullmatch(r'[a-z_][a-z0-9_-]{0,31}',a) or a=='root' for a in config.get('ssh_accounts',[])):
        raise ValueError('explicit_nonroot_ssh_accounts_required')
    return config


def make_admission(config,api=None):
    from sharing import SharingAdmission
    return SharingAdmission(config,api)


def group_name(machine, device):
    return 'group:loom-access-'+hashlib.sha256((machine+'\0'+device).encode()).hexdigest()[:16]


def machine_tag(config):
    return 'tag:loom-machine-'+hashlib.sha256(config['device_id'].encode()).hexdigest()[:12]




class Admission:
    def __init__(self, config, api=None):
        self.config=validate(config)
        self.api=api or TailnetAPI(config['credential_file'])
        self.path=Path(config['database']);self.path.parent.mkdir(parents=True,exist_ok=True)
        self.lock=threading.RLock()
        with self.db() as c:
            c.executescript('''CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY, request TEXT, action TEXT, actor TEXT, at REAL);
              CREATE TABLE IF NOT EXISTS policy_backups(id INTEGER PRIMARY KEY, request TEXT, etag TEXT, body TEXT, at REAL);''')
        self.path.chmod(0o600)

    @contextmanager
    def db(self):
        c=sqlite3.connect(self.path,timeout=15);c.row_factory=sqlite3.Row
        try:
            with c: yield c
        finally: c.close()

    def is_admin(self, user):
        return any(user.get('issuer')==a['issuer'] and user.get('subject')==a['subject'] for a in self.config['admins'])






    @property
    def tailpath(self): return '/tailnet/'+quote(self.config['tailnet'],safe='')



    def check_device(self):
        device,_=self.api.call('GET','/device/'+quote(self.config['device_id'],safe=''))
        if not set(self.config['addresses']).issubset(device.get('addresses',[])): raise ValueError('machine_identity_changed')
        return device




    def run(self, stopped):
        while not stopped.is_set():
            try:self.tick()
            except (OSError,sqlite3.Error):
                # Temporary storage contention must not permanently kill reconciliation.
                import logging
                logging.error('Access request storage unavailable; reconciliation will retry')
            stopped.wait(15)
