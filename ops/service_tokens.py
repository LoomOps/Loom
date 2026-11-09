"""Permanent, individually revocable resident-agent credentials issued by Dashboard Admins."""
import hashlib
import json
import secrets
import time
import uuid

SCOPES = {'read', 'submit', 'cancel', 'events'}


def init(c):
    c.executescript('''
    CREATE TABLE IF NOT EXISTS resident_tokens(
      id TEXT PRIMARY KEY, digest TEXT NOT NULL UNIQUE, label TEXT NOT NULL,
      owner TEXT NOT NULL, scopes TEXT NOT NULL, created REAL NOT NULL,
      created_by TEXT NOT NULL, revoked REAL, revoked_by TEXT);
    ''')


def require_admin(actor):
    identity = actor.get('identity', {})
    if identity.get('provider') != 'dashboard' or identity.get('access_admin') is not True:
        raise PermissionError('Dashboard Admin required')


def eligible_owners(c, actor):
    identity = actor.get('identity', {})
    logins = identity.get('token_owner_logins', [])
    admins = identity.get('token_owner_admins', [])
    eligible = {x.strip().casefold() for x in logins if isinstance(x, str)}
    admin_ids = set()
    for admin in admins:
        if not isinstance(admin, dict): continue
        row = c.execute('SELECT profile_id FROM auth_identities WHERE issuer=? AND subject=?',
                        (admin.get('issuer'), admin.get('subject'))).fetchone()
        if row: admin_ids.add(row[0])
    # The requesting Admin has already passed exact native Admin verification.
    admin_ids.add(actor['id'])
    return [dict(row) for row in c.execute('SELECT id,login,display_name FROM user_profiles ORDER BY login')
            if row['id'] in admin_ids or row['login'].strip().casefold() in eligible]


def listing(c, actor):
    require_admin(actor)
    tokens = [dict(r) for r in c.execute('SELECT id,label,owner,scopes,created,created_by,revoked,revoked_by FROM resident_tokens ORDER BY created DESC')]
    for t in tokens:
        t['scopes'] = json.loads(t['scopes']); t['expires'] = None
    return {'tokens': tokens, 'owners': eligible_owners(c, actor),
            'owner_profiles': [dict(r) for r in c.execute('SELECT id,login,display_name FROM user_profiles ORDER BY login')],
            'expires': None}


def issue(c, actor, body):
    require_admin(actor)
    if not isinstance(body, dict) or set(body) != {'label', 'owner', 'scopes'}:
        raise ValueError('Provide label, owner and scopes')
    label, owner, scopes = body['label'], body['owner'], body['scopes']
    if not isinstance(label, str) or not 1 <= len(label.strip()) <= 80 or any(ord(x) < 32 for x in label):
        raise ValueError('Invalid token label')
    if not isinstance(scopes, list) or not scopes or any(not isinstance(x, str) or x not in SCOPES for x in scopes):
        raise ValueError('Invalid token scopes')
    if not isinstance(owner, str) or not c.execute('SELECT 1 FROM user_profiles WHERE id=?', (owner,)).fetchone():
        raise ValueError('Select a registered Loom user')
    if owner not in {p['id'] for p in eligible_owners(c, actor)}:
        raise ValueError('Select a current Admin or approved member')
    token = 'loom_' + secrets.token_urlsafe(32); key = uuid.uuid4().hex
    c.execute('INSERT INTO resident_tokens VALUES(?,?,?,?,?,?,?,NULL,NULL)',
              (key, hashlib.sha256(token.encode()).hexdigest(), label.strip(), owner,
               json.dumps(sorted(set(scopes))), time.time(), actor['id']))
    return {'credential_id': key, 'token': token, 'owner': owner, 'expires': None}


def revoke(c, actor, body):
    require_admin(actor)
    if not isinstance(body, dict) or set(body) != {'id'} or not isinstance(body['id'], str):
        raise ValueError('Provide token id')
    if not c.execute('SELECT 1 FROM resident_tokens WHERE id=?', (body['id'],)).fetchone():
        raise ValueError('Unknown token')
    c.execute('UPDATE resident_tokens SET revoked=?,revoked_by=? WHERE id=? AND revoked IS NULL',
              (time.time(), actor['id'], body['id']))
    return {'revoked': True, 'id': body['id']}


def authenticate(c, digest):
    row = c.execute('SELECT id,owner,scopes FROM resident_tokens WHERE digest=? AND revoked IS NULL', (digest,)).fetchone()
    if not row: return None
    # Attribution and event recipients stay the selected user; issuance does not
    # grant Dashboard Admin privileges or copy the installation owner credential.
    return {'id': row[1], 'profile_id': row[1], 'scopes': json.loads(row[2]),
            'owners': [row[1]], 'notify': [row[1]], 'credential_id': row[0]}
