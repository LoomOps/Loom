"""Firefox, retained Dashboard relay and the native Ops process on private fixture roots."""
import base64,hashlib,json,os,socket,sqlite3,subprocess,tempfile,time,unittest
from unittest.mock import patch
from pathlib import Path
from urllib.parse import urlsplit
from urllib.error import HTTPError
from urllib.request import Request,urlopen
import test_workspace as reference
from verify_workspace import Workspace
ROOT=Path(__file__).resolve().parents[3]
class NativeWorkspaceTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  reference.WorkspaceBrowserTests.setUpClass.__func__(cls)
  temp=tempfile.TemporaryDirectory(prefix='loom-native-browser-');cls.addClassCleanup(temp.cleanup);root=Path(temp.name);data=root/'data';data.mkdir();(data/'.loom-storage.json').write_text('{"storage_id":"browser-data"}');components=root/'components';components.mkdir();cfg=root/'config.json';cfg.write_text(json.dumps({'machine_id':'browser-fixture','storage_id':'browser-data','profiles':{'browser.profile.r1':{'schema':'loom-execution/v2','revision':'browser.profile.r1','sha256':'a'*64,'params_schema':{'type':'object'},'runtime':{'executor':'container','runtime_id':'cpu','command':['/bin/true']},'inputs':{},'outputs':None,'limits':{'timeout_seconds':30,'max_attempts':1}}},'actors':[{'id':'owner','scopes':['read','submit','cancel','events'],'owners':['owner'],'notify':['owner']}]}))
  with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
  cls.native='http://127.0.0.1:'+str(port);log=open(root/'ops.log','wb');cls.addClassCleanup(log.close)
  process=subprocess.Popen([str(ROOT/'target/debug/loom-ops'),'--config',str(cfg),'--state',str(root/'state'),'--data',str(data),'--components',str(components),'--listen','127.0.0.1:'+str(port)],env={**os.environ,'LOOM_DEV_NO_AUTH':'1','LOOM_SDK_BINARY':str(ROOT/'target/debug/loom-sdk')},stdout=log,stderr=log)
  cls.addClassCleanup(process.wait,10);cls.addClassCleanup(process.terminate)
  for _ in range(100):
   try:
    with urlopen(cls.native,timeout=1):break
   except OSError:time.sleep(.05)
  cls.web.upstream=urlsplit(cls.native)
  cls.native_root=root
 def wd(self,path,body=None):return reference.request(self.session+path,body)['value']
 def call(self,group,action,arguments):
  with urlopen(Request(self.native+'/api/actions',data=json.dumps({'group':group,'action':action,'arguments':arguments}).encode(),headers={'Content-Type':'application/json'}),timeout=5) as response:return json.load(response)
 def setUp(self):
  self.ui=Workspace(self.wd);self.wd('/window/rect',{'width':1440,'height':1000});self.wd('/url',{'url':self.base});self.ui.top();self.ui.js('localStorage.clear();sessionStorage.clear()');self.wd('/refresh',{});self.ui.connect()
 def test_metadata_template_browser_and_typed_robot_properties(self):
  base=self.call('results','get_metadata_template',{'id':'loom.dataset','revision':1});base_ref={k:base[k] for k in ('id','revision','sha256')}
  definition={'label':'Contact <img src=x onerror=alert(1)>','kind':'dataset','base_ref':base_ref,'schema':{'type':'object','properties':{'extensions':{'type':'object','properties':{'friction':{'type':'number'}}}}},'fields':[{'id':'friction','label':'Friction','pointer':'/extensions/friction','section':'extensions','format':'number'}]}
  t=self.call('results','register_metadata_template',{'id':'ui.contact','key':'ui-contact-template','reason':'Browser fixture format','definition':definition});pin={k:t[k] for k in ('id','revision','sha256')}
  (self.native_root/'data/template.json').write_text('{}')
  metadata={'template_ref':pin,'research':{'robot':{'status':'recorded','model':'Franka Panda'},'collection':{'status':'recorded','count':0,'unit':'episodes','split':{'train':0,'validation':'unrecorded','test':'not_applicable'}},'extensions':{'friction':0.2}}}
  op={'operation':'import','schema':'loom-asset/v3','source':{'system':'browser-fixture','id':'typed-contact','revision':'1'},'definition':{'kind':'dataset','label':'Contact demonstrations','metadata':metadata,'primary':{'machine_id':'browser-fixture','storage_id':'browser-data','ref':'template.json'}}}
  plan=self.call('results','reconcile_plan',{'key':'ui-contact-import','proposal':{'schema':'loom-reconcile/v1','operations':[op]}});args={'plan_id':plan['plan_id']};self.call('results','reconcile_validate',args);self.call('results','reconcile_apply',args)
  with sqlite3.connect(self.native_root/'state/ops.sqlite3') as db:identity=db.execute("SELECT id FROM asset_revisions WHERE json_extract(manifest,'$.origin.source.id')='typed-contact'").fetchone()[0]
  ui=self.ui;ui.nav('Datasets');ui.route(identity)
  ui.wait("return document.querySelector('[data-research-fact=robot]')?.textContent.includes('Franka Panda')")
  self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-research-overview [data-research-section]')][0]?.dataset.researchSection==='context'"))
  self.assertIn('train 0 / validation Unrecorded / test N/A',ui.js("return document.querySelector('.pa-research-overview').innerText"))
  self.assertIn('0.2',ui.js("return document.querySelector('[data-research-section=extensions]').textContent"))
  ui.nav('Loom Use');ui.click('[data-metadata-templates]');ui.wait("return !!document.querySelector('[data-template-id=\"ui.contact\"]')");ui.click('[data-template-id="ui.contact"]')
  ui.wait("return !!document.querySelector('[data-template-copy]')")
  self.assertFalse(ui.js("return !!document.querySelector('[data-hub-dialog] img[src=x]')"))
  self.assertIn(definition['label'],ui.js("return document.querySelector('[data-hub-dialog]').textContent"))
  for width in (1440,500):
   ui.top();self.wd('/window/rect',{'width':width,'height':1000});ui.frame()
   for theme in ('light','dark'):
    ui.click('[data-theme='+theme+']');self.assertTrue(ui.js("const d=document.querySelector('[data-hub-dialog]');return d.scrollWidth<=d.clientWidth+1"))
    if folder:=os.environ.get('LOOM_TEST_SCREENSHOTS'):
     output=Path(folder);output.mkdir(parents=True,exist_ok=True);ui.top();(output/f'metadata-templates-{width}-{theme}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
 def test_builtin_reporting_contracts_are_browsable_and_legacy_missing_is_explicit(self):
  ui=self.ui;ui.nav('Loom Use');ui.click('[data-report-contracts]')
  ui.wait("return !!document.querySelector('[data-report-contract-id=\"loom.report.policy\"]')")
  ui.click('[data-report-contract-id="loom.report.policy"]')
  ui.wait("return document.querySelector('[data-report-contract-detail]')?.textContent.includes('Policy instructions')")
  self.assertIn('Built-in',ui.js("return document.querySelector('[data-report-contract-list]').textContent"))
  self.assertIn('Action dimension',ui.js("return document.querySelector('[data-report-contract-detail]').textContent"))
  self.assertFalse(ui.js("return !!document.querySelector('[data-hub-dialog] [data-contract-register]')"))
  for width in (1440,500):
   ui.top();self.wd('/window/rect',{'width':width,'height':1000});ui.frame()
   for theme in ('light','dark'):
    ui.click('[data-theme='+theme+']');self.assertTrue(ui.js("const d=document.querySelector('[data-hub-dialog]');return d.scrollWidth<=d.clientWidth+1"))
 def test_verified_report_views_show_exact_instructions_channels_and_separate_phase_curves(self):
  job=self.call('jobs','submit',{'key':'browser-reports','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False}})['id'];identity=job+'.report-policy'
  def field(id,label,pointer,format,**extra):return dict(id=id,label=label,pointer=pointer,format=format,unit='',**extra)
  phase='carry <img src=x onerror=window.reportInjected=true>'
  source=[('instructions','instructions',{'texts':[{'id':'phase0','phase':'approach','text':'Approach <target> with open gripper.'}]},[field('texts','Instructions','/texts','instructions')]),('validation','learning_series',{'points':[{'step':1,'value':.2,'phase':'approach','split':'validation'},{'step':2,'value':.1,'phase':'approach','split':'validation'},{'step':1,'value':.4,'phase':phase,'split':'validation'},{'step':2,'value':.3,'phase':phase,'split':'validation'}]},[field('points','Validation','/points','series',x_pointer='/step',y_pointer='/value',x_label='Optimizer step',y_label='MSE')]),('errors','action_metrics',{'measurements':[{'channel_index':0,'error':'mse','value':.04,'unit':'rad²','phase':'approach','split':'test','population':10}]},[field('errors','Channel errors','/measurements','table',columns=[{'label':l,'pointer':p,'format':'text'} for l,p in [('Channel','/channel_index'),('Error','/error'),('Value','/value'),('Unit','/unit')]])])]
  components=[];reports=[]
  for name,role,data,fields in source:
   raw=json.dumps({'schema':'loom-report/v1','data':data}).encode();file='browser-report-'+name+'.json';(self.native_root/'data'/file).write_bytes(raw);sha=hashlib.sha256(raw).hexdigest();components.append({'ref':file,'size':len(raw),'sha256':sha});reports.append({'requirement_id':name,'label':name,'role':role,'state':'recorded','required':True,'component':file,'sha256':sha,'fields':fields})
  entity={'id':identity,'revision':1,'kind':'model','label':'Report view fixture','job':job,'components':components,'relations':[],'metadata':{'research':{'robot':{'status':'recorded','model':'Fixture robot'},'model':{'role':'policy','action_dimension':1}}},'reporting':{'state':'validated','contract_ref':{'id':'browser.fixture','revision':1,'sha256':'f'*64},'activity':'training','reports':reports,'inputs':[]}}
  with sqlite3.connect(self.native_root/'state/ops.sqlite3') as db:
   db.execute("INSERT INTO entity_revisions VALUES(?,1,?,'model','owner','owner',?)",(identity,job,json.dumps(entity)))
   for c in components:db.execute("INSERT INTO entity_locations VALUES(?,1,?,'browser-fixture','browser-data',?,'available')",(identity,c['ref'],c['ref']))
  for _ in range(100):
   try:
    resolved=self.call('results','resolve_item',{'native_ref':{'kind':'entity','identity':{'id':identity,'revision':1}}});break
   except HTTPError:time.sleep(.05)
  else:self.fail('Native report Item was not indexed')
  ui=self.ui;ui.nav('Policies');ui.route(identity)
  try:ui.wait("return document.querySelector('[data-report-content]')?.textContent.includes('Approach <target> with open gripper.')")
  except AssertionError:self.fail(ui.js("return document.querySelector('#pa-content').innerText"))
  self.assertFalse(ui.js("return !!document.querySelector('[data-report-content] target')"))
  self.assertEqual(ui.js("return document.querySelector('.pa-document-pane.is-active .pa-heading .pa-eyebrow').textContent"),'POLICY')
  ui.click('[data-report-tab=learning]');ui.wait("return document.querySelectorAll('.pa-report-chart polyline').length===2")
  self.assertTrue(ui.js("return [...document.querySelector('[data-report-filter=phase]').options].some(o=>o.value===arguments[0])",phase))
  self.assertFalse(ui.js("return !!document.querySelector('[data-report-content] img')||!!window.reportInjected"))
  ui.js("const s=document.querySelector('[data-report-filter=phase]');s.value=arguments[0];s.dispatchEvent(new Event('change',{bubbles:true}))",phase);ui.wait("return document.querySelectorAll('.pa-report-chart polyline').length===1")
  ui.click('[data-report-tab=metrics]');ui.wait("return document.querySelector('[data-report-content]')?.textContent.includes('rad²')")
  for width in (1440,500):
   ui.top();self.wd('/window/rect',{'width':width,'height':1000});ui.frame()
   self.assertLessEqual(ui.js("return document.querySelector('.pa-report-tabs').getBoundingClientRect().height"),110)
   self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-report-tabs button')].every(b=>b.getBoundingClientRect().width<200)"))
  ui.click('[data-report-tab=contract]');self.assertIn('browser.fixture',ui.js("return document.querySelector('[data-report-content]').textContent"))
  excluded=json.loads(json.dumps(entity));excluded['id']=job+'.unconditional-policy';excluded['reporting']['reports']=[r for r in reports if r['role']!='instructions'];excluded['reporting']['exclusions']=[{'id':'instructions','role':'instructions','reason':'Verified language-free input interface'}]
  with sqlite3.connect(self.native_root/'state/ops.sqlite3') as db:db.execute("INSERT INTO entity_revisions VALUES(?,1,?,'model','owner','owner',?)",(excluded['id'],job,json.dumps(excluded)))
  for _ in range(100):
   try:self.call('results','resolve_item',{'native_ref':{'kind':'entity','identity':{'id':excluded['id'],'revision':1}}});break
   except HTTPError:time.sleep(.05)
  ui.route(excluded['id']);ui.click('[data-report-tab=instructions]');ui.wait("return document.querySelector('[data-report-content]')?.textContent.includes('Not applicable')")

 def test_native_scenario_create_revision_and_render_without_injected_markup(self):
  task=self.call('results','register_scenario',{'key':'native-ui-context','definition':{'label':'Native <img src=x onerror=alert(1)>','description':'Fixture context'}})
  ui=self.ui;ui.nav('Models');self.assertFalse(ui.js("return !!document.querySelector('[data-scenario-create]')"));ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')");ui.wait("return [...document.querySelector('[data-research-task]').options].some(o=>o.value===arguments[0]+':1')",task['id']);ui.route(task['id']);ui.wait("return !!document.querySelector('[data-scenario-edit]')");self.assertFalse(ui.js("return !!document.querySelector('#pa-content img[src=x]')"));self.assertNotIn('Unassigned',ui.js("return document.querySelector('#pa-content').textContent"))
  ui.click('[data-scenario-edit]');ui.js("const f=document.querySelector('[data-scenario-form]');f.elements.label.value='Native context revised';f.elements.reason.value='Protocol revision fixture';f.querySelector('button[type=submit]').click()");ui.wait("return !document.querySelector('[data-hub-dialog][open]')");ui.wait("return document.querySelector('[data-scenario-revision]')?.options.length===2")
  for width in (1440,500):
   ui.top();self.wd('/window/rect',{'width':width,'height':1000});ui.frame()
   for theme in ('light','dark'):
    ui.click('[data-theme='+theme+']');self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"));output=os.environ.get('LOOM_TEST_SCREENSHOTS')
    if output:
     path=Path(output);path.mkdir(parents=True,exist_ok=True);ui.top();(path/f'native-scenario-{width}-{theme}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
 def test_system_builtin_profile_is_mcp_only_and_user_jobs_keep_queue_visibility(self):
  self.ui.nav('Profiles');self.ui.wait("return document.querySelector('#pa-content').innerText.includes('browser.profile.r1')");self.assertNotIn('cpu.smoke',self.ui.js("return document.querySelector('#pa-content').innerText"));self.assertEqual(next(p for p in self.call('components','profiles',{}) if p['kind']=='cpu.smoke')['owner_kind'],'system')
  job=self.call('jobs','submit',{'key':'browser-cpu','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':1,'fail':False}})
  self.assertIsInstance(job['id'],str);queue=self.call('jobs','queue',{});self.assertIn(job['id'],[j['id'] for j in queue['entries']])

 def test_scenario_profile_link_navigates_to_the_exact_profile_and_not_unrelated_jobs(self):
  scenario=self.call('results','register_scenario',{'key':'profile-context','definition':{'label':'Exact profile context'}})
  self.call('results','link_scenario',{'key':'profile-link','scenario_ref':{'id':scenario['id'],'revision':1},'target':{'kind':'profile','identity':{'kind':'browser.profile.r1','sha256':'a'*64}},'relation':'implements','provenance':{'reason':'Browser profile association fixture'}})
  job=self.call('jobs','submit',{'key':'unrelated-profile-job','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False}})
  self.wd('/url',{'url':self.base});self.ui.connect()
  self.ui.route('profile:browser.profile.r1')
  self.ui.wait("return document.querySelector('#pa-content').textContent.includes('Exact profile context')")
  self.ui.route(job['id'])
  self.assertNotIn('Exact profile context',self.ui.js("return document.querySelector('#pa-content').textContent"))
  self.ui.route(scenario['id'])
  self.ui.wait("return !!document.querySelector('#pa-content [data-entity=\"profile:browser.profile.r1\"]')")
  self.ui.click('#pa-content [data-entity="profile:browser.profile.r1"]')
  self.ui.wait("return document.querySelector('#pa-content').textContent.includes('Exact profile context')&&!!document.querySelector('[data-share-route=\"profile:browser.profile.r1\"]')")

 def test_server_item_search_preserves_string_ids_and_escapes_labels(self):
  name='Search fixture <img src=x onerror=alert(1)>'
  self.call('results','register_scenario',{'key':'search-ui','definition':{'label':name}})
  for _ in range(100):
   if self.call('results','search_items',{'text':'Search fixture'})['items']:break
   time.sleep(.05)
  self.ui.click('[data-item-search]')
  self.ui.js("document.querySelector('[data-item-search-form] input').value='Search fixture';document.querySelector('[data-item-search-form] button').click()")
  self.ui.wait("return !!document.querySelector('[data-search-item]')")
  self.assertIn(name,self.ui.js("return document.querySelector('[data-search-results]').textContent"))
  self.assertFalse(self.ui.js("return !!document.querySelector('.pa-item-search img')"))
  self.ui.click('[data-search-item]');self.ui.wait("return document.querySelector('[data-search-detail]').textContent.includes('Revision')")
  self.assertTrue(self.ui.js("return document.querySelector('[data-search-item]').dataset.searchItem.length>16"))

  # An old failed transport must not overwrite a newer selected detail.
  self.ui.top()
  self.ui.js("const original=window.fetch;window.fetch=function(url,options){if(String(url)==='/api/search/actions'&&JSON.parse(options.body).action==='get_item'&&!window.__rejectOldDetail)return new Promise((resolve,reject)=>window.__rejectOldDetail=reject);return original.call(window,url,options)};window.__restoreSearchFetch=()=>window.fetch=original")
  try:
   self.ui.frame();self.ui.click('[data-search-item]')
   self.ui.top();self.ui.wait("return !!window.__rejectOldDetail")
   self.ui.frame();self.ui.click('[data-search-item]')
   self.ui.wait("return document.querySelector('[data-search-detail]').textContent.includes('Revision')")
   current=self.ui.js("return document.querySelector('[data-search-detail]').textContent")
   self.ui.top();self.ui.js("window.__rejectOldDetail(Error('Old transport failed'))")
   time.sleep(.2);self.ui.frame()
   self.assertEqual(self.ui.js("return document.querySelector('[data-search-detail]').textContent"),current)
  finally:
   self.ui.top();self.ui.js("window.__restoreSearchFetch()");self.ui.frame()

  for width in (1440,500):
   self.ui.top();self.wd('/window/rect',{'width':width,'height':1000});self.ui.frame()
   for theme in ('light','dark'):
    self.ui.click('[data-theme='+theme+']')
    self.assertTrue(self.ui.js("return [...document.querySelectorAll('.pa-item-search')].every(p=>p.scrollWidth<=p.clientWidth+1)"))
    output=os.environ.get('LOOM_TEST_SCREENSHOTS')
    if output:
     path=Path(output);path.mkdir(parents=True,exist_ok=True);self.ui.top();(path/f'native-item-search-{width}-{theme}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));self.ui.frame()

 def test_native_catalog_pages_and_exact_history_are_server_backed(self):
  for n in range(65):
   self.call('results','register_scenario',{'key':'page-scenario-'+str(n),'definition':{'label':f'Paged Scenario {n:03}'}})
  self.wd('/url',{'url':self.base});self.ui.connect();self.ui.nav('Models')
  self.ui.wait("return [...document.querySelector('[data-research-task]').options].filter(e=>e.textContent.startsWith('Paged Scenario')).length===65")
  self.assertFalse(self.ui.js("return !!document.querySelector('#pa-nav [data-go=scenarios]')"))
  with urlopen(self.native+'/api/dashboard/bootstrap',timeout=5) as response:bootstrap=json.load(response)
  self.assertTrue(bootstrap['capabilities']['catalog_pages']);self.assertIsInstance(bootstrap['activity']['days'],list)
  self.ui.nav('Profiles');self.ui.wait("return document.querySelector('#pa-content').textContent.includes('browser.profile.r1')")

 def test_unloaded_native_job_link_does_not_depend_on_name_search(self):
  self.ui.nav('Loom Use')
  job=self.call('jobs','submit',{'key':'unloaded-job-link','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False}})['id']
  for _ in range(100):
   try:self.call('results','resolve_item',{'native_ref':{'kind':'job','identity':{'id':job}}});break
   except HTTPError:time.sleep(.05)
  else:self.fail('Job identity was not indexed')
  original=reference.server.WebHandler.relay
  def relay(handler,method,path,body=None,*,media=False):
   args=json.loads(body) if body and path=='/api/actions' else {}
   if args.get('action')=='search_items' and args.get('arguments',{}).get('text')==job:
    return handler.error(404,'ID is not a name search term')
   return original(handler,method,path,body,media=media)
  with patch.object(reference.server.WebHandler,'relay',relay):
   self.ui.route(job)
   self.ui.wait("const h=document.querySelector('.pa-document-pane.is-active h1');return h&&!h.textContent.includes('기록을 찾을 수 없습니다')")
   self.assertTrue(self.ui.js("return !!document.querySelector('[data-share-route=\"'+arguments[0]+'\"]')",job))

 def test_native_machine_and_inspector_use_the_collected_host_sample(self):
  sample={'observed_at':time.time(),'host':{'hostname':'observed-machine','ip':'192.0.2.4','ip_kind':'LAN','os':'Fixture Linux','kernel':'fixture-kernel'},'engine':{'version':'29.9.3'},'containers':[]}
  (self.native_root/'state/docker-status.json').write_text(json.dumps(sample))
  (self.native_root/'state/dashboard-gpus.json').write_text(json.dumps({'status':'ok','observed_at':time.time(),'data':[{'index':0,'name':'Fixture GPU','driver_version':'580.178.04'}]}))
  self.wd('/url',{'url':self.base});self.ui.connect()
  self.ui.wait("return document.querySelector('.pa-workspace').textContent.includes('observed-machine')")
  self.ui.wait("return document.querySelector('[data-resource-panel]').textContent.includes('data/0')")
  self.assertNotIn('undefined',self.ui.js("return document.querySelector('.pa-workspace').textContent"))
  self.ui.wait("const text=document.querySelector('.pa-sidebar-bottom').textContent;return text.includes('NVIDIA Driver · 580.178.04')&&text.includes('Docker · 29.9.3')")
  with urlopen(self.native+'/api/dashboard/resources',timeout=5) as response:resources=json.load(response)
  self.assertEqual(resources['host']['hostname'],'observed-machine')
  self.assertEqual(resources['sections']['memory']['status'],'ok')
  self.assertIsInstance(resources['sections']['leases']['data']['registered'],list)

 def seed_paged_research(self):
  with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
   for kind in ('dataset','model','run','issue'):
    for n in range(65):
     ident=f'page-{kind}-{n:03}'
     body={'original':{'id':ident,'kind':kind,'label':f'Paged {kind} {n:03}','namespace':'fixture','properties':{},'metrics':[],'relations':[],'artifacts':[]},'locations':[]}
     db.execute('INSERT OR IGNORE INTO history_records(source,kind,id,revision,snapshot,body) VALUES(?,?,?,?,?,?)',('catalog',kind,ident,'r1','s1',json.dumps(body)))
  for kind in ('dataset','model','experiment','issue'):
   for _ in range(150):
    page=self.call('results','search_items',{'text':'Paged','kinds':[kind],'view':'all','role':'all','limit':100})
    if len(page['items'])>=65:break
    time.sleep(.05)
   self.assertGreaterEqual(len(page['items']),65)
  # More recent Items deliberately displace these records from the bootstrap.
  for n in range(55):self.call('results','register_scenario',{'key':f'after-lists-{n}','definition':{'label':f'Recent context {n}'}})
  self.wd('/url',{'url':self.base});self.ui.connect()

 def test_native_research_lists_load_records_outside_the_bootstrap(self):
  self.seed_paged_research()
  for nav,kind in [('Datasets','dataset'),('Models','model'),('Experiments','run'),('Troubleshooting','issue')]:
   self.ui.nav(nav)
   self.ui.wait("return document.querySelector('#pa-content').textContent.includes(arguments[0])",f'Paged {kind}')
   page_kind='experiment' if kind=='run' else kind
   self.ui.wait("return !!document.querySelector('[data-catalog-more='+arguments[0]+']')",page_kind)
   self.ui.click(f'[data-catalog-more={page_kind}]')
   self.ui.wait("return [...document.querySelectorAll('#pa-content [data-entity]')].filter(e=>e.textContent.startsWith(arguments[0])).length===65",f'Paged {kind}')

 def test_background_bootstrap_preserves_pages_and_reloads_after_revision_changes(self):
  self.seed_paged_research()
  for nav,kind in [('Datasets','dataset'),('Models','model'),('Experiments','run')]:
   self.ui.nav(nav)
   self.ui.wait("return document.querySelector('#pa-content').textContent.includes(arguments[0])",f'Paged {kind}')
   page_kind='experiment' if kind=='run' else kind
   self.ui.wait("return !!document.querySelector('[data-catalog-more='+arguments[0]+']')",page_kind)
   self.ui.click('[data-catalog-more='+page_kind+']')
   self.ui.wait("return [...document.querySelectorAll('#pa-content [data-entity]')].filter(e=>e.textContent.startsWith(arguments[0])).length===65",f'Paged {kind}')
   for changed in (False,True):
    if changed:
     self.call('results','register_scenario',{'key':'refresh-'+kind,'definition':{'label':'Refresh '+kind}})
    self.ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
    self.ui.wait("return document.querySelector('#pa-content').textContent.includes(arguments[0])",f'Paged {kind}')
    time.sleep(.5)
    self.assertIn(f'Paged {kind}',self.ui.js("return document.querySelector('#pa-content').textContent"))

 def test_task_filter_loads_exact_links_and_retains_source_snapshots(self):
  task=self.call('results','register_scenario',{'key':'filter-task','definition':{'label':'Shared grasp task'}})
  identity={'source':'catalog','kind':'model','id':'task-model','revision':'r1','snapshot':'exact'}
  with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
   for snapshot in ['exact','other']:
    body={'original':{'id':'task-model','kind':'model','label':'Linked model '+snapshot,'properties':{},'metrics':[],'relations':[],'artifacts':[]},'locations':[]}
    db.execute('INSERT INTO history_records(source,kind,id,revision,snapshot,body) VALUES(?,?,?,?,?,?)',('catalog','model','task-model','r1',snapshot,json.dumps(body)))
  self.call('results','link_scenario',{'key':'filter-link','scenario_ref':{'id':task['id'],'revision':1},'target':{'kind':'historical','identity':identity},'relation':'produced_for','provenance':{'reason':'Exact historical model'}})
  for _ in range(100):
   page=self.call('system','dashboard_page',{'kinds':['model'],'scenario_ref':{'id':task['id'],'revision':1}})
   if page['entities']:break
   time.sleep(.05)
  self.assertEqual(len(page['entities']),1)
  self.assertEqual(page['entities'][0]['loom_history']['snapshot'],'exact')
  self.wd('/url',{'url':self.base});self.ui.connect();self.ui.nav('Models')
  self.ui.js("const field=document.querySelector('[data-research-task]');field.value=arguments[0]+':1';field.dispatchEvent(new Event('change',{bubbles:true}))",task['id'])
  self.ui.wait("return document.querySelector('#pa-content').textContent.includes('Linked model exact')")
  self.assertNotIn('Linked model other',self.ui.js("return document.querySelector('#pa-content').textContent"))
  self.assertTrue(self.ui.js("return !!document.querySelector('.pa-task-label')"))
  for width in (1440,500):
   self.ui.top();self.wd('/window/rect',{'width':width,'height':1000});self.ui.frame()
   for theme in ('light','dark'):
    self.ui.click('[data-theme='+theme+']');self.assertTrue(self.ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"))


 def test_explorer_fetches_registered_files_and_roots_without_the_inspector(self):
  self.ui.top();self.ui.js('sessionStorage.clear()')
  self.wd('/url',{'url':self.base+'/#/views/explorer'});self.ui.connect()
  self.ui.wait("return !!document.querySelector('.pa-finder-favorites [data-column-node=\\\"storage/data/0\\\"]')")
  self.assertFalse(self.ui.js("return !!document.querySelector('[data-resource-panel]')"))

 def test_open_history_detail_survives_a_new_bootstrap_revision(self):
  identity={'source':'catalog','kind':'model','id':'detail-refresh-model','revision':'r1','snapshot':'detail'}
  with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
   body={'original':{'id':identity['id'],'kind':'model','label':'Detail refresh retained','properties':{},'metrics':[],'relations':[],'artifacts':[]},'locations':[]}
   db.execute('INSERT INTO history_records(source,kind,id,revision,snapshot,body) VALUES(?,?,?,?,?,?)',(*identity.values(),json.dumps(body)))
  for _ in range(100):
   page=self.call('system','dashboard_page',{'kinds':['model'],'text':'Detail refresh retained'})
   if page['entities']:break
   time.sleep(.05)
  ident=page['entities'][0]['id']
  for n in range(55):self.call('results','register_scenario',{'key':'after-detail-'+str(n),'definition':{'label':'Recent detail context '+str(n)}})
  self.wd('/url',{'url':self.base});self.ui.connect();self.ui.nav('Models')
  for _ in range(5):
   if self.ui.js("return !!document.querySelector('[data-entity=\\\"'+arguments[0]+'\\\"]')",ident):break
   self.ui.click('[data-catalog-more=model]');time.sleep(.2)
  self.ui.entity(ident)
  for changed in (False,True):
   if changed:self.call('results','register_scenario',{'key':'after-open-detail','definition':{'label':'Change while reading'}})
   self.ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
   time.sleep(.7)
   self.assertIn('Detail refresh retained',self.ui.js("return document.querySelector('#pa-content').textContent"))
   self.assertTrue(self.ui.js("return !!document.querySelector('[data-share-route=\"'+arguments[0]+'\"]')",ident))


 def test_direct_comparison_pages_candidates_and_retains_selection_on_reload(self):
  def insert(kind,ident,body):
   with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
    db.execute('INSERT INTO history_records(source,kind,id,revision,snapshot,body) VALUES(?,?,?,?,?,?)',('catalog',kind,ident,'r1','cold-comparison',json.dumps({'original':body,'locations':[],'record':{'created':1700000000}})))
  insert('model','cold-model',{'id':'cold-model','kind':'model','label':'Cold comparison model','properties':{},'metrics':[],'relations':[],'artifacts':[]})
  for _ in range(100):
   model=self.call('system','dashboard_page',{'kinds':['model'],'text':'Cold comparison model'})['entities']
   if model:break
   time.sleep(.05)
  insert('run','cold-run',{'id':'cold-run','kind':'run','label':'Cold comparison result','properties':{},'metrics':[{'id':'score','label':'Score','kind':'scalar','unit':'m','observations':[{'value':.5}]}],'relations':[{'kind':'evaluated_model','id':model[0]['id']}],'artifacts':[]})
  for _ in range(100):
   rows=self.call('system','dashboard_page',{'kinds':['experiment'],'text':'Cold comparison result'})['entities']
   if rows:break
   time.sleep(.05)
  for n in range(55):self.call('results','register_scenario',{'key':'cold-context-'+str(n),'definition':{'label':'Recent comparison context '+str(n)}})
  self.wd('/url',{'url':self.base+'/#/views/comparison'});self.ui.connect()
  self.ui.wait("return [...document.querySelector('[data-compare-model]').options].some(e=>e.textContent==='Cold comparison model')")
  self.ui.js("const e=document.querySelector('[data-compare-model]');e.value=arguments[0];e.dispatchEvent(new Event('change',{bubbles:true}))",model[0]['id'])
  self.ui.wait("return [...document.querySelector('[data-compare-run]').options].some(e=>e.textContent.includes('Cold comparison result'))")
  self.ui.js("document.querySelector('[data-compare-run]').value=arguments[0]",rows[0]['id'])
  self.ui.click('[data-compare-add]')
  self.ui.wait("return !!document.querySelector('[data-compare-remove]')")
  self.ui.top();self.wd('/refresh',{});self.ui.connect()
  self.ui.wait("return !!document.querySelector('[data-compare-remove]')")
  self.assertEqual(self.ui.js("return document.querySelector('[data-compare-remove]').dataset.compareRemove"),rows[0]['id'])

 def test_aaa_revision_refresh_keeps_loaded_depth_and_visible_output_anchor(self):
  for kind,nav in [('model','Models'),('dataset','Datasets'),('run','Experiments')]:
   with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
    for n in range(65):
     ident=f'anchor-{kind}-{n:03}'
     body={'original':{'id':ident,'kind':kind,'label':f'Anchor {kind} {n:03}','summary':'Measured output description '*12,'properties':{},'metrics':[],'relations':[],'artifacts':[]},'locations':[],'record':{'created':1700000000+n}}
     db.execute('INSERT INTO history_records VALUES(?,?,?,?,?,?)',('catalog',kind,ident,'r1','anchor',json.dumps(body)))
   query_kind='experiment' if kind=='run' else kind
   for _ in range(150):
    page=self.call('system','dashboard_page',{'kinds':[query_kind],'text':'Anchor '+kind,'limit':100})
    if len(page['entities'])==65 and not page.get('partial'):break
    time.sleep(.05)
   self.wd('/url',{'url':self.base});self.ui.connect();self.ui.nav(nav)
   self.ui.wait("return !!document.querySelector('[data-catalog-more='+arguments[0]+']')",query_kind)
   self.ui.click('[data-catalog-more='+query_kind+']')
   self.ui.wait("return document.querySelectorAll('[data-output-id]').length===65")
   if kind=='dataset':self.ui.click('[data-dataset-family=unassigned]>summary')
   self.ui.js("document.querySelector('.pa-pane-body').scrollTop=1200")
   time.sleep(.2)
   capture="""const b=document.querySelector('.pa-pane-body'),edge=b.getBoundingClientRect(),e=[...b.querySelectorAll('[data-output-id]')].find(e=>e.getClientRects().length&&e.getBoundingClientRect().bottom>edge.top);return {id:e.dataset.outputId,offset:e.getBoundingClientRect().top-edge.top,top:b.scrollTop,count:b.querySelectorAll('[data-output-id]').length};"""
   before=self.ui.js(capture);self.assertGreater(before['top'],500,{'kind':kind,'layout':self.ui.js("return [...document.querySelectorAll('.pa-pane-body')].map(p=>({height:p.clientHeight,scroll:p.scrollHeight,families:[...p.querySelectorAll('[data-dataset-family]')].map(f=>({id:f.dataset.datasetFamily,open:f.open})),rows:p.querySelectorAll('[data-output-id]').length}))")})
   for n in range(55):self.call('results','register_scenario',{'key':f'anchor-revision-{kind}-{n}','definition':{'label':f'Recent context {kind} {n}'}})
   self.ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
   for _ in range(12):
    sample=self.ui.js(capture)
    self.assertEqual(sample['id'],before['id']);self.assertLess(abs(sample['offset']-before['offset']),2);self.assertEqual(sample['count'],65)
    time.sleep(.1)
   # Delete an item above the viewport in this disposable authority.
   with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
    db.execute('DELETE FROM history_records WHERE source=? AND kind=? AND id=?',('catalog',kind,f'anchor-{kind}-064'))
   self.call('results','register_scenario',{'key':'anchor-deletion-'+kind,'definition':{'label':'Deletion refresh '+kind}})
   self.ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
   try:self.ui.wait("return document.querySelectorAll('[data-output-id]').length===64")
   except AssertionError:
    self.fail(str({'kind':kind,'page':self.call('system','dashboard_page',{'kinds':[query_kind],'limit':100}),'ui':self.ui.js("return {count:document.querySelectorAll('[data-output-id]').length,pager:document.querySelector('.pa-catalog-pagination')?.textContent}")}))
   after=self.ui.js(capture);self.assertEqual(after['id'],before['id']);self.assertLess(abs(after['offset']-before['offset']),2)
   # This class shares a native fixture: leave no extra output pages for later tests.
   with sqlite3.connect(self.native_root/'state/ops.sqlite3',timeout=10) as db:
    db.execute('DELETE FROM history_records WHERE source=? AND kind=? AND snapshot=?',('catalog',kind,'anchor'))
   for _ in range(100):
    try:page=self.call('results','search_items',{'kinds':[query_kind],'text':'Anchor '+kind,'view':'all','role':'all'})
    except HTTPError as error:
     if error.code!=409:raise
     time.sleep(.05);continue
    if not page['items'] and not page.get('partial'):break
    time.sleep(.05)
   self.assertFalse(page['items'])
 def test_registry_archive_restore_filters_and_preserved_exact_detail(self):
  self.call('components','register',{'id':'ui-archive','manifest':{'label':'UI archive fixture'},'files':{'main.rs':'fn main() {}'}})
  ui=self.ui;ui.route('component:owner:ui-archive:r1');ui.wait("return !!document.querySelector('.pa-entity-menu')")
  ui.click('.pa-entity-menu summary');ui.click('[data-registry-state=archived]')
  ui.wait("return !!document.querySelector('[data-registry-apply]')")
  self.assertIn('Definitions, files and Job history are preserved',ui.js("return document.querySelector('[data-hub-dialog]').textContent"))
  ui.js("document.querySelector('[data-registry-apply] input[name=reason]').value='Mistaken fixture'")
  ui.click('[data-registry-apply] button');ui.wait("return !document.querySelector('[data-hub-dialog]')")
  self.assertEqual(self.call('components','get',{'owner':'owner','id':'ui-archive','revision':1})['lifecycle']['state'],'archived')
  ui.click('[data-command-open]');ui.wait("return !!document.querySelector('[data-item-search-form]')")
  ui.js("const f=document.querySelector('[data-item-search-form]');f.elements.text.value='UI archive fixture';f.elements.view.value='all';f.elements.kind.value='component'")
  ui.click('[data-item-search-form] button');ui.wait("return document.querySelector('[data-search-status]')?.textContent==='No results'")
  ui.click('[data-hub-close]')
  ui.nav('Runtimes');ui.click('[data-hub-page=components]')
  ui.js("const s=document.querySelector('[data-research-view]');s.value='all';s.dispatchEvent(new Event('change',{bubbles:true}))")
  ui.wait("return !!document.querySelector('[data-registry-filter]')")
  ui.wait("return !document.querySelector('[data-extension-card=\"component:owner:ui-archive:r1\"]')")
  ui.js("const s=document.querySelector('[data-registry-filter]');s.value='archived';s.dispatchEvent(new Event('change',{bubbles:true}))")
  ui.wait("return !!document.querySelector('[data-extension-card=\"component:owner:ui-archive:r1\"]')")
  ui.click('[data-entity="component:owner:ui-archive:r1"]');ui.click('.pa-entity-menu summary');ui.click('[data-registry-state=active]')
  ui.wait("return !!document.querySelector('[data-registry-apply]')")
  for width in (1440,500):
   ui.top();self.wd('/window/rect',{'width':width,'height':1000});ui.frame()
   for theme in ('light','dark'):
    ui.click('[data-theme='+theme+']');self.assertTrue(ui.js("const d=document.querySelector('[data-hub-dialog]');return d.scrollWidth<=d.clientWidth+1"))
  ui.js("document.querySelector('[data-registry-apply] input[name=reason]').value='Restore fixture'");ui.click('[data-registry-apply] button')
  ui.wait("return !document.querySelector('[data-hub-dialog]')")
  self.assertEqual(self.call('components','get',{'owner':'owner','id':'ui-archive','revision':1})['lifecycle']['state'],'active')
 def test_registry_template_batch_requires_explicit_related_selection(self):
  base=self.call('results','get_metadata_template',{'id':'loom.dataset'})
  def definition(label,base):return {'kind':'dataset','label':label,'base_ref':{k:base[k] for k in ('id','revision','sha256')},'schema':{'type':'object'},'fields':[]}
  parent=self.call('results','register_metadata_template',{'id':'ui.parent','definition':definition('Parent template',base),'reason':'fixture','key':'ui-parent'})
  self.call('results','register_metadata_template',{'id':'ui.child','definition':definition('Child template',parent),'reason':'fixture','key':'ui-child'})
  ui=self.ui;ui.nav('Loom Use');ui.click('[data-metadata-templates]');ui.wait("return !!document.querySelector('[data-template-id=\"ui.parent\"]')")
  ui.click('[data-template-id="ui.parent"]');ui.wait("return !!document.querySelector('[data-registry-state=archived]')");ui.click('[data-registry-state=archived]')
  ui.wait("return document.querySelectorAll('[data-registry-selected]').length>=2")
  self.assertEqual(ui.js("return [...document.querySelectorAll('[data-registry-selected]')].filter(x=>x.checked).length"),1)
  ui.click('[data-registry-selected]:not(:checked):not(:disabled)');ui.wait("return document.querySelector('[data-registry-apply] button')?.textContent.includes('2')")
  ui.js("document.querySelector('[data-registry-apply] input[name=reason]').value='Archive mistaken template family'");ui.click('[data-registry-apply] button');ui.wait("return !document.querySelector('[data-hub-dialog]')")
  for id in ('ui.parent','ui.child'):self.assertEqual(self.call('results','get_metadata_template',{'id':id})['lifecycle']['state'],'archived')
  ui.click('[data-metadata-templates]');ui.js("const s=document.querySelector('[data-registry-browser-filter]');s.value='archived';s.dispatchEvent(new Event('change',{bubbles:true}))")
  ui.wait("return !!document.querySelector('[data-template-id=\"ui.parent\"]') && !!document.querySelector('[data-template-id=\"ui.child\"]')")
