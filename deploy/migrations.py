"""Cumulative, checksum-bound upgrade plans for an external administrator agent.

The release owns migration code. The host owns receipts. Neither release discovery
nor an operational Job executes these scripts.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import uuid


def version(value):
    match=re.fullmatch(r'(\d+)\.(\d+)\.(\d+)(?:-(alpha|beta|rc)\.(\d+))?',value)
    if not match:raise ValueError('Unsupported release version: '+value)
    return (*map(int,match.group(1,2,3)),{'alpha':0,'beta':1,'rc':2,None:3}[match[4]],int(match[5] or 0))


def save(path,value):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    owner=path.parent.stat()
    fd,name=tempfile.mkstemp(prefix='.'+path.name+'-',dir=path.parent)
    try:
        with os.fdopen(fd,'w') as f:
            os.fchmod(f.fileno(),0o600)
            if os.geteuid()==0:os.fchown(f.fileno(),owner.st_uid,owner.st_gid)
            json.dump(value,f,sort_keys=True,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
        os.replace(name,path)
    finally:Path(name).unlink(missing_ok=True)


def ledger(root):
    path=Path(root)/'runtime/migrations.json'
    if path.is_symlink():raise ValueError('Migration ledger must not be a symlink')
    value=json.loads(path.read_text()) if path.exists() else {'schema':'loom-migrations/v1','completed':{}}
    if value.get('schema')!='loom-migrations/v1' or not isinstance(value.get('completed'),dict):raise ValueError('Invalid migration ledger')
    return value


def installed(root):
    from tailnet import env_values
    env=env_values(Path(root)/'build.env')
    return env.get('LOOM_BUILD_ID') or env.get('LOOM_OPS_IMAGE','').rsplit(':',1)[-1]


def plan(root,release):
    root=Path(root);release=Path(release);source=release/'source'
    file=source/'deploy/migrations/catalog.json'
    if not file.exists():return {'schema':'loom-upgrade-plan/v1','steps':[],'legacy':True}
    if file.is_symlink() or not file.resolve().is_relative_to(source.resolve()):raise ValueError('Unsafe migration catalog')
    catalog=json.loads(file.read_text());target=json.loads((release/'result.json').read_text())['build_id'];current=installed(root)
    start,end=version(current),version(target)
    if end<=start:raise ValueError('Target release must be newer than the installed release')
    if catalog.get('schema')!='loom-migration-catalog/v1':raise ValueError('Invalid migration catalog')
    if start<version(catalog['minimum_version']):raise ValueError('Unsupported upgrade origin; use the documented bridge release')
    declared=json.loads((release/'manifest.json').read_text())['files']
    if declared.get('deploy/migrations/catalog.json')!=hashlib.sha256(file.read_bytes()).hexdigest():raise ValueError('Migration catalog is not bound to the release manifest')
    all_steps={};pending={};receipts=ledger(root)['completed']
    for entry in catalog['steps']:
        key=entry['id']
        if not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,100}',key) or key in all_steps:raise ValueError('Duplicate or invalid migration ID')
        all_steps[key]=entry
        if version(entry['introduced'])>end:continue
        rel=entry['script'];script=source/rel
        if Path(rel).is_absolute() or '..' in Path(rel).parts or script.is_symlink() or not script.resolve().is_relative_to(source.resolve()):raise ValueError('Unsafe migration script')
        digest=hashlib.sha256(script.read_bytes()).hexdigest()
        if declared.get(rel)!=digest:raise ValueError('Migration script checksum mismatch')
        receipt=receipts.get(key)
        if receipt and receipt.get('sha256')!=digest:raise ValueError('Completed migration code changed: '+key)
        timeout=entry.get('timeout_seconds',120)
        if type(timeout) is not int or not 1<=timeout<=3600:raise ValueError('Migration needs a bounded timeout')
        pending[key]={**entry,'sha256':digest,'completed':bool(receipt),'timeout_seconds':timeout}
    ordered=[];visiting=set();visited=set()
    def visit(key):
        if key in visiting:raise ValueError('Migration dependency cycle')
        if key in visited:return
        if key not in pending:raise ValueError('Missing or future migration dependency: '+key)
        visiting.add(key)
        for dependency in pending[key].get('requires',[]):visit(dependency)
        visiting.remove(key);visited.add(key);ordered.append(pending[key])
    for key in pending:visit(key)
    return {'schema':'loom-upgrade-plan/v1','from':current,'to':target,'steps':ordered,
            'instructions':'source/docs/UPGRADE.md','modes':['update','rebuild'],
            'preserve':['accounts','tokens','installation_identity','registries','revisions','history','storage_references'],
            'legacy':False}


def invoke(step,phase,root,release,platform):
    script=Path(release)/'source'/step['script']
    if hashlib.sha256(script.read_bytes()).hexdigest()!=step['sha256']:raise ValueError('Migration code changed after planning')
    result=subprocess.run([sys.executable,'-I',str(script),phase,'--instance',str(root),'--platform',str(platform)],
                          capture_output=True,text=True,timeout=step['timeout_seconds'])
    # Never forward exception text, environment, credentials or provider bodies.
    if result.returncode:
        incident=uuid.uuid4().hex;directory=Path(root)/'diagnostics/upgrades'
        directory.mkdir(parents=True,exist_ok=True,mode=0o700);directory.chmod(0o700)
        save(directory/(incident+'.json'),{'id':incident,'migration':step['id'],'phase':phase,
             'returncode':result.returncode,'script_sha256':step['sha256']})
        raise RuntimeError('Migration '+step['id']+' failed in '+phase+'; private incident '+incident)
    try:value=json.loads(result.stdout)
    except ValueError:raise ValueError('Migration must return a JSON status') from None
    if value.get('status') not in ('needed','done'):raise ValueError('Invalid migration status')
    return value['status']


def apply(plan,root,release,platform):
    state=ledger(root)
    for step in plan['steps']:
        # Always inspect reality, including a crash between the effect and receipt.
        status=invoke(step,'check',root,release,platform)
        if status!='done':
            if step['id'] in state['completed']:raise ValueError('Completed migration no longer verifies: '+step['id'])
            if invoke(step,'apply',root,release,platform)!='done':raise ValueError('Migration did not complete')
        if invoke(step,'verify',root,release,platform)!='done':raise ValueError('Migration verification failed')
        state['completed'][step['id']]={'sha256':step['sha256'],'release':plan.get('to')}
        save(Path(root)/'runtime/migrations.json',state)
    return list(state['completed'])
