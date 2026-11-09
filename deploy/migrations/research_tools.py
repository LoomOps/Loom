"""Idempotent additive registry migration; never classifies research code by name."""
import argparse
import json
from pathlib import Path
import sqlite3


def run(root,phase):
    db=Path(root)/'runtime/ops.sqlite3'
    with sqlite3.connect('file:'+str(db)+'?mode=rw',uri=True) as c:
        done=bool(c.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='tools'").fetchone())
        if done:
            columns=[r[1] for r in c.execute('PRAGMA table_info(tools)')]
            if not {'owner','id','revision','manifest','sha256'}<=set(columns):raise ValueError('Unexpected existing Tool registry schema')
        if not done and phase=='apply':
            c.execute('CREATE TABLE tools(owner TEXT,id TEXT,revision INTEGER,manifest TEXT,sha256 TEXT,PRIMARY KEY(owner,id,revision))');done=True
    return {'status':'done' if done else 'needed'}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['check','apply','verify'])
    p.add_argument('--instance',type=Path,required=True);p.add_argument('--platform',type=Path,required=True)
    a=p.parse_args();print(json.dumps(run(a.instance,a.phase)))
