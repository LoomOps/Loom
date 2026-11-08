"""Assign existing item responsibility without rewriting execution or history snapshots."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import sys
import uuid
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ops'))
import ownership


def digest(c, query):
    value=hashlib.sha256()
    for row in c.execute(query):value.update(json.dumps(tuple(row),sort_keys=True).encode())
    return value.hexdigest()


def migrate(instance, login, name, backup):
    backup.mkdir(parents=True,mode=0o700,exist_ok=False)
    database=instance/'runtime/ops.sqlite3'
    with sqlite3.connect(database) as c:
        c.row_factory=sqlite3.Row
        with sqlite3.connect(backup/'ops.sqlite3') as target:c.backup(target)
        frozen={query:digest(c,query) for query in (
            'SELECT * FROM jobs ORDER BY id','SELECT * FROM entity_revisions ORDER BY id,revision',
            'SELECT * FROM history_records ORDER BY source,kind,id,revision,snapshot')}
        ownership.init(c)
        c.execute('BEGIN IMMEDIATE')
        person=ownership.register(c,login,name)
        counts={}
        for kind,identity in ownership.inventory(c):
            changed=ownership.assign(c,kind,identity,person['id'],person['id'])
            counts[kind]=counts.get(kind,0)+int(changed)
        for query,expected in frozen.items():
            if digest(c,query)!=expected:raise RuntimeError('Original records unexpectedly changed')
    installation=None
    for filename in ('api.json','worker.json'):
        path=instance/'config'/filename
        if not path.exists():continue
        shutil.copy2(path,backup/filename)
        config=json.loads(path.read_text())
        installation=config.get('installation_id') or installation or str(uuid.uuid4())
        config['installation_id']=installation
        for actor in config.get('actors',[]):
            if actor['id']=='owner':actor['profile_id']=person['id']
        # Keep the mounted file inode and its existing permissions/owner.
        path.write_text(json.dumps(config,indent=2)+'\n')
    report={'instance':str(instance),'profile':person,'assigned':counts,'original_records_unchanged':True,
            'installation_id':installation,'backup':str(backup),'existing_machine_id_preserved':True}
    (backup/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    for path in backup.iterdir():path.chmod(0o600)
    return report


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--instance',type=Path,required=True);p.add_argument('--login',required=True)
    p.add_argument('--name',required=True);p.add_argument('--backup',type=Path,required=True)
    a=p.parse_args()
    print(json.dumps(migrate(a.instance,a.login,a.name,a.backup),ensure_ascii=False))
