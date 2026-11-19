"""Native HTTP acceptance of explicit, bounded reconciliation and online maintenance."""
import hashlib,json,sqlite3,time,unittest
from pathlib import Path
from urllib.error import HTTPError
import test_service as service_fixture

class ReconciliationTests(unittest.TestCase):
 setUp=service_fixture.RustServiceTests.setUp
 stop=service_fixture.RustServiceTests.stop
 get=service_fixture.RustServiceTests.get
 post=service_fixture.RustServiceTests.post
 call=service_fixture.RustServiceTests.call
 def test_v3_scientific_import_selects_a_template_and_preserves_bytes(self):
  raw=b'preserved scientific fixture\n';(self.data/'template.txt').write_bytes(raw)
  t=self.call('results','get_metadata_template',{'id':'loom.dataset','revision':1});pin={k:t[k] for k in ('id','revision','sha256')}
  definition={'kind':'dataset','label':'Source demonstrations','primary':{'machine_id':'fixture','storage_id':'fixture-data','ref':'template.txt'},'metadata':{'template_ref':pin,'research':{'robot':{'status':'recorded','model':'Franka Panda'},'collection':{'status':'recorded','count':0,'unit':'episodes'}}}}
  op={'operation':'import','schema':'loom-asset/v3','source':{'system':'fixture','id':'typed-dataset','revision':'1'},'definition':definition}
  request={'key':'template-plan','proposal':{'schema':'loom-reconcile/v1','operations':[op]}}
  with self.assertRaises(HTTPError) as missing:self.call('results','reconcile_plan',request)
  self.assertEqual(missing.exception.code,400)
  self.assertEqual(json.load(missing.exception)['code'],'task_required')
  task=self.call('results','register_scenario',{'key':'import-task','definition':{'label':'Grasp'}})
  definition['scenario_ref']={'id':task['id'],'revision':1}
  organization={'path':[{'id':'original','label':'Original'}],'task_ref':{'id':'wrong','revision':1}}
  definition['metadata']['research']['organization']=organization
  with self.assertRaises(HTTPError) as mismatch:self.call('results','reconcile_plan',request)
  self.assertEqual(json.load(mismatch.exception)['code'],'organization_task')
  organization['task_ref']=definition['scenario_ref']
  p=self.call('results','reconcile_plan',{'key':'template-plan','proposal':{'schema':'loom-reconcile/v1','operations':[op]}})
  args={'plan_id':p['plan_id']};self.call('results','reconcile_validate',args);applied=self.call('results','reconcile_apply',args)
  self.assertEqual(applied,self.call('results','reconcile_apply',args))
  with sqlite3.connect(self.root/'state/ops.sqlite3') as db:
   row=json.loads(db.execute('SELECT manifest FROM asset_revisions').fetchone()[0]);self.assertEqual(row['label'],'Franka Panda · Source demonstrations');self.assertEqual(row['metadata_template'],pin);self.assertEqual(row['scenario_ref']['id'],task['id']);self.assertEqual(row['catalog_class'],'regular')
   self.assertEqual(db.execute('SELECT count(*) FROM jobs').fetchone()[0],0)
  self.assertEqual((self.data/'template.txt').read_bytes(),raw)
  reference={**op,'source':{**op['source'],'id':'reference-dataset'},'definition':{**definition,'catalog_class':'reference'}}
  reference['definition'].pop('scenario_ref')
  request={'key':'reference-plan','proposal':{'schema':'loom-reconcile/v1','operations':[reference]}}
  with self.assertRaises(HTTPError) as missing:self.call('results','reconcile_plan',request)
  self.assertEqual(missing.exception.code,400)
  self.assertEqual(json.load(missing.exception)['code'],'task_required')
  reference['definition']['scenario_ref']=definition['scenario_ref']
  ref=self.call('results','reconcile_plan',request)
  args={'plan_id':ref['plan_id']};self.call('results','reconcile_validate',args);done=self.call('results','reconcile_apply',args)
  result=self.call('results','get',{'id':done['receipt']['operations'][0]['id'],'revision':1})
  self.assertEqual(result['catalog_class'],'reference')
  self.assertEqual(result['scenario_ref']['id'],task['id'])

 def test_policy_and_evaluation_imports_resolve_exact_inputs_in_the_same_task(self):
  (self.data/'evidence.txt').write_text('source evidence')
  def task(label):
   t=self.call('results','register_scenario',{'key':label,'definition':{'label':label}})
   return {'id':t['id'],'revision':1}
  def proposal(kind,name,context,relations):
   return {'key':name,'proposal':{'schema':'loom-reconcile/v1','operations':[{
    'operation':'import','schema':'loom-asset/v2','source':{'system':'fixture','id':name,'revision':'1'},
    'definition':{'kind':kind,'label':name,'scenario_ref':context,'relations':relations,
                  'primary':{'machine_id':'fixture','storage_id':'fixture-data','ref':'evidence.txt'}}}]}}
  def accept(request):
   plan=self.call('results','reconcile_plan',request)
   args={'plan_id':plan['plan_id']};self.call('results','reconcile_validate',args)
   done=self.call('results','reconcile_apply',args)
   return {'id':done['receipt']['operations'][0]['id'],'revision':1}
  square=task('Square');other=task('Other')
  dataset=accept(proposal('dataset','D1',square,[]))
  trained={'kind':'trained_on',**dataset}
  for request,code in [(proposal('model','unbound',square,[]),'catalog_input_required'),
                       (proposal('model','wrong-task',other,[trained]),'catalog_input_task'),
                       (proposal('model','wrong-revision',square,[{**trained,'revision':2}]),'catalog_input_task')]:
   with self.assertRaises(HTTPError) as error:self.call('results','reconcile_plan',request)
   self.assertEqual(json.load(error.exception)['code'],code)
  policy=accept(proposal('model','BC-RNN',square,[trained]))
  experiment=accept(proposal('experiment','Evaluation',square,[{'kind':'evaluated_on',**dataset},{'kind':'evaluated_model',**policy}]))
  entity=self.call('results','resolve_item',{'native_ref':{'kind':'entity','identity':experiment}})
  dto=self.call('system','dashboard_entity',{'item_id':entity['item_id'],'revision_id':entity['revision_id'],'records_only':True})['entities'][0]
  self.assertEqual(dto['catalog_inputs']['datasets'][0]['ref']['identity'],dataset)
  self.assertEqual(dto['catalog_inputs']['policies'][0]['ref']['identity'],policy)

 def import_file(self,key,name='original.txt',kind='artifact',metadata=None):
  definition={'kind':kind,'label':'Preserved '+kind,'metadata':metadata or {},'primary':{'machine_id':'fixture','storage_id':'fixture-data','ref':name}}
  op={'operation':'import','schema':'loom-asset/v2','source':{'system':'preserved-fixture','id':key,'revision':'original'},'definition':definition}
  plan=self.call('results','reconcile_plan',{'key':key,'proposal':{'schema':'loom-reconcile/v1','operations':[op]}})
  args={'plan_id':plan['plan_id']}
  self.call('results','reconcile_validate',args)
  return self.call('results','reconcile_apply',args)
 def search(self,**query):
  for _ in range(150):
   try:page=self.call('results','search_items',query)
   except HTTPError as error:
    if error.code!=503:raise
    body=json.load(error)
    if body.get('code')!='index_unavailable':raise
    time.sleep(.05);continue
   if page['items'] and not page['partial']:return page
   time.sleep(.05)
  self.fail('Search did not catch up')
 def test_import_rebind_and_shadow_switch_preserve_identities_without_jobs(self):
  (self.data/'original.txt').write_bytes(b'preserved bytes\n')
  applied=self.import_file('import-original');receipt=applied['receipt'];native=receipt['operations'][0]
  self.assertEqual(receipt['created_jobs'],0)
  hit=self.search(kinds=['artifact'])['items'][0]
  item=self.call('results','resolve_item',{'native_ref':{'kind':'entity','identity':{'id':native['id'],'revision':1}}})
  self.assertEqual(hit['item_id'],item['item_id'])
  files=self.search(kinds=['file'],parent_item_id=item['item_id'],view='all',role='all')
  file=files['items'][0];before=self.call('results','read_file',{'file_item_id':file['item_id']})
  (self.data/'moved.txt').write_bytes(b'preserved bytes\n')
  locator={'machine_id':'fixture','storage_id':'fixture-data','ref':'moved.txt'}
  request={'key':'add-copy','proposal':{'schema':'loom-reconcile/v1','operations':[{'operation':'bind_file','file_item_id':file['item_id'],'locator':locator}]}}
  plan=self.call('results','reconcile_plan',request);args={'plan_id':plan['plan_id']}
  self.call('results','reconcile_validate',args);bound=self.call('results','reconcile_apply',args)
  self.assertTrue(bound['receipt']['operations'][0]['changed'])
  (self.data/'original.txt').unlink()
  self.assertEqual(self.call('results','read_file',{'file_item_id':file['item_id']})['data_base64'],before['data_base64'])
  task=self.call('system','rebuild_search',{'key':'online','max_batches':1000})
  for _ in range(150):
   progress=self.call('system','catalog_task',{'id':task['id']})
   self.assertNotEqual(progress['state'],'failed',progress)
   if progress['state']=='complete':break
   time.sleep(.05)
  self.assertEqual(progress['state'],'complete')
  self.assertEqual(self.call('results','get_file',{'file_item_id':file['item_id']})['item_id'],file['item_id'])
  with sqlite3.connect(self.root/'state/ops.sqlite3') as db:
   self.assertEqual(db.execute('select count(*) from jobs').fetchone()[0],0)
   self.assertEqual(db.execute('select count(*) from attempts').fetchone()[0],0)
   self.assertEqual(db.execute('select count(*) from asset_revisions').fetchone()[0],1)
 def test_changed_source_between_validate_and_apply_rolls_back_entire_plan(self):
  (self.data/'original.txt').write_text('before')
  op={'operation':'import','schema':'loom-asset/v2','source':{'system':'fixture','id':'mutated','revision':'1'},'definition':{'kind':'artifact','label':'Evidence','primary':{'machine_id':'fixture','storage_id':'fixture-data','ref':'original.txt'}}}
  plan=self.call('results','reconcile_plan',{'key':'stale','proposal':{'schema':'loom-reconcile/v1','operations':[op]}});args={'plan_id':plan['plan_id']}
  self.call('results','reconcile_validate',args);(self.data/'original.txt').write_text('after')
  with self.assertRaises(HTTPError) as error:self.call('results','reconcile_apply',args)
  self.assertEqual(error.exception.code,409)
  with sqlite3.connect(self.root/'state/ops.sqlite3') as db:self.assertEqual(db.execute('select count(*) from asset_revisions').fetchone()[0],0)

 def test_large_import_ranges_use_current_validation_and_try_matching_alternate_copy(self):
  body=b'large-fixture\n'*(6*1024*1024//14+1)
  (self.data/'original.txt').write_bytes(body)
  self.import_file('large-range')
  hit=self.search(kinds=['artifact'])['items'][0]
  file=self.search(kinds=['file'],parent_item_id=hit['item_id'],view='all',role='all')['items'][0]
  first=self.call('results','read_file',{'file_item_id':file['item_id'],'offset':'5242880','limit':32})
  import base64
  self.assertEqual(base64.b64decode(first['data_base64']),body[5242880:5242880+32])
  (self.data/'second.txt').write_bytes(body)
  locator={'machine_id':'fixture','storage_id':'fixture-data','ref':'second.txt'}
  plan=self.call('results','reconcile_plan',{'key':'large-copy','proposal':{'schema':'loom-reconcile/v1','operations':[{'operation':'bind_file','file_item_id':file['item_id'],'locator':locator}]}})
  args={'plan_id':plan['plan_id']};self.call('results','reconcile_validate',args);self.call('results','reconcile_apply',args)
  (self.data/'original.txt').write_bytes(b'changed')
  read=self.call('results','read_file',{'file_item_id':file['item_id'],'offset':'5242880','limit':32})
  self.assertEqual(read['data_base64'],first['data_base64'])
