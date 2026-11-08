"""Build a pinned checkout into a local install kit without exporting Docker images."""
import argparse,importlib.util,json,platform,shutil,subprocess,sys,hashlib,tempfile
from pathlib import Path
import release
import guide_bundle

def ensure_runtime():
 # BuildKit's cache is not the local image store used by the executor/installer.
 if subprocess.run(['docker','image','inspect','python:3.12-slim'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode:
  subprocess.run(['docker','pull','python:3.12-slim'],check=True)
 return subprocess.check_output(['docker','image','inspect','python:3.12-slim','--format','{{.Id}}'],text=True).strip()


def resume_release(version,destination):
 folder=destination/'releases'/version
 result=json.loads((folder/'result.json').read_text());manifest=json.loads((folder/'manifest.json').read_text())
 if result['status']!='built' or result['build_id']!=version:raise ValueError('Only packaging of a completed immutable build can resume')
 if Path(result['snapshot']).resolve()!=folder.resolve():raise ValueError('Unexpected release snapshot path')
 for name,digest in manifest['files'].items():
  if Path(name).is_absolute() or '..' in Path(name).parts:raise ValueError('Unsafe snapshot member')
  if hashlib.sha256((folder/'source'/name).read_bytes()).hexdigest()!=digest:raise ValueError('Snapshot changed; do not resume this release')
 for role,tag in result['images'].items():
  actual=subprocess.check_output(['docker','image','inspect',tag,'--format','{{.Id}}'],text=True).strip()
  if actual!=result['image_ids'][role]:raise ValueError('Immutable release image changed')
 return result


def prepare(version,destination,resume=False):
 destination=Path(destination)
 if (destination/'installer').exists():raise ValueError('Installation kit already exists')
 ensure_runtime()
 result=resume_release(version,destination) if resume else release.build(version,destination/'releases',None)
 snapshot=Path(result['snapshot']);source=snapshot/'source';kit=Path(tempfile.mkdtemp(prefix='.installer-',dir=destination))
 shutil.copytree(source,kit/'platform')
 for name in ['setup.py','doctor.py','smoke.py','INSTALL.md','install-plan.example.json','tailnet-plan.example.json','access-plan.example.json']:
  shutil.copy2(source/'deploy/installer'/name,kit/name)
 shutil.copytree(source/'deploy/installer/steps',kit/'steps')
 # Generate guides inside the pinned image; the host needs only standard Python.
 guide(result['images']['ops'],kit/'loom-agent-kit.zip')
 subprocess.run(['docker','image','inspect','python:3.12-slim'],check=True,stdout=subprocess.DEVNULL)
 images={name:{'tag':tag,'id':result['image_ids'][name]} for name,tag in result['images'].items()}
 images['python']={'tag':'python:3.12-slim','id':subprocess.check_output(['docker','image','inspect','python:3.12-slim','--format','{{.Id}}'],text=True).strip()}
 import hashlib
 files={str(p.relative_to(kit)):hashlib.sha256(p.read_bytes()).hexdigest() for p in kit.rglob('*') if p.is_file()}
 architecture=json.loads(subprocess.check_output(['docker','image','inspect',images['ops']['tag']]))[0]['Architecture']
 info={'schema':'loom-install-kit/v1','installer_revision':3,'build_id':version,'git_commit':result['git_commit'],'architecture':architecture,'images':images,'files':files,'image_transport':'local'}
 (kit/'manifest.json').write_text(json.dumps(info,indent=2)+'\n')
 final=destination/'installer';kit.rename(final);kit=final
 return {'kit':str(kit),'release':str(snapshot),'next':'sudo python3 '+str(kit/'setup.py')+' check /absolute/install-plan.json'}

def guide(image,destination):
 Path(destination).write_bytes(guide_bundle.generate(image))

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('version');p.add_argument('--destination',type=Path,required=True);p.add_argument('--resume',action='store_true',help='Resume packaging after a completed immutable image build');a=p.parse_args()
 print(json.dumps(prepare(a.version,a.destination,a.resume),indent=2))
