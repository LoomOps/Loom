"""Build an immutable alpha release without changing either live instance."""
import argparse
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from zoneinfo import ZoneInfo
import snapshot
from host_identity import admin_reexec_for,build_owner

ROOT=Path(__file__).resolve().parents[1]


def prepare(version,destination):
    if not re.fullmatch(r'\d+\.\d+\.\d+-alpha\.\d+',version):
        raise ValueError('Expected a version such as 0.1.0-alpha.1')
    target=destination/version
    target.mkdir(parents=True,exist_ok=False)
    source=target/'source';source.mkdir()
    files=list(snapshot.sources())
    hashes={str(path.relative_to(ROOT)):hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    info={'build_id':version,'channel':'alpha','created_at':datetime.now(ZoneInfo('Asia/Seoul')).isoformat(),
        'source_sha256':hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest(),
        'git_commit':snapshot.git_revision(ROOT),'images':{'ops':'loom-ops:'+version,'dashboard':'loom-dashboard:'+version}}
    for path in files:
        output=source/path.relative_to(ROOT);output.parent.mkdir(parents=True,exist_ok=True)
        if path.is_symlink():output.symlink_to(os.readlink(path))
        else:output.write_bytes(path.read_bytes());output.chmod(path.stat().st_mode & 0o777)
    for component in ('ops','dashboard'):
        local=snapshot.component_files(component,files,hashes)
        (source/component/'build-info.json').write_text(json.dumps({**info,'component':component,'files':local},indent=2)+'\n')
    (target/'manifest.json').write_text(json.dumps({**info,'files':hashes},indent=2)+'\n')
    (target/'build.env').write_text(f'LOOM_BUILD_ID={version}\nLOOM_OPS_IMAGE=loom-ops:{version}\nLOOM_DASHBOARD_IMAGE=loom-dashboard:{version}\nLOOM_DEV_NO_AUTH=0\n')
    return target,info


def build(version,destination,instance):
    engine=['docker']
    if subprocess.run(engine+['info'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode:engine=['sudo','-n','docker']
    for name in ['ops','dashboard']:
        if subprocess.run(engine+['image','inspect','loom-'+name+':'+version],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0:
            raise ValueError('Release image tag already exists')
    target,info=prepare(version,destination)
    env={**os.environ,'LOOM_INSTANCE':str(instance),'LOOM_PROJECT':'loom','LOOM_BUILD_ENV':str(target/'build.env')}
    result={**info,'snapshot':str(target),'status':'building'}
    try:
        for name in ('ops','dashboard'):
            context=target/'source' if name=='ops' else target/'source/dashboard'
            dockerfile=target/'source'/name/('Dockerfile.loom' if name=='ops' else 'Dockerfile')
            subprocess.run(engine+['build','-t',info['images'][name],'-f',str(dockerfile),str(context)],check=True)
        result['image_ids']={name:subprocess.check_output(engine+['image','inspect',tag,'--format','{{.Id}}'],text=True).strip() for name,tag in info['images'].items()}
        result['status']='built'
    except BaseException:
        result['status']='failed'
        raise
    finally:
        (target/'result.json').write_text(json.dumps(result,indent=2)+'\n')
        build_owner(target)
    return result


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('version');p.add_argument('--destination',type=Path,default=Path('/loom/builds/alpha'))
    p.add_argument('--instance',type=Path,default=Path('/loom/ops'))
    a=p.parse_args();admin_reexec_for(a.destination);print(json.dumps(build(a.version,a.destination,a.instance),indent=2))
