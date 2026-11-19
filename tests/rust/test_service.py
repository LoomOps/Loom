"""Real Rust HTTP/MCP/producer process acceptance using disposable installation roots."""
from native_binaries import binary as native_binary
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tarfile
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
  self.process=subprocess.Popen([str(native_binary('loom-ops')),'--config',str(cfg),'--state',str(self.root/'state'),'--data',str(self.data),'--components',str(self.components),'--listen','127.0.0.1:'+str(port)],env={**os.environ,'LOOM_SDK_BINARY':str(native_binary('loom-sdk')),'LOOM_DEV_NO_AUTH':'0'},stdout=self.log,stderr=self.log)
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
 def get(self,path,timeout=5):
  with urlopen(Request(self.url+path,headers={'Authorization':'Bearer '+self.token}),timeout=timeout) as r:return json.load(r)
 def post(self,path,body,token=None):
  with urlopen(Request(self.url+path,data=json.dumps(body).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+(token or self.token)}),timeout=5) as r:return json.load(r)
 def call(self,group,action,args=None):return self.post('/api/actions',{'group':group,'action':action,'arguments':args or {}})
 def submit(self,key,seconds=0,**more):return self.call('jobs','submit',{'key':key,'kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':seconds,'fail':False},**more})['id']
 def test_metadata_templates_use_the_same_http_and_mcp_contract(self):
  listing=self.call('results','metadata_templates',{'kind':'dataset','limit':1});self.assertEqual(listing['items'][0]['id'],'loom.dataset');self.assertNotIn('definition',listing['items'][0])
  t=self.call('results','get_metadata_template',{'id':'loom.dataset','revision':1});pin={k:t[k] for k in ('id','revision','sha256')}
  args={'kind':'dataset','template_ref':pin,'research':{'robot':{'status':'recorded','model':'Franka Panda'},'collection':{'status':'recorded','count':0,'unit':'episodes'}}}
  http=self.call('results','validate_metadata',args)
  reply=self.post('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'results','arguments':{'action':'validate_metadata','arguments':args}}})
  self.assertEqual(json.loads(reply['result']['content'][0]['text']),http);self.assertEqual(http['scientific_acceptance'],'unverified')
  args['research']['collection']['count']=[{'episode':1}]
  with self.assertRaises(HTTPError) as error:self.call('results','validate_metadata',args)
  self.assertEqual(error.exception.code,400)
  self.assertIn('metadata',json.dumps(self.call('system','guide')))
 def test_report_contracts_share_http_mcp_and_published_schemas(self):
  contract=self.call('results','get_report_contract',{'id':'loom.report.sdg','revision':1})
  reply=self.post('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'results','arguments':{'action':'get_report_contract','arguments':{'id':'loom.report.sdg','revision':1}}}})
  self.assertEqual(json.loads(reply['result']['content'][0]['text']),contract)
  self.assertEqual(contract['origin'],'builtin')
  self.assertTrue(self.call('results','report_contracts',{'kind':'dataset','limit':1})['items'])
  coverage=self.call('results','validate_report_contract',{'definition':contract['definition']})
  self.assertEqual(len(coverage['definition']['effective_requirements']),3)
  args={'id':'http.sdg','key':'http-sdg','reason':'Reusable coverage fixture','definition':contract['definition']}
  created=self.call('results','register_report_contract',args)
  self.assertEqual(created,self.call('results','register_report_contract',args))
  with self.assertRaises(HTTPError) as error:self.call('results','validate_report_contract',{'definition':{'label':'Empty','output_kind':'experiment','activity':'comparison','requirements':[]}})
  self.assertEqual(error.exception.code,400)
  self.assertIn('reporting',json.dumps(self.call('system','guide')))
 def wait(self,id,state='complete'):
  for _ in range(100):
   job=self.call('jobs','get',{'id':id})
   if job['state']==state:return job
   time.sleep(.1)
  self.fail((job,(self.root/'service.log').read_text()))
 def test_development_category_is_visible_and_uses_normal_resource_execution(self):
  job=self.wait(self.submit('observer-development',category='development',presentation={'scope':'research','role':'main'}))
  self.assertEqual(job['category'],'development');self.assertEqual(job['spec']['kind'],'cpu.smoke');self.assertTrue(job['result']['artifact_ready'])
  listing=self.call('jobs','list',{'category':'development','view':'all'})
  rows=listing if isinstance(listing,list) else listing.get('jobs',[])
  self.assertIn(job['id'],[e['id'] for e in rows])
  contract=self.call('system','contract');self.assertIn('development',json.dumps(contract))
 def test_actual_rust_job_output_and_recipient_ack(self):
  job=self.wait(self.submit('native'));self.assertTrue(job['result']['artifact_ready']);self.assertTrue(job['result']['execution']['process_group_returned']);self.assertEqual(len(job['attempts']),1)
  entity=self.call('results','get',{'id':job['id']+'.observation','revision':1});self.assertEqual(entity['kind'],'artifact');self.assertEqual(entity['locations'][0]['status'],'available')
  args={'id':job['id'],'attempt':job['attempts'][0]['id'],'offset':0,'limit':65536}
  self.assertTrue(self.call('jobs','logs',args)['available'])
  for patch in ({'offset':-1},{'limit':0},{'limit':65537},{'attempt':'not-this-job'}):
   with self.assertRaises(HTTPError) as denied:self.call('jobs','logs',{**args,**patch})
   self.assertEqual(denied.exception.code,403 if 'attempt' in patch else 400)
  (self.data/'runs'/job['id']/args['attempt']/'worker.log').unlink()
  missing=self.call('jobs','logs',args);self.assertFalse(missing['available']);self.assertEqual(missing['next_offset'],0)

  events=self.call('events','list',{'kind':'all'});completion=next(e for e in events if e['job']==job['id'] and e['code']=='complete');self.call('events','ack',{'seq':completion['seq']});self.assertFalse(any(e['seq']==completion['seq'] for e in self.call('events','list',{'kind':'all'})))
 def test_real_mcp_action_groups_and_guide(self):
  init=self.post('/mcp',{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-03-26'}});self.assertEqual(init['result']['protocolVersion'],'2025-03-26')
  tools=self.post('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/list'});self.assertEqual({t['name'] for t in tools['result']['tools']},{'jobs','units','recovery','events','components','results','system'})
  for tool in tools['result']['tools']:
   name=tool['name'];action=tool['inputSchema']['properties']['action']['enum'][0]
   self.assertEqual(self.call('system','describe',{'group':name,'action':action})['action'],action)
  guide=self.post('/mcp',{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'system','arguments':{'action':'guide','arguments':{'id':'authoring'}}}});data=json.loads(guide['result']['content'][0]['text']);self.assertIn('Rust',data['content']);self.assertIn('loom',guide['result']['_meta'])
  for guide_id in ('job-cases','catalog-cases'):
   self.assertIn(guide_id,init['result']['instructions'])
  for guide_id in ('job-cases','job-intake-cases','job-control-cases','catalog-cases'):
   args={'action':'guide','arguments':{'id':guide_id}}
   reply=self.post('/mcp',{'jsonrpc':'2.0','id':5,'method':'tools/call','params':{'name':'system','arguments':args}})['result']
   self.assertEqual(json.loads(reply['content'][0]['text']),self.call('system','guide',{'id':guide_id}))
  identity=guide['result']['_meta']['loom'];marker=f"Loom {identity['build_id']} | contract {identity['contract_revision'][:12]} | guide {identity['guide_revision'][:12]}"
  self.assertEqual(guide['result']['content'][-1]['text'],marker)
  error=self.post('/mcp',{'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'jobs','arguments':{'action':'get','arguments':{'id':'missing'}}}})['result']
  self.assertTrue(error['isError']);self.assertEqual(error['content'][-1]['text'],marker)
  self.assertNotEqual(error['content'][0]['text'],marker)
  with urlopen(Request(self.url+'/api/status',headers={'Authorization':'Bearer '+self.token}),timeout=5) as response:
   self.assertEqual(response.headers['X-Loom-Revision'],identity['revision'])

 def test_job_unit_pipelining_and_history_have_distinct_identities(self):
  self.assertEqual(self.call('system','status')['execution_model'],'job-unit/v1')
  owner={'owner':'owner','notify':'owner'}
  smoke={'kind':'cpu.smoke','params':{'seconds':2,'fail':False}}
  task=self.call('results','register_scenario',{'key':'pipeline-task','definition':{'label':'Generate demonstrations'}})
  plans=[]
  for name in ('a','b'):
   proposed={**owner,'key':f'research-{name}','outcome':{
    'goal':f'Generate dataset {name}',
    'deliverables':[{'alias':'dataset','kind':'dataset','from':{'unit':'generate','entity':'dataset'}}]},
    'plan':{'sealed':False,'units':[
     {'id':'preflight','label':'Prepare inputs','phase':'prepare','execution':smoke},
     {'id':'generate','label':'Generate demonstrations','depends_on':['preflight']}]}}
   if name=='a':
    with self.assertRaises(HTTPError) as missing:self.call('jobs','submit_research',proposed)
    self.assertEqual(missing.exception.code,400)
    invalid={**proposed,'scenario_ref':{'id':task['id'],'revision':999}}
    with self.assertRaises(HTTPError) as unknown:self.call('jobs','submit_research',invalid)
    self.assertEqual(unknown.exception.code,404)
   proposed['scenario_ref']={'id':task['id'],'revision':1}
   accepted=self.call('jobs','submit_research',proposed)
   self.assertEqual(accepted,self.call('jobs','submit_research',proposed))
   plans.append(accepted)
  alone=self.call('units','submit',{**owner,'key':'standalone-smoke',**smoke,'execution_mode':'smoke'})['id']
  for _ in range(120):
   queue=self.call('system','dashboard_queue')['queue']
   if len([u for u in queue['unit_entries'] if u['state']=='running'])==3:break
   time.sleep(.025)
  else:self.fail(queue)
  self.assertEqual({j['id'] for j in queue['job_entries']},{p['id'] for p in plans})
  self.assertTrue(all(j['state']=='preparing' for j in queue['job_entries']))
  for plan in plans:
   job=self.call('jobs','get',{'id':plan['id']})
   member=plan['units']['preflight']
   self.assertEqual(job['entry_type'],'job')
   self.assertEqual(job['spec']['scenario_ref'],{'id':task['id'],'revision':1})
   self.assertNotIn('executions',job)
   self.assertEqual(job['spec']['outcome']['deliverables'][0]['from']['unit'],'generate')
   self.assertEqual(job['units'][0]['id'],member)
   exact=self.call('units','get',{'id':member})
   self.assertEqual(exact['unit']['job_id'],job['id'])
   self.assertNotIn('parent_job',exact['presentation'])
   self.assertNotIn('execution_membership',exact)
   self.wait(member)
  self.assertIsNone(self.wait(alone)['unit']['job_id'])
  for _ in range(100):
   page=self.call('system','dashboard_page',{'kinds':['job'],'view':'all','role':'all','limit':50})
   if not page['partial']:break
   time.sleep(.05)
  else:self.fail(page)
  items=page['entities']
  self.assertEqual({e['id'] for e in items if e['entry_type']=='unit'},{alone})
  self.assertEqual({e['id'] for e in items if e['entry_type']=='job'},{p['id'] for p in plans})
  with self.assertRaises(HTTPError) as invalid:
   self.call('units','submit',{**owner,'key':'not-a-child',**smoke,'presentation':{'scope':'research','role':'support','parent_job':plans[0]['id']}})
  self.assertEqual(invalid.exception.code,400)

 def test_published_walkthrough_requests_execute_against_isolated_loom(self):
  def examples(guide):
   content=self.call('system','guide',{'id':guide})['content']
   return [json.loads(block.split("```",1)[0]) for block in content.split("```json\n")[1:]]
  observed=self.wait(self.submit('walkthrough-existing-job'))
  status_request=next(e for e in examples('job-cases') if e.get('method')=='tools/call')
  status_request['params']['arguments']['arguments']['id']=observed['id']
  reply=self.post('/mcp',status_request)['result']
  self.assertFalse(reply.get('isError',False),reply)
  self.assertEqual(json.loads(reply['content'][0]['text'])['id'],observed['id'])
  before_jobs=self.call('jobs','list',{'view':'all'})

  catalog=examples('catalog-cases')
  template=self.post('/api/actions',next(e for e in catalog if e['action']=='get_metadata_template'))
  plan=next(e for e in catalog if e['action']=='reconcile_plan')
  definition=plan['arguments']['proposal']['operations'][0]['definition']
  task=self.call('results','register_scenario',{'key':'walkthrough-task','definition':{'label':'Grasp'}})
  definition['scenario_ref']={'id':task['id'],'revision':task['revision']}
  archive=self.data/definition['primary']['ref']
  archive.parent.mkdir()
  demo=archive.parent/'benchmark.json'
  demo.write_text('{"release":"2026.01"}')
  with tarfile.open(archive,'w') as tar:tar.add(demo,arcname='benchmark.json')
  definition['primary']['sha256']=hashlib.sha256(archive.read_bytes()).hexdigest()
  definition['metadata']['template_ref']['sha256']=template['sha256']
  def publish_import():
   accepted=self.post('/api/actions',plan)
   for action in ('reconcile_validate','reconcile_apply'):
    call=next(e for e in catalog if e['action']==action)
    call['arguments']['plan_id']=accepted['plan_id']
    response=self.post('/api/actions',call)
    if action=='reconcile_validate':self.assertEqual(response['state'],'validated')
   self.assertEqual(response['receipt']['created_jobs'],0)
   created=response['receipt']['operations'][0]
   return self.call('results','get',{'id':created['id'],'revision':created['revision']})

  imported=publish_import()
  self.assertEqual(imported['kind'],'dataset')
  self.assertEqual(imported['origin']['type'],'import')
  self.assertEqual(imported['origin']['source']['revision'],'2026.01')
  self.assertIn('available',[loc['status'] for loc in imported['locations']])

  restored=self.data/'restored-benchmark.tar'
  restored.write_bytes(archive.read_bytes())
  reconnect=next(e for e in catalog if e['action']=='reconnect')
  component=imported['components'][0]
  reconnect['arguments'].update(id=imported['id'],revision=imported['revision'],
                                component=component['ref'],ref=restored.name,storage='data/0')
  self.assertEqual(self.post('/api/actions',reconnect)['status'],'available')
  self.assertIn(restored.name,[loc['ref'] for loc in self.call(
   'results','get',{'id':imported['id'],'revision':imported['revision']})['locations']])

  plan['arguments']['key']='held-out-subset-fixture'
  plan['arguments']['proposal']['operations'][0]['source']['id']='grasp-heldout'
  child=publish_import()
  parent_ref={'kind':'entity','identity':{'id':imported['id'],'revision':imported['revision']}}
  child_ref={'kind':'entity','identity':{'id':child['id'],'revision':child['revision']}}
  digest=None
  for link in (e for e in catalog if e['action']=='link_dataset'):
   args=link['arguments']
   args.update(target=child_ref,parent=parent_ref)
   args['evidence'][0].update(target=parent_ref,component=component['ref'],sha256=component['sha256'])
   if not args['dry_run']:args['expected_digest']=digest
   response=self.post('/api/actions',link)
   self.assertEqual(response['stored'],not args['dry_run'])
   digest=response['digest']
  self.assertIsNotNone(digest)
  derived=self.call('results','get',child_ref['identity'])
  self.assertEqual(derived['dataset_lineage']['parents'][0]['parent'],parent_ref)
  self.assertEqual(self.call('jobs','list',{'view':'all'}),before_jobs)


 def test_scenario_job_pin_and_reuse_are_real_api_operations(self):
  s=self.call('results','register_scenario',{'key':'s','definition':{'label':'Fixture task'}})
  job=self.wait(self.submit('first',scenario_ref={'id':s['id'],'revision':1}));self.call('results','update_scenario',{'key':'s2','id':s['id'],'expected_revision':1,'definition':{'label':'Updated task'},'reason':'New protocol'})
  self.assertEqual(self.call('jobs','get',{'id':job['id']})['scenario_ref']['revision'],1)
  link=self.call('results','link_scenario',{'key':'link','scenario_ref':{'id':s['id'],'revision':2},'target':{'kind':'entity','identity':{'id':job['id']+'.observation','revision':1}},'relation':'used_by','provenance':{'reason':'Explicit reuse'}})
  graph=self.call('results','scenario_graph',{'id':s['id']});self.assertEqual(graph['links'][0]['id'],link['id']);self.assertEqual(graph['links'][0]['target'],link['target']);self.assertEqual(len(self.call('results','list',{'view':'all'})),1)
 def test_cancelled_native_process_has_no_published_result(self):
  id=self.submit('cancel',seconds=10);self.wait(id,'running');self.call('jobs','cancel',{'id':id});job=self.wait(id,'cancelled');self.assertFalse((job.get('result') or {}).get('artifact_ready'));self.assertFalse(self.call('results','list',{'view':'all'}))
 def test_open_plan_mcp_and_concurrent_http_seal_share_one_execution(self):
  from concurrent.futures import ThreadPoolExecutor
  slot={'id':'collect','label':'Collect evidence'}
  args={'key':'collect-once','owner':'owner','notify':'owner','outcome':{'goal':'Publish the observation','deliverables':[{'kind':'artifact','alias':'out','from':{'node':'collect','entity':'observation'}}]},'plan':{'sealed':False,'units':[slot]}}
  preview={'outcome':args['outcome']}
  envelope=self.post('/mcp',{'jsonrpc':'2.0','id':8,'method':'tools/call','params':{'name':'jobs','arguments':{'action':'preview_outcome','arguments':preview}}})
  self.assertEqual(json.loads(envelope['result']['content'][0]['text']),self.call('jobs','preview_outcome',preview))
  response=self.post('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'jobs','arguments':{'action':'create','arguments':args}}})
  created=json.loads(response['result']['content'][0]['text'])
  self.assertEqual(created,self.call('jobs','create',args))
  root=created['id']
  self.assertEqual(self.call('jobs','queue')['logical_entries'][0]['reason'],'awaiting_plan')
  slot['execution']={'kind':'cpu.smoke','params':{'seconds':0,'fail':False}}
  def bind(key):
   req={'id':root,'key':key,'reason':'Bind Profile','expected_revision':1,'plan':{'sealed':True,'units':[slot]}}
   try:return self.call('jobs','revise_plan',req)
   except HTTPError as exc:return exc.code
  with ThreadPoolExecutor(max_workers=2) as workers:receipts=list(workers.map(bind,['bind-a','bind-b']))
  self.assertEqual(sum(isinstance(r,dict) for r in receipts),1)
  self.assertIn(409,receipts)
  done=self.wait(root)
  self.assertEqual(len(done['executions']),1)
  self.assertEqual(len(done['executions'][0]['attempts']),1)
  self.assertEqual(done['result']['outputs']['out']['id'],'observation')
  self.assertEqual([(e['job'],e['code']) for e in self.call('events','list',{'kind':'all'})],[(root,'complete')])
 def test_scheduled_jobs_have_independent_completion_and_visibility(self):
  first=self.submit('intake')
  spec={'owner':'owner','notify':'owner','kind':'cpu.smoke','params':{'seconds':0,'fail':False}}
  second=self.call('jobs','submit',{**spec,'key':'evaluate','execution_mode':'pilot','pilot':{'preflight_for':'full evaluation','reduced_scope':'one test case'},'depends_on':[first]})['id']
  self.wait(second)
  events=self.call('events','list',{'kind':'all'})
  self.assertEqual({e['job'] for e in events if e['code']=='complete'},{first,second})
  for _ in range(100):
   page=self.call('system','dashboard_page',{'kinds':['job'],'view':'all','role':'all','limit':100})
   if {first,second} <= {e['id'] for e in page['entities']}:break
   time.sleep(.05)
  else:self.fail('Independent Jobs missing from paged discovery')
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
  if os.environ.get('LOOM_USE_REFERENCE'):
   self.client_acceptance(Path(os.environ['LOOM_USE_REFERENCE']));return
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
r=t.rpc({'jsonrpc':'2.0','id':2,'method':'tools/list'});assert {t['name'] for t in r['result']['tools']}=={'jobs','units','recovery','events','components','results','system'}
r=t.rpc({'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'system','arguments':{'action':'status','arguments':{}}}});assert json.loads(r['result']['content'][0]['text'])['engine']=='rust'
r=t.rpc({'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'system','arguments':{'action':'guide','arguments':{}}}});assert json.loads(r['result']['content'][0]['text'])['manifest']['revision'];assert 'loom' in r['result']['_meta']
'''
  result=subprocess.run([os.sys.executable,'-c',code,str(cfg)],cwd=package/'clients',env={**os.environ,'PYTHONPATH':str(package/'clients')},capture_output=True,text=True,timeout=25);self.assertEqual(result.returncode,0,result.stderr)
 def test_restart_preserves_original_submission_and_no_second_attempt(self):
  id=self.submit('restart',seconds=10);self.wait(id,'running')
  for _ in range(100):
   started=self.call('jobs','get',{'id':id})
   if started['attempts'][0]['started'] is not None:break
   time.sleep(.05)
  self.assertIsNotNone(started['attempts'][0]['started']);args=self.process.args;self.process.kill();self.process.wait(timeout=5)
  self.process=subprocess.Popen(args,env={**os.environ,'LOOM_SDK_BINARY':str(native_binary('loom-sdk')),'LOOM_DEV_NO_AUTH':'0'},stdout=self.log,stderr=self.log)
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
  time.sleep(.15);self.process=subprocess.Popen(self.process.args,env={**os.environ,'LOOM_SDK_BINARY':str(native_binary('loom-sdk')),'LOOM_DEV_NO_AUTH':'0'},stdout=self.log,stderr=self.log)
  for _ in range(100):
   if self.process.poll() is not None:self.fail((self.root/'service.log').read_text())
   try:self.get('/api/status');break
   except OSError:time.sleep(.05)
  self.wait(self.submit('remote-owned'))
  for malformed in ([],True):
   with self.assertRaises(HTTPError) as invalid:urlopen(Request(config['remote_fence']['url']+'/status',data=json.dumps(malformed).encode(),headers={'Authorization':'Bearer '+credential.read_text(),'Content-Type':'application/json'}),timeout=5)
   self.assertEqual(invalid.exception.code,400)
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
  # Preserve 5,001 results and the <2s mutation-lane bound. Bulk read completion
  # has a separate deadline because debug builds and shared CI CPUs vary.
  import concurrent.futures,sqlite3
  id=self.submit('concurrent-cancel',seconds=10);self.wait(id,'running');spec={'kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False},'category':'other','execution_mode':'smoke','depends_on':[],'output_storage':'data/0','presentation':{'scope':'operations','role':'test'}}
  with sqlite3.connect(self.root/'state/ops.sqlite3',timeout=10) as db:db.executemany("INSERT INTO jobs(id,submitter,idem,spec,owner,notify,state,created,cancel) VALUES(?,'owner',?,?,'owner','owner','cancelled',1,1)",[(f'synthetic-{i}',f'synthetic-{i}',json.dumps(spec)) for i in range(5000)])
  with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
   pending=pool.submit(self.get,'/api/dashboard/catalog',timeout=60);time.sleep(.1);started=time.monotonic();self.call('jobs','cancel',{'id':id});elapsed=time.monotonic()-started;self.assertLess(elapsed,2.0);catalog=pending.result(timeout=60);self.assertGreaterEqual(len(catalog['entities']),5001)
  self.assertTrue(all(e.get('group_summary') is None for e in catalog['entities'] if e['kind']=='job' and e['id'].startswith('synthetic-')))
  self.wait(id,'cancelled')

 def test_dashboard_navigation_burst_keeps_catalog_and_inspector_available(self):
  import concurrent.futures,threading
  for n in range(60):self.call('results','register_scenario',{'key':f'burst-{n}','definition':{'label':f'Navigation context {n}'}})
  barrier=threading.Barrier(12)
  def request(n):
   barrier.wait(timeout=5)
   if n%4==0:return self.get('/api/status')
   if n%4==1:return self.get('/api/dashboard/resources')
   return self.call('system','dashboard_page',{'kinds':['scenario'],'view':'all','role':'all','limit':50})
  with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:responses=list(pool.map(request,range(12)))
  self.assertEqual(len(responses),12)
  self.assertTrue(all('sections' in responses[n] for n in range(1,12,4)))
  self.assertTrue(all('entities' in responses[n] for n in range(12) if n%4>1))

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

 def test_metadata_capture_is_shared_by_http_mcp_and_catalog_without_source_rewrite(self):
  import sqlite3
  body={'entity':{'id':'metadata-fixture','kind':'dataset','label':'Measured dataset','properties':{'physics_hz':240},'relations':[],'artifacts':[]}}
  db=sqlite3.connect(self.root/'state/ops.sqlite3');db.execute("INSERT INTO history_records VALUES('fixture','dataset','metadata-fixture','r1','snapshot',?)",(json.dumps(body),));db.commit();db.close()
  target={'kind':'historical','identity':{'source':'fixture','kind':'dataset','id':'metadata-fixture','revision':'r1','snapshot':'snapshot'}}
  evidence_args={'target':target,'pointer':'/entity/properties/physics_hz'}
  evidence=self.call('results','get_report_evidence',evidence_args)
  self.assertEqual(evidence['value'],240)
  self.assertEqual(evidence['sha256'],hashlib.sha256(json.dumps(body).encode()).hexdigest())
  reply=self.post('/mcp',{'jsonrpc':'2.0','id':77,'method':'tools/call','params':{'name':'results','arguments':{'action':'get_report_evidence','arguments':evidence_args}}})
  self.assertEqual(json.loads(reply['result']['content'][0]['text']),evidence)
  preview=self.call('results','refresh_metadata',{'target':target,'reason':'fixture capture'})
  args={'target':target,'reason':'fixture capture','dry_run':False,'key':'capture-http-mcp','expected_source_sha256':preview['source_sha256']}
  reply=self.post('/mcp',{'jsonrpc':'2.0','id':78,'method':'tools/call','params':{'name':'results','arguments':{'action':'refresh_metadata','arguments':args}}})
  self.assertFalse(reply['result'].get('isError',False));receipt=json.loads(reply['result']['content'][0]['text']);self.assertTrue(receipt['stored'])
  self.assertEqual(self.call('results','refresh_metadata',args),receipt)
  db=sqlite3.connect(self.root/'state/ops.sqlite3');self.assertEqual(json.loads(db.execute("SELECT body FROM history_records WHERE id='metadata-fixture'").fetchone()[0]),body);self.assertEqual(db.execute("SELECT count(*) FROM rust_annotations WHERE kind='research_metadata'").fetchone()[0],1);db.close()
