"""Real SQLite/files and producer process; only host installation guard is mocked."""
import json
from pathlib import Path
from unittest.mock import patch
import ops,storage,registry,execution,dashboard_read,runtime_client
from test_support import IsolatedOps

class StorageResultTests(IsolatedOps):
 def setUp(self):
  super().setUp();self.extra=self.root/'extra';self.extra.mkdir()
  (self.extra/'.loom-storage.json').write_text(json.dumps({'storage_id':'extra-id'}))
  self.config['data_stores']={'data/1':{'path':str(self.extra),'storage_id':'extra-id','mode':'rw'}}
 def completed(self):
  spec={**self.spec(),'output_storage':'data/1'};job=self.submit(spec);row=self.claim(job)
  with patch.object(ops,'guard'):
   ops.execute(row,self.config)
   ops.process_result({**self.row(job),'retries':0},self.config)
  self.assertEqual(self.row(job)['state'],'complete');return job,spec
 def test_secondary_output_db_catalog_media_logs_and_downstream_resolution(self):
  job,spec=self.completed();result=json.loads(self.row(job)['result']);entity=result['entities'][0]
  self.assertEqual(entity['locator']['storage_id'],'extra-id')
  self.assertFalse((self.data/'runs'/job).exists())
  self.assertEqual(self.submit(spec),job)
  with self.assertRaisesRegex(ValueError,'Idempotency'):self.submit({**spec,'output_storage':'data/0'})
  with ops.db() as c:
   registered=registry.view(c,self.owner,job+'.observation');self.assertEqual(registered['locations'][0]['storage_id'],'extra-id')
   catalog,media=dashboard_read.projection(c,self.owner,self.config,self.local,self.data)
   item=next(e for e in catalog['entities'] if e['id']==job+'.observation')
   self.assertTrue(item['loom_files'][0]['path'].startswith('data/1/'))
   descriptor,stream=dashboard_read.open_media(media,item['loom_media'][0]['id'],self.data)
   with stream:self.assertIn(b'synthetic',stream.read())
   profile={'inputs':{'source':'runs'}};params={'source':{'job':job,'entity':'observation'}}
   resolved=execution.resolve_inputs(profile,params,self.config,self.data,ops.relative,ops.file_sha,lambda j:c.execute('SELECT * FROM jobs WHERE id=?',(j,)).fetchone())
   self.assertTrue(Path(resolved['source']).is_relative_to(self.extra))
  logs=ops.operate('get_job_logs',{'id':job,'attempt':self.row(job)['attempt'],'offset':0,'limit':1024},self.owner,self.config)
  self.assertTrue(logs['available'])
  locator={**entity['locator'],'sha256':entity['sha256']}
  execution.admit_inputs(profile,{'source':locator},self.config,self.data,ops.relative,ops.ident,[])
  with self.assertRaises(ValueError):execution.admit_inputs(profile,{'source':{**locator,'storage_id':'missing'}},self.config,self.data,ops.relative,ops.ident,[])
  (self.extra/entity['locator']['ref']).write_text('corrupted')
  with self.assertRaises(ValueError):execution.resolve_inputs(profile,{'source':locator},self.config,self.data,ops.relative,ops.file_sha,None)
 def test_unknown_store_missing_disk_and_changed_binding_never_fall_back(self):
  with self.assertRaises(ValueError):self.submit({**self.spec(),'output_storage':'data/99'})
  job=self.submit({**self.spec(),'output_storage':'data/1'});row=self.claim(job)
  (self.extra/'.loom-storage.json').write_text(json.dumps({'storage_id':'replacement'}))
  with patch.object(ops,'guard'):ops.execute(row,self.config)
  self.assertEqual(self.row(job)['state'],'failed');self.assertFalse((self.data/'runs'/job).exists())
  with self.assertRaises(ValueError):self.submit({**self.spec('new'),'output_storage':'data/1'})
 def test_default_storage_and_reconnect_secondary(self):
  job,spec=self.completed();source=self.extra/json.loads(self.row(job)['result'])['entities'][0]['locator']['ref']
  (self.extra/'copy.json').write_bytes(source.read_bytes())
  result=ops.operate('reconnect_location',{'id':job+'.observation','revision':1,'component':'observation.json','ref':'copy.json','storage':'data/1'},self.owner,self.config)
  self.assertEqual(result['status'],'available')
  row=self.claim(self.submit(self.spec('primary')))
  with patch.object(ops,'guard'):
   ops.execute(row,self.config);ops.process_result({**self.row(row['id']),'retries':0},self.config)
  self.assertEqual(json.loads(self.row(row['id'])['result'])['entities'][0]['locator']['storage_id'],'fixture-data')
 def test_container_maps_secondary_work_and_pinned_input_without_full_store_mount(self):
  folder=self.extra/'runs/job/attempt';folder.mkdir(parents=True)
  source=self.extra/'input.txt';source.write_text('input')
  request=folder/'request.json';request.write_text(json.dumps({'params':{'input':str(source)}}))
  self.config['executor']={'host_data':'/host/primary','storage':{'data/1':'/host/secondary'}}
  spec={'output_storage':'data/1','_output_storage_id':'extra-id','profile':{'runtime':{'command':['python3'],'runtime_id':'python'},'inputs':{'input':''}}}
  def broker(config,action,payload):
   if action=='start':
    self.assertEqual(payload['host_output'],'/host/secondary/runs/job/attempt')
    self.assertEqual(payload['output_storage'],'data/1');self.assertNotIn('storage',payload)
    self.assertEqual(payload['inputs'],{'input':{'storage':'data/1','ref':'input.txt'}})
    return {'ticket':payload['ticket']}
   return {'returned':True,'exit_code':0}
  with patch.object(runtime_client,'call',side_effect=broker):runtime_client.Container(self.config,{'id':'job','attempt':'attempt','owner':'owner'},spec,'run',request,folder,self.data)
  value=json.loads((folder/'run-request.json').read_text());self.assertEqual(value['output_dir'],'/work');self.assertEqual(value['params']['input'],'/inputs/input')
