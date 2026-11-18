"""Capture a numbered, immutable internal code snapshot; no service restart."""
import argparse
import io
from datetime import datetime
import fcntl
import hashlib
import json
from pathlib import Path
import os
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
import subprocess
import tarfile
from uuid import uuid4
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = {'evidence', '__pycache__', '.git', 'node_modules', 'secrets'}


def sources():
    for top in ('ops', 'dashboard', 'components', 'deploy', 'loom_use', 'docs', 'executor', 'tests', 'rust', 'build_support'):
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
    for name in ('LICENSE','THIRD_PARTY_NOTICES.md','AGENTS.md','README.md','CHANGELOG.md','INSTALL.md','.dockerignore','Cargo.toml','Cargo.lock','rust-toolchain.toml','BUILD.bazel','MODULE.bazel','MODULE.bazel.lock','.bazelrc','.bazelversion'):
        if (ROOT/name).is_file():yield ROOT/name


def component_files(component,files,hashes):
    local={str(p.relative_to(ROOT/component)):hashes[str(p.relative_to(ROOT))]
           for p in files if p.is_relative_to(ROOT/component)}
    if component=='ops':
        local.update({'../'+str(p.relative_to(ROOT)):hashes[str(p.relative_to(ROOT))]
                      for p in files if p.is_relative_to(ROOT/'loom_use')})
        if 'dashboard/diagnostics.py' in hashes:local['../diagnostics/diagnostics.py']=hashes['dashboard/diagnostics.py']
    return local


def capture(destination, instance=None):
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
        # Numbering is local to the destination; Docker tags share one namespace.
        tag = f'{build_id}-{uuid4().hex}'
        info = {'build_id':build_id, 'channel':'dev-build', 'created_at':now.isoformat(),
                'source_sha256':digest, 'git_commit':commit.stdout.strip() if commit.returncode==0 else None,
                'images':{'ops':'loom-ops:'+tag,'dashboard':'loom-dashboard:'+tag}}
        generated = {}
        for component in ('ops','dashboard'):
            local = component_files(component,files,hashes)
            generated[component+'/build-info.json'] = (json.dumps({**info,'component':component,'files':local},indent=2)+'\n').encode()
        (target/'manifest.json').write_text(json.dumps({**info,'files':hashes},indent=2)+'\n')
        with tarfile.open(target/'source.tar.gz','w:gz',dereference=False) as archive:
            for path in files:
                archive.add(path,arcname=str(path.relative_to(ROOT)),recursive=False)
            for name,content in generated.items():
                entry=tarfile.TarInfo(name);entry.size=len(content);entry.mode=0o644
                archive.addfile(entry,io.BytesIO(content))
        result={**info,'snapshot':str(target)}
        if instance is not None:select(result,instance)
        return result


def select(info,instance):
    """Explicit development alias selection, separate from artifact capture."""
    from host_identity import build_owner
    instance=Path(instance)
    owner=(instance/'compose.env').stat()
    path=instance/'build.env'
    path.write_text(f"LOOM_BUILD_ID={info['build_id']}\nLOOM_OPS_IMAGE={info['images']['ops']}\nLOOM_DASHBOARD_IMAGE={info['images']['dashboard']}\n")
    os.chown(path,owner.st_uid,owner.st_gid)
    build_owner(Path(info['snapshot']))


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination',type=Path,default=ROOT/'target/dev-builds')
    parser.add_argument('--instance',type=Path,help='Explicit dev instance alias selection; artifact capture alone is inert')
    args=parser.parse_args()
    print(json.dumps(capture(args.destination,args.instance),indent=2))


def git_revision(root):
    """Commit is provenance; source_sha256 still describes uncommitted content."""
    import subprocess
    try:return subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True,stderr=subprocess.DEVNULL).strip()
    except (OSError,subprocess.CalledProcessError):return None
