"""Build a native development acceptance kit. No installation or service changes."""
import argparse,hashlib,json,os,shutil,subprocess,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BINARIES=('loom-ops','loom-host','loom-sdk','loom-migrate')
def build(output,cargo='cargo',profile='release'):
    output=Path(output)
    if output.exists():raise ValueError('Choose a new kit directory; existing artifacts are immutable')
    command=[cargo,'build','--workspace','--locked']+(['--release'] if profile=='release' else [])
    subprocess.run(command,cwd=ROOT,check=True)
    metadata=json.loads(subprocess.check_output([cargo,'metadata','--locked','--format-version','1'],cwd=ROOT))
    source=Path(metadata['target_directory'])/('release' if profile=='release' else 'debug')
    output.mkdir(parents=True)
    for name in BINARIES:
        shutil.copy2(source/name,output/name);(output/name).chmod(0o755)
    for name in ('LICENSE','THIRD_PARTY_NOTICES.md'):shutil.copy2(ROOT/name,output/name)
    sdk=output/'sdk/loom-sdk';(sdk/'src').mkdir(parents=True);shutil.copy2(ROOT/'rust/crates/sdk/src/lib.rs',sdk/'src/lib.rs');shutil.copy2(ROOT/'rust/templates/sdk-Cargo.toml',sdk/'Cargo.toml');shutil.copy2(ROOT/'LICENSE',sdk/'LICENSE')
    shutil.copytree(ROOT/'rust/templates',output/'templates')
    licenses=[]
    for package in metadata['packages']:
        if package.get('source') is None:continue
        root=Path(package['manifest_path']).parent;files=[]
        for path in sorted(root.iterdir()):
            if path.is_file() and path.name.upper().startswith(('LICENSE','COPYING','NOTICE','COPYRIGHT')):
                target=output/'licenses'/(package['name']+'-'+package['version'])/path.name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target);files.append(str(target.relative_to(output)))
        licenses.append({'name':package['name'],'version':package['version'],'license':package['license'],'files':files})
    (output/'sbom.json').write_text(json.dumps({'schema':'loom-native-sbom/v1','packages':licenses},indent=2)+'\n')
    info=json.loads(subprocess.check_output([str(output/'loom-ops'),'--build-info']))
    (output/'loom-agent-kit.zip').write_bytes(subprocess.check_output([str(output/'loom-ops'),'--guide-bundle']))
    files={str(p.relative_to(output)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(output.rglob('*')) if p.is_file()}
    manifest={'schema':'loom-native-kit/v1','version':tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']['version'],'build':info,'files':files,'profile':profile,'scope':'native development acceptance; no operational deployment'}
    (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    return {'kit':str(output),'files':len(files),'version':manifest['version']}
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--cargo',default=os.environ.get('CARGO','cargo'));p.add_argument('--profile',choices=['release','debug'],default='release');a=p.parse_args();print(json.dumps(build(a.output,a.cargo,a.profile),indent=2))
