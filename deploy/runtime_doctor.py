"""Opt-in, bounded executor probe; never submits research or changes runtime policy."""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import sys
import tempfile
import time
import uuid

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'executor'))
from runtime_policy import gpu_driver_capabilities,runtime_network
from migrations import save

PROBE=r'''
import ctypes,json,os,sys,tempfile
from pathlib import Path
request=json.loads(Path('/work/probe.json').read_text())
result={'uid':os.getuid(),'gid':os.getgid(),'paths':[], 'libraries':[], 'scratch_writable':False}
result['driver_capabilities']=os.environ.get('NVIDIA_DRIVER_CAPABILITIES')
for path in request['paths']:
 p=Path(path);result['paths'].append({'path':path,'readable':os.access(p,os.R_OK|(os.X_OK if p.is_dir() else 0))})
with tempfile.TemporaryDirectory(dir='/tmp') as scratch:
 p=Path(scratch)/'write';p.write_text('probe');result['scratch_writable']=p.read_text()=='probe'
result['root_readonly']=any(v[4]=='/' and 'ro' in v[5].split(',') for v in (line.split() for line in Path('/proc/self/mountinfo').read_text().splitlines()))
for name in request['libraries']:
 try:ctypes.CDLL(name);loaded=True
 except OSError:loaded=False
 result['libraries'].append({'name':name,'loaded':loaded})
result['passed']=(result['uid']==request['uid'] and result['gid']==request['gid'] and result['scratch_writable'] and result['root_readonly'] and (not request['gpu'] or result['driver_capabilities']==request['driver_capabilities']) and all(p['readable'] for p in result['paths']) and all(p['loaded'] for p in result['libraries']))
Path('/work/probe-result.json').write_text(json.dumps(result))
raise SystemExit(0 if result['passed'] else 1)
'''


class ExecutorError(RuntimeError):
    def __init__(self,status):self.status=status;super().__init__('Executor request failed: HTTP '+str(status))


def call(executor,token,action,data):
    connection=http.client.HTTPConnection('localhost',timeout=10)
    connection.sock=socket.socket(socket.AF_UNIX);connection.sock.settimeout(10)
    try:
        connection.sock.connect(str(executor/'socket/executor.sock'))
        connection.request('POST','/'+action,json.dumps(data),{'Authorization':'Bearer '+token,'Content-Type':'application/json'})
        response=connection.getresponse();body=response.read(262145)
        if response.status!=200:raise ExecutorError(response.status)
        if len(body)>262144:raise ValueError('Executor response exceeds diagnostic limit')
        return json.loads(body)
    finally:connection.close()


def inspect(root,executor,runtime_id,gpu=False,paths=()):
    root,executor=Path(root).resolve(),Path(executor).resolve()
    config=json.loads((executor/'config.json').read_text());token=(root/'secrets/executor-token').read_text().strip()
    matches=[(name,c) for name,c in config['instances'].items() if c['token_sha256']==hashlib.sha256(token.encode()).hexdigest()]
    if len(matches)!=1:raise ValueError('Select the matching installation executor credential')
    instance,grant=matches[0]
    if runtime_id not in grant['runtimes']:raise ValueError('Runtime outside installation grant')
    runtime=config['runtimes'][runtime_id]
    if gpu and not grant.get('gpu'):raise ValueError('GPU disabled for this installation')
    targets=[m['target'] for m in runtime.get('mounts',[])]+list(paths)
    if any(not isinstance(p,str) or not Path(p).is_absolute() or '\x00' in p for p in targets):raise ValueError('Probe paths must be absolute container paths')
    result={'schema':'loom-runtime-doctor/v1','status':'unverified','instance':instance,'runtime_id':runtime_id,
        'image':runtime['image'],'uid':grant['uid'],'gid':grant['gid'],'gpu':gpu,
        'network':runtime_network(runtime),'driver_capabilities':gpu_driver_capabilities(runtime).split(','),
        'paths':list(dict.fromkeys(targets)),'scope':'execution UID/GID, selected mounts, read-only root, scratch and driver-library availability; no scene, model inference or evaluation'}
    return result,token,grant


def run(root,executor,runtime_id,*,gpu=False,paths=(),timeout=60,request=call,clock=time.monotonic,pause=time.sleep):
    if type(timeout) is not int or not 1<=timeout<=120:raise ValueError('Probe timeout must be 1 to 120 seconds')
    root,executor=Path(root).resolve(),Path(executor).resolve()
    result,token,grant=inspect(root,executor,runtime_id,gpu,paths)
    work=Path(tempfile.mkdtemp(prefix='.loom-runtime-doctor-',dir=grant['data']))
    os.chown(work,grant['uid'],grant['gid']);work.chmod(0o700)
    ticket='runtime-doctor.'+uuid.uuid4().hex
    result.update(ticket= result['instance']+'.'+ticket,workspace=str(work),cleanup='not_confirmed')
    record=root/'diagnostics/runtime-doctor'/(ticket+'.json');record.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
    result['record']=str(record);returned=False;attempted=False
    libraries=[]
    if gpu:
        libraries=['libcuda.so.1']
        if 'utility' in result['driver_capabilities']:libraries.append('libnvidia-ml.so.1')
        if 'graphics' in result['driver_capabilities']:libraries.append('libEGL_nvidia.so.0')
    payload={'uid':grant['uid'],'gid':grant['gid'],'paths':result['paths'],'libraries':libraries,
             'gpu':gpu,'driver_capabilities':','.join(result['driver_capabilities'])}
    config=work/'probe.json';config.write_text(json.dumps(payload));config.chmod(0o644)
    try:
        save(record,result);attempted=True
        state=request(executor,token,'start',{'ticket':ticket,'job':'runtime-doctor','stage':'doctor',
            'owner':'administrator','runtime_id':runtime_id,'lane':'gpu' if gpu else 'cpu',
            'host_output':str(work),'command':['python3','-I','-c',PROBE],'inputs':{}})
        if state.get('busy'):
            returned=True;result.update(status='pending',reason='gpu_busy');return result
        deadline=clock()+timeout
        while True:
            state=request(executor,token,'status',{'ticket':result['ticket']})
            if state.get('returned'):returned=True;break
            if clock()>=deadline:raise TimeoutError('Runtime diagnostic deadline reached')
            pause(.2)
        probe=work/'probe-result.json'
        if probe.is_symlink() or not probe.is_file() or probe.stat().st_size>65536:raise ValueError('Missing or invalid probe evidence')
        details=json.loads(probe.read_text())
        result.update(status='passed' if state.get('exit_code')==0 and details.get('passed') is True else 'failed',probe=details,image_id=state.get('image_id'))
        return result
    except Exception as exc:
        result.update(status='unverified' if not returned else 'failed',error_type=type(exc).__name__)
        return result
    finally:
        if attempted and not returned:
            try:returned=request(executor,token,'cancel',{'ticket':result['ticket']}).get('returned') is True
            except Exception:pass
        if returned:
            if result['status']=='unverified':result['status']='failed'
            try:request(executor,token,'logs',{'ticket':result['ticket']})
            except Exception:pass
            shutil.rmtree(work);result['cleanup']='returned_and_workspace_removed'
        else:result['cleanup']='unconfirmed; workspace retained, inspect this ticket before retrying'
        save(record,result)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--root',type=Path,default=Path('/loom/ops'))
    parser.add_argument('--executor',type=Path,default=Path('/loom/shared/executor'));parser.add_argument('--runtime',required=True)
    parser.add_argument('--gpu',action='store_true');parser.add_argument('--path',action='append',default=[])
    parser.add_argument('--run',action='store_true',help='Explicitly execute the bounded probe through the real broker')
    parser.add_argument('--timeout',type=int,default=60);args=parser.parse_args()
    try:
        result=run(args.root,args.executor,args.runtime,gpu=args.gpu,paths=args.path,timeout=args.timeout) if args.run else inspect(args.root,args.executor,args.runtime,args.gpu,args.path)[0]
        print(json.dumps(result,indent=2));raise SystemExit(0 if result['status']=='passed' or not args.run else 1)
    except (OSError,ValueError,KeyError) as exc:parser.exit(1,type(exc).__name__+': inspect runtime configuration and local permissions; no credentials are printed.\n')
