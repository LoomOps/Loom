"""Real executor logic/SQLite, mocked Docker; no host daemon or GPU access."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import socketserver
import sys
from unittest.mock import patch
from test_support import IsolatedOps

class ExecutorBoundaryTests(IsolatedOps):
 def test_registered_runtime_executes_pinned_image_and_enforces_instance_grant(self):
  import runtime_store
  manifest={'runtime':{'image':'sha256:'+'b'*64,'network':'none'},'validation':{'status':'passed'},'dependencies':[]}
  ref=runtime_store.register(self.executor_root,'sim',manifest,['alpha'])
  request=self.request(lane='cpu');request.pop('runtime_id');request['runtime_ref']=ref
  with self.assertRaises(PermissionError):self.executor.handle('other','start',request)
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',{**request,'runtime_ref':{'id':'sim','revision':1}})
  result=self.executor.handle('alpha','start',request)
  self.assertIn('ticket',result)
  self.assertIn('sha256:'+'b'*64,next(c for c in self.calls if c[0]=='create'))
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',{**request,'ticket':'different','runtime_id':'python'})

 def test_unreturned_runtime_validation_and_installer_lock_block_gpu_only(self):
  import runtime_store,fcntl
  with (self.executor_root/'runtime-install.lock').open('a') as f:
   fcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB)
   self.assertTrue(self.executor.handle('alpha','start',self.request())['busy'])
   self.assertIn('ticket',self.executor.handle('dev','start',self.request(lane='cpu')))
  runtime_store.installation(self.executor_root,'attempt','sim','failed',{'validation_gpu':True,'container_returned':False})
  self.assertTrue(self.executor.handle('alpha','start',self.request())['busy'])
  self.assertFalse(any(c[0]=='create' and '--gpus' in c for c in self.calls))

 def test_runtime_owner_labels_do_not_partition_shared_jobs(self):
  self.executor.CONFIG['runtimes']['python']['owners']=['someone-else']
  request={**self.request(),'owner':'colleague'}
  started=self.executor.handle('alpha','start',request)
  self.assertIn('ticket',started)

 def setUp(self):
  super().setUp()
  self.executor_root=self.root/'executor';self.executor_root.mkdir()
  self.output=self.data/'runs';self.output.mkdir()
  config={'uid':os.getuid(),'gid':os.getgid(),'components':str(self.root/'components'),'runtimes':{'python':{'image':'sha256:'+'a'*64}},
    'instances':{i:{'uid':1000,'gid':os.getgid() or 1000,'gpu':i!='dev','runtimes':['python'],'data':str(self.data),
      'token_sha256':'0'*64} for i in ('alpha','other','dev')}}
  config['gid']=os.getgid() or 1000
  (self.executor_root/'config.json').write_text(json.dumps(config))
  directory=Path(__file__).resolve().parents[2]/'executor'
  sys.path.insert(0,str(directory));self.addCleanup(sys.path.remove,str(directory))
  spec=importlib.util.spec_from_file_location('loom_executor_fixture',directory/'server.py');self.executor=importlib.util.module_from_spec(spec)
  with patch.object(sys,'argv',['executor',str(self.executor_root)]),patch.object(socketserver.BaseServer,'serve_forever'),patch('os.chown'):
   spec.loader.exec_module(self.executor)
  self.addCleanup(self.executor.server.server_close)
  self.containers={};self.calls=[]
  p=patch.object(self.executor,'docker',side_effect=self.docker);p.start();self.addCleanup(p.stop)
 def docker(self,*args):
  self.calls.append(args)
  if args[0]=='create':
   name=args[args.index('--name')+1]
   if name in self.containers:raise RuntimeError('Container name already exists')
   labels=dict(args[i+1].split('=',1) for i,arg in enumerate(args[:-1]) if arg=='--label')
   self.containers[name]={'Config':{'Labels':labels},'State':{'Status':'created','ExitCode':0},'Image':'sha256:'+'a'*64};return name
  name=args[-1]
  if name not in self.containers:raise RuntimeError('No such container')
  item=self.containers[name]
  if args[0]=='inspect':return json.dumps([item])
  if args[0]=='start':item['State']['Status']='running'
  if args[0]=='stop':item['State'].update(Status='exited',ExitCode=137)
  return ''
 def request(self,ticket='job.attempt.run',lane='gpu'):
  return {'ticket':ticket,'job':'job','attempt':'attempt','owner':'owner','stage':'run','runtime_id':'python','lane':lane,
    'host_output':str(self.output),'command':['python3','/component/run.py'],'inputs':{}}
 def test_two_instances_compete_for_one_gpu_and_only_actual_return_releases(self):
  with ThreadPoolExecutor(max_workers=2) as pool:
   results=list(pool.map(lambda instance:(instance,self.executor.handle(instance,'start',self.request())),('alpha','other')))
  winner=next((i,r) for i,r in results if 'ticket' in r);loser=next(i for i,r in results if r.get('busy'))
  self.assertEqual(sum(r.get('busy',False) for _,r in results),1)
  self.executor.handle(winner[0],'cancel',{'ticket':winner[1]['ticket']})
  next_=self.executor.handle(loser,'start',self.request());self.assertFalse(next_['returned'])
  with self.executor.db() as c:self.assertEqual(c.execute("SELECT count(*) FROM tickets WHERE state!='returned'").fetchone()[0],1)
 def test_unknown_container_does_not_release_gpu_and_other_instance_cannot_cancel(self):
  first=self.executor.handle('alpha','start',self.request())
  with self.executor.db() as c:container=c.execute('SELECT container FROM tickets').fetchone()[0]
  del self.containers[container]
  self.assertFalse(self.executor.handle('alpha','status',{'ticket':first['ticket']})['returned'])
  self.assertTrue(self.executor.handle('other','start',self.request())['busy'])
  with self.assertRaises(PermissionError):self.executor.handle('other','cancel',{'ticket':first['ticket']})
 def test_idempotent_ticket_conflict_and_container_isolation_arguments(self):
  request=self.request();first=self.executor.handle('alpha','start',request)
  self.assertEqual(self.executor.handle('alpha','start',request)['ticket'],first['ticket'])
  self.assertEqual(sum(c[0]=='create' for c in self.calls),1)
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',{**request,'command':['different']})
  argv=next(c for c in self.calls if c[0]=='create')
  self.assertIn('--read-only',argv);self.assertIn('--cap-drop=ALL',argv)
  self.assertEqual(argv[argv.index('--network')+1],'none');self.assertNotIn('--privileged',argv)
 def test_tools_mount_only_registered_store_readonly_and_reject_escape(self):
  base=Path(self.executor.CONFIG['components'])/'.tools';tool=base/'owner/planner/revisions/1';tool.mkdir(parents=True)
  self.executor.handle('alpha','start',{**self.request(lane='cpu'),'tools':{'planner':str(tool)}})
  argv=next(c for c in self.calls if c[0]=='create')
  self.assertIn('type=bind,source='+str(tool)+',target=/tools/planner,readonly',argv)
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',{**self.request(ticket='escape.attempt.run',lane='cpu'),'tools':{'planner':'/etc'}})

 def test_administrator_graphics_and_bridge_keep_container_isolation(self):
  self.executor.CONFIG['runtimes']['python'].update(driver_capabilities=['compute','utility','graphics'],network='bridge')
  self.executor.handle('alpha','start',{**self.request(),'env':{'NVIDIA_DRIVER_CAPABILITIES':'all'}})
  argv=next(c for c in self.calls if c[0]=='create')
  environments=[argv[i+1] for i,v in enumerate(argv[:-1]) if v=='--env']
  self.assertEqual([v for v in environments if v.startswith('NVIDIA_DRIVER_CAPABILITIES=')][-1],'NVIDIA_DRIVER_CAPABILITIES=compute,utility,graphics')
  self.assertEqual(argv[argv.index('--network')+1],'bridge')
  self.assertIn('--read-only',argv);self.assertIn('--cap-drop=ALL',argv)
  self.assertEqual(argv[argv.index('--user')+1],'1000:'+str(os.getgid() or 1000))

 def test_driver_capability_default_invalid_policy_and_job_overrides(self):
  from runtime_policy import gpu_driver_capabilities
  self.assertEqual(gpu_driver_capabilities({}),'compute,utility')
  for bad in ('all',[],['all'],['graphics','graphics'],['graphics;anything'],[None]):
   with self.subTest(bad=bad),self.assertRaises(ValueError):gpu_driver_capabilities({'driver_capabilities':bad})
  for field,value in (('driver_capabilities',['graphics']),('network','bridge')):
   with self.assertRaises(PermissionError):self.executor.handle('alpha','start',{**self.request(),field:value})
  self.executor.CONFIG['runtimes']['python']['network']='host'
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',self.request())
  self.assertFalse(any(c[0]=='create' for c in self.calls))

 def test_declared_images_use_only_administrator_driver_and_network_policy(self):
  from runtime_policy import declared
  policy={'enabled':True,'network':'bridge','driver_capabilities':['compute','utility','graphics']}
  runtime=declared({'image':'trainer:local'},policy,lambda *a:'sha256:'+'a'*64)
  self.assertEqual(runtime['network'],'bridge');self.assertEqual(runtime['driver_capabilities'],policy['driver_capabilities'])
  self.assertEqual(declared({'image':'trainer:local'}, {'enabled':True},lambda *a:'sha256:'+'a'*64)['network'],'none')
 def test_instance_grants_and_path_escape_reject_before_container_creation(self):
  with self.assertRaises(PermissionError):self.executor.handle('dev','start',self.request())
  for data in [{**self.request(),'host_output':str(self.root)}, {**self.request(),'inputs':{'escape':'/etc/passwd'}}, {**self.request(),'runtime_id':'missing'}]:
   with self.subTest(data=data),self.assertRaises((ValueError,PermissionError)):self.executor.handle('alpha','start',data)
  self.assertFalse(any(c[0]=='create' for c in self.calls))
 def test_created_ticket_can_restart_without_second_container(self):
  first=self.executor.handle('alpha','start',self.request())
  with self.executor.db() as c:row=c.execute('SELECT * FROM tickets').fetchone()
  self.containers[row['container']]['State']['Status']='created'
  self.executor.handle('alpha','start',self.request())
  self.assertEqual(sum(c[0]=='create' for c in self.calls),1)
  self.assertEqual(sum(c[0]=='start' for c in self.calls),2)

 def test_create_effect_survives_inspect_failure_and_replay_adopts_once(self):
  normal=self.docker
  def interrupted(*args):
   if args[0]=='inspect':raise RuntimeError('Interrupted before ticket commit')
   return normal(*args)
  with patch.object(self.executor,'docker',side_effect=interrupted):
   with self.assertRaises(RuntimeError):self.executor.handle('alpha','start',self.request())
  with self.executor.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM tickets').fetchone()[0],0)
  self.assertEqual(len(self.containers),1)
  result=self.executor.handle('alpha','start',self.request())
  self.assertEqual(result['state'],'running');self.assertEqual(len(self.containers),1)
  self.assertEqual(sum(c[0]=='start' for c in self.calls),1)
  self.executor.handle('alpha','start',self.request())
  self.assertEqual(sum(c[0]=='start' for c in self.calls),1)
 def orphan(self):
  normal=self.docker
  def interrupted(*args):
   if args[0]=='inspect':raise RuntimeError('No ticket committed')
   return normal(*args)
  with patch.object(self.executor,'docker',side_effect=interrupted):
   with self.assertRaises(RuntimeError):self.executor.handle('alpha','start',self.request())
  return next(iter(self.containers.values()))
 def test_create_timeout_with_successful_docker_effect_recovers(self):
  import subprocess
  normal=self.docker
  def timeout(*args):
   value=normal(*args)
   if args[0]=='create':raise subprocess.TimeoutExpired('docker create',45)
   return value
  with patch.object(self.executor,'docker',side_effect=timeout):
   self.assertEqual(self.executor.handle('alpha','start',self.request())['state'],'running')
  self.assertEqual(len(self.containers),1)
 def test_foreign_or_changed_orphan_is_never_started_or_removed(self):
  row=self.orphan();original=dict(row['Config']['Labels'])
  for field in original:
   with self.subTest(field=field):
    row['Config']['Labels']={**original,field:'foreign'}
    with self.assertRaises(ValueError):self.executor.handle('alpha','start',self.request())
  row['Config']['Labels']=original
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',{**self.request(),'command':['different']})
  self.executor.CONFIG['runtimes']['python']['memory']='4g'
  with self.assertRaises(ValueError):self.executor.handle('alpha','start',self.request())
  self.assertFalse(any(c[0] in ('start','stop','rm') for c in self.calls))
  with self.executor.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM tickets').fetchone()[0],0)
 def test_unrecorded_running_or_exited_container_not_replayed(self):
  row=self.orphan()
  for state in ('running','exited','dead'):
   row['State']['Status']=state
   with self.subTest(state=state),self.assertRaises(ValueError):self.executor.handle('alpha','start',self.request())
  self.assertFalse(any(c[0] in ('start','stop','rm') for c in self.calls))
 def test_database_failure_after_create_replay_and_returned_ticket_not_restarted(self):
  import sqlite3
  with self.executor.db() as c:c.execute("CREATE TRIGGER reject_ticket BEFORE INSERT ON tickets BEGIN SELECT RAISE(ABORT,'injected commit failure'); END")
  with self.assertRaises(sqlite3.IntegrityError):self.executor.handle('alpha','start',self.request())
  with self.executor.db() as c:c.execute('DROP TRIGGER reject_ticket')
  result=self.executor.handle('alpha','start',self.request())
  self.executor.handle('alpha','cancel',{'ticket':result['ticket']})
  replay=self.executor.handle('alpha','start',self.request())
  self.assertTrue(replay['returned']);self.assertEqual(replay['exit_code'],137)
  self.assertEqual(sum(c[0]=='start' for c in self.calls),1)
 def test_orphan_replay_respects_current_gpu_ownership(self):
  self.orphan()
  other=self.executor.handle('other','start',self.request())
  self.assertTrue(self.executor.handle('alpha','start',self.request())['busy'])
  self.executor.handle('other','cancel',{'ticket':other['ticket']})
  self.assertEqual(self.executor.handle('alpha','start',self.request())['state'],'running')

 def test_selected_store_is_rw_without_changing_default_input_isolation(self):
  extra=self.root/'store';extra.mkdir()
  self.executor.CONFIG['instances']['alpha']['storage']={'data/1':str(extra)}
  self.executor.handle('alpha','start',{**self.request(),'storage':['data/1']})
  argv=next(c for c in self.calls if c[0]=='create')
  self.assertIn(f'type=bind,source={extra},target=/loom/data/1',argv)
  self.assertIn('--read-only',argv)

 def test_unregistered_or_changed_storage_is_rejected_before_docker(self):
  extra=self.root/'store';extra.mkdir();link=self.root/'linked';link.symlink_to(extra)
  self.executor.CONFIG['instances']['alpha']['storage']={'data/1':str(link)}
  for stores in [['data/1'],['data/2'],['/etc'],['data/1','data/1'],'data/1']:
   with self.subTest(stores=stores),self.assertRaises((ValueError,PermissionError)):
    self.executor.handle('alpha','start',{**self.request(),'storage':stores})
  self.assertFalse(any(c[0]=='create' for c in self.calls))

 def test_secondary_workspace_and_relative_input_are_resolved_from_registered_root(self):
  extra=self.root/'store';extra.mkdir();output=extra/'runs/job/attempt';output.mkdir(parents=True)
  source=extra/'dataset.txt';source.write_text('data')
  self.executor.CONFIG['instances']['alpha']['storage']={'data/1':str(extra)}
  request={**self.request(),'output_storage':'data/1','host_output':str(output),'inputs':{'dataset':{'storage':'data/1','ref':'dataset.txt'}}}
  self.executor.handle('alpha','start',request)
  argv=next(c for c in self.calls if c[0]=='create')
  self.assertIn(f'type=bind,source={output},target=/work',argv)
  self.assertIn(f'type=bind,source={source},target=/inputs/dataset,readonly',argv)
  self.assertNotIn(f'type=bind,source={extra},target=/loom/data/1',argv)
