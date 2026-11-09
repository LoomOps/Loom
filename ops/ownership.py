"""Current item responsibility, separate from immutable execution provenance."""
import hashlib
import json
import time


def init(c):
    c.executescript('''
    CREATE TABLE IF NOT EXISTS auth_identities(
      issuer TEXT NOT NULL, subject TEXT NOT NULL, profile_id TEXT NOT NULL,
      PRIMARY KEY(issuer,subject));
    CREATE TABLE IF NOT EXISTS user_profiles(
      id TEXT PRIMARY KEY, provider TEXT NOT NULL, login TEXT NOT NULL,
      display_name TEXT NOT NULL, updated REAL NOT NULL);
    CREATE TABLE IF NOT EXISTS item_owners(
      kind TEXT NOT NULL, id TEXT NOT NULL, owner TEXT NOT NULL,
      updated REAL NOT NULL, updated_by TEXT NOT NULL, PRIMARY KEY(kind,id));
    CREATE TABLE IF NOT EXISTS ownership_history(
      seq INTEGER PRIMARY KEY, kind TEXT NOT NULL, id TEXT NOT NULL,
      previous_owner TEXT, owner TEXT NOT NULL, changed_by TEXT NOT NULL, changed REAL NOT NULL);
    ''')


def register_identity(c, info):
    prior=c.execute('SELECT profile_id FROM auth_identities WHERE issuer=? AND subject=?',
                    (info['issuer'],info['subject'])).fetchone()
    identity=prior[0] if prior else info['profile_id']
    login=info.get('login') or identity
    name=info.get('name') or login
    row=c.execute('SELECT display_name FROM user_profiles WHERE id=?',(identity,)).fetchone()
    if not row:
        c.execute('INSERT INTO user_profiles VALUES(?,?,?,?,?)',(identity,'tailscale' if identity.startswith('ts-') else 'dashboard',login,name,time.time()))
    elif row[0]!=name:
        c.execute('UPDATE user_profiles SET display_name=?,updated=? WHERE id=?',(name,time.time(),identity))
    if not prior:
        c.execute('INSERT INTO auth_identities VALUES(?,?,?)',(info['issuer'],info['subject'],identity))
    return profile(c,identity)


def register(c, login, name, provider='tailscale'):
    login=login.strip().casefold()
    identity='ts-'+hashlib.sha256(login.encode()).hexdigest()[:24] if provider=='tailscale' else login
    existing=c.execute('SELECT display_name FROM user_profiles WHERE id=?',(identity,)).fetchone()
    if existing and existing[0]==(name or login):return profile(c,identity)
    c.execute('''INSERT INTO user_profiles VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET
        display_name=excluded.display_name,updated=excluded.updated''',
        (identity,provider,login,name or login,time.time()))
    return profile(c,identity)


def profile(c, identity):
    row=c.execute('SELECT id,provider,login,display_name FROM user_profiles WHERE id=?',(identity,)).fetchone()
    return dict(row) if row else {'id':identity,'provider':'local','login':None,'display_name':identity or 'Unassigned'}

def extension(c, kind, namespace, identity):
    """Display responsibility without moving immutable namespace paths or pins."""
    row=c.execute('SELECT owner FROM item_owners WHERE kind=? AND id=?',(kind,namespace+':'+identity)).fetchone()
    responsible=row[0] if row else namespace
    builtin=responsible=='built-in'
    person={'id':'built-in','provider':'builtin','login':None,'display_name':'Built-in'} if builtin else profile(c,responsible)
    return {'namespace':namespace,'owner_profile':person,'owner_name':person['display_name'],
            'owner_kind':'built-in' if builtin else 'user'}


def assign(c, kind, identity, owner, author):
    if not c.execute('SELECT 1 FROM user_profiles WHERE id=?',(owner,)).fetchone():
        raise ValueError('Owner profile is not registered')
    previous=c.execute('SELECT owner FROM item_owners WHERE kind=? AND id=?',(kind,identity)).fetchone()
    if previous and previous[0]==owner:return False
    now=time.time()
    c.execute('INSERT INTO ownership_history(kind,id,previous_owner,owner,changed_by,changed) VALUES(?,?,?,?,?,?)',
              (kind,identity,previous[0] if previous else None,owner,author,now))
    c.execute('''INSERT INTO item_owners VALUES(?,?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET
        owner=excluded.owner,updated=excluded.updated,updated_by=excluded.updated_by''',
        (kind,identity,owner,now,author))
    return True


def decorate(c, kind, item, fallback=None):
    identity=item['id']
    row=c.execute('SELECT owner FROM item_owners WHERE kind=? AND id=?',(kind,identity)).fetchone()
    old=item.get('owner') or item.get('properties',{}).get('owner') or fallback
    owner=row[0] if row else old
    person=profile(c,owner)
    return {**item,'owner':owner,'owner_profile':person,
            'properties':{**item.get('properties',{}),'owner':owner,'owner_name':person['display_name']}}


def inventory(c):
    """Current items, including imported views; source snapshots stay immutable."""
    items={('job',r[0]) for r in c.execute('SELECT id FROM jobs')}
    items.update(('entity',r[0]) for r in c.execute('SELECT id FROM entity_revisions'))
    for row in c.execute('SELECT source,kind,id,body FROM history_records'):
        body=json.loads(row['body'])
        if row['kind']=='job_view' and 'entity' in body:
            items.add(('job',body['entity']['id']))
        elif row['source']=='catalog' and 'original' in body:
            item=body['original'];items.add(('job' if item.get('kind')=='job' else 'entity',item['id']))
        items.add(('history',':'.join((row['source'],row['kind'],row['id']))))
    for table,kind in (('components','component'),('execution_profiles','profile')):
        items.update((kind,owner+':'+identity) for owner,identity in c.execute(f'SELECT DISTINCT owner,id FROM {table}'))
    return sorted(items)
