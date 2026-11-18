"""Build a native development acceptance kit. No installation or service changes."""
import argparse,hashlib,json,os,shutil,subprocess,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BINARIES=('loom-ops','loom-host','loom-sdk','loom-migrate')
def build(output,cargo='cargo',profile='release',prebuilt=None,build_info=None):
    output=Path(output)
    if output.exists():raise ValueError('Choose a new kit directory; existing artifacts are immutable')
    env=dict(os.environ)
    if build_info is not None:env['LOOM_BUILD_INFO']=str(Path(build_info).resolve())
    if prebuilt:
        source=Path(prebuilt).resolve()
        receipt=json.loads((source/'platform.json').read_text())
        if receipt.get('schema')!='loom-native-build/v1':raise ValueError('Native source receipt required')
        import snapshot
        current={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in snapshot.sources()}
        if receipt.get('sources')!=current:raise ValueError('Prebuilt native source inputs differ; rebuild the selected graph')
        platform=json.loads((ROOT/'build_support/platform.json').read_text())
        if any(receipt.get(k)!=v for k,v in platform.items() if k!='schema'):raise ValueError('Prebuilt native build platform differs')
        for name in BINARIES:
            if not (source/name).is_file() or hashlib.sha256((source/name).read_bytes()).hexdigest()!=receipt['files'].get(name):raise ValueError('Native artifact mismatch: '+name)
        if build_info is not None:raise ValueError('Prebuilt identity cannot be changed during kit assembly')
    else:
        command=[cargo,'build','--workspace','--locked']+(['--release'] if profile=='release' else [])
        subprocess.run(command,cwd=ROOT,check=True,env=env)
    metadata=json.loads(subprocess.check_output([cargo,'metadata','--locked','--format-version','1'],cwd=ROOT,env=env))
    if not prebuilt:source=Path(metadata['target_directory'])/('release' if profile=='release' else 'debug')
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
    manifest={'schema':'loom-native-kit/v1','version':info['version'],'build':info,'files':files,'profile':('prebuilt-pinned' if prebuilt else profile),'scope':'native development acceptance; no operational deployment'}
    if prebuilt:
        after={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in snapshot.sources()}
        if after!=current:raise ValueError('Source inputs changed during native kit assembly; output has no accepted manifest')
    (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    return {'kit':str(output),'files':len(files),'version':manifest['version']}
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--cargo',default=os.environ.get('CARGO','cargo'));p.add_argument('--profile',choices=['release','debug'],default='release');p.add_argument('--prebuilt',type=Path);p.add_argument('--build-info',type=Path);a=p.parse_args();print(json.dumps(build(a.output,a.cargo,a.profile,a.prebuilt,a.build_info),indent=2))
