"""Adopt measured installed environments without rebuilding or changing Job pins.

Read-only, CPU-only metadata probes; no scientific/GPU readiness claim. Host
mount evidence comes only from administrator policy, never from a Job request.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import uuid
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'executor'))
import runtime_store
from runtime import installation_lock

PROBE=r'''
import sys,json,pathlib,importlib.metadata as metadata,tomllib,subprocess,shutil
result={'python':sys.version.split()[0],'packages':[],'extensions':[],'versions':{}}
for d in metadata.distributions():
 name=d.metadata.get('Name')
 if name:result['packages'].append({'name':name,'version':d.version,'scope':'image'})
if shutil.which('dpkg-query'):
 query=subprocess.run(['dpkg-query','-W','-f','${binary:Package}\t${Version}\n'],capture_output=True,text=True,check=True,timeout=10)
 for line in query.stdout.splitlines():
  name,version=line.split('\t',1);result['packages'].append({'name':name,'version':version,'scope':'image-os','kind':'system'})
root=pathlib.Path('/isaac-sim')
if root.is_dir():
 kit=root/'kit/python/bin/python3'
 if kit.is_file():
  measured=subprocess.run([str(kit),'--version'],capture_output=True,text=True,check=True,timeout=10)
  result['versions']['isaac-python']=measured.stdout.strip().removeprefix('Python ')
 for name in ['VERSION','VERSION.txt']:
  p=root/name
  if p.is_file():result['versions']['isaacsim']=p.read_text().strip()
 for pattern in ['exts/*/config/extension.toml','extscache/*/config/extension.toml','extsDeprecated/*/config/extension.toml']:
  for p in root.glob(pattern):
   try:
    d=tomllib.loads(p.read_text());v=d.get('package',{}).get('version')
    if v:result['extensions'].append({'name':p.parent.parent.name,'version':str(v),'scope':'isaac-extension','kind':'extension','deprecated':'extsDeprecated' in p.parts,'origin':{'type':'bundled-extension'}})
   except (OSError,ValueError):pass
print(json.dumps(result))
'''


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def mount_metadata(policy, dependency_root=None):
    import email,tomllib
    dependencies=[];pins={};mounts=[]
    for mount in policy.get('mounts',[]):
        source=Path(mount['source'])
        if not source.exists():raise ValueError('Configured mount is missing: '+str(source))
        mounts.append({'source':str(source),'target':mount['target'],'readonly':mount.get('readonly',True)})
        if not source.is_dir():
            if source.suffix in ('.json','.toml','.kit'):pins[str(source)]=sha(source)
            continue
        # Distribution metadata only: never recurse into data, model or asset payloads.
        sites=[source,*source.glob('lib/python*/site-packages')]
        scope=mount['target']
        origin={'type':'managed-directory' if dependency_root and source.resolve().is_relative_to(Path(dependency_root).resolve()) else 'external-mount','path':str(source)}
        for site in sites:
            for path in sorted(site.glob('*.dist-info/METADATA')):
                message=email.message_from_bytes(path.read_bytes());name=message.get('Name');version=message.get('Version')
                if name and version:dependencies.append({'name':name,'version':version,'scope':scope,'kind':'python','origin':origin})
                pins[str(path)]=sha(path)
        p=source/'pyvenv.cfg'
        if p.is_file():
            pins[str(p)]=sha(p)
            for line in p.read_text().splitlines():
                if line.startswith('version ='):dependencies.append({'name':'Python','version':line.split('=',1)[1].strip(),'scope':scope,'kind':'python'})
        if (source/'.git').exists():
            query=['git','-c','safe.directory='+str(source),'-c','core.fsmonitor=false','-c','core.hooksPath=/dev/null','-C',str(source),'rev-parse','HEAD']
            result=subprocess.run(query,capture_output=True,text=True,timeout=15)
            if result.returncode==0:
                dependencies.append({'name':source.name,'version':result.stdout.strip(),'scope':scope,'kind':'git'})
                head=source/'.git/HEAD'
                if head.is_file():pins[str(head)]=sha(head)
        for relative in ['source/isaaclab/config/extension.toml','third_party/IsaacLab/source/isaaclab/config/extension.toml']:
            p=source/relative
            if p.is_file():
                pins[str(p)]=sha(p);value=tomllib.loads(p.read_text()).get('package',{}).get('version')
                if value:dependencies.append({'name':'isaaclab-extension','version':str(value),'scope':scope,'kind':'source'})
                release=p.parents[3]/'VERSION'
                if release.is_file():
                    pins[str(release)]=sha(release)
                    dependencies.append({'name':'IsaacLab','version':release.read_text().strip(),'scope':scope,'kind':'source-release'})
    # Same host directory can legitimately be mounted at multiple import paths.
    for dep in dependencies:
        if 'origin' not in dep:
            mount=next((m for m in mounts if m['target']==dep.get('scope')),None)
            if mount:dep['origin']={'type':'managed-directory' if dependency_root and Path(mount['source']).resolve().is_relative_to(Path(dependency_root).resolve()) else 'external-mount','path':mount['source']}
    return dependencies,pins,mounts


def capture(executor,instance,runtime_id,*,python,run=subprocess.run):
    config=json.loads((executor/'config.json').read_text())
    if runtime_id not in config['instances'][instance].get('runtimes',[]):raise PermissionError('Runtime outside instance grant')
    policy=config['runtimes'][runtime_id];identity='inventory-'+runtime_id
    inspected=run(['docker','image','inspect',policy['image']],capture_output=True,text=True,check=True,timeout=15)
    image=json.loads(inspected.stdout)[0]['Id'];name='loom-inventory-'+uuid.uuid4().hex
    account=config['instances'][instance]
    dependency_root=runtime_store.dependencies_root(config,instance) if account.get('dependencies') else None
    command=['docker','run','--name',name,'--read-only','--network','none','--cap-drop','ALL','--security-opt','no-new-privileges',
        '--user',str(account['uid'])+':'+str(account['gid']),'--cpus','1','--memory','1g','--pids-limit','64','--entrypoint',python,image,'-B','-c',PROBE]
    with installation_lock(executor):
        # A bounded metadata probe does not consume a GPU or mount research data.
        try:
            measured=run(command,capture_output=True,text=True,check=True,timeout=60)
            inventory=json.loads(measured.stdout)
        finally:
            cleanup=run(['docker','rm','-f',name],capture_output=True,text=True,timeout=20)
            if cleanup.returncode:raise RuntimeError('Metadata probe container return unconfirmed')
        dependencies,pins,mounts=mount_metadata(policy,dependency_root)
        dependencies += [{'name':'Python','version':inventory['python'],'scope':'image','kind':'python'}]
        driver=Path('/proc/driver/nvidia/version')
        if policy.get('driver_capabilities') and driver.is_file():
            import re
            version=re.search(r'Kernel Module[^\n]*?\b([0-9]{3}\.[0-9.]+)',driver.read_text())
            if version:
                pins[str(driver)]=sha(driver)
                dependencies.append({'name':'NVIDIA driver','version':version.group(1),'scope':'host','kind':'driver'})
        dependencies += inventory['packages']+inventory['extensions']
        dependencies += [{'name':key,'version':value,'scope':'image','kind':'python' if key=='isaac-python' else 'framework'} for key,value in inventory['versions'].items()]
        unique={(d['name'].casefold(),d.get('scope',''),d['version'],bool(d.get('deprecated'))):d for d in dependencies}
        for dep in unique.values():
            if 'origin' not in dep:dep['origin']={'type':'host' if dep.get('scope')=='host' else 'image'}
        manifest={'label':runtime_id,'runtime':{**policy,'image':image},'dependencies':list(unique.values()),
            'validation':{'status':'passed','checks':['non-root-readonly-cpu-metadata-probe','configured-mount-metadata'],
                          'scope':'installed-metadata; no GPU, imports or scientific acceptance'},
            'reconciliation':{'runtime_id':runtime_id,'policy_sha256':runtime_store.digest(policy),'image_id':image,
                'source_pins':pins,'mounts':mounts,'dependency_directory':account.get('dependencies'),'scope':'installed package metadata and declared mount inventory'}}
        ref=runtime_store.register(executor,identity,manifest,[instance])
        return {**ref,'dependencies':len(unique),'metadata_files':len(pins),'container_returned':True}


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--executor',type=Path,required=True);p.add_argument('--instance',required=True)
    p.add_argument('--runtime',required=True);p.add_argument('--python',required=True)
    a=p.parse_args();print(json.dumps(capture(a.executor,a.instance,a.runtime,python=a.python),indent=2))

if __name__=='__main__':main()
