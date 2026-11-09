"""Capture a numbered, immutable internal code snapshot; no service restart."""
import argparse
from datetime import datetime
import fcntl
import hashlib
import json
from pathlib import Path
import os
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from host_identity import admin_reexec_for,build_owner
import subprocess
import tarfile
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = {'evidence', '__pycache__', '.git', 'node_modules', 'secrets'}


def sources():
    for top in ('ops', 'dashboard', 'components', 'deploy', 'loom_use', 'docs', 'executor', 'tests'):
        for path in sorted((ROOT/top).rglob('*')):
            if EXCLUDED.intersection(path.relative_to(ROOT).parts):
                continue
            if path.name == 'build-info.json' or path.suffix in {'.pyc','.sqlite3','.log','.pem','.env'}:
                continue
            if path.name.endswith(('.pending.html','private.json')) or path.name == 'client-tokens.json':
                continue
            if path.is_symlink():
                if not path.resolve().is_relative_to(ROOT):
                    raise ValueError(f'Source link leaves the checkout: {path.relative_to(ROOT)}')
            if path.is_file():
                yield path
    for name in ('LICENSE','THIRD_PARTY_NOTICES.md','AGENTS.md','README.md','CHANGELOG.md','INSTALL.md','.dockerignore'):
        if (ROOT/name).is_file():yield ROOT/name


def component_files(component,files,hashes):
    local={str(p.relative_to(ROOT/component)):hashes[str(p.relative_to(ROOT))]
           for p in files if p.is_relative_to(ROOT/component)}
    if component=='ops':
        local.update({'../'+str(p.relative_to(ROOT)):hashes[str(p.relative_to(ROOT))]
                      for p in files if p.is_relative_to(ROOT/'loom_use')})
        if 'dashboard/diagnostics.py' in hashes:local['../diagnostics/diagnostics.py']=hashes['dashboard/diagnostics.py']
    return local


def capture(destination, instance):
    destination.mkdir(parents=True, exist_ok=True)
    with (destination/'.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        now = datetime.now(ZoneInfo('Asia/Seoul'))
        prefix = 'dev-build.'+now.strftime('%Y%m%d')+'.'
        ids = [int(p.name.removeprefix(prefix)) for p in destination.glob(prefix+'*')
               if p.is_dir() and p.name.removeprefix(prefix).isdigit()]
        build_id = prefix+f'{max(ids, default=0)+1:03d}'
        target = destination/build_id
        target.mkdir()
        files = list(sources())
        hashes = {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
        digest = hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest()
        commit = subprocess.run(['git','-C',str(ROOT),'rev-parse','HEAD'],capture_output=True,text=True)
        info = {'build_id':build_id, 'channel':'dev-build', 'created_at':now.isoformat(),
                'source_sha256':digest, 'git_commit':commit.stdout.strip() if commit.returncode==0 else None,
                'images':{'ops':'loom-ops:'+build_id,'dashboard':'loom-dashboard:'+build_id}}
        for component in ('ops','dashboard'):
            local = component_files(component,files,hashes)
            (ROOT/component/'build-info.json').write_text(json.dumps({**info,'component':component,'files':local},indent=2)+'\n')
        (target/'manifest.json').write_text(json.dumps({**info,'files':hashes},indent=2)+'\n')
        with tarfile.open(target/'source.tar.gz','w:gz',dereference=False) as archive:
            for path in files+[ROOT/'ops/build-info.json',ROOT/'dashboard/build-info.json']:
                archive.add(path,arcname=str(path.relative_to(ROOT)),recursive=False)
        (instance/'build.env').write_text(f'LOOM_BUILD_ID={build_id}\nLOOM_OPS_IMAGE=loom-ops:{build_id}\nLOOM_DASHBOARD_IMAGE=loom-dashboard:{build_id}\n')
        info_file=instance/'build.env'
        owner=(instance/'compose.env').stat()
        os.chown(info_file,owner.st_uid,owner.st_gid)
        build_owner(target)
        print(json.dumps({**info,'snapshot':str(target)},indent=2))


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination',type=Path,default=Path('/loom/builds/dev-build'))
    parser.add_argument('--instance',type=Path,default=Path('/loom/ops/dev-build'))
    args=parser.parse_args()
    admin_reexec_for(args.destination)
    capture(args.destination,args.instance)


def git_revision(root):
    """Commit is provenance; source_sha256 still describes uncommitted content."""
    import subprocess
    try:return subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True,stderr=subprocess.DEVNULL).strip()
    except (OSError,subprocess.CalledProcessError):return None
