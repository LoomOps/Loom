"""Apply the online handoff from this node's local filesystem only."""
import argparse
import fcntl
import json
import sqlite3
from pathlib import Path

from host_layout import config_path, write_config, write_json
from remote_fence import call

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('node',type=Path)
p.add_argument('action',choices=('seal','activate','return'))
p.add_argument('--role',choices=('source','target'),required=True)
p.add_argument('--authority',required=True)
p.add_argument('--ca-file',type=Path)
p.add_argument('--token-file',type=Path,required=True)
p.add_argument('--manifest-sha256',required=True)
a=p.parse_args()
node=a.node
runtime=node/'runtime'
settings=json.loads(config_path(node).read_text())
token=a.token_file.read_text().strip()
digest=a.manifest_sha256
with (runtime/'worker.lock').open('a') as lock:
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    with sqlite3.connect(runtime/'ops.sqlite3') as db:
        db.execute('BEGIN IMMEDIATE')
        if db.execute("SELECT 1 FROM jobs WHERE state IN ('running','processing','result_failed')").fetchone():
            raise ValueError('Local Work is running or has an unexported result')
        mode=db.execute('SELECT mode FROM node_control').fetchone()[0]
        if a.action=='seal':
            installed=settings.get('remote_fence',{})
            if a.role!='source' or mode!='fenced' or installed.get('url')!=a.authority:
                raise ValueError('Stop and export the source before sealing')
            receipt=runtime/'source-handoff.json'
            expected={'manifest_sha256':digest}
            if receipt.exists() and json.loads(receipt.read_text())!=expected:
                raise ValueError('This source already sealed a different archive')
            if not receipt.exists():
                write_json(receipt,expected)
        elif a.role=='target':
            receipt=json.loads((runtime/'restore.json').read_text())
            if receipt.get('manifest_sha256')!=digest or receipt.get('mode')!='relocate':
                raise ValueError('The local relocation receipt does not match this grant')
        else:
            receipt=json.loads((runtime/'source-handoff.json').read_text())
            if receipt['manifest_sha256']!=digest:
                raise ValueError('The source handoff receipt does not match')
        if a.action=='return':
            if a.role!='target':
                raise ValueError('Only the target can return the handoff')
            db.execute("UPDATE node_control SET mode='fenced'")
    epoch=settings.get('remote_fence',{}).get('epoch')
    binding={'machine_id':settings['machine_id'],'storage_id':settings['storage_id']}
    response=call(a.authority,token,a.action,ca_file=a.ca_file,archive=digest,epoch=epoch,binding=binding)
    if a.action=='activate':
        settings['remote_fence']={'url':a.authority,'epoch':response['epoch'],
                                  'token_path':'/etc/loom/fence-token','binding':binding}
        if a.ca_file:
            settings['remote_fence']['ca_file']='/etc/loom/fence-ca.pem'
        write_config(node,settings)
        with sqlite3.connect(runtime/'ops.sqlite3') as db:
            db.execute("UPDATE node_control SET mode='active'")
print(json.dumps(response))
