"""Explicit local-operator queue pause/resume for a drained installation."""
import argparse
import json
import os
from pathlib import Path
import sqlite3
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ops'))
import dispatch_control

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('action',choices=('pause','resume','status'))
    p.add_argument('--instance',type=Path,required=True)
    p.add_argument('--key');p.add_argument('--reason',default='')
    a=p.parse_args();db=a.instance.resolve()/'runtime/ops.sqlite3'
    if not db.is_file() or not (a.instance/'compose.env').is_file():raise ValueError('Select an existing installation')
    if os.geteuid()==0:
        owner=db.stat();os.setgid(owner.st_gid);os.setuid(owner.st_uid)
    with sqlite3.connect('file:'+str(db)+'?mode=rw',uri=True,timeout=15) as c:
        result=dispatch_control.status(c,True) if a.action=='status' else dispatch_control.change(c,a.action=='pause',a.key,a.reason)
    print(json.dumps(result,indent=2))
