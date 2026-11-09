"""Real files and markers with mocked host boundaries; no production mutation."""
import copy,hashlib,json,os,sys,tempfile,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import storage_parity

class StorageParityTests(unittest.TestCase):
 def setUp(self):
  temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup);self.base=Path(temp.name);self.root=self.base/'ops'
  (self.root/'config').mkdir(parents=True);(self.root/'secrets').mkdir()
  (self.root/'secrets/executor-token').write_text('fixture-token')
  self.hosts={f'data/{n}':str(self.base/('store'+str(n))) for n in (0,1)}
  self.stores={n:{'path':'/loom/'+n,'mode':'rw','storage_id':'id-'+n[-1]} for n in self.hosts}
  for n,path in self.hosts.items():
   Path(path).mkdir();self.save(Path(path)/'.loom-storage.json',{'storage_id':self.stores[n]['storage_id']})
  self.api={'storage_id':'id-0','data_stores':self.stores,'executor':{'host_data':self.hosts['data/0'],'storage':self.hosts}}
  self.configs()
  self.container={'Config':{'User':str(os.getuid())+':'+str(os.getgid()),'Env':['LOOM_DATA_PATHS=/loom/data/0:/loom/data/1']},'Mounts':
   [{'Source':p,'Destination':self.stores[n]['path'],'RW':True} for n,p in self.hosts.items()]+
   [{'Source':self.hosts['data/0'],'Destination':'/loom/bulk','RW':True}]+
   [{'Source':str(self.root/'config'/(n+'.json')),'Destination':'/etc/loom/'+n+'.json','RW':False} for n in ('api','worker')]}
  self.executor={'instances':{'fixture':{'token_sha256':hashlib.sha256(b'fixture-token').hexdigest(),'uid':os.getuid(),'gid':os.getgid(),'data':self.hosts['data/0'],'storage':self.hosts}}}
  self.status={'storage_id':'id-0','data_stores':self.stores}
 def save(self,path,value):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value))
 def configs(self):
  for n in ('api','worker'):self.save(self.root/'config'/(n+'.json'),self.api)
 def validate(self):return storage_parity.validate(self.root,self.container,self.executor,self.status,list(self.hosts.values()))
 def test_previous_install_without_plan_uses_effective_storage(self):
  result=self.validate();self.assertEqual(result['status'],'passed');self.assertIn('absent',result['installation_plan'])
 def test_old_bulk_alias_rejected(self):
  self.container['Mounts'][2]['Source']=self.hosts['data/1']
  with self.assertRaisesRegex(ValueError,'OPS_BULK'):self.validate()
 def test_missing_store_worker_disagreement_and_duplicate_marker(self):
  original=copy.deepcopy(self.api);self.api['executor']['storage']={'data/0':self.hosts['data/0']};self.configs()
  with self.assertRaisesRegex(ValueError,'storage set'):self.validate()
  self.api=original;self.configs();self.save(self.root/'config/worker.json',{})
  with self.assertRaisesRegex(ValueError,'worker'):self.validate()
  self.configs();self.save(Path(self.hosts['data/1'])/'.loom-storage.json',{'storage_id':'id-0'})
  with self.assertRaisesRegex(ValueError,'identity'):self.validate()
 def test_ro_mount_explorer_and_executor_disagreement(self):
  for change,message in [('mount','mount/grant'),('explorer','Explorer data'),('executor','executor grant')]:
   with self.subTest(change=change):
    old=copy.deepcopy(self.container);ex=copy.deepcopy(self.executor)
    if change=='mount':self.container['Mounts'][1]['RW']=False
    if change=='explorer':self.container['Config']['Env']=['LOOM_DATA_PATHS=/loom/data/0']
    if change=='executor':self.executor['instances']['fixture']['storage']={'data/0':self.hosts['data/0']}
    with self.assertRaisesRegex(ValueError,message):self.validate()
    self.container=old;self.executor=ex
 def test_old_plan_boot_dependency_and_live_api(self):
  self.save(self.root/'config/storage-plan.json',{'primary':self.hosts['data/1'],'read_write':list(self.hosts.values())})
  with self.assertRaisesRegex(ValueError,'plan'):self.validate()
  (self.root/'config/storage-plan.json').unlink()
  with self.assertRaisesRegex(ValueError,'Boot mount'):storage_parity.validate(self.root,self.container,self.executor,boot_mounts=[self.hosts['data/0']])
  self.status['storage_id']='old'
  with self.assertRaisesRegex(ValueError,'Running API'):self.validate()
 def test_ssh_metadata_matches_actual_grants(self):
  state=self.base/'ssh';self.save(self.root/'config/ssh.json',{'state':str(state)})
  self.save(state/'config.json',{'stores':self.hosts});self.save(state/'storage.json',self.hosts)
  self.save(self.root/'ssh-info/machine.json',{'data_paths':self.hosts});self.validate()
  self.save(state/'storage.json',{'data/0':self.hosts['data/1']})
  with self.assertRaisesRegex(ValueError,'SSH/Explorer'):self.validate()
 def test_host_access_probe_fails_before_container_probe(self):
  self.save(self.base/'shared/executor/config.json',self.executor);calls=[]
  def run(*args):
   calls.append(args)
   if args[:2]==('docker','ps'):return 'ops-fixture'
   if args[:2]==('docker','inspect'):return json.dumps([self.container])
   if args[0]=='systemctl':return ' '.join(self.hosts.values())
   if args[0]=='runuser':raise PermissionError('fixture ACL denied')
   return ''
  with self.assertRaises(PermissionError):storage_parity.inspect(self.root,runner=run)
  self.assertTrue(any(c[0]=='runuser' for c in calls));self.assertFalse(any(c[:2]==('docker','exec') for c in calls))

if __name__=='__main__':unittest.main()
