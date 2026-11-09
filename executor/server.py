"""Local host executor. Only Ops holds the Unix-socket credential; Jobs never do."""
import hashlib
from runtime_policy import declared, gpu_driver_capabilities, runtime_network
import hmac
import json
import os
from pathlib import Path
import re
import socketserver
import sqlite3
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler

ROOT=Path(sys.argv[1]);CONFIG=json.loads((ROOT/'config.json').read_text());LOCK=threading.RLock()
if any(v['uid']<=0 or v['gid']<=0 for v in CONFIG['instances'].values()):
    raise ValueError('Job instances require non-root UID/GID')
ID=re.compile(r'[a-zA-Z0-9_.-]{1,128}')

def docker(*args):
    proc=subprocess.run(['docker',*args],capture_output=True,text=True,timeout=45)
    if proc.returncode:raise RuntimeError(proc.stderr[-2000:])
    return proc.stdout

def inside(root,value):
    path=Path(value).resolve();base=Path(root).resolve()
    if not path.is_relative_to(base):raise ValueError('Path outside executor grant')
    return path

def db():
    c=sqlite3.connect(ROOT/'executor.sqlite3');c.row_factory=sqlite3.Row
    c.executescript('CREATE TABLE IF NOT EXISTS tickets(id TEXT PRIMARY KEY, instance TEXT, container TEXT, gpu INTEGER, state TEXT, request TEXT);')
    columns={r[1] for r in c.execute('PRAGMA table_info(tickets)')}
    if 'exit_code' not in columns:c.execute('ALTER TABLE tickets ADD COLUMN exit_code INTEGER')
    if 'image_id' not in columns:c.execute('ALTER TABLE tickets ADD COLUMN image_id TEXT')
    if 'logs' not in columns:c.execute('ALTER TABLE tickets ADD COLUMN logs TEXT')
    return c

def observed(row):
    if row['state']=='returned' and row['exit_code'] is not None:return {'state':'exited','returned':True,'exit_code':row['exit_code'],'image_id':row['image_id']}
    try:
        state=json.loads(docker('inspect',row['container']))[0]['State']
    except RuntimeError:
        return {'state':'unknown','returned':False}
    done=state['Status'] in ('exited','dead')
    if done:
        with db() as c:c.execute("UPDATE tickets SET state='returned',exit_code=? WHERE id=?",(state['ExitCode'],row['id']))
    return {'state':state['Status'],'returned':done,'exit_code':state['ExitCode'] if done else None}

def create_or_reuse(command, name, labels):
    """Recover a create whose Docker effect survived an uncommitted ticket.

    Never remove or start an unrecorded container unless all request/launch
    fingerprints match and it is still unstarted. A stored ticket remains the
    authority for running/exited containers and is committed before start.
    """
    try:
        docker(*command)
    except (RuntimeError, subprocess.TimeoutExpired) as creation_error:
        try:
            container=json.loads(docker('inspect',name))[0]
        except (RuntimeError, subprocess.TimeoutExpired, ValueError, IndexError):
            raise creation_error
    else:
        container=json.loads(docker('inspect',name))[0]
    actual=container.get('Config',{}).get('Labels') or {}
    if any(actual.get(key)!=value for key,value in labels.items()):
        raise ValueError('Container identity conflict; existing container left unchanged')
    if container.get('State',{}).get('Status')!='created':
        raise ValueError('Unrecorded container is not unstarted; existing container left unchanged')
    return container


def handle(instance,action,data):
    cfg=CONFIG['instances'][instance]
    with LOCK:
        if action=='resolve':
            runtime=declared(data,cfg.get('declared_runtime',{}),docker)
            return {'image':runtime['image'],'resources':{k:runtime[k] for k in ('cpus','memory','pids')}}
        if action=='start':
            if {'network','driver_capabilities'} & data.keys():raise PermissionError('Runtime network and driver capabilities are administrator policy')
            ticket=data['ticket']
            if not isinstance(ticket,str) or not ID.fullmatch(ticket):raise ValueError('Invalid ticket')
            ticket=instance+'.'+ticket
            with db() as c:
                old=c.execute('SELECT * FROM tickets WHERE id=?',(ticket,)).fetchone()
                if old:
                    if old['request']!=json.dumps(data,sort_keys=True):raise ValueError('Ticket conflict')
                    state=observed(old)
                    if state['state']=='created':docker('start',old['container']);state=observed(old)
                    return {'ticket':ticket,**state}
                if 'image' in data:
                    runtime=declared(data,cfg.get('declared_runtime',{}),docker)
                    if runtime['image']!=data['image']:raise ValueError('Execution requires a pinned image identity')
                else:
                    runtime=CONFIG['runtimes'].get(data['runtime_id'])
                    if not runtime or data['runtime_id'] not in cfg['runtimes']:raise PermissionError('Runtime outside instance grant')
                # Owner is attribution, not runtime authorization. The instance
                # runtime allowlist, GPU policy and resource limits still apply.
                gpu=data.get('lane')=='gpu'
                if gpu and not cfg.get('gpu'):raise PermissionError('GPU disabled for this instance')
                if gpu:
                    for row in c.execute("SELECT * FROM tickets WHERE gpu=1 AND state!='returned'").fetchall():
                        if not observed(row)['returned']:return {'busy':True}
                output_name=data.get('output_storage','data/0')
                output_root=cfg['data'] if output_name=='data/0' else cfg.get('storage',{}).get(output_name)
                if not output_root:raise PermissionError('Output storage outside instance grant')
                folder=inside(output_root,data['host_output'])
                if not folder.is_dir():raise ValueError('Missing Job workspace')
                name='loom-job-'+hashlib.sha256(ticket.encode()).hexdigest()[:24]
                argv=data['command']
                if not isinstance(argv,list) or not argv or any(not isinstance(x,str) or '\0' in x for x in argv):raise ValueError('Expected argv')
                cmd=['create','--name',name,'--label','loom.job='+data['job'],'--label','loom.instance='+instance,
                     '--label','loom.stage='+data['stage'],'--init','--user',f"{cfg['uid']}:{cfg['gid']}",
                     '--cap-drop=ALL','--security-opt=no-new-privileges','--read-only','--pids-limit',str(runtime.get('pids',512)),
                     '--cpus',str(runtime.get('cpus',4)),'--memory',runtime.get('memory','8g'),
                     '--network',runtime_network(runtime),'--tmpfs','/tmp:rw,nosuid,size=1g','--workdir','/work',
                     '--mount',f'type=bind,source={folder},target=/work',
                     '--env','HOME=/tmp','--env','PYTHONDONTWRITEBYTECODE=1','--env','LOOM_OUTPUT_DIR=/work']
                for key,value in sorted(data.get('env',{}).items()):
                    if not isinstance(key,str) or not isinstance(value,str) or '\0' in key+value or '=' in key:raise ValueError('Invalid environment')
                    cmd+=['--env',key+'='+value]
                for name_,ref in sorted(data.get('inputs',{}).items()):
                    if not ID.fullmatch(name_):raise ValueError('Invalid input name')
                    if isinstance(ref,dict):
                        if set(ref)!={'storage','ref'} or ref['storage'] not in cfg.get('storage',{}):raise PermissionError('Input storage outside instance grant')
                        base=cfg['storage'][ref['storage']];ref=str(Path(base)/ref['ref'])
                    else:base=cfg['data']
                    source=inside(base,ref)
                    if not source.is_file():raise ValueError('Input file unavailable')
                    cmd+=['--mount',f'type=bind,source={source},target=/inputs/{name_},readonly']
                if data.get('component'):
                    component=inside(cfg.get('components',CONFIG['components']),data['component'])
                    if not component.is_dir():raise ValueError('Component snapshot unavailable')
                    cmd+=['--mount',f'type=bind,source={component},target=/component,readonly']
                for alias,ref in sorted(data.get('tools',{}).items()):
                    if not ID.fullmatch(alias):raise ValueError('Invalid Tool alias')
                    components=Path(cfg.get('components',CONFIG['components']))
                    tool_root=inside(components,components/'.tools')
                    source=inside(tool_root,ref)
                    if not source.is_dir():raise ValueError('Tool snapshot unavailable')
                    cmd+=['--mount',f'type=bind,source={source},target=/tools/{alias},readonly']
                stores=data.get('storage',[])
                if not isinstance(stores,list) or any(not isinstance(s,str) or not re.fullmatch(r'data/[0-9]+',s) for s in stores) or len(stores)!=len(set(stores)):raise ValueError('Invalid storage identifiers')
                for store in stores:
                    if store not in cfg.get('storage',{}):raise PermissionError('Storage outside instance grant')
                    source=Path(cfg['storage'][store])
                    if source.resolve()!=source or not source.is_dir():raise ValueError('Storage path changed or unavailable')
                    cmd+=['--mount',f'type=bind,source={source},target=/loom/{store}']
                for mount in runtime.get('mounts',[]):
                    # Only the host administrator defines runtime library/asset mounts.
                    cmd+=['--mount',f"type=bind,source={mount['source']},target={mount['target']}"+(',readonly' if mount.get('readonly',True) else '')]
                if gpu:cmd+=['--gpus','device='+','.join(CONFIG.get('gpu_devices',['0'])),'--env','NVIDIA_DRIVER_CAPABILITIES='+gpu_driver_capabilities(runtime)]
                cmd+=['--entrypoint',argv[0],runtime['image'],*argv[1:]]
                labels={'loom.instance':instance,'loom.job':data['job'],'loom.stage':data['stage'],
                        'loom.ticket':ticket,
                        'loom.request-sha256':hashlib.sha256(json.dumps(data,sort_keys=True).encode()).hexdigest(),
                        'loom.launch-sha256':hashlib.sha256(json.dumps(cmd).encode()).hexdigest()}
                # Add identity before the image/entrypoint argv, never as runner arguments.
                cmd[1:1]=[item for key,value in labels.items() for item in ('--label',key+'='+value)]
                container=create_or_reuse(cmd,name,labels)
                c.execute('INSERT INTO tickets(id,instance,container,gpu,state,request,image_id) VALUES(?,?,?,?,?,?,?)',(ticket,instance,name,int(gpu),'created',json.dumps(data,sort_keys=True),container['Image']))
            docker('start',name)
            return {'ticket':ticket,'state':'running','returned':False,'image_id':json.loads(docker('inspect',name))[0]['Image']}
        if action=='recover':
            if not ID.fullmatch(data['job']) or not ID.fullmatch(data['attempt']):raise ValueError('Invalid recovery identity')
            result=[]
            with db() as c:rows=c.execute('SELECT * FROM tickets WHERE instance=? AND id LIKE ?',(instance,instance+'.'+data['job']+'.'+data['attempt']+'.%')).fetchall()
            for row in rows:
                request=json.loads(row['request'])
                if request['stage'] not in ('run','prepare'):continue
                state=observed(row);natural=state['returned']
                if not natural:
                    docker('stop','--time','10',row['container']);state=observed(row)
                if not state['returned']:raise RuntimeError('Recovery did not return container')
                result.append({'ticket':row['id'],'stage':request['stage'],'natural_return':natural,**state})
            return result
        ticket=data['ticket']
        with db() as c:row=c.execute('SELECT * FROM tickets WHERE id=? AND instance=?',(ticket,instance)).fetchone()
        if not row:raise PermissionError('Ticket outside instance')
        if action=='cancel':
            state=observed(row)
            if not state['returned']:
                docker('stop','--time','10',row['container'])
            return observed(row)
        if action=='status':return observed(row)
        if action in ('logs','log_range'):
            folder=ROOT/'logs';folder.mkdir(mode=0o700,exist_ok=True)
            logfile=folder/(hashlib.sha256(ticket.encode()).hexdigest()+'.log')
            if row['logs'] is None:
                temp=logfile.with_suffix('.tmp')
                with temp.open('wb') as stream:
                    process=subprocess.run(['docker','logs',row['container']],stdout=stream,stderr=subprocess.STDOUT,timeout=30)
                if process.returncode:temp.unlink(missing_ok=True);raise RuntimeError('Container logs unavailable')
                temp.replace(logfile)
                if observed(row)['returned']:
                    with logfile.open('rb') as f:f.seek(max(0,logfile.stat().st_size-262144));tail=f.read().decode('utf-8','replace')
                    with db() as c:c.execute('UPDATE tickets SET logs=? WHERE id=?',(tail,row['id']))
                    docker('rm',row['container'])
            if action=='log_range':
                offset=data.get('offset',0);limit=data.get('limit',65536)
                if type(offset) is not int or offset<0 or type(limit) is not int or not 1<=limit<=65536:raise ValueError('Invalid log cursor')
                if not logfile.exists():return {'available':False,'offset':offset,'next_offset':offset,'text':''}
                with logfile.open('rb') as f:
                    f.seek(offset);text=f.read(limit).decode('utf-8','replace');next_offset=f.tell()
                return {'available':True,'offset':offset,'next_offset':next_offset,'text':text}
            if row['logs'] is not None:return {'text':row['logs']}
            with logfile.open('rb') as f:f.seek(max(0,logfile.stat().st_size-262144));return {'text':f.read().decode('utf-8','replace')}
        raise ValueError('Unknown executor action')

class Handler(BaseHTTPRequestHandler):
    def log_message(self,*_):pass
    def do_POST(self):
        try:
            token=self.headers.get('Authorization','').removeprefix('Bearer ')
            digest=hashlib.sha256(token.encode()).hexdigest()
            instance=next((k for k,v in CONFIG['instances'].items() if hmac.compare_digest(v['token_sha256'],digest)),None)
            if not instance:raise PermissionError('Invalid executor credential')
            length=int(self.headers.get('Content-Length','0'))
            if not 0<length<=131072:raise ValueError('Request too large')
            result=handle(instance,self.path.strip('/'),json.loads(self.rfile.read(length)));status=200
        except PermissionError as e:status=403;result={'error':str(e)}
        except (ValueError,KeyError,TypeError) as e:status=400;result={'error':str(e)}
        except Exception as e:status=503;result={'error':type(e).__name__+': '+str(e)}
        body=json.dumps(result).encode();self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
class Server(socketserver.ThreadingMixIn,socketserver.UnixStreamServer):daemon_threads=True
(ROOT/'socket').mkdir(exist_ok=True)
socket=ROOT/'socket/executor.sock'
if socket.exists():socket.unlink()
server=Server(str(socket),Handler);os.chmod(socket,0o660);os.chown(socket,CONFIG['uid'],CONFIG['gid'])
# During UID migration, old and new instance users retain only socket access.
for uid in sorted({v['uid'] for v in CONFIG['instances'].values() if v['gid']!=CONFIG['gid']}):
    subprocess.run(['setfacl','-m',f'u:{uid}:rw',str(socket)],check=True)
server.serve_forever()
