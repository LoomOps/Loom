"""Extract immutable native artifacts from a verified image without starting it."""
import hashlib,json,subprocess
from pathlib import Path
BINARIES=('loom-ops','loom-host','loom-sdk','loom-migrate')
def extract(image,destination,engine=('docker',),run=subprocess.run):
    destination=Path(destination)
    if destination.exists():raise ValueError('Native artifact destination already exists')
    destination.mkdir(parents=True)
    created=run([*engine,'create','--network','none','--entrypoint','/bin/false',image],check=True,capture_output=True,text=True)
    container=created.stdout.strip()
    try:
        if not container or not all(c in '0123456789abcdef' for c in container):raise ValueError('Invalid image container identity')
        for name in BINARIES:
            run([*engine,'cp',container+':/usr/local/bin/'+name,str(destination/name)],check=True,capture_output=True)
            (destination/name).chmod(0o755)
    finally:
        if container:run([*engine,'rm',container],check=True,capture_output=True)
    files={name:hashlib.sha256((destination/name).read_bytes()).hexdigest() for name in BINARIES}
    (destination/'manifest.json').write_text(json.dumps({'schema':'loom-native-artifacts/v1','image':image,'files':files},indent=2)+'\n')
    return files

def verify(destination):
    root=Path(destination);manifest=json.loads((root/'manifest.json').read_text())
    if manifest.get('schema')!='loom-native-artifacts/v1' or set(manifest['files'])!=set(BINARIES):raise ValueError('Invalid native manifest')
    for name,digest in manifest['files'].items():
        path=root/name
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest()!=digest:raise ValueError('Native artifact changed')
    return manifest

def verify_release(release):
    release=Path(release)
    if not (release/'source/Cargo.toml').is_file():return None
    info=json.loads((release/'result.json').read_text());manifest=verify(release/'native')
    if manifest['files']!=info.get('native') or manifest['image']!=info['image_ids']['ops']:
        raise ValueError('Native artifacts do not belong to the immutable Ops image')
    return manifest
