"""Opt-in immutable native image acceptance on disposable CPU-only roots."""
import base64,hashlib,json,os,re,socket,subprocess,sys,tempfile,time,unittest,uuid
from pathlib import Path
from urllib.request import Request,urlopen
ROOT=Path(__file__).resolve().parents[2]
IMAGE=os.environ.get('LOOM_ITEM_IMAGE_ID','')
@unittest.skipUnless(re.fullmatch(r'sha256:[a-f0-9]{64}',IMAGE),'Provide an already built LOOM_ITEM_IMAGE_ID for isolated image acceptance')
class ItemImageTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='loom-item-image-');self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
  for name in ('state','data','components'):(self.root/name).mkdir()
  (self.root/'data/.loom-storage.json').write_text('{"storage_id":"image-data"}')
  self.token=uuid.uuid4().hex;self.container=None;self.addCleanup(self.stop)
  self.config={'machine_id':'image-fixture','storage_id':'image-data','actors':[{'id':'owner','scopes':['read','submit','cancel','events'],'owners':['owner'],'notify':['owner'],'token_sha256':hashlib.sha256(self.token.encode()).hexdigest()}]}
  (self.root/'config.json').write_text(json.dumps(self.config));(self.root/'config.json').chmod(0o600)
  self.start()
 def mounts(self):
  args=[]
  for name in ('state','data','components'):args+=['--mount',f'type=bind,source={self.root/name},target=/{name}']
  return args+['--mount',f'type=bind,source={self.root/"config.json"},target=/config.json,readonly']
 def start(self):
  command=['docker','run','--detach','--read-only','--user',f'{os.getuid()}:{os.getgid()}','--cap-drop','ALL','--security-opt','no-new-privileges','--pids-limit','128','--cpus','2','--memory','1g','--publish','127.0.0.1::8080',*self.mounts(),IMAGE,'--config','/config.json','--state','/state','--data','/data','--components','/components']
  self.container=subprocess.check_output(command,text=True,timeout=20).strip()
  info=json.loads(subprocess.check_output(['docker','inspect',self.container],text=True,timeout=5))[0];port=info['NetworkSettings']['Ports']['8080/tcp'][0]['HostPort'];self.url='http://127.0.0.1:'+port
  self.assertFalse(any(m['Destination']=='/var/run/docker.sock' for m in info['Mounts']))
  for _ in range(100):
   try:self.get('/api/status');return
   except OSError:time.sleep(.05)
  self.fail('Native image did not start')
 def stop(self):
  if self.container:
   subprocess.run(['docker','rm','--force',self.container],capture_output=True,timeout=20);self.container=None
 def get(self,path):
  with urlopen(Request(self.url+path,headers={'Authorization':'Bearer '+self.token}),timeout=10) as response:return json.load(response)
 def call(self,group,action,args):
  with urlopen(Request(self.url+'/api/actions',data=json.dumps({'group':group,'action':action,'arguments':args}).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+self.token}),timeout=10) as response:return json.load(response)
 def test_readonly_native_image_search_rebuild_and_packaged_clients_preserve_execution(self):
  absent=subprocess.run(['docker','exec',self.container,'sh','-c','command -v python3 || command -v python'],capture_output=True,text=True,timeout=10)
  self.assertNotEqual(absent.returncode,0);self.assertEqual(self.get('/api/status')['engine'],'rust')
  # Installed storage parity must work in the actual Python-free image.
  probe='for path do test -r "$path" && test -w "$path" && test -x "$path" && test -r "$path/.loom-storage.json" && test -s "$path/.loom-storage.json" || exit 1; done'
  good=subprocess.run(['docker','exec',self.container,'/bin/sh','-ec',probe,'loom-storage-probe','/data'],capture_output=True,timeout=5)
  self.assertEqual(good.returncode,0,good.stderr)
  denied=subprocess.run(['docker','exec',self.container,'/bin/sh','-ec',probe,'loom-storage-probe','/missing'],capture_output=True,timeout=5)
  self.assertNotEqual(denied.returncode,0)
  job=self.call('jobs','submit',{'key':'image-search','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False}})
  for _ in range(150):
   record=self.call('jobs','get',{'id':job['id']})
   if record['state']=='complete':break
   time.sleep(.05)
  self.assertEqual(record['state'],'complete');self.assertEqual(len(record['attempts']),1)
  for _ in range(150):
   page=self.call('results','search_items',{'text':job['id'],'kinds':['artifact'],'view':'all','role':'all'})
   if page['items']:
    files=self.call('results','item_files',{'item_id':page['items'][0]['item_id']})
    if files['items']:break
   time.sleep(.05)
  file=files['items'][0];file_id=file['item_id'];self.assertIsInstance(file_id,str);self.assertGreater(int(file_id),2**53)
  data=self.call('results','read_file',{'file_item_id':file_id});self.assertEqual(base64.b64decode(data['data_base64']),b'{"synthetic":true}\n')
  self.assertEqual(hashlib.sha256(base64.b64decode(data['data_base64'])).hexdigest(),file['sha256'])
  streams=self.call('jobs','log_streams',{'id':job['id']});self.assertTrue(streams['items']);self.assertTrue(self.call('jobs','read_log',{'stream_id':streams['items'][0]['item_id']})['closed'])
  # The distributed native image includes reversible registry state and its schema.
  self.call('components','register',{'id':'image-archive','manifest':{'label':'Archive fixture'},'files':{'main.rs':'fn main() {}'}})
  target={'kind':'component','owner':'owner','id':'image-archive','revision':1}
  before=self.call('components','get',{'owner':'owner','id':'image-archive','revision':1})
  preview=self.call('components','inspect_lifecycle',{'targets':[target],'state':'archived'})
  request={'targets':[target],'state':'archived','reason':'Isolated image acceptance','key':'image-archive','expected_digest':preview['digest']}
  receipt=self.call('components','set_lifecycle',request)
  self.assertEqual(receipt,self.call('components','set_lifecycle',request))
  self.assertFalse(any(v['id']=='image-archive' for v in self.call('components','list',{'include_archived':False})))
  # Current published and optional candidate clients call the actual new APIs.
  import importlib.util
  spec=importlib.util.spec_from_file_location('item_image_client_source',ROOT/'tests/ops/client_source.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
  packages=[module.PACKAGE]
  if os.environ.get('LOOM_USE_CANDIDATE'):packages.append(Path(os.environ['LOOM_USE_CANDIDATE']))
  token=self.root/'client-token.json';token.write_text(json.dumps({'token':self.token}));token.chmod(0o600)
  config=self.root/'client.json';config.write_text(json.dumps({'endpoint':self.url+'/mcp','credential':str(token)}))
  code="""import json,sys
from pathlib import Path
sys.path.insert(0,str(Path(sys.argv[1])/'clients'))
from stdio import OpsTransport
client=OpsTransport(Path(sys.argv[2]));r=client.rpc({'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'results','arguments':{'action':'get_file','arguments':{'file_item_id':sys.argv[3]}}}})
v=json.loads(r['result']['content'][0]['text']);assert v['item_id']==sys.argv[3] and isinstance(v['item_id'],str)
"""
  for package in packages:
   result=subprocess.run([sys.executable,'-I','-c',code,str(package),str(config),file_id],capture_output=True,text=True,timeout=20);self.assertEqual(result.returncode,0,result.stderr)
  self.stop()
  command=['docker','run','--rm','--user',f'{os.getuid()}:{os.getgid()}','--entrypoint','loom-migrate',*self.mounts(),IMAGE,'catalog-rebuild','--config','/config.json','--state','/state','--data','/data','--components','/components','--batches','100']
  result=subprocess.run(command,capture_output=True,text=True,timeout=30);self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(json.loads(result.stdout)['status'],'ready')
  self.start();rebuilt=self.call('results','get_file',{'file_item_id':file_id});self.assertEqual(rebuilt['item_id'],file_id)
  self.assertEqual(self.call('jobs','submit',{'key':'image-search','kind':'cpu.smoke','owner':'owner','notify':'owner','params':{'seconds':0,'fail':False}})['id'],job['id'])
  self.assertEqual(len(self.call('jobs','get',{'id':job['id']})['attempts']),1)
  self.assertEqual(sum(e['job']==job['id'] and e['code']=='complete' for e in self.call('events','list',{'kind':'all'})),1)
  archived=self.call('components','get',{'owner':'owner','id':'image-archive','revision':1})
  self.assertEqual(archived['lifecycle']['state'],'archived');self.assertEqual(archived['manifest'],before['manifest'])
  preview=self.call('components','inspect_lifecycle',{'targets':[target],'state':'active'})
  self.call('components','set_lifecycle',{'targets':[target],'state':'active','reason':'Restore isolated registration','key':'image-restore','expected_digest':preview['digest']})
  self.assertEqual(self.call('components','get',{'owner':'owner','id':'image-archive','revision':1})['lifecycle']['state'],'active')
