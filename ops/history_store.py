"""Immutable history read model shared by native and imported records."""
import json
import time
import hashlib
import storage
import ownership
import re
from pathlib import Path

SOURCES = {'queue':'queue.sqlite3','catalog':'catalog.sqlite3','run-history':'index.sqlite3','publication':'publication.sqlite3','loom-ops':'loom-ops.sqlite3'}

KEY = re.compile(r'[A-Za-z0-9][A-Za-z0-9_.:-]{0,191}\Z')


def init(db):
    ownership.init(db)
    db.executescript("""
    CREATE TABLE IF NOT EXISTS history_imports(
        id TEXT PRIMARY KEY, manifest_sha256 TEXT NOT NULL, imported_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS history_records(
        source TEXT NOT NULL, kind TEXT NOT NULL, id TEXT NOT NULL,
        revision TEXT NOT NULL, snapshot TEXT NOT NULL,
        body TEXT NOT NULL, PRIMARY KEY(source,kind,id,revision,snapshot));
    CREATE INDEX IF NOT EXISTS history_search ON history_records(kind,id,source);
    """)


def encoded(value):
    return json.dumps(value,sort_keys=True,allow_nan=False,ensure_ascii=False)


def under(root, ref):
    if not isinstance(ref,str):
        raise ValueError('Invalid location ref')
    path=Path(ref)
    if not path.parts or path.is_absolute() or '..' in path.parts:
        raise ValueError('Invalid relative location')
    resolved=(root/path).resolve()
    if not resolved.is_relative_to(root.resolve()):
        raise ValueError('Location escapes declared root')
    return resolved


def list_records(db, kind, offset, limit):
    if not isinstance(kind,str) or (kind and not KEY.fullmatch(kind)) or type(offset) is not int or offset<0 or type(limit) is not int or not 1<=limit<=100:
        raise ValueError('Invalid history filter or page')
    return [dict(row) for row in db.execute('''SELECT source,kind,id,revision,snapshot FROM history_records
        WHERE (?='' OR kind=?) ORDER BY kind,id,source,revision,snapshot LIMIT ? OFFSET ?''',(kind,kind,limit,offset))]


def get_record(db, source, kind, identity, revision, config=None, bulk=None):
    if source.removeprefix('archive.') not in SOURCES or any(not isinstance(value,str) or not KEY.fullmatch(value) for value in (kind,identity,revision)):
        raise ValueError('Invalid history record key')
    row=db.execute('SELECT * FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? ORDER BY rowid DESC LIMIT 1',
                   (source,kind,identity,revision)).fetchone()
    if row is None:
        raise ValueError('History record missing')
    result={**dict(row),'body':json.loads(row['body'])}
    key=':'.join((source,kind,identity))
    person=ownership.decorate(db,'history',{'id':key})
    if config is not None and bulk is not None:
        result['current_availability']=availability(result['body'].get('locations',[]),config,bulk)
    return {**result,'owner':person['owner'],'owner_profile':person['owner_profile']}


def availability(locations,config,bulk):
    """Current registered-copy observations, never rewrite an imported snapshot.

    A source_ref alone is not a locator: data/1 names have changed across installs.
    Do not discover or expose unregistered host directories to a remote reader.
    """
    checked=[]
    for item in locations:
        value={'source_ref':item.get('source_ref'),'recorded_status':item.get('status'),
               'status':'not_connected'}
        if item.get('target_ref') and item.get('machine_id')==config.get('machine_id'):
            matches=[v for v in storage.stores(config,bulk).values() if v.get('storage_id')==item.get('storage_id')]
            if len(matches)==1:
                try:
                    base=Path(matches[0]['path'])
                    marker=json.loads((base/'.loom-storage.json').read_text())
                    if marker.get('storage_id')!=item.get('storage_id'):raise ValueError('Storage identity changed')
                    p=under(base,item['target_ref'])
                    if not p.exists():value['status']='missing'
                    elif p.is_file():
                        size=p.stat().st_size;value['size']=size
                        value['status']='present_unverified'
                        if item.get('size') is not None and size!=item['size']:value['status']='mismatch'
                        elif item.get('sha256') and size<=1024*1024:
                            value['status']='verified' if hashlib.sha256(p.read_bytes()).hexdigest()==item['sha256'] else 'mismatch'
                except PermissionError:value['status']='permission_denied'
                except (OSError,ValueError,TypeError):value['status']='unavailable'
        checked.append(value)
    return {'checked_at':time.time(),'scope':'registered_copies_only',
            'note':'Recorded missing is an import-time observation, not proof the original was deleted. Unconnected originals require explicit storage mapping; no inference readiness is implied.',
            'locations':checked}
