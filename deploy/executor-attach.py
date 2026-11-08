"""Attach a host executor with its own credential to one installed instance."""
import argparse,hashlib,json,os,secrets,sys,shutil,subprocess,sqlite3
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ops'))
from host_layout import write_config,write_json
from host_identity import directory,protected_chain
p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);p.add_argument('--executor',type=Path,default=Path('/loom/shared/executor'));p.add_argument('--instance',required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--gpu',action='store_true');p.add_argument('--install-service',action='store_true');p.add_argument('--uid',type=int);p.add_argument('--gid',type=int);a=p.parse_args()
env=dict(line.split('=',1) for line in (a.root/'compose.env').read_text().splitlines() if '=' in line)
uid,gid=int(env['OPS_UID']),int(env['OPS_GID'])
if a.uid not in (None,uid) or a.gid not in (None,gid):raise SystemExit('Executor UID/GID must match the installed instance')
a.uid,a.gid=uid,gid
if uid<=0 or gid<=0:raise SystemExit("Job execution requires a non-root UID/GID")
if os.geteuid()!=0:raise SystemExit('Host executor installation requires sudo')
# The broker and every parent must be protected from service-user replacement.
directory(a.executor,0,0,0o755)
protected_chain(a.executor)
if a.install_service and (a.executor/'executor.sqlite3').exists():
    with sqlite3.connect(a.executor/'executor.sqlite3') as c:
        if c.execute("SELECT 1 FROM tickets WHERE state!='returned' LIMIT 1").fetchone():raise SystemExit('Wait for executor tickets to return before reinstalling its service')
path=a.executor/'config.json'
settings=json.loads(path.read_text()) if path.exists() else {'uid':0,'gid':a.gid,'components':'/loom/components','gpu_devices':['0'],'runtimes':{'python':{'image':'python:3.12-slim','memory':'4g','cpus':2,'network':'none'}},'instances':{}}
if a.instance in settings['instances']:raise SystemExit('Executor instance already registered')
token=secrets.token_urlsafe(32)
settings['instances'][a.instance]={'uid':a.uid,'gid':a.gid,'data':str(a.data.resolve()),'components':env.get('LOOM_COMPONENTS_HOST','/loom/components'),'gpu':a.gpu,'runtimes':list(settings['runtimes']),'declared_runtime':{'enabled':True,'max_cpus':4,'max_memory':'8g','max_pids':1024},'token_sha256':hashlib.sha256(token.encode()).hexdigest()}
write_json(path,settings);os.chown(path,0,0);path.chmod(0o600)
secret=a.root/'secrets/executor-token';secret.write_text(token);secret.chmod(0o600)
config=json.loads((a.root/'config/api.json').read_text());config.update(runtime_ids=list(settings['runtimes']),allow_gpu=a.gpu,
    executor={'socket':'/run/loom-executor/executor.sock','credential':'/etc/loom/executor-token','host_data':str(a.data.resolve()),'host_components':env.get('LOOM_COMPONENTS_HOST','/loom/components')})
write_config(a.root,config)
(a.root/'executor.env').write_text(f"LOOM_EXECUTOR_HOST={a.executor}\nLOOM_COMPONENTS_HOST={env.get('LOOM_COMPONENTS_HOST','/loom/components')}\n")
for file in [secret,a.root/'config/api.json',a.root/'config/worker.json',a.root/'executor.env']:os.chown(file,a.uid,a.gid)
if a.install_service:
    if os.geteuid()!=0:raise SystemExit('--install-service requires host administration')
    source=Path(__file__).resolve().parents[1]
    for name in ('server.py','runtime_policy.py'):
        destination=a.executor/name
        if destination.is_symlink():raise SystemExit('Linked executor source')
        shutil.copyfile(source/'executor'/name,destination);os.chown(destination,0,0);destination.chmod(0o644)
    unit=(source/'executor/loom-executor.service').read_text().replace('/loom/shared/executor',str(a.executor))
    Path('/etc/systemd/system/loom-executor.service').write_text(unit)
    subprocess.run(['systemctl','daemon-reload'],check=True)
    subprocess.run(['systemctl','enable','loom-executor.service'],check=True)
    subprocess.run(['systemctl','restart','loom-executor.service'],check=True)
print('Attached executor'+(' and installed host service' if a.install_service else '; start/restart the host executor before starting this instance.'))
