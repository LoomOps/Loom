"""Prepare a relocatable installer bundle for an existing immutable release."""
import argparse,hashlib,json,shutil,subprocess,tarfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
HOST_FILES={
 'docs':['TAILSCALE-SSO.md','ACCESS-ENROLLMENT.md','UPDATES.md'],
 'dashboard':['admission.py','sharing.py','sso.py'],
 'deploy':['install.py','executor-attach.py','host_identity.py','host_services.py','collect-dashboard-gpus.py','collect-observations.py','compose.sh','tailnet.py','access_setup.py','update.py','guide_bundle.py','release_check_setup.py'],
 'ops':['host_layout.py','release_updates.py','docker_status.py','host_metrics.py','remote_fence.py','compose.loom.yaml','compose.executor.yaml','compose.observations.yaml','compose.data.yaml'],
 'executor':['server.py','runtime_policy.py','loom-executor.service'],
}

def build(release,output,guide):
 info=json.loads((release/'result.json').read_text())
 if info['status']!='built':raise ValueError('A completed immutable release is required')
 if output.exists():raise ValueError('Choose a new output directory')
 engine=['docker']
 if subprocess.run(engine+['info'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode:engine=['sudo','-n','docker']
 tags=[info['images']['ops'],info['images']['dashboard'],'python:3.12-slim']
 inspected=json.loads(subprocess.check_output(engine+['image','inspect',*tags]))
 for role,item in zip(['ops','dashboard'],inspected):
  if item['Id']!=info['image_ids'][role]:raise ValueError('Release image identity mismatch')
 if len({i['Architecture'] for i in inspected})!=1:raise ValueError('Mixed image architectures')
 output.mkdir(parents=True)
 for name in ('LICENSE','THIRD_PARTY_NOTICES.md'):shutil.copy2(release/'source'/name,output/name)
 shutil.copytree(release/'source',output/'source')
 for directory,names in HOST_FILES.items():
  for name in names:
   dest=output/'platform'/directory/name;dest.parent.mkdir(parents=True,exist_ok=True)
   shutil.copy2(release/'source'/directory/name,dest)
 for name in ['setup.py','doctor.py','smoke.py','INSTALL.md','install-plan.example.json','tailnet-plan.example.json','access-plan.example.json']:
  shutil.copy2(release/'source/deploy/installer'/name,output/name)
 shutil.copytree(release/'source/deploy/installer/steps',output/'steps')
 (output/'AGENTS.md').write_text('Read INSTALL.md before installing. Follow every numbered step and rerun the read-only doctor. Reread INSTALL.md after each step. Full native identity and Gateway are mandatory; lead handoff with the Gateway URL. This is a new-machine installer, not an upgrade or data migration tool.\n')
 # A caller-supplied guide must describe this exact release, not the live tree.
 import zipfile
 with zipfile.ZipFile(guide) as z:
  manifests=[n for n in z.namelist() if n.endswith('manifest.json')]
  if not any(json.loads(z.read(n)).get('build_id')==info['build_id'] for n in manifests):raise ValueError('Guide build does not match release')
 shutil.copy2(guide,output/'loom-agent-kit.zip')
 with (output/'images.tar').open('wb') as stream:
  subprocess.run(engine+['image','save',*tags],stdout=stream,check=True)
 images={role:{'tag':tag,'id':item['Id']} for role,tag,item in zip(['ops','dashboard','python'],tags,inspected)}
 files={str(p.relative_to(output)):sha(p) for p in output.rglob('*') if p.is_file()}
 manifest={'schema':'loom-install-kit/v1','installer_revision':3,'build_id':info['build_id'],'architecture':inspected[0]['Architecture'],'images':images,'files':files}
 (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 archive=output.with_name(output.name+'.tar.gz')
 with tarfile.open(archive,'w:gz') as t:t.add(output,arcname=output.name)
 archive.with_name(archive.name+'.sha256').write_text(sha(archive)+'  '+archive.name+'\n')
 return {'directory':str(output),'archive':str(archive),'bytes':archive.stat().st_size,'build':info['build_id'],'architecture':manifest['architecture']}

def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--release',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--guide',type=Path,required=True)
 a=p.parse_args();print(json.dumps(build(a.release,a.output,a.guide),indent=2))
