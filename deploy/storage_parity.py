"""Read-only effective storage checks for installed instances, including older installs.

No remapping, identity creation, ACL changes or data copies are performed here.
"""
import argparse
import hashlib
import json
from pathlib import Path
import pwd
import re
import subprocess
import sys


def read(path):return json.loads(Path(path).read_text())


def validate(root,container,executor,status=None,boot_mounts=None):
    root=Path(root);api=read(root/'config/api.json');worker=read(root/'config/worker.json')
    ex=api.get('executor',{});hosts=ex.get('storage') or {'data/0':ex.get('host_data')}
    if not hosts.get('data/0') or hosts['data/0']!=ex.get('host_data'):raise ValueError('Primary executor storage differs')
    stores=api.get('data_stores') or {'data/0':{'path':'/loom/bulk','storage_id':api['storage_id'],'mode':'rw'}}
    if set(hosts)!=set(stores) or stores['data/0']['storage_id']!=api['storage_id']:raise ValueError('Registered storage set differs from executor grants')
    for key in ('storage_id','data_stores','executor'):
        if worker.get(key)!=api.get(key):raise ValueError('API and worker storage configuration differs: '+key)
    mounts={m['Destination']:m for m in container['Mounts']}
    if mounts.get('/loom/bulk',{}).get('Source')!=hosts['data/0']:raise ValueError('Effective OPS_BULK differs from primary storage')
    for path in ('api','worker'):
        if mounts.get('/etc/loom/'+path+'.json',{}).get('Source')!=str(root/'config'/(path+'.json')):raise ValueError('Running configuration mount differs')
    ids=set()
    for name,item in stores.items():
        if not re.fullmatch(r'data/[0-9]+',name):raise ValueError('Invalid store selector')
        host=Path(hosts[name]);marker=read(host/'.loom-storage.json')
        if not host.is_absolute() or host.resolve()!=host or not host.is_dir():raise ValueError('Storage redirected or unavailable: '+name)
        if not item.get('storage_id') or marker!={'storage_id':item['storage_id']} or item['storage_id'] in ids:raise ValueError('Missing, changed or duplicate storage identity: '+name)
        ids.add(item['storage_id'])
        mount=mounts.get(item['path'],{})
        if mount.get('Source')!=str(host) or not mount.get('RW') or item.get('mode')!='rw':raise ValueError('Effective storage mount/grant differs: '+name)
    declared={key for key in mounts if re.fullmatch(r'/loom/data/[0-9]+',key)}
    expected={item['path'] for item in stores.values() if item['path']!='/loom/bulk'}
    if declared!=expected:raise ValueError('Explorer mounts differ from registered stores')
    env=dict(value.split('=',1) for value in container['Config'].get('Env',[]) if '=' in value)
    if set(filter(None,env.get('LOOM_DATA_PATHS','').split(':')))!=expected:raise ValueError('Explorer data paths differ from registered stores')
    token=hashlib.sha256((root/'secrets/executor-token').read_text().strip().encode()).hexdigest()
    granted=[v for v in executor['instances'].values() if v.get('token_sha256')==token]
    if len(granted)!=1 or granted[0].get('data')!=hosts['data/0'] or (granted[0].get('storage') or {'data/0':granted[0]['data']})!=hosts:raise ValueError('Host executor grant differs')
    if container['Config'].get('User')!=str(granted[0]['uid'])+':'+str(granted[0]['gid']):raise ValueError('Host executor identity differs from Ops')
    plan=root/'config/storage-plan.json'
    if plan.exists():
        value=read(plan)
        if value.get('primary')!=hosts['data/0'] or set(value.get('read_write',[]))!=set(hosts.values()):raise ValueError('Storage plan differs from effective grants')
    if boot_mounts is not None:
        required=[Path(p) for p in boot_mounts]
        if any(not any(Path(path)==p or Path(path).is_relative_to(p) for p in required) for path in hosts.values()):raise ValueError('Boot mount dependencies omit registered storage')
    if status is not None:
        live=status.get('data_stores',{})
        if status.get('storage_id')!=api['storage_id'] or set(live)!=set(stores) or any(live[n].get('storage_id')!=v['storage_id'] or live[n].get('mode')!=v['mode'] for n,v in stores.items()):raise ValueError('Running API storage identities differ')
    ssh=root/'config/ssh.json'
    if ssh.exists():
        state=Path(read(ssh)['state'])
        for value in (read(state/'config.json').get('stores'),read(state/'storage.json'),read(root/'ssh-info/machine.json').get('data_paths')):
            if value!=hosts:raise ValueError('SSH/Explorer host storage bindings differ')
    return {'status':'passed','stores':{name:{'host_path':hosts[name],'storage_id':item['storage_id']} for name,item in stores.items()},
            'uid':granted[0]['uid'],'gid':granted[0]['gid'],'installation_plan':'checked' if plan.exists() else 'absent; effective configuration checked',
            'scope':'configuration, actual mounts, persistent identities, runtime access and boot dependencies; no payload or semantic validation'}


def run(*args):return subprocess.check_output([str(v) for v in args],text=True,stderr=subprocess.DEVNULL,timeout=20).strip()


def inspect(root,project='loom',status=None,runner=run):
    root=Path(root)
    ids=runner('docker','ps','-q','--filter','label=com.docker.compose.project='+project,'--filter','label=com.docker.compose.service=loom-ops').split()
    if len(ids)!=1:raise ValueError('Storage check requires one running Ops service')
    container=json.loads(runner('docker','inspect',ids[0]))[0]
    executor=read(root.parent/'shared/executor/config.json')
    boot=runner('systemctl','show','loom-stack.service','--property=RequiresMountsFor','--value').split()
    result=validate(root,container,executor,status,boot)
    paths=[v['host_path'] for v in result['stores'].values()]
    # Check host ancestor traversal too: container aliases can hide a missing /storage ACL.
    check="import json,os,sys; from pathlib import Path; paths=json.loads(sys.argv[1]); assert all(os.access(p,os.R_OK|os.W_OK|os.X_OK) and (Path(p)/'.loom-storage.json').read_text() for p in paths)"
    user=pwd.getpwuid(result['uid']).pw_name
    runner('runuser','-u',user,'--',sys.executable,'-I','-c',check,json.dumps(paths))
    runner('docker','exec','--user',str(result['uid'])+':'+str(result['gid']),ids[0],'python3','-I','-c',check,
           json.dumps([v['path'] for v in read(root/'config/api.json').get('data_stores',{'data/0':{'path':'/loom/bulk'}}).values()]+['/loom/bulk']))
    return result


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--instance',type=Path,required=True);parser.add_argument('--project',default='loom');args=parser.parse_args()
    try:print(json.dumps(inspect(args.instance,args.project),indent=2))
    except (OSError,ValueError,KeyError,subprocess.SubprocessError) as exc:
        print(json.dumps({'status':'failed','error':type(exc).__name__,'next':'Compare storage plan, Ops/worker mounts, marker IDs, executor/SSH grants, boot dependencies and service-user ACLs. Never remap an active Job.'}));raise SystemExit(1)
