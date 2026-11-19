"""Add common Item identity tables; projection backfill is separate and resumable."""
import argparse,json,sqlite3,subprocess,sys
from pathlib import Path
SOURCE=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(SOURCE/'deploy'))
import native_artifacts
from tailnet import env_values

def run(root,platform,phase):
    root=Path(root);state=root/'runtime'
    with sqlite3.connect((state/'ops.sqlite3').as_uri()+'?mode=ro',uri=True) as db:
        version=db.execute('PRAGMA user_version').fetchone()[0]
        if version in (6,7,8):
            if db.execute('PRAGMA integrity_check').fetchone()[0]!='ok' or not db.execute('SELECT length(origin)=16 FROM catalog_meta WHERE id=1').fetchone()[0]:raise ValueError('Catalog authority failed validation')
            return {'status':'done'}
    if version==4 and phase!='apply':return {'status':'needed'}
    if version!=5:raise ValueError('Native schema 5 must be installed first')
    if phase!='apply':return {'status':'needed'}
    native=SOURCE.parent/'native';native_artifacts.verify(native)
    config=json.loads((root/'config/api.json').read_text());env=env_values(root/'compose.env');host=config.get('executor',{})
    data=Path(host.get('host_data') or env['OPS_BULK']);components=Path(host.get('host_components') or env['LOOM_COMPONENTS_HOST'])
    for name,store in config.get('data_stores',{}).items():
        actual=host.get('storage',{}).get(name)
        if actual:store['path']=actual
        elif name=='data/0':store['path']=str(data)
        else:raise ValueError('Secondary store host mapping is missing')
    from state_snapshot import catalog_host_path
    catalog=config.get('catalog',{})
    for key,default in (('manifest_directory','catalog-manifests'),('index_directory','search')):
        if key in catalog:catalog[key]=str(catalog_host_path(root,catalog[key],default))
    folder=state/'catalog-cutover-v1';folder.mkdir(mode=0o700,exist_ok=True)
    projected=folder/'offline-config.json'
    from migrations import save
    save(projected,config)
    subprocess.run([str(native/'loom-migrate'),'catalog-install','--state',str(state),'--config',str(projected),'--data',str(data),'--components',str(components),'--backup',str(folder/'before')],check=True,capture_output=True,text=True,timeout=600)
    return {'status':'done'}
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['check','apply','verify']);p.add_argument('--instance',type=Path,required=True);p.add_argument('--platform',type=Path,required=True);a=p.parse_args();print(json.dumps(run(a.instance,a.platform,a.phase)))
