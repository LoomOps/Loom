"""Real Rust HTTP/MCP/producer process acceptance using disposable installation roots."""
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
from urllib.request import Request,urlopen
from urllib.error import HTTPError

ROOT=Path(__file__).resolve().parents[2]
class RustServiceTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='loom-native-http-');self.addCleanup(self.temp.cleanup)
  self.root=Path(self.temp.name);self.data=self.root/'data';self.data.mkdir();(self.data/'.loom-storage.json').write_text(json.dumps({'storage_id':'fixture-data'}));self.components=self.root/'components';self.components.mkdir();self.token='fixture-token-only'
  cfg=self.root/'config.json';cfg.write_text(json.dumps({'machine_id':'fixture','storage_id':'fixture-data','actors':[{'id':'owner','scopes':['read','submit','cancel','events'],'owners':['owner'],'notify':['owner'],'token_sha256':hashlib.sha256(self.token.encode()).hexdigest()}]}))
  with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
  self.url='http://127.0.0.1:'+str(port);self.log=open(self.root/'service.log','wb');self.addCleanup(self.log.close)
  self.process=subprocess.Popen([str(ROOT/'target/debug/loom-ops'),'--config',str(cfg),'--state',str(self.root/'state'),'--data',str(self.data),'--components',str(self.components),'--listen','127.0.0.1:'+str(port)],env={**os.environ,'LOOM_SDK_BINARY':str(ROOT/'target/debug/loom-sdk'),'LOOM_DEV_NO_AUTH':'0'},stdout=self.log,stderr=self.log)
  self.addCleanup(self.stop)
  for _ in range(100):
   if self.process.poll() is not None:self.fail((self.root/'service.log').read_text())
   try:self.get('/');break
   except OSError:time.sleep(.05)
  else:self.fail('Rust service did not start')
 def stop(self):
  if self.process.poll() is None:
   self.process.terminate()
   try:self.process.wait(timeout=8)
   except subprocess.TimeoutExpired:self.process.kill();self.process.wait(timeout=5)
 def get(self,path):
  with urlopen(Request(self.url+path,headers={'Authorization':'Bearer '+self.token}),timeout=5) as r:return json.load(r)
 def post(self,path,body,token=None):
  with urlopen(Request(self.url+path,data=json.dumps(body).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+(token or self.token)}),timeout=5) as r:return json.load(r)
 def call(self,group,action,args=None):return self.post('/api/actions',{'group':group,'action':action,'arguments':args or {}})
 def submit(self,key,seconds=0,**more):return self.call('jobs','submit',{'key':key,'kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':seconds,'fail':False},**more})['id']
 def wait(self,id,state='complete'):
  for _ in range(100):
   job=self.call('jobs','get',{'id':id})
   if job['state']==state:return job
   time.sleep(.1)
  self.fail((job,(self.root/'service.log').read_text()))
 def test_actual_rust_job_output_and_recipient_ack(self):
  job=self.wait(self.submit('native'));self.assertTrue(job['result']['artifact_ready']);self.assertTrue(job['result']['execution']['process_group_returned']);self.assertEqual(len(job['attempts']),1)
  entity=self.call('results','get',{'id':job['id']+'.observation','revision':1});self.assertEqual(entity['kind'],'artifact');self.assertEqual(entity['locations'][0]['status'],'available')
  events=self.call('events','list',{'kind':'all'});completion=next(e for e in events if e['job']==job['id'] and e['code']=='complete');self.call('events','ack',{'seq':completion['seq']});self.assertFalse(any(e['seq']==completion['seq'] for e in self.call('events','list',{'kind':'all'})))
 def test_real_mcp_six_groups_and_guide(self):
  init=self.post('/mcp',{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-03-26'}});self.assertEqual(init['result']['protocolVersion'],'2025-03-26')
  tools=self.post('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/list'});self.assertEqual({t['name'] for t in tools['result']['tools']},{'jobs','recovery','events','components','results','system'})
  guide=self.post('/mcp',{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'system','arguments':{'action':'guide','arguments':{'id':'authoring'}}}});data=json.loads(guide['result']['content'][0]['text']);self.assertIn('Rust',data['content']);self.assertIn('loom',guide['result']['_meta'])
 def test_scenario_job_pin_and_reuse_are_real_api_operations(self):
  s=self.call('results','register_scenario',{'key':'s','definition':{'label':'Fixture task'}})
  job=self.wait(self.submit('first',scenario_ref={'id':s['id'],'revision':1}));self.call('results','update_scenario',{'key':'s2','id':s['id'],'expected_revision':1,'definition':{'label':'Updated task'},'reason':'New protocol'})
  self.assertEqual(self.call('jobs','get',{'id':job['id']})['scenario_ref']['revision'],1)
  link=self.call('results','link_scenario',{'key':'link','scenario_ref':{'id':s['id'],'revision':2},'target':{'kind':'entity','identity':{'id':job['id']+'.observation','revision':1}},'relation':'used_by','provenance':{'reason':'Explicit reuse'}})
  graph=self.call('results','scenario_graph',{'id':s['id']});self.assertEqual(graph['links'][0]['id'],link['id']);self.assertEqual(graph['links'][0]['target'],link['target']);self.assertEqual(len(self.call('results','list',{'view':'all'})),1)
 def test_cancelled_native_process_has_no_published_result(self):
  id=self.submit('cancel',seconds=10);self.wait(id,'running');self.call('jobs','cancel',{'id':id});job=self.wait(id,'cancelled');self.assertFalse((job.get('result') or {}).get('artifact_ready'));self.assertFalse(self.call('results','list',{'view':'all'}))
 def test_workflow_has_one_public_completion(self):
  spec={'kind':'cpu.smoke','params':{'seconds':0,'fail':False}}
  workflow=self.call('jobs','submit_workflow',{'key':'graph','label':'One outcome','owner':'owner','notify':'owner','nodes':[{'id':'a','label':'first','spec':spec},{'id':'b','label':'second','depends_on':['a'],'spec':spec}],'outputs':{'final':{'node':'b','entity':'observation'}}})
  done=self.wait(workflow['id']);self.assertTrue(done['workflow']);self.assertEqual(len(done['executions']),2)
  events=self.call('events','list',{'kind':'all'});self.assertEqual([(e['job'],e['code']) for e in events if e['code']=='complete'],[(workflow['id'],'complete')])
  streams=self.call('jobs','log_streams',{'id':workflow['id']});self.assertGreaterEqual(len(streams['items']),2);self.assertTrue(all(s['job']!=workflow['id'] for s in streams['items']))
 def test_bad_credential_and_origin_cannot_submit(self):
  with self.assertRaises(HTTPError) as denied:self.post('/api/actions',{'group':'jobs','action':'queue','arguments':{}},token='wrong')
  self.assertEqual(denied.exception.code,401)
  with self.assertRaises(HTTPError):urlopen(Request(self.url+'/api/status',headers={'Authorization':'Bearer '+self.token,'Origin':'https://untrusted.invalid'}))
 def test_registered_file_preview_verifies_current_bytes(self):
  job=self.wait(self.submit('preview'));catalog=self.get('/api/dashboard/catalog');entity=next(e for e in catalog['entities'] if e['id']==job['id']+'.observation');preview=entity['loom_media'][0]
  request=Request(self.url+'/api/dashboard/media/'+preview['id'],headers={'Authorization':'Bearer '+self.token})
  with urlopen(request,timeout=5) as response:
   self.assertEqual(response.read(),b'{"synthetic":true}\n');self.assertEqual(response.headers['X-Content-Type-Options'],'nosniff')
  location=self.call('results','get',{'id':entity['id'],'revision':1})['locations'][0];(self.data/location['ref']).write_text('tampered')
  with self.assertRaises(HTTPError):urlopen(request,timeout=5)
 def test_reference_packaged_client_uses_real_native_mcp(self):
  import importlib.util,sys
  spec=importlib.util.spec_from_file_location('reference_client_fixture',ROOT/'tests/ops/client_source.py');reference=importlib.util.module_from_spec(spec);spec.loader.exec_module(reference)
  self.client_acceptance(reference.PACKAGE)
  candidate=os.environ.get('LOOM_USE_CANDIDATE')
  if candidate:self.client_acceptance(Path(candidate))
 def client_acceptance(self,package):
  credential=self.root/'client-token.json';credential.write_text(json.dumps({'token':self.token}));credential.chmod(0o600)
  cfg=self.root/'client.json';cfg.write_text(json.dumps({'endpoint':self.url+'/mcp','credential':str(credential)}))
  code='''import json,sys
from pathlib import Path
from stdio import OpsTransport
t=OpsTransport(Path(sys.argv[1]));r=t.rpc({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-03-26'}});assert r['result']['protocolVersion']=='2025-03-26'
r=t.rpc({'jsonrpc':'2.0','id':2,'method':'tools/list'});assert len(r['result']['tools'])==6
r=t.rpc({'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'system','arguments':{'action':'status','arguments':{}}}});assert json.loads(r['result']['content'][0]['text'])['engine']=='rust'
'''
  result=subprocess.run([os.sys.executable,'-c',code,str(cfg)],cwd=package/'clients',capture_output=True,text=True,timeout=25);self.assertEqual(result.returncode,0,result.stderr)
 def test_restart_preserves_original_submission_and_no_second_attempt(self):
  id=self.submit('restart',seconds=10);self.wait(id,'running')
  for _ in range(100):
   started=self.call('jobs','get',{'id':id})
   if started['attempts'][0]['started'] is not None:break
   time.sleep(.05)
  self.assertIsNotNone(started['attempts'][0]['started']);args=self.process.args;self.process.kill();self.process.wait(timeout=5)
  self.process=subprocess.Popen(args,env={**os.environ,'LOOM_SDK_BINARY':str(ROOT/'target/debug/loom-sdk'),'LOOM_DEV_NO_AUTH':'0'},stdout=self.log,stderr=self.log)
  for _ in range(100):
   try:
    job=self.call('jobs','get',{'id':id});break
   except OSError:time.sleep(.05)
  self.assertEqual(self.submit('restart',seconds=10),id)
  for _ in range(100):
   job=self.call('jobs','get',{'id':id})
   if job['state']=='interrupted':break
   time.sleep(.1)
  self.assertEqual(job['state'],'interrupted');self.assertEqual(len(job['attempts']),1);self.assertFalse((job.get('result') or {}).get('artifact_ready'))
 def test_remote_handoff_session_blocks_transfer_until_native_worker_returns(self):
  import sqlite3
  self.stop();authority=self.root/'authority.sqlite3';policy=self.root/'policy.json';credential=self.root/'authority-token';credential.write_text('authority-fixture-only');credential.chmod(0o600);binding={'machine_id':'fixture','storage_id':'fixture-data'}
  policy.write_text(json.dumps({'source':{'digest':hashlib.sha256(credential.read_bytes()).hexdigest(),'binding':binding}}))
  subprocess.run([os.sys.executable,str(ROOT/'ops/remote_fence.py'),str(authority),str(policy),'--initialize'],check=True,capture_output=True,timeout=5)
  with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
  process=subprocess.Popen([os.sys.executable,str(ROOT/'ops/remote_fence.py'),str(authority),str(policy),'--bind','127.0.0.1','--port',str(port)],stdout=self.log,stderr=self.log);self.addCleanup(process.wait,5);self.addCleanup(process.terminate)
  cfg=self.root/'config.json';config=json.loads(cfg.read_text());config['remote_fence']={'url':'http://127.0.0.1:'+str(port),'epoch':0,'binding':binding,'token_path':str(credential)};cfg.write_text(json.dumps(config))
  time.sleep(.15);self.process=subprocess.Popen(self.process.args,env={**os.environ,'LOOM_SDK_BINARY':str(ROOT/'target/debug/loom-sdk'),'LOOM_DEV_NO_AUTH':'0'},stdout=self.log,stderr=self.log)
  for _ in range(100):
   if self.process.poll() is not None:self.fail((self.root/'service.log').read_text())
   try:self.get('/api/status');break
   except OSError:time.sleep(.05)
  self.wait(self.submit('remote-owned'))
  with sqlite3.connect(authority) as db:self.assertIsNotNone(db.execute('SELECT session FROM fence').fetchone()[0])
  body=json.dumps({'archive':'a'*64,'epoch':0,'binding':binding}).encode()
  with self.assertRaises(HTTPError) as denied:urlopen(Request(config['remote_fence']['url']+'/seal',data=body,headers={'Authorization':'Bearer '+credential.read_text(),'Content-Type':'application/json'}),timeout=5)
  self.assertEqual(denied.exception.code,403);self.stop()
  with sqlite3.connect(authority) as db:self.assertIsNone(db.execute('SELECT session FROM fence').fetchone()[0])
 def test_historical_binding_replays_after_the_verified_copy_is_moved(self):
  import sqlite3
  path=self.data/'bound.txt';path.write_bytes(b'original');sha=hashlib.sha256(path.read_bytes()).hexdigest();body={'original':{'id':'preserved','label':'Preserved','kind':'dataset','properties':{}},'locations':[{'source_ref':'old.txt','sha256':sha,'size':8,'status':'missing'}]}
  with sqlite3.connect(self.root/'state/ops.sqlite3') as db:db.execute("INSERT INTO history_records VALUES('catalog','dataset','preserved','r1','s1',?)",(json.dumps(body),))
  args={'key':'binding-replay','identity':{'source':'catalog','kind':'dataset','id':'preserved','revision':'r1','snapshot':'s1','file':'old.txt'},'locator':{'machine_id':'fixture','storage_id':'fixture-data','ref':'bound.txt'},'sha256':sha,'size':8};accepted=self.call('results','bind_historical_location',args);path.unlink();self.assertEqual(accepted,self.call('results','bind_historical_location',args))
 def test_large_catalog_read_does_not_occupy_the_mutation_lane(self):
  import concurrent.futures,sqlite3
  id=self.submit('concurrent-cancel',seconds=10);self.wait(id,'running');spec={'kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False},'category':'other','execution_mode':'smoke','depends_on':[],'output_storage':'data/0','presentation':{'scope':'operations','role':'test'}}
  with sqlite3.connect(self.root/'state/ops.sqlite3',timeout=10) as db:db.executemany("INSERT INTO jobs(id,submitter,idem,spec,owner,notify,state,created,cancel) VALUES(?,'owner',?,?,'owner','owner','cancelled',1,1)",[(f'synthetic-{i}',f'synthetic-{i}',json.dumps(spec)) for i in range(5000)])
  with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
   pending=pool.submit(self.get,'/api/dashboard/catalog');time.sleep(.1);started=time.monotonic();self.call('jobs','cancel',{'id':id});elapsed=time.monotonic()-started;self.assertLess(elapsed,2.0);catalog=pending.result(timeout=20);self.assertGreaterEqual(len(catalog['entities']),5001)
  self.wait(id,'cancelled')

 def test_mcp_invalid_requests_and_action_arguments_keep_protocol_errors(self):
  for body in ({'jsonrpc':'2.0','id':True,'method':'ping'},{'jsonrpc':'1.0','id':1,'method':'ping'},{'jsonrpc':'2.0','id':1}):
   self.assertEqual(self.post('/mcp',body)['error']['code'],-32600)
  self.assertEqual(self.post('/mcp',{'jsonrpc':'2.0','id':'x','method':'not-a-method'})['error']['code'],-32601)
  value=self.post('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'jobs','arguments':{'action':'list','arguments':{'unexpected':True}}}})
  self.assertTrue(value['result']['isError'])

 def test_common_search_resolves_real_output_and_reads_effect_logs_without_duplicate_events(self):
  id=self.submit('indexed-native');self.wait(id)
  for _ in range(150):
   result=self.call('results','search_items',{'text':id,'kinds':['artifact'],'view':'all','role':'all'})
   if result['items']:break
   time.sleep(.05)
  self.assertEqual(len(result['items']),1)
  item=result['items'][0];self.assertIsInstance(item['item_id'],str);self.assertGreater(int(item['item_id']),2**53)
  for _ in range(100):
   files=self.call('results','item_files',{'item_id':item['item_id']})
   if files['items']:break
   time.sleep(.05)
  file=files['items'][0];self.assertEqual(self.call('results','get_file',{'file_item_id':file['item_id']})['sha256'],file['sha256'])
  import base64
  content=self.call('results','read_file',{'file_item_id':file['item_id']});self.assertEqual(base64.b64decode(content['data_base64']),b'{"synthetic":true}\n')
  streams=self.call('jobs','log_streams',{'id':id});self.assertTrue(streams['items']);stream=next(s for s in streams['items'] if s['stage']=='run')
  logs=self.call('jobs','read_log',{'stream_id':stream['item_id'],'tail':True});self.assertIsInstance(logs['offset'],str);self.assertTrue(logs['closed'])
  events=self.call('events','list',{'kind':'all'});self.assertEqual(sum(e['job']==id and e['code']=='complete' for e in events),1)
  self.assertNotIn('catalog.sqlite3',json.dumps(self.call('system','search_status',{})))
