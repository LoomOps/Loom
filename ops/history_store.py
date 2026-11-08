"""Immutable history read model shared by native and imported records."""
import json
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


def get_record(db, source, kind, identity, revision):
    if source.removeprefix('archive.') not in SOURCES or any(not isinstance(value,str) or not KEY.fullmatch(value) for value in (kind,identity,revision)):
        raise ValueError('Invalid history record key')
    row=db.execute('SELECT * FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? ORDER BY rowid DESC LIMIT 1',
                   (source,kind,identity,revision)).fetchone()
    if row is None:
        raise ValueError('History record missing')
    result={**dict(row),'body':json.loads(row['body'])}
    key=':'.join((source,kind,identity))
    person=ownership.decorate(db,'history',{'id':key})
    return {**result,'owner':person['owner'],'owner_profile':person['owner_profile']}
