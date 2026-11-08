"""Build numbered dev images from their saved source, then update the dev-build alias."""
import json
from pathlib import Path
import subprocess
import sys
import tarfile
from host_identity import admin_reexec_for,build_owner
admin_reexec_for(Path("/loom/builds/dev-build"))

ROOT = Path(__file__).resolve().parents[1]
capture = subprocess.check_output([sys.executable, '-B', str(ROOT/'deploy/snapshot.py')],text=True)
info = json.loads(capture)
snapshot = Path(info['snapshot'])
source = snapshot/'source'
source.mkdir()
with tarfile.open(snapshot/'source.tar.gz') as archive:
    archive.extractall(source,filter='data')
print('Building '+info['build_id'],flush=True)
try:
    subprocess.run(['bash',str(source/'deploy/compose.sh'),'build'],check=True)
    engine=['docker']
    if subprocess.run(engine+['info'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode:
        engine=['sudo','-n','docker']
    result={**info,'status':'built','image_ids':{}}
    for component, tag in info['images'].items():
        result['image_ids'][component]=subprocess.check_output(engine+['image','inspect',tag,'--format','{{.Id}}'],text=True).strip()
        subprocess.run(engine+['tag',tag,'loom-'+component+':dev-build'],check=True)
    (snapshot/'result.json').write_text(json.dumps(result,indent=2)+'\n')
except subprocess.CalledProcessError:
    (snapshot/'result.json').write_text(json.dumps({**info,'status':'failed'},indent=2)+'\n')
    raise
finally:
    build_owner(snapshot)
print(json.dumps(result,indent=2))
