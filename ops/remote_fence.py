"""Single, non-replicated online transfer authority for one logical node."""
import argparse
import errno
import hashlib
import hmac
import json
import os
import secrets
import sqlite3
import ssl
import time
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import HTTPSHandler, HTTPRedirectHandler, ProxyHandler, Request, build_opener


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):
        return None


def call(url,token,action,*,ca_file=None,**body):
    endpoint=urlsplit(url)
    if endpoint.scheme!='https' and not (endpoint.scheme=='http' and endpoint.hostname in
                                          ('127.0.0.1','fence-authority')):
        raise ValueError('Remote authority requires HTTPS outside the isolated fixture')
    if endpoint.username or endpoint.password or endpoint.path or endpoint.query or endpoint.fragment:
        raise ValueError('Invalid authority origin')
    req=Request(url+'/'+action,data=json.dumps(body).encode(),
                headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
    handlers=[ProxyHandler({}),NoRedirect()]
    if ca_file:
        handlers.append(HTTPSHandler(context=ssl.create_default_context(cafile=ca_file)))
    with build_opener(*handlers).open(req,timeout=5) as response:
        return json.load(response)


def apply(c,actor,action,body):
    row=c.execute('SELECT active,phase,epoch,session,archive FROM fence WHERE id=1').fetchone()
    active,phase,epoch,session,archive=row
    if action=='status':
        return {'active':active,'phase':phase,'epoch':epoch,'worker_claimed':session is not None}
    if action=='seal':
        if actor=='source' and phase=='to_target' and body.get('archive')==archive:
            return apply(c,actor,'status',{})
        if actor!='source' or phase!='source' or session or archive is not None:
            raise PermissionError('Source cannot seal with an owned worker')
        digest=body['archive']
        if not isinstance(digest,str) or len(digest)!=64 or any(x not in '0123456789abcdef' for x in digest):
            raise ValueError('Invalid archive identity')
        c.execute("UPDATE fence SET active=NULL,phase='to_target',archive=?,epoch=epoch+1 WHERE id=1",(digest,))
    elif action=='activate':
        if body.get('archive')!=archive:
            raise PermissionError('Stale or different transfer')
        expected={'to_target':'target','to_source':'source'}
        if active==actor and phase==actor:
            return {'active':actor,'epoch':epoch}
        if expected.get(phase)!=actor or session:
            raise PermissionError('Node has no activation grant')
        c.execute('UPDATE fence SET active=?,phase=?,epoch=epoch+1 WHERE id=1',(actor,actor))
    elif action=='return':
        if actor=='target' and phase=='to_source' and body.get('archive')==archive:
            return apply(c,actor,'status',{})
        if actor!='target' or active!=actor or phase!='target' or session or body.get('epoch')!=epoch:
            raise PermissionError('Target still owns a worker or is not active')
        if body.get('archive')!=archive:
            raise PermissionError('Stale or different transfer')
        c.execute("UPDATE fence SET active=NULL,phase='to_source',epoch=epoch+1 WHERE id=1")
    elif action in ('claim','check','release'):
        if active!=actor or body.get('epoch')!=epoch:
            raise PermissionError('Worker epoch is not current')
        claim=body.get('session')
        if not isinstance(claim,str) or len(claim)!=64 or any(x not in '0123456789abcdef' for x in claim):
            raise ValueError('Invalid session identity')
        digest=hashlib.sha256(claim.encode()).hexdigest()
        if action=='claim':
            if session not in (None,digest):
                raise PermissionError('Another worker owns this epoch')
            c.execute('UPDATE fence SET session=? WHERE id=1',(digest,))
        elif session!=digest:
            raise PermissionError('Session does not own the worker grant')
        elif action=='release':
            c.execute('UPDATE fence SET session=NULL WHERE id=1')
        return {'active':actor,'epoch':epoch,'authorized':True}
    else:
        raise ValueError('Unknown fence operation')
    return apply(c,actor,'status',{})


def initialize(path):
    fd=os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600)
    os.close(fd)
    with sqlite3.connect(path) as c:
        c.execute('CREATE TABLE fence(id INTEGER PRIMARY KEY,active TEXT,phase TEXT,epoch INTEGER,session TEXT,archive TEXT)')
        c.execute("INSERT INTO fence VALUES(1,'source','source',0,NULL,NULL)")


def listener(host,port,handler,timeout=120,*,clock=time.monotonic,sleep=time.sleep,factory=ThreadingHTTPServer):
    """Wait only for a not-yet-assigned address; other bind failures stay errors."""
    deadline=clock()+timeout
    while True:
        try:return factory((host,port),handler)
        except OSError as exc:
            if exc.errno!=errno.EADDRNOTAVAIL:raise
            remaining=deadline-clock()
            if remaining<=0:raise TimeoutError('Configured authority bind address did not become available') from exc
            sleep(min(1,remaining))


def serve(path,policy,host,port,cert=None,key=None):
    database=path.resolve().as_uri()+'?mode=rw'
    with sqlite3.connect(database,uri=True) as c:
        if not c.execute('SELECT 1 FROM fence WHERE id=1').fetchone():
            raise ValueError('Missing durable authority state')
    class Handler(BaseHTTPRequestHandler):
        def log_message(self,*_):
            pass

        def setup(self):
            self.request.settimeout(5)
            super().setup()

        def do_POST(self):
            try:
                length=int(self.headers.get('Content-Length','0'))
                if not 0<length<=2048:
                    raise ValueError('Invalid request')
                value=self.headers.get('Authorization','')
                if not value.startswith('Bearer '):
                    raise PermissionError('Machine credential required')
                digest=hashlib.sha256(value[7:].encode()).hexdigest()
                actor=next((role for role,grant in policy.items() if hmac.compare_digest(digest,grant['digest'])),None)
                if actor is None:
                    raise PermissionError('Invalid machine credential')
                body=json.loads(self.rfile.read(length))
                if body.get('binding') != policy[actor]['binding']:
                    raise PermissionError('Machine/storage binding does not match the grant')
                with sqlite3.connect(database,uri=True,timeout=10) as c:
                    c.execute('BEGIN IMMEDIATE')
                    result=apply(c,actor,self.path.lstrip('/'),body)
                status=200
            except PermissionError as e:
                status,result=403,{'error':str(e)}
            except (ValueError,TypeError,KeyError) as e:
                status,result=400,{'error':str(e)}
            payload=json.dumps(result).encode()
            self.send_response(status)
            self.send_header('Content-Type','application/json')
            self.send_header('Content-Length',str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
    server=listener(host,port,Handler)
    if cert:
        context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version=ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(cert,key)
        server.socket=context.wrap_socket(server.socket,server_side=True)
    server.serve_forever()


class WorkerGrant:
    def __init__(self,settings):
        self.url=settings['url']
        self.token=Path(settings['token_path']).read_text().strip()
        self.epoch=settings['epoch']
        self.binding=settings['binding']
        self.ca_file=settings.get('ca_file')
        self.session=secrets.token_hex(32)

    def request(self,action):
        return call(self.url,self.token,action,ca_file=self.ca_file,epoch=self.epoch,session=self.session,binding=self.binding)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('database',type=Path);p.add_argument('policy',type=Path)
    p.add_argument('--bind',default='127.0.0.1');p.add_argument('--port',type=int)
    p.add_argument('--tls-cert',type=Path);p.add_argument('--tls-key',type=Path)
    p.add_argument('--initialize',action='store_true')
    a=p.parse_args()
    if a.initialize:
        initialize(a.database)
    elif a.port is not None:
        serve(a.database,json.loads(a.policy.read_text()),a.bind,a.port,a.tls_cert,a.tls_key)
    else:
        p.error('Specify --port or initialize the authority once with --initialize')
