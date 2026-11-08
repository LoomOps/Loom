"""Index retained files from a failed Attempt without changing the Job result."""
import argparse
import fcntl
import json
import sqlite3
from pathlib import Path

import registry
from host_layout import config_path
from ops import ident, relative
from profiles import file_sha

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('node',type=Path)
p.add_argument('job')
p.add_argument('output_id')
p.add_argument('ref')
p.add_argument('sha256')
p.add_argument('--component',action='append',default=[])
a=p.parse_args()
root=a.node.resolve(strict=True)
cfg=json.loads(config_path(root).read_text())
env=dict(line.split('=',1) for line in (root/'compose.env').read_text().splitlines())
bulk=Path(env['OPS_BULK']).resolve(strict=True)
with (root/'runtime/worker.lock').open('a') as lock:
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    with sqlite3.connect(root/'runtime/ops.sqlite3') as c:
        c.row_factory=sqlite3.Row
        row=c.execute('SELECT * FROM jobs WHERE id=?',(ident(a.job),)).fetchone()
        if registry.mode(c)!='active' or not row or row['state'] not in ('failed','cancelled','interrupted'):
            raise ValueError('Expected an active node and retained failed Attempt')
        path=relative(bulk,a.ref)
        if not path.is_relative_to(bulk/'runs'/row['id']/row['attempt']) or file_sha(path)!=a.sha256:
            raise ValueError('Retained file does not match the exact Attempt and digest')
        members=[{'ref':ref,'sha256':file_sha(relative(path.parent,ref))} for ref in a.component]
        item={'id':ident(a.output_id),'kind':'artifact','sha256':a.sha256,
              'locator':{'machine_id':cfg['machine_id'],'storage_id':cfg['storage_id'],'ref':a.ref},
              'provenance':{'classification':'retained_failed_parent','parent_execution_state':row['state']},
              'components':members}
        registry.index(c,row,{'entities':[item],'metrics':[]},cfg,bulk,relative)
print(a.job+'.'+a.output_id)
