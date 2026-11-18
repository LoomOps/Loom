"""Local, uncached CPU acceptance of image assembly from exact native artifacts."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.error import URLError
from urllib.request import Request,urlopen
import uuid


def docker(*args):
    return subprocess.check_output(['docker',*args],text=True,stderr=subprocess.STDOUT,timeout=120).strip()

image='loom-native-acceptance:'+uuid.uuid4().hex
container=None
try:
    with tempfile.TemporaryDirectory(prefix='loom-image-acceptance-') as temporary:
        root=Path(temporary);context=root/'context';context.mkdir()
        for name,path in zip(['loom-ops','loom-sdk'],sys.argv[1:]):shutil.copy2(Path(path).resolve(),context/name)
        (context/'Dockerfile').write_text('FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc\nCOPY loom-ops loom-sdk /usr/local/bin/\nENTRYPOINT ["loom-ops"]\n')
        print(docker('build','-t',image,str(context)),flush=True)
        fixture=root/'fixture';fixture.mkdir()
        for name in ['state','data','components']:(fixture/name).mkdir()
        (fixture/'data/.loom-storage.json').write_text('{"storage_id":"image-fixture"}')
        token='isolated-image-only'
        (fixture/'config.json').write_text(json.dumps({'machine_id':'image-fixture','storage_id':'image-fixture','actors':[{'id':'owner','scopes':['read','submit','cancel','events'],'owners':['owner'],'notify':['owner'],'token_sha256':hashlib.sha256(token.encode()).hexdigest()}]}))
        container=docker('run','-d','--user',f'{os.getuid()}:{os.getgid()}','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,size=32m','-p','127.0.0.1::8080','-v',str(fixture)+':/fixture:rw','-e','LOOM_SDK_BINARY=/usr/local/bin/loom-sdk',image,'--config','/fixture/config.json','--state','/fixture/state','--data','/fixture/data','--components','/fixture/components','--listen','0.0.0.0:8080')
        address=docker('port',container,'8080/tcp')
        url='http://'+address
        def call(group,action,arguments):
            request=Request(url+'/api/actions',data=json.dumps({'group':group,'action':action,'arguments':arguments}).encode(),headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
            with urlopen(request,timeout=3) as response:return json.load(response)
        for _ in range(100):
            try:status=call('system','status',{});break
            except (OSError,URLError):time.sleep(.1)
        else:raise RuntimeError('Image API did not start: '+docker('logs',container))
        expected=json.loads(subprocess.check_output([sys.argv[1],'--build-info'],text=True))
        assert status['build_id']==expected['build_id']
        job=call('jobs','submit',{'key':'image-smoke','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False}})['id']
        for _ in range(100):
            result=call('jobs','get',{'id':job})
            if result['state']=='complete':break
            time.sleep(.1)
        else:raise RuntimeError(json.dumps(result))
        assert result['result']['artifact_ready']
        print('Image API + real CPU producer + result publication passed',flush=True)
except BaseException:
    if container:print(docker('logs',container),flush=True)
    raise
finally:
    if container:print(docker('rm','-f',container))
    subprocess.run(['docker','image','rm',image],capture_output=True,timeout=30)
