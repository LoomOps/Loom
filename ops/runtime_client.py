"""Cancellable container tickets through a private host executor socket."""
import storage
import hashlib
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import time

class Connection(http.client.HTTPConnection):
    def __init__(self,path):super().__init__('localhost',timeout=50);self.path=path
    def connect(self):self.sock=socket.socket(socket.AF_UNIX);self.sock.settimeout(self.timeout);self.sock.connect(self.path)

def call(config,action,body):
    conn=Connection(config['socket'])
    try:
        token=Path(config['credential']).read_text().strip()
        conn.request('POST','/'+action,json.dumps(body),{'Authorization':'Bearer '+token,'Content-Type':'application/json'})
        reply=conn.getresponse();result=json.loads(reply.read())
        if reply.status!=200:raise RuntimeError(result.get('error','Executor unavailable'))
        return result
    finally:conn.close()

class Busy(RuntimeError):pass

class Container:
    def __init__(self,config,row,spec,stage,request,folder,bulk):
        self.config=config['executor'];self.returncode=None;self.log=None;self.next_poll=0;self._logs_collected=False;self.folder=folder;self.stage=stage
        runner=spec['profile']['prepare'] if stage=='prepare' else spec['profile'].get(stage,spec['profile']['runtime'])
        component=spec['profile'].get('component_path')
        if component:
            for ref,expected in spec['profile'].get('source_pins',{}).items():
                if hashlib.sha256((Path(component)/ref).read_bytes()).hexdigest()!=expected:raise ValueError('Component revision changed')
        data=json.loads(request.read_text());inputs={}
        output_root,_=storage.output(config,bulk,spec)
        output_name=spec.get('output_storage','data/0')
        host_root=self.config['host_data'] if output_name=='data/0' else self.config.get('storage',{}).get(output_name)
        if not host_root:raise ValueError('Output storage outside executor grant')
        for name in spec['profile'].get('inputs',{}):
            value=data['params'].get(name)
            if not isinstance(value,str):continue
            for store,item in storage.stores(config,bulk).items():
                if Path(value).is_relative_to(item['path']):
                    ref=str(Path(value).relative_to(item['path']))
                    inputs[name]=str(Path(self.config['host_data'])/ref) if store=='data/0' else {'storage':store,'ref':ref}
                    data['params'][name]='/inputs/'+name;break
            else:raise ValueError('Input outside configured stores')
        stores=runner.get('storage',[])
        if set(stores)-set(self.config.get('storage',{})):raise ValueError('Storage outside instance grant')
        data['storage']={name:'/loom/'+name for name in stores}
        data['output_dir']='/work';data['job_id']=row['id'];data['attempt_id']=row['attempt'];data['stage']=stage
        target=folder/(stage+'-request.json');target.write_text(json.dumps(data))
        payload={'ticket':row['id']+'.'+row['attempt']+'.'+stage+('.r'+str(spec['profile']['_hook_retry']) if '_hook_retry' in spec['profile'] else ''),'job':row['id'],'owner':row['owner'],'stage':stage,
            'runtime_id':runner.get('runtime_id'),'lane':runner.get('lane','cpu'),'command':[*runner['command'],'/work/'+target.name],
            'env':runner.get('env',{}),'host_output':str(Path(host_root)/folder.relative_to(output_root)),
            'inputs':inputs,'component':spec['profile'].get('component_path')}
        if output_name!='data/0':payload['output_storage']=output_name
        if stores:payload['storage']=stores
        if 'image' in runner:payload.update(image=runner['image'],resources=runner.get('resources',{}))
        if component and 'host_components' in self.config:
            payload['component']=str(Path(self.config['host_components'])/Path(component).relative_to('/loom/components'))
        state=call(self.config,'start',payload)
        if state.get('busy'):raise Busy('GPU is owned by another instance')
        self.ticket=state['ticket']
        ticket_data={'ticket':self.ticket,'stage':stage,'runtime_id':runner.get('runtime_id'),'image_id':state.get('image_id'),'lane':runner.get('lane','cpu')}
        (folder/(stage+'-ticket.json')).write_text(json.dumps(ticket_data))
        if stage in ('run','prepare'):(folder/'container-ticket.json').write_text(json.dumps(ticket_data))
        self.poll()
    def __enter__(self):return self
    def __exit__(self,*args):
        if self.returncode is None:self.stop()
        if not self._logs_collected:
            try:
                name='worker.log' if self.stage=='run' else self.stage+'.log'
                with (self.folder/name).open('a') as output:output.write(self.logs())
            except (RuntimeError,OSError):pass
        return False
    def poll(self):
        if self.returncode is not None or time.monotonic()<self.next_poll:return self.returncode
        state=call(self.config,'status',{'ticket':self.ticket});self.next_poll=time.monotonic()+.5
        if state['returned']:self.returncode=state['exit_code']
        return self.returncode
    def wait(self,timeout=None):
        end=time.monotonic()+(timeout or 60)
        while self.poll() is None:
            if time.monotonic()>end:raise subprocess.TimeoutExpired('container',timeout)
            time.sleep(.1)
        return self.returncode
    def stop(self):
        state=call(self.config,'cancel',{'ticket':self.ticket})
        if not state['returned']:raise RuntimeError('Container did not return')
        self.returncode=state['exit_code'];return 'container_stopped'
    def logs(self):
        text=call(self.config,'logs',{'ticket':self.ticket})['text'];self._logs_collected=True;return text
