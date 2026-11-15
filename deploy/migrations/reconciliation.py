"""Add the explicit reconciliation authority without rewriting scientific records."""
import argparse,json,sqlite3,subprocess,sys
from pathlib import Path
SOURCE=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(SOURCE/'deploy'))
import native_artifacts
from migrations import save
from state_snapshot import catalog_host_path
from tailnet import env_values

def run(root,platform,phase):
 root=Path(root);state=root/'runtime'
 with sqlite3.connect((state/'ops.sqlite3').as_uri()+'?mode=ro',uri=True) as db:
  version=db.execute('PRAGMA user_version').fetchone()[0]
  if version==7:
   tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
   if db.execute('PRAGMA integrity_check').fetchone()[0]!='ok' or not {'catalog_meta','reconcile_plans','asset_imports','file_bindings','log_retention','catalog_tasks','asset_revisions','asset_locations'}<=tables:raise ValueError('Reconciliation authority is incomplete')
   return {'status':'done'}
 if phase!='apply' and version in (4,5,6):return {'status':'needed'}
 if version!=6:raise ValueError('Common Item schema 6 must be installed first')
 native=SOURCE.parent/'native';native_artifacts.verify(native)
 config=json.loads((root/'config/api.json').read_text());env=env_values(root/'compose.env');host=config.get('executor',{})
 data=Path(host.get('host_data') or env['OPS_BULK']);components=Path(host.get('host_components') or env['LOOM_COMPONENTS_HOST'])
 for name,store in config.get('data_stores',{}).items():
  actual=host.get('storage',{}).get(name) or (str(data) if name=='data/0' else None)
  if not actual:raise ValueError('Missing explicit secondary host root')
  store['path']=actual
 for key,default in [('manifest_directory','catalog-manifests'),('index_directory','search')]:
  if key in config.get('catalog',{}):config['catalog'][key]=str(catalog_host_path(root,config['catalog'][key],default))
 folder=state/'reconcile-cutover-v1';folder.mkdir(mode=0o700,exist_ok=True);save(folder/'offline-config.json',config)
 subprocess.run([str(native/'loom-migrate'),'reconcile-install','--state',str(state),'--config',str(folder/'offline-config.json'),'--data',str(data),'--components',str(components),'--backup',str(folder/'before')],check=True,capture_output=True,text=True,timeout=600)
 return {'status':'done'}
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['check','apply','verify']);p.add_argument('--instance',type=Path,required=True);p.add_argument('--platform',type=Path,required=True);a=p.parse_args();print(json.dumps(run(a.instance,a.platform,a.phase)))
