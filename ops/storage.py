"""Registered data roots and stable artifact identities; no caller-supplied host paths."""
import json
import re
from pathlib import Path

SELECTOR={'type':'string','pattern':'^data/[0-9]+$','default':'data/0',
          'description':'Registered output store from system.status.data_stores. Defaults to data/0; frozen per Job.'}

def stores(config,bulk):
    result={'data/0':{'path':str(bulk),'storage_id':config.get('storage_id'),'mode':'rw'}}
    for name,item in config.get('data_stores',{}).items():
        if name!='data/0' and re.fullmatch(r'data/[0-9]+',name) and item.get('storage_id'):
            result[name]=dict(item)
    return result

def select(config,bulk,name='data/0',expected=None,verify=False):
    if not isinstance(name,str) or name not in stores(config,bulk):raise ValueError('Unknown output storage')
    item=stores(config,bulk)[name];path=Path(item['path'])
    if item.get('mode')!='rw' or not path.is_absolute():raise ValueError('Storage is not writable')
    if expected is not None and item['storage_id']!=expected:raise ValueError('Job storage identity changed')
    if verify and name!='data/0':
        if path.resolve()!=path or not path.is_dir():raise ValueError('Storage unavailable or redirected')
        if json.loads((path/'.loom-storage.json').read_text())!={'storage_id':item['storage_id']}:raise ValueError('Storage identity mismatch')
    return path,item['storage_id']

def output(config,bulk,spec,verify=True):
    return select(config,bulk,spec.get('output_storage','data/0'),spec.get('_output_storage_id'),verify)

def locator(config,bulk,value):
    if value['machine_id']!=config['machine_id']:raise ValueError('Input outside machine grant')
    matches=[name for name,item in stores(config,bulk).items() if item['storage_id']==value['storage_id']]
    if len(matches)!=1:raise ValueError('Unknown or ambiguous storage identity')
    return select(config,bulk,matches[0],verify=True)[0]

def folder(config,bulk,row,relative,attempt=None):
    spec=json.loads(row['spec']) if isinstance(row['spec'],str) else row['spec']
    base,_=output(config,bulk,spec)
    return relative(base,f"runs/{row['id']}/{attempt or row['attempt']}")
