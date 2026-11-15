"""Explicit drained cutover to native authority; no host roots or secrets in source."""
import argparse,json,os,sqlite3,subprocess,sys
from pathlib import Path
SOURCE=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(SOURCE/'deploy'))
import native_artifacts
from tailnet import env_values

def run(root,platform,phase):
    root=Path(root);platform=Path(platform);state=root/'runtime'
    with sqlite3.connect('file:'+str(state/'ops.sqlite3')+'?mode=ro',uri=True) as db:
        version=db.execute('PRAGMA user_version').fetchone()[0]
    pending=state/'native-cutover-v1/before'
    receipt=pending/'receipt.json'
    interrupted=version==5 and (pending/'intent.json').is_file() and not receipt.is_file()
    if version==5 and receipt.is_file():
        proof=json.loads(receipt.read_text())
        if proof.get('schema')!='loom-rust-migration/v1' or proof.get('state')!=str(state) or proof.get('before',{}).get('schema')!=4 or proof.get('after',{}).get('schema')!=5:
            raise ValueError('Native cutover receipt does not verify this instance')
    if version==5 and not interrupted:return {'status':'done'}
    if version not in (4,5):raise ValueError('Bridge release schema 4 is required before native cutover')
    if phase!='apply':return {'status':'needed'}
    native=SOURCE.parent/'native';native_artifacts.verify(native)
    binary=native/'loom-migrate';config=json.loads((root/'config/api.json').read_text());env=env_values(root/'compose.env')
    executor=Path(env_values(root/'executor.env').get('LOOM_EXECUTOR_HOST',platform/'shared/executor'))
    host=config.get('executor',{});data=Path(host.get('host_data') or env['OPS_BULK']);components=Path(host.get('host_components') or env['LOOM_COMPONENTS_HOST'])
    # Container paths are projected for this offline check only. Original config is preserved.
    for name,store in config.get('data_stores',{}).items():
        actual=host.get('storage',{}).get(name)
        if actual:store['path']=actual
        elif name=='data/0':store['path']=str(data)
        else:raise ValueError('Secondary store host mapping is missing')
    folder=state/'native-cutover-v1';folder.mkdir(mode=0o700,exist_ok=True);folder.chmod(0o700)
    projected=folder/'offline-config.json'
    with projected.open('w') as stream:os.fchmod(stream.fileno(),0o600);json.dump(config,stream)
    command=[str(binary),'apply','--state',str(state),'--config',str(projected),'--data',str(data),'--components',str(components),'--backup',str(folder/'before'),'--executor-root',str(executor)]
    subprocess.run(command,check=True,capture_output=True,text=True,timeout=600)
    return {'status':'done'}
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['check','apply','verify']);p.add_argument('--instance',type=Path,required=True);p.add_argument('--platform',type=Path,required=True);a=p.parse_args();print(json.dumps(run(a.instance,a.platform,a.phase)))
