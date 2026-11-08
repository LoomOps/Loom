"""Local operator credential issuance and revocation. Token files never enter Compose."""
import argparse
import json
import os
import sqlite3
from pathlib import Path
import access
from ops import ident

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('node',type=Path)
p.add_argument('command',choices=('issue','revoke'))
p.add_argument('id')
p.add_argument('--kind',choices=('user','service'),default='user')
p.add_argument('--scope',action='append',choices=('read','submit','cancel','events'),default=[])
p.add_argument('--owner',action='append',default=[])
p.add_argument('--notify',action='append',default=[])
p.add_argument('--ttl',type=int,default=3600)
p.add_argument('--output',type=Path)
a=p.parse_args()
db=a.node/'runtime/ops.sqlite3'
with sqlite3.connect(db.as_uri()+'?mode=rw',uri=True) as c:
    if a.command=='issue':
        if a.output is None or a.output.resolve().parent!=(a.node/'secrets').resolve():
            p.error('Specify an unused file in the node secrets directory')
        if a.output.exists():
            p.error('Token output already exists')
        grant={'scopes':sorted(set(a.scope)),'owners':sorted(map(ident,a.owner)),'notify':sorted(map(ident,a.notify))}
        result=access.issue(c,ident(a.id),a.kind,grant,a.ttl)
        fd=os.open(a.output,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'w') as f:
            json.dump(result,f);f.flush();os.fsync(f.fileno())
        print(result['credential_id'])
    else:
        if c.execute('UPDATE credentials SET revoked=unixepoch() WHERE id=? AND revoked IS NULL',(ident(a.id),)).rowcount!=1:
            raise SystemExit('Unknown active credential')
