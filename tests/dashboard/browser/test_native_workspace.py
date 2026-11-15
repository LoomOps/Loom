"""Firefox, retained Dashboard relay and the native Ops process on private fixture roots."""
import base64,json,os,socket,subprocess,tempfile,time,unittest
from pathlib import Path
from urllib.parse import urlsplit
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
 def wd(self,path,body=None):return reference.request(self.session+path,body)['value']
 def call(self,group,action,arguments):
  with urlopen(Request(self.native+'/api/actions',data=json.dumps({'group':group,'action':action,'arguments':arguments}).encode(),headers={'Content-Type':'application/json'}),timeout=5) as response:return json.load(response)
 def setUp(self):
  self.ui=Workspace(self.wd);self.wd('/window/rect',{'width':1440,'height':1000});self.wd('/url',{'url':self.base});self.ui.connect()
 def test_native_scenario_create_revision_and_render_without_injected_markup(self):
  ui=self.ui;ui.nav('Scenarios');ui.click('[data-scenario-create]');ui.js("const f=document.querySelector('[data-scenario-form]');f.elements.label.value='Native <img src=x onerror=alert(1)>';f.elements.description.value='Fixture context';f.elements.protocol.value='{}';f.querySelector('button[type=submit]').click()")
  ui.wait("return !document.querySelector('[data-hub-dialog][open]')");ui.wait("return document.querySelector('#pa-content').textContent.includes('Native <img')");ui.click('#pa-content [data-entity]');ui.wait("return !!document.querySelector('[data-scenario-edit]')");self.assertFalse(ui.js("return !!document.querySelector('#pa-content img[src=x]')"));self.assertNotIn('Unassigned',ui.js("return document.querySelector('#pa-content').textContent"))
  ui.click('[data-scenario-edit]');ui.js("const f=document.querySelector('[data-scenario-form]');f.elements.label.value='Native context revised';f.elements.reason.value='Protocol revision fixture';f.querySelector('button[type=submit]').click()");ui.wait("return !document.querySelector('[data-hub-dialog][open]')");ui.wait("return document.querySelector('[data-scenario-revision]')?.options.length===2")
  for width in (1440,500):
   ui.top();self.wd('/window/rect',{'width':width,'height':1000});ui.frame()
   for theme in ('light','dark'):
    ui.click('[data-theme='+theme+']');self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"));output=os.environ.get('LOOM_TEST_SCREENSHOTS')
    if output:
     path=Path(output);path.mkdir(parents=True,exist_ok=True);ui.top();(path/f'native-scenario-{width}-{theme}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
 def test_native_builtin_profile_and_all_queue_occupants_are_visible(self):
  self.ui.nav('Profiles');self.ui.wait("return document.querySelector('#pa-content').innerText.includes('cpu.smoke')")
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
