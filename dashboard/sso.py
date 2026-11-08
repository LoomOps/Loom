"""Local sessions bound to the current trusted Tailscale ingress identity.

The issuer is a stable namespace; it is never fetched.
"""
import base64,hashlib,hmac,json,secrets,sqlite3,time
from contextlib import contextmanager
from email.header import decode_header,make_header
from http.cookies import SimpleCookie
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler

class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def cookie_value(headers, name):
    cookie = SimpleCookie()
    try:
        cookie.load(headers.get('Cookie', ''))
        return cookie[name].value if name in cookie else ''
    except Exception:
        return ''


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def validate_url(value, *, origin=False):
    u = urlsplit(value)
    if u.scheme != 'https' or not u.hostname or u.username or u.password or u.query or u.fragment or (origin and u.path not in ('', '/')):
        raise ValueError('Identity requires credential-free HTTPS URLs')
    return value.rstrip('/')


class SSO:
    # Session cookie and DB locations are stable installation state.
    cookie='__Host-loom-session'
    flow_cookie='__Host-loom-login'

    def __init__(self,config):
        self.config={**config,'provider':'tailscale','mode':'network'}
        validate_url(config['issuer']);self.issuer=config['issuer']
        self.origin=validate_url(config['origin'],origin=True)
        self.ttl=config.get('session_seconds',604800)
        if type(self.ttl) is not int or not 60<=self.ttl<=604800:raise ValueError('Invalid session lifetime')
        self.users=config.get('network_users',{})
        if not isinstance(self.users,dict) or any(not isinstance(k,str) or '@' not in k or not isinstance(v,dict) or not v.get('subject') for k,v in self.users.items()):raise ValueError('Invalid native user bindings')
        self.users={k.strip().casefold():v for k,v in self.users.items()}
        self.key=Path(config['relay_key_file']).read_bytes().strip()
        if len(self.key)<32:raise ValueError('Dashboard relay key must have at least 32 bytes')
        self.path=Path(config['session_db']);self.path.parent.mkdir(parents=True,exist_ok=True)
        with self.db() as c:c.execute('CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,identity TEXT NOT NULL,csrf TEXT NOT NULL,expires REAL NOT NULL)')
        self.path.chmod(0o600)

    @contextmanager
    def db(self):
        c=sqlite3.connect(self.path,timeout=10)
        try:
            with c:yield c
        finally:c.close()

    def set_cookie(self,name,value,ttl):
        return f'{name}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={ttl}'

    def authorize(self,network,admission=None):
        login=str(network.get('Tailscale-User-Login') or '').strip().casefold()
        if not login:return None
        name=str(make_header(decode_header(network.get('Tailscale-User-Name') or login)))[:256]
        binding=self.users.get(login)
        if binding:return {'issuer':self.issuer,'subject':binding['subject'],'login':login,'name':name,'authentication':'tailscale-native'}
        identity=admission.authorize_network(network) if admission and hasattr(admission,'authorize_network') else None
        return {**identity,'issuer':self.issuer} if identity else None

    def issue(self,identity,headers):
        if identity.get('issuer')!=self.issuer or identity.get('authentication') not in ('tailscale-native','tailscale-sharing'):raise ValueError('Verified network identity required')
        token,csrf=secrets.token_urlsafe(32),secrets.token_urlsafe(32)
        with self.db() as c:
            c.execute('DELETE FROM sessions WHERE expires<=? OR id=?',(time.time(),digest(cookie_value(headers,self.cookie))))
            c.execute('INSERT INTO sessions VALUES(?,?,?,?)',(digest(token),json.dumps(identity),csrf,time.time()+self.ttl))
        return self.set_cookie(self.cookie,token,self.ttl)

    def session(self,headers):
        with self.db() as c:row=c.execute('SELECT identity,csrf,expires FROM sessions WHERE id=? AND expires>?',(digest(cookie_value(headers,self.cookie)),time.time())).fetchone()
        return {'user':json.loads(row[0]),'csrf':row[1],'expires':row[2]} if row else None

    def sharing_session(self,identity,headers):
        if identity.get('authentication')!='tailscale-sharing' or not identity.get('share_request'):raise ValueError('approved_share_required')
        return self.issue({**identity,'issuer':self.issuer},headers)

    def logout(self,headers):
        with self.db() as c:c.execute('DELETE FROM sessions WHERE id=?',(digest(cookie_value(headers,self.cookie)),))
        return self.set_cookie(self.cookie,'',0)

    def assertion(self,identity):
        payload=base64.urlsafe_b64encode(json.dumps({'identity':identity,'aud':'loom-dashboard-relay','exp':int(time.time())+30},separators=(',',':')).encode()).decode().rstrip('=')
        return payload+'.'+hmac.new(self.key,payload.encode(),hashlib.sha256).hexdigest()
