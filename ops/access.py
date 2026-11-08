"""Durable scoped credentials and idempotent owner delivery transitions."""
import hashlib
import hmac
import json
import secrets
import time
import uuid
import ipaddress
import base64
from pathlib import Path


def dashboard_actor(config, headers):
    """A short-lived signed identity from the authenticated Dashboard, never browser headers."""
    values = headers.get_all('X-Loom-Dashboard-Identity', [])
    if not values:
        return None
    policy = config.get('dashboard_auth', {})
    if not policy.get('enabled') or len(values) != 1 or len(values[0]) > 8192:
        raise PermissionError('Invalid dashboard identity')
    try:
        payload, signature = values[0].split('.')
        key = Path(policy['relay_key_file']).read_bytes().strip()
        expected = hmac.new(key, payload.encode(), hashlib.sha256).hexdigest()
        if len(key) < 32 or not hmac.compare_digest(expected, signature):
            raise ValueError()
        claims = json.loads(base64.urlsafe_b64decode(payload + '=' * (-len(payload) % 4)))
        person = claims['identity']
        if not isinstance(person,dict):raise ValueError()
        if claims['aud'] != 'loom-dashboard-relay' or not time.time() < claims['exp'] <= time.time()+60:
            raise ValueError()
        if person['issuer'] != policy['issuer'] or not isinstance(person['subject'], str) or not person['subject']:
            raise ValueError()
        login = person.get('login')
        link = policy.get('link_tailscale_email') is True and isinstance(login,str) and bool(login.strip())
        principal = ('ts-' + hashlib.sha256(login.strip().casefold().encode()).hexdigest()[:24] if link else
                     'identity-' + hashlib.sha256((person['issuer']+'\0'+person['subject']).encode()).hexdigest()[:24])
        binding = config.get('tailscale_auth',{}).get('users',{}).get(login.strip().casefold()) if link else None
        actor = next((a for a in config['actors'] if a['id']==binding),None) if binding else None
        if binding and actor is None:
            raise ValueError()
        actor = actor or {'id':principal,'scopes':['read','submit','cancel','events'],'owners':[principal],'notify':[principal]}
        return {**actor, 'identity':{**person,'provider':'dashboard','link_tailscale_email':link,'profile_id':principal}}
    except (ValueError, TypeError, KeyError, OSError):
        raise PermissionError('Invalid dashboard identity') from None


def tailscale_actor(config, headers, peer):
    """Accept Serve identity only from explicitly trusted ingress addresses.

    The ingress and all processes able to reach its private backend are part of
    the trust boundary. Tailnet organization roles are deliberately not inferred.
    """
    policy = config.get('tailscale_auth', {})
    if not policy.get('enabled'):
        return None
    try:
        trusted = any(ipaddress.ip_address(peer) == ipaddress.ip_address(address)
                      for address in policy.get('trusted_proxies', []))
    except ValueError:
        trusted = False
    if not trusted:
        return None
    logins = headers.get_all('Tailscale-User-Login', [])
    if not logins:
        return None  # Tagged devices have no user identity; they need credentials.
    if len(logins) != 1 or not logins[0].strip() or len(logins[0]) > 320:
        raise PermissionError('Invalid Tailscale identity')
    login = logins[0].strip().casefold()
    bindings = {key.casefold(): value for key, value in policy.get('users', {}).items()}
    principal = bindings.get(login)
    if principal:
        actor = next((a for a in config['actors'] if a['id'] == principal), None)
        if actor is None:
            raise PermissionError('Tailscale principal is not configured')
    elif policy.get('allow_new_users') is True:
        principal = 'ts-' + hashlib.sha256(login.encode()).hexdigest()[:24]
        actor = {'id': principal, 'scopes': ['read', 'submit', 'cancel', 'events'],
                 'owners': [principal], 'notify': [principal]}
    else:
        raise PermissionError('Tailscale user is not registered with Loom')
    return {**actor, 'identity': {'provider': 'tailscale', 'login': login,
                                'name': headers.get('Tailscale-User-Name', '')[:256],
                                'tailnet_role': None}}


def init(c):
    c.executescript("""
    CREATE TABLE IF NOT EXISTS principals(
      id TEXT PRIMARY KEY, kind TEXT NOT NULL, grant_json TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS credentials(
      id TEXT PRIMARY KEY, principal TEXT NOT NULL REFERENCES principals(id),
      digest TEXT NOT NULL UNIQUE, expires REAL NOT NULL, revoked REAL);
    CREATE TABLE IF NOT EXISTS delivery(
      event INTEGER PRIMARY KEY REFERENCES events(seq), state TEXT NOT NULL,
      receipt TEXT, version INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE IF NOT EXISTS delivery_history(
      event INTEGER NOT NULL REFERENCES events(seq), version INTEGER NOT NULL,
      state TEXT NOT NULL, receipt TEXT NOT NULL, recorded REAL NOT NULL,
      PRIMARY KEY(event,version));
    """)


def authenticate(c, bootstrap, digest):
    for actor in bootstrap:
        if hmac.compare_digest(actor['token_sha256'],digest):
            return actor
    row=c.execute("""SELECT p.id,p.grant_json FROM credentials t JOIN principals p ON p.id=t.principal
        WHERE t.digest=? AND t.revoked IS NULL AND t.expires>?""",(digest,time.time())).fetchone()
    if row:
        return {'id':row[0],**json.loads(row[1])}
    raise PermissionError('Invalid or expired credential')


def issue(c,principal,kind,grant,ttl):
    if kind not in ('user','service') or type(ttl) is not int or not 60<=ttl<=86400:
        raise ValueError('Invalid principal kind or TTL')
    c.execute('INSERT OR IGNORE INTO principals VALUES(?,?,?)',(principal,kind,json.dumps(grant,sort_keys=True)))
    if tuple(c.execute('SELECT kind,grant_json FROM principals WHERE id=?',(principal,)).fetchone())!=(kind,json.dumps(grant,sort_keys=True)):
        raise ValueError('Existing principal grant differs')
    token=secrets.token_urlsafe(32)
    ident=uuid.uuid4().hex
    c.execute('INSERT INTO credentials VALUES(?,?,?,?,NULL)',
              (ident,principal,hashlib.sha256(token.encode()).hexdigest(),time.time()+ttl))
    return {'credential_id':ident,'token':token}


def delivery(c,actor,event,state,receipt,version):
    if type(event) is not int or type(version) is not int or version<0:
        raise ValueError('Invalid event or delivery version')
    row=c.execute('SELECT target FROM events WHERE seq=?',(event,)).fetchone()
    if not row or actor['id']!=row[0]:
        raise PermissionError('Event outside grant')
    transitions={'queued':{'sent','send_failed'},'send_failed':{'sent','send_failed'},
                 'sent':{'native_input'},'native_input':{'claimed'},
                 'claimed':{'progress','completed'},'progress':{'progress','completed'}}
    c.execute('INSERT OR IGNORE INTO delivery(event,state) VALUES(?,?)',(event,'queued'))
    old=c.execute('SELECT state,receipt,version FROM delivery WHERE event=?',(event,)).fetchone()
    previous=c.execute('SELECT state,receipt FROM delivery_history WHERE event=? AND version=?',
                       (event,version+1)).fetchone()
    if previous and tuple(previous)==(state,receipt):
        return {'event':event,'state':state,'version':version+1}
    if old[2]==version+1 and old[0]==state and old[1]==receipt:
        return {'event':event,'state':state,'version':old[2]}
    if old[2]!=version or state not in transitions.get(old[0],()) or not isinstance(receipt,str) or not 1<=len(receipt)<=512:
        raise ValueError('Invalid delivery transition')
    c.execute('UPDATE delivery SET state=?,receipt=?,version=version+1 WHERE event=? AND version=?',
              (state,receipt,event,version))
    c.execute('INSERT INTO delivery_history VALUES(?,?,?,?,?)',(event,version+1,state,receipt,time.time()))
    return {'event':event,'state':state,'version':version+1}
