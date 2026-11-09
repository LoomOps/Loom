"""Install/diagnose the optional managed host SSH capability. Does not issue accounts."""
import argparse,hashlib,json,os,shutil,subprocess,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).parent))
from host_services import unit

SOURCE=Path(__file__).resolve().parents[1]

def paths(root):
    key=hashlib.sha256(str(root.resolve()).encode()).hexdigest()[:12]
    return Path('/var/lib/loom-ssh')/key,Path('/run/loom-ssh')/key,'loom-ssh-'+key+'.service'

def run(*args):return subprocess.check_output(args,text=True,stderr=subprocess.DEVNULL).strip()

def protected(path,mode=0o700):
    if any(p.is_symlink() for p in [path,*path.parents]):raise ValueError('linked_host_path')
    path.mkdir(parents=True,exist_ok=True,mode=mode)
    os.chown(path,0,0);path.chmod(mode)
    # The root helper's parent directories must not be writable by the service user.
    for p in path.parents:
        st=p.stat()
        if st.st_uid!=0 or st.st_mode&0o022:raise ValueError('unprotected_host_path')

def share_stores(root,state,activate):
    api=json.loads((root/'config/api.json').read_text());executor=api.get('executor',{})
    stores=executor.get('storage') or {'data/0':executor.get('host_data')}
    stores={k:str(Path(v).resolve()) for k,v in stores.items() if v}
    group='loomssh_'+state.name
    for path in stores.values():
        p=Path(path)
        if p==Path('/') or p==root or root.is_relative_to(p) or p.is_relative_to(root):raise ValueError('storage_overlaps_service_state')
        if not p.is_dir():raise ValueError('storage_missing')
    if activate:
        import grp
        try:grp.getgrnam(group)
        except KeyError:run('groupadd','--system',group)
        previous=json.loads((state/'storage.json').read_text()) if (state/'storage.json').exists() else {}
        for key,path in stores.items():
            if previous.get(key)==path:continue
            # Only explicitly registered data stores; never service state or dependencies.
            for ancestor in reversed(Path(path).parents):
                if str(ancestor)!='/':run('setfacl','-m','g:'+group+':--x',str(ancestor))
            run('setfacl','-R','-P','-m','g:'+group+':rwX',path)
            # Default ACL is applied to directories only, without following symlinks.
            for folder,dirs,_ in os.walk(path,followlinks=False):
                run('setfacl','-m','d:g:'+group+':rwx',folder)
        (state/'storage.json').write_text(json.dumps(stores))
    return group,stores


def configure_service(state,name):
    dest=Path('/usr/local/lib/loom-ssh/ssh_host.py')
    unit(name,f'''[Unit]
Description=Loom personal SSH account manager
After=tailscaled.service
[Service]
Type=simple
User=root
UMask=0077
RuntimeDirectory=loom-ssh/{state.name}
RuntimeDirectoryMode=0755
RuntimeDirectoryPreserve=yes
ExecStart=/usr/bin/python3 -B {dest} {state}/config.json
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=false
ReadWritePaths=/etc /home /var/lib/loom-ssh /run
[Install]
WantedBy=multi-user.target
''')
    dropin=Path('/etc/systemd/system/loom-stack.service.d');dropin.mkdir(parents=True,exist_ok=True)
    (dropin/'ssh.conf').write_text('[Unit]\nRequires='+name+'\nAfter='+name+'\n')

def install(root,activate=True):
    root=root.resolve()
    if (root/'development').exists():raise ValueError('SSH host management is not enabled in dev-build; use isolated test fixtures')
    c=json.loads((root/'config/access-plan.json').read_text());state,runtime,name=paths(root)
    protected(Path('/var/lib/loom-ssh'));protected(state)
    services=Path('/usr/local/lib/loom-ssh');protected(services,0o755)
    dest=services/'ssh_host.py';temporary=services/'ssh_host.pending';shutil.copyfile(SOURCE/'deploy/ssh_host.py',temporary);os.chown(temporary,0,0);temporary.chmod(0o644);os.replace(temporary,dest)
    uid,gid=root.stat().st_uid,root.stat().st_gid
    group,stores=share_stores(root,state,activate)
    config={'shared_group':group,'stores':stores,'state':str(state),'socket':str(runtime/'control.sock'),'client_uid':uid,'client_gid':gid,'homes':'/home'}
    (state/'config.json').write_text(json.dumps(config));(state/'config.json').chmod(0o600)
    info=root/'ssh-info';info.mkdir(exist_ok=True);os.chown(info,uid,gid);info.chmod(0o755)
    if not (info/'connections.json').exists():(info/'connections.json').write_text('{}')
    os.chown(info/'connections.json',uid,gid)
    from urllib.parse import urlsplit
    (info/'machine.json').write_text(json.dumps({'host':urlsplit(c['dashboard_origin']).hostname,'addresses':c['addresses'],'data_paths':stores}))
    os.chown(info/'machine.json',uid,gid)
    configure_service(state,name)
    # Only the private Dashboard receives the control socket; Ops sees metadata only.
    overlay={'services':{'loom-dashboard':{'environment':{'LOOM_SSH_SOCKET':'/run/loom-ssh/control.sock','LOOM_SSH_EXPORT':'/var/lib/loom-ssh-info'},'volumes':[
        {'type':'bind','source':str(runtime),'target':'/run/loom-ssh','read_only':True,'bind':{'create_host_path':False}},
        {'type':'bind','source':str(info),'target':'/var/lib/loom-ssh-info','bind':{'create_host_path':False}}]},
        'loom-ops':{'volumes':[{'type':'bind','source':str(info),'target':'/etc/loom/ssh-info','read_only':True,'bind':{'create_host_path':False}}]}}}
    (root/'compose-ssh.yaml').write_text(json.dumps(overlay,indent=2)+'\n');os.chown(root/'compose-ssh.yaml',uid,gid)
    (root/'config/ssh.json').write_text(json.dumps({'enabled':True,'unit':name,'state':str(state),'socket':str(runtime/'control.sock')}));os.chown(root/'config/ssh.json',uid,gid)
    if activate:
        run('tailscale','set','--ssh=true')
        run('systemctl','daemon-reload');run('systemctl','enable','--now',name);run('systemctl','restart',name)
    return {'installed':True,'unit':name,'accounts_created':0,'compose_apply_required':True}

def doctor(root):
    p=root/'config/ssh.json'
    if not p.exists():return {'enabled':False,'ready':True}
    c=json.loads(p.read_text());state,runtime,name=paths(root)
    checks={}
    checks['configuration']=c.get('unit')==name and c.get('socket')==str(runtime/'control.sock')
    try:checks['service']=run('systemctl','is-active',name)=='active' and run('systemctl','is-enabled',name)=='enabled'
    except subprocess.CalledProcessError:checks['service']=False
    try:checks['tailscale_ssh']=json.loads(run('tailscale','debug','prefs')).get('RunSSH') is True
    except (ValueError,subprocess.CalledProcessError):checks['tailscale_ssh']=False
    sys.path.insert(0,str(SOURCE/'dashboard'));from ssh_access import host
    prior=os.environ.get('LOOM_SSH_SOCKET');os.environ['LOOM_SSH_SOCKET']=c['socket']
    try:host('list');checks['socket']=True
    except (OSError,ValueError):checks['socket']=False
    finally:
        if prior is None:os.environ.pop('LOOM_SSH_SOCKET',None)
        else:os.environ['LOOM_SSH_SOCKET']=prior
    checks['metadata']=(root/'ssh-info/connections.json').is_file() and (root/'ssh-info/machine.json').is_file()
    try:
        code=Path('/usr/local/lib/loom-ssh/ssh_host.py');st=code.stat()
        checks['protected_helper']=st.st_uid==0 and not st.st_mode&0o022 and not code.is_symlink()
        api=json.loads((root/'config/api.json').read_text());ex=api.get('executor',{})
        wanted=ex.get('storage') or {'data/0':ex.get('host_data')}
        checks['storage_bindings']=json.loads((state/'storage.json').read_text())=={k:str(Path(v).resolve()) for k,v in wanted.items() if v}
        mounts={}
        for role in ('loom-ops','loom-dashboard','loom-gateway'):
            ids=run('docker','ps','-q','--filter','label=com.docker.compose.project=loom','--filter','label=com.docker.compose.service='+role).split()
            if len(ids)!=1:raise ValueError('service_not_unique')
            item=json.loads(run('docker','inspect',ids[0]))[0];mounts[role]={m['Destination']:m for m in item['Mounts']}
            if role=='loom-dashboard':
                run('docker','exec',ids[0],'python3','-c',"from ssh_access import host; assert isinstance(host('list')['accounts'],list)")
        ops=mounts['loom-ops'];dash=mounts['loom-dashboard'];gateway=mounts['loom-gateway']
        checks['mounts']=dash.get('/run/loom-ssh',{}).get('Source')==str(runtime) and dash.get('/var/lib/loom-ssh-info',{}).get('Source')==str(root/'ssh-info') and ops.get('/etc/loom/ssh-info',{}).get('Source')==str(root/'ssh-info') and ops.get('/etc/loom/ssh-info',{}).get('RW') is False and '/run/loom-ssh' not in ops and not any('loom-ssh' in k for k in gateway)
    except (OSError,ValueError,KeyError,subprocess.CalledProcessError):checks['host_integration']=False
    return {'enabled':True,'ready':all(checks.values()),'checks':checks,'live_login':'not_tested'}

def disable_all(root):
    p=root/'config/ssh.json'
    if not p.exists():return
    c=json.loads(p.read_text());from ssh_host import Accounts
    accounts=Accounts(json.loads((Path(c['state'])/'config.json').read_text()))
    for row in accounts.handle({'action':'list'})['accounts']:accounts.handle({'action':'disable','owner':row['owner'],'username':row['name']})
    run('systemctl','disable','--now',c['unit'])
    (root/'ssh-info/connections.json').write_text('{}')
    # Accounts and homes are retained. Tailscale policy cleanup uses its owned journal.

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('action',choices=['install','doctor','disable-all']);p.add_argument('root',type=Path)
    a=p.parse_args();result=install(a.root) if a.action=='install' else doctor(a.root) if a.action=='doctor' else disable_all(a.root)
    print(json.dumps(result,indent=2))
