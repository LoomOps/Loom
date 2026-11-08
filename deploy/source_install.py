"""Build a pinned checkout into a local install kit without exporting Docker images."""
import argparse,importlib.util,json,platform,shutil,subprocess,sys
from pathlib import Path
import release
import guide_bundle

def prepare(version,destination):
 destination=Path(destination);result=release.build(version,destination/'releases',None)
 snapshot=Path(result['snapshot']);source=snapshot/'source';kit=destination/'installer'
 if kit.exists():raise ValueError('Choose an unused installation staging directory')
 kit.mkdir(parents=True)
 shutil.copytree(source,kit/'platform')
 for name in ['setup.py','INSTALL.md','install-plan.example.json','tailnet-plan.example.json','access-plan.example.json']:
  shutil.copy2(source/'deploy/installer'/name,kit/name)
 # Generate guides inside the pinned image; the host needs only standard Python.
 guide(result['images']['ops'],kit/'loom-agent-kit.zip')
 subprocess.run(['docker','image','inspect','python:3.12-slim'],check=True,stdout=subprocess.DEVNULL)
 images={name:{'tag':tag,'id':result['image_ids'][name]} for name,tag in result['images'].items()}
 images['python']={'tag':'python:3.12-slim','id':subprocess.check_output(['docker','image','inspect','python:3.12-slim','--format','{{.Id}}'],text=True).strip()}
 import hashlib
 files={str(p.relative_to(kit)):hashlib.sha256(p.read_bytes()).hexdigest() for p in kit.rglob('*') if p.is_file()}
 architecture=json.loads(subprocess.check_output(['docker','image','inspect',images['ops']['tag']]))[0]['Architecture']
 info={'schema':'loom-install-kit/v1','installer_revision':2,'build_id':version,'git_commit':result['git_commit'],'architecture':architecture,'images':images,'files':files,'image_transport':'local'}
 (kit/'manifest.json').write_text(json.dumps(info,indent=2)+'\n')
 return {'kit':str(kit),'release':str(snapshot),'next':'sudo python3 '+str(kit/'setup.py')+' check /absolute/install-plan.json'}

def guide(image,destination):
 Path(destination).write_bytes(guide_bundle.generate(image))

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('version');p.add_argument('--destination',type=Path,required=True);a=p.parse_args()
 print(json.dumps(prepare(a.version,a.destination),indent=2))
