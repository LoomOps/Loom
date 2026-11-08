"""Install a clean Loom instance. Existing paths are never overwritten."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import uuid
import subprocess
import sys
import re
sys.path.insert(0,str(Path(__file__).resolve().parent))
from host_identity import identity, platform_layout, own_tree
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ops'))
from host_layout import create_layout,write_config,write_json


def install(root,data,api_port,dashboard_port,tls_port,project,ops_image,dashboard_image,uid=None,gid=None):
    if root.exists() or (data/'.loom-storage.json').exists() or (data/'.loom-storage.json').is_symlink():raise ValueError('Choose a new instance and an unclaimed data directory')
    existing_data=data.exists()
    if existing_data and not data.is_dir():raise ValueError('Data must be a directory')
    if len({api_port,dashboard_port,tls_port})!=3 or any(not 1024<=v<=65535 for v in (api_port,dashboard_port,tls_port)):raise ValueError('Choose three different unprivileged ports')
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,63}',project):raise ValueError('Invalid Compose project name')
    managed=uid is None and gid is None
    uid,gid=identity(uid,gid)
    if managed:platform_layout(root.parent,uid,gid)
    else:root.parent.mkdir(parents=True,exist_ok=True)
    create_layout(root);data.mkdir(parents=True,exist_ok=True)
    storage=secrets.token_hex(12);token=secrets.token_urlsafe(32)
    (data/'.loom-storage.json').write_text(json.dumps({'storage_id':storage}))
    st=data.stat();device=f'{os.major(st.st_dev)}:{os.minor(st.st_dev)}'
    installation_id=str(uuid.uuid4())
    config={'machine_id':'loom-'+installation_id,'machine_name':project,'installation_id':installation_id,'storage_id':storage,'storage_device':device,'inventory':{},'profiles':{},'runtime_ids':[],
        'actors':[{'id':'owner','owners':['owner'],'notify':['owner'],'scopes':['read','submit','cancel','events'],
                   'token_sha256':hashlib.sha256(token.encode()).hexdigest()}]}
    (root/'secrets/releases').mkdir(mode=0o700)
    write_config(root,config);write_json(root/'secrets/client-tokens.json',{'owner':token})
    tls=root/'secrets/tls';tls.mkdir(mode=0o700)
    subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','365','-subj','/CN=localhost',
        '-addext','subjectAltName=DNS:localhost,IP:127.0.0.1','-keyout',str(tls/'server-key.pem'),'-out',str(tls/'server.pem')],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    (tls/'server-key.pem').chmod(0o600)
    components=root.parent/'components'/project;profiles=root.parent/'profiles'/project
    for path in (components,profiles):
        if not path.exists():
            path.mkdir(parents=True);os.chown(path,uid,gid)
    values={'OPS_LOCAL':root,'OPS_RUNTIME':root/'runtime','OPS_API_CONFIG':root/'config/api.json','OPS_WORKER_CONFIG':root/'config/worker.json',
        'OPS_BULK':data,'OPS_UID':uid,'OPS_GID':gid,'OPS_PORT':api_port,'OPS_DASHBOARD_PORT':dashboard_port,'OPS_TLS_BIND':'127.0.0.1','OPS_TLS_PORT':tls_port,
        'LOOM_COMPONENTS_HOST':components,'LOOM_PROFILES_HOST':profiles}
    (root/'compose.env').write_text(''.join(f'{k}={v}\n' for k,v in values.items()));(root/'network.env').write_text('')
    (root/'build.env').write_text(f'LOOM_OPS_IMAGE={ops_image}\nLOOM_DASHBOARD_IMAGE={dashboard_image}\nLOOM_DEV_NO_AUTH=0\n')
    own_tree(root,uid,gid)
    if not existing_data:own_tree(data,uid,gid)
    else:os.chown(data/'.loom-storage.json',uid,gid)
    if managed:
        from host_services import install_observations
        install_observations(root.parent,root,project,uid,gid)
    return {'uid':uid,'gid':gid,'service_account':'loom' if managed else None,'root':str(root),'credential_file':str(root/'secrets/client-tokens.json'),'api_port':api_port,'dashboard_port':dashboard_port,
        'next':'Attach an executor instance/runtime to enable user component execution. Built-in CPU diagnostic is available.'}

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=Path('/loom/ops'));p.add_argument('--data',type=Path,required=True)
    p.add_argument('--api-port',type=int,required=True);p.add_argument('--dashboard-port',type=int,required=True);p.add_argument('--tls-port',type=int,required=True)
    p.add_argument('--project',default='loom');p.add_argument('--ops-image',required=True);p.add_argument('--dashboard-image',required=True)
    p.add_argument('--uid',type=int);p.add_argument('--gid',type=int)
    a=p.parse_args();print(json.dumps(install(a.root,a.data,a.api_port,a.dashboard_port,a.tls_port,a.project,a.ops_image,a.dashboard_image,a.uid,a.gid)))
