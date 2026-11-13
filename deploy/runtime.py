"""Plan/install a research Runtime using the installation's dependency directory.

This is an explicitly authorized host-administrator operation, never an MCP
bootstrap action. Docker builds execute reviewed recipes. Jobs cannot invoke it.
"""
import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import tarfile
import time
from urllib.request import urlopen
from urllib.parse import urlsplit
import uuid
import zipfile

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'executor'))
import runtime_store
from runtime_policy import options


def settings(installation, executor, instance):
    config=json.loads((executor/'config.json').read_text())
    if instance not in config['instances']:
        raise ValueError('Unknown executor instance')
    plan=installation/'config/installation-plan.json'
    planned=json.loads(plan.read_text()).get('dependencies') if plan.exists() else None
    if plan.exists() and json.loads(plan.read_text()).get('data') not in (None,config['instances'][instance].get('data')):
        raise ValueError('Installation plan does not belong to this executor instance')
    configured=config['instances'][instance].get('dependencies')
    if configured and planned and configured!=planned:
        raise ValueError('Executor and installation dependency directories differ')
    selected=configured or planned
    if not selected:
        raise ValueError('Configure the installation dependency directory before installing Runtimes')
    config['instances'][instance]['dependencies']=selected
    return config,runtime_store.dependencies_root(config,instance)


def files(root):
    result={}
    for p in sorted(root.rglob('*')):
        if p.is_symlink() or not (p.is_file() or p.is_dir()):
            raise ValueError('Recipe trees cannot contain links or special files')
        if p.is_file():
            with p.open('rb') as stream:result[str(p.relative_to(root))]=hashlib.file_digest(stream,'sha256').hexdigest()
    return result


def recipe(source):
    source=source.resolve()
    definition=json.loads((source/'runtime.json').read_text())
    if not isinstance(definition,dict):raise ValueError('Runtime recipe must be an object')
    if set(definition)-{'id','label','description','dependencies','primary_dependencies','runtime','validation'}:
        raise ValueError('Unknown Runtime recipe fields')
    if not runtime_store.ID.fullmatch(definition.get('id','')):
        raise ValueError('Invalid Runtime ID')
    runtime=definition.get('runtime',{})
    if not isinstance(runtime,dict):raise ValueError('Runtime policy must be an object')
    if set(runtime)-{'cpus','memory','pids','network','driver_capabilities'}:
        raise ValueError('Runtime recipes cannot select host mounts or Docker flags')
    options({k:runtime[k] for k in ('network','driver_capabilities') if k in runtime})
    if type(runtime.get('cpus',4)) not in (int,float) or not 0<runtime.get('cpus',4)<=256:
        raise ValueError('Invalid Runtime CPU limit')
    if not re.fullmatch(r'[1-9][0-9]*[kmg]',runtime.get('memory','8g')) or type(runtime.get('pids',512)) is not int or not 16<=runtime.get('pids',512)<=65536:
        raise ValueError('Invalid Runtime resource limits')
    validation=definition.get('validation',{})
    if not isinstance(validation,dict):raise ValueError('Validation must be an object')
    command=validation.get('command')
    if set(validation)-{'command','gpu','timeout','checks'} or not isinstance(command,list) or not command or any(not isinstance(v,str) or not v or '\0' in v for v in command):
        raise ValueError('A validation argv is required')
    if not isinstance(validation.get('checks',[]),list) or any(not isinstance(v,str) or not v for v in validation.get('checks',[])):
        raise ValueError('Validation checks must be names')
    if type(validation.get('gpu',False)) is not bool or type(validation.get('timeout',120)) is not int or not 1<=validation.get('timeout',120)<=1800:
        raise ValueError('Invalid validation GPU/timeout policy')
    names=set()
    if not isinstance(definition.get('dependencies',[]),list):raise ValueError('Dependencies must be an array')
    for dep in definition.get('dependencies',[]):
        if not isinstance(dep,dict) or set(dep)-{'name','version','kind','url','sha256','archive'} or not runtime_store.ID.fullmatch(dep.get('name','')) or not isinstance(dep.get('version'),str) or not dep['version']:
            raise ValueError('Dependencies require unique names and exact version strings')
        if dep['name'] in names:raise ValueError('Duplicate dependency name')
        names.add(dep['name'])
        if 'url' in dep:
            url=urlsplit(dep['url']) if isinstance(dep['url'],str) else None
            if (not url or url.scheme!='https' or not url.hostname or url.username or url.password or url.query or url.fragment
                    or not isinstance(dep.get('sha256'),str) or not re.fullmatch(r'[a-f0-9]{64}',dep['sha256'])):
                raise ValueError('Downloaded dependencies require credential-free HTTPS URLs without queries/fragments and SHA-256')
        if dep.get('archive','file') not in ('file','tar','zip'):raise ValueError('Unknown dependency archive format')
    runtime_store.validate_highlights(definition)
    pins=files(source)
    if 'Dockerfile' not in pins:raise ValueError('A reproducible Dockerfile is required')
    return definition,pins


def unpack(archive, destination, kind):
    destination.mkdir()
    if kind=='file':
        shutil.copy2(archive,destination/'download');return
    if kind=='tar':
        with tarfile.open(archive) as bundle:
            for item in bundle:
                p=Path(item.name)
                if p.is_absolute() or '..' in p.parts or not (item.isfile() or item.isdir()):
                    raise ValueError('Unsafe dependency archive member')
                target=destination/p
                if item.isdir():target.mkdir(parents=True,exist_ok=True)
                else:
                    target.parent.mkdir(parents=True,exist_ok=True)
                    with bundle.extractfile(item) as src,target.open('xb') as out:shutil.copyfileobj(src,out)
                    target.chmod(0o644 | (item.mode & 0o111))
    else:
        with zipfile.ZipFile(archive) as bundle:
            for item in bundle.infolist():
                p=Path(item.filename)
                if p.is_absolute() or '..' in p.parts or (item.external_attr>>16)&0o170000==0o120000:
                    raise ValueError('Unsafe dependency archive member')
                target=destination/p
                if item.is_dir():target.mkdir(parents=True,exist_ok=True)
                else:
                    target.parent.mkdir(parents=True,exist_ok=True)
                    with bundle.open(item) as src,target.open('xb') as out:shutil.copyfileobj(src,out)
                    target.chmod(0o644 | ((item.external_attr>>16) & 0o111))


def download(dep, target):
    archives=target.parent/'.downloads'
    if archives.is_symlink():raise ValueError('Download evidence directory is redirected')
    archives.mkdir(exist_ok=True)
    archive=archives/target.name
    with urlopen(dep['url'],timeout=60) as response,archive.open('xb') as out:
        shutil.copyfileobj(response,out)
    with archive.open('rb') as stream:actual=hashlib.file_digest(stream,'sha256').hexdigest()
    if actual!=dep['sha256']:raise ValueError('Dependency hash mismatch: '+dep['name'])
    unpack(archive,target,dep.get('archive','file'))


def return_validation(run,name):
    result=run(['docker','rm','-f',name],check=False,timeout=30,capture_output=True)
    if result.returncode:
        # A successful empty inventory proves absence after an interrupted rm.
        probe=run(['docker','container','ls','-a','--filter','name=^/'+name+'$','--format','{{.Names}}'],check=False,timeout=30,capture_output=True)
        if probe.returncode or probe.stdout.strip():
            raise RuntimeError('Validation container return is unconfirmed; inspect '+name)


def measured_validation(path,definition):
    with path.open('rb') as stream:
        stream.seek(max(0,path.stat().st_size-65536));lines=stream.read().decode('utf-8','replace').splitlines()
    result=None
    for line in reversed(lines):
        try: candidate=json.loads(line)
        except ValueError:continue
        if isinstance(candidate,dict) and isinstance(candidate.get('dependencies'),dict):result=candidate;break
    expected={d['name']:d['version'] for d in definition.get('dependencies',[])}
    if result is None or result['dependencies']!=expected:
        raise ValueError('Validation must report the exact installed dependency versions as JSON')
    if any(result.get('checks',{}).get(name) is not True for name in definition['validation'].get('checks',[])):
        raise ValueError('Required runtime validation checks did not pass')
    with path.open('rb') as stream:log_sha=hashlib.file_digest(stream,'sha256').hexdigest()
    return {'observed_dependencies':result['dependencies'],'checks':result.get('checks',{}),'log_sha256':log_sha}


@contextmanager
def installation_lock(executor):
    # Shared by every Runtime installer on this executor. Never race GPU smoke
    # with normal dispatch: the executor takes a shared lock around GPU admission.
    lock=executor/'runtime-install.lock'
    if lock.is_symlink():raise ValueError('Linked runtime installation lock')
    with lock.open('a') as f:
        fcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB)
        yield


def install(installation, executor, instance, source, run=subprocess.run):
    config,deps=settings(installation,executor,instance)
    definition,pins=recipe(source)
    parent=deps/'installations'
    if parent.is_symlink():raise ValueError('Dependency installations directory is redirected')
    parent.mkdir(exist_ok=True)
    base=parent/definition['id']
    if base.is_symlink():raise ValueError('Runtime installation directory is redirected')
    base.mkdir(exist_ok=True)
    with installation_lock(executor):
        if runtime_store.validation_fenced(executor):raise ValueError('Reconcile the previous Runtime validation container before installation')
        tickets=executor/'executor.sqlite3'
        if tickets.exists():
            with sqlite3.connect(tickets.as_uri()+'?mode=ro',uri=True) as c:
                if c.execute("SELECT 1 FROM tickets WHERE state!='returned' AND gpu=1 LIMIT 1").fetchone():
                    raise ValueError('Wait for GPU tickets to return before Runtime validation')
        uid=uuid.uuid4().hex
        folder=base/uid;folder.mkdir()
        evidence={'instance':instance,'path':str(folder),'recipe_sha256':runtime_store.digest(pins),'source_pins':pins}
        def state(value):runtime_store.installation(executor,uid,definition['id'],value,evidence)
        state('staging')
        try:
            context=folder/'context';shutil.copytree(source,context)
            if files(context)!=pins:raise ValueError('Recipe changed during staging')
            incoming=context/'dependencies'
            if incoming.exists():raise ValueError('dependencies is reserved for verified downloads')
            incoming.mkdir()
            for dep in definition.get('dependencies',[]):
                if dep.get('url'):download(dep,incoming/dep['name'])
            evidence['context_pins']=files(context)
            imagefile=folder/'image-id'
            state('building')
            with (folder/'build.log').open('wb') as log:
                run(['docker','build','--iidfile',str(imagefile),str(context)],check=True,timeout=7200,stdout=log,stderr=subprocess.STDOUT)
            image=imagefile.read_text().strip()
            if not re.fullmatch(r'sha256:[0-9a-f]{64}',image):raise ValueError('Build did not return an immutable image ID')
            policy=definition.get('runtime',{})
            smoke=definition['validation'];host=config['instances'][instance]
            if smoke.get('gpu') and not host.get('gpu'):raise PermissionError('GPU validation outside instance grant')
            name='loom-runtime-check-'+uid
            command=['docker','run','--name',name,'--init','--user',str(host['uid'])+':'+str(host['gid']),
                     '--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--network',policy.get('network','none'),
                     '--cpus',str(policy.get('cpus',4)),'--memory',policy.get('memory','8g'),'--pids-limit',str(policy.get('pids',512)),
                     '--tmpfs','/tmp:rw,nosuid,size=1g','--env','HOME=/tmp']
            if smoke.get('gpu'):
                command+=['--gpus','device='+','.join(config.get('gpu_devices',['0'])),'--env','NVIDIA_DRIVER_CAPABILITIES='+','.join(policy.get('driver_capabilities',['compute','utility']))]
            command+=['--entrypoint',smoke['command'][0],image,*smoke['command'][1:]]
            evidence.update(validation_container=name,validation_gpu=smoke.get('gpu',False),container_returned=False)
            state('validating')
            try:
                with (folder/'validation.log').open('wb') as log:
                    run(command,check=True,timeout=smoke.get('timeout',120),stdout=log,stderr=subprocess.STDOUT)
            finally:
                return_validation(run,name)
                evidence['container_returned']=True
            measured=measured_validation(folder/'validation.log',definition)
            # Recipes and packages survive Docker cache deletion under the configured root.
            manifest={k:v for k,v in definition.items() if k!='validation'}
            manifest['dependencies']=[{**d,'origin':{'type':'managed-directory','path':str(folder/'context/dependencies'/d['name'])} if 'url' in d else {'type':'image'}} for d in manifest.get('dependencies',[])]
            effective_policy={'cpus':4,'memory':'8g','pids':512,**options({k:policy[k] for k in ('network','driver_capabilities') if k in policy}),**policy,'image':image}
            manifest.update(runtime=effective_policy,installation=evidence,
                validation={'status':'passed','command':smoke['command'],'gpu':smoke.get('gpu',False),'completed_at':time.time(),**measured})
            (folder/'receipt.json').write_text(json.dumps(manifest,indent=2))
            ref=runtime_store.register(executor,definition['id'],manifest,[instance])
            evidence['runtime_ref']=ref
            state('ready')
            return {**ref,'path':str(folder),'status':'ready'}
        except BaseException as exc:
            evidence['error']=type(exc).__name__
            state('failed')
            raise


def reconcile(executor, instance, attempt, run=subprocess.run):
    """Return one known validation container; preserve successful registrations."""
    with installation_lock(executor):
        c=runtime_store.connect(executor)
        if c is None:raise ValueError('Unknown installation attempt')
        try:
            row=c.execute('SELECT * FROM runtime_installations WHERE id=?',(attempt,)).fetchone()
            if not row:raise ValueError('Unknown installation attempt')
            evidence=json.loads(row['evidence'])
            if evidence.get('instance')!=instance:
                raise PermissionError('Installation attempt belongs to another instance')
            # Registration may have committed just before its ready receipt was
            # lost. Discover the exact catalog revision, never build it again.
            ref=None
            for item in c.execute('''SELECT r.* FROM runtime_revisions r JOIN runtime_grants g
                    ON (r.id=g.id AND r.revision=g.revision)
                    WHERE g.instance=? AND r.id=? ORDER BY r.revision DESC''',(instance,row['runtime'])):
                recorded=json.loads(item['manifest']).get('installation',{})
                if evidence.get('path') and recorded.get('path')==evidence['path']:
                    ref={k:item[k] for k in ('id','revision','sha256')}
                    runtime_store.detail(executor,instance,ref)
                    break
        finally:c.close()
        name='loom-runtime-check-'+attempt
        if evidence.get('validation_container')!=name:raise ValueError('No validation container to reconcile')
        if row['state']=='ready' and not ref:raise ValueError('Ready installation has no verified Runtime revision')
        changed=evidence.get('container_returned') is not True
        if changed:
            return_validation(run,name)
            evidence['container_returned']=True
        state='ready' if ref else 'failed'
        if ref and evidence.get('runtime_ref')!=ref:
            evidence['runtime_ref']=ref;changed=True
        if changed or row['state']!=state:
            runtime_store.installation(executor,attempt,row['runtime'],state,evidence)
        result={'attempt':attempt,'container_returned':True,
                'status':'ready' if ref else 'failed','registered':ref is not None}
        if ref:result['runtime_ref']=ref
        return result


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('action',choices=['plan','install','list','reconcile'])
    p.add_argument('--installation',type=Path,required=True)
    p.add_argument('--executor',type=Path,required=True)
    p.add_argument('--instance',required=True)
    p.add_argument('--source',type=Path)
    p.add_argument('--attempt',help='Failed installation ID to reconcile; never resubmits compute')
    a=p.parse_args()
    if a.action in ('list','reconcile'):
        config=json.loads((a.executor/'config.json').read_text())
        if a.instance not in config['instances']:raise ValueError('Unknown executor instance')
    else:config,deps=settings(a.installation,a.executor,a.instance)
    if a.action=='reconcile':
        if not a.attempt or not re.fullmatch(r'[a-f0-9]{32}',a.attempt):p.error('--attempt must be an installation ID')
        result=reconcile(a.executor,a.instance,a.attempt,subprocess.run)
    elif a.action=='list':result=runtime_store.listing(a.executor,config,a.instance)
    else:
        if not a.source:p.error('--source is required')
        definition,pins=recipe(a.source)
        if a.action=='plan':result={'id':definition['id'],'dependencies':str(deps),'recipe_sha256':runtime_store.digest(pins),'definition':definition,'executes':False}
        else:result=install(a.installation,a.executor,a.instance,a.source)
    print(json.dumps(result,indent=2))


if __name__=='__main__':main()
