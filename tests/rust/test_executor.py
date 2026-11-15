"""Isolated CPU Docker/Unix executor acceptance. No installed service or GPU use."""
import hashlib,json,os,socket,sqlite3,subprocess,tempfile,time,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
@unittest.skipUnless(os.environ.get('LOOM_NATIVE_DOCKER_TEST')=='1','Set LOOM_NATIVE_DOCKER_TEST=1 for isolated CPU Docker acceptance')
class NativeExecutorTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='loom-native-executor-');self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name);self.host=self.root/'host';self.host.mkdir();self.data=self.root/'data';self.data.mkdir();self.components=self.root/'components';self.components.mkdir();self.deps=self.root/'dependencies';self.deps.mkdir();self.token='private-fixture-only'
  image=subprocess.run(['docker','image','inspect','python:3.12-slim','--format','{{.Id}}'],capture_output=True,text=True,check=True,timeout=15).stdout.strip();self.image=image
  account={'uid':os.getuid(),'gid':os.getgid(),'data':str(self.data),'components':str(self.components),'storage':{'data/0':str(self.data)},'runtimes':['cpu'],'dependencies':str(self.deps),'token_sha256':hashlib.sha256(self.token.encode()).hexdigest()}
  config={'uid':os.getuid(),'gid':os.getgid(),'components':str(self.components),'instances':{'one':account,'two':{**account,'token_sha256':hashlib.sha256(b'other-token').hexdigest()}},'runtimes':{'cpu':{'image':image,'network':'none','cpus':1,'memory':'128m','pids':32}}}
  (self.host/'config.json').write_text(json.dumps(config));self.log=open(self.root/'host.log','wb');self.addCleanup(self.log.close);self.process=None;self.addCleanup(self.cleanup);self.start()
 def start(self):
  self.process=subprocess.Popen([str(ROOT/'target/debug/loom-host'),'serve',str(self.host)],stdout=self.log,stderr=self.log)
  for _ in range(100):
   if self.process.poll() is not None:self.fail((self.root/'host.log').read_text())
   try:self.rpc('drain_status',{});return
   except OSError:time.sleep(.05)
  self.fail('Executor did not start')
 def stop(self):
  if self.process and self.process.poll() is None:self.process.terminate();self.process.wait(timeout=5)
 def cleanup(self):
  self.stop()
  if (self.host/'executor.sqlite3').exists():
   with sqlite3.connect(self.host/'executor.sqlite3') as db:
    containers=[r[0] for r in db.execute('SELECT container FROM tickets')]
   for container in containers:subprocess.run(['docker','rm','-f',container],capture_output=True,timeout=20)
 def rpc(self,action,value,token=None,status=200):
  raw=json.dumps(value).encode()
  with socket.socket(socket.AF_UNIX) as stream:
   stream.settimeout(30);stream.connect(str(self.host/'socket/executor.sock'));stream.sendall(('POST /'+action+' HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer '+(token or self.token)+'\r\nContent-Length: '+str(len(raw))+'\r\n\r\n').encode()+raw);parts=[]
   while chunk:=stream.recv(65536):parts.append(chunk)
  header,body=b''.join(parts).split(b'\r\n\r\n',1);self.assertEqual(int(header.split()[1]),status,body);return json.loads(body)
 def request(self,command):return {'ticket':'job.attempt.run','job':'job','stage':'run','lane':'cpu','runtime_id':'cpu','command':command,'host_output':str(self.data)}
 def wait(self,ticket):
  for _ in range(100):
   state=self.rpc('status',{'ticket':ticket})
   if state['returned']:return state
   time.sleep(.05)
  self.fail(state)
 def test_real_cpu_return_and_private_ticket_scope(self):
  accepted=self.rpc('start',self.request(['/bin/sh','-c','printf actual > /work/receipt.txt']));state=self.wait(accepted['ticket']);self.assertEqual(state['exit_code'],0);self.assertEqual((self.data/'receipt.txt').read_text(),'actual')
  self.rpc('status',{'ticket':accepted['ticket']},token='other-token',status=404)
  self.assertTrue(self.rpc('drain_status',{})['ready']);self.assertEqual((self.host/'executor.sqlite3').stat().st_mode&0o777,0o600)
 def test_restart_reconciles_same_container_and_cancellation_returns_owner(self):
  request=self.request(['/bin/sh','-c','sleep 30']);first=self.rpc('start',request);self.stop();self.start();again=self.rpc('start',request);self.assertEqual(first['ticket'],again['ticket']);self.assertEqual(again['state'],'running')
  returned=self.rpc('cancel',{'ticket':first['ticket']});self.assertTrue(returned['returned']);self.assertTrue(self.rpc('drain_status',{})['ready'])
  with sqlite3.connect(self.host/'executor.sqlite3') as db:self.assertEqual(db.execute('SELECT count(*) FROM tickets').fetchone()[0],1)
 def test_native_runtime_registration_binds_verified_cpu_image(self):
  first=self.rpc('start',self.request(['/bin/true']));self.assertEqual(self.wait(first['ticket'])['exit_code'],0)
  manifest={'label':'CPU acceptance','runtime':{'image':self.image,'network':'none','cpus':1,'memory':'128m','pids':32},'dependencies':[],'validation':{'status':'passed','scope':'CPU /bin/true only; no scientific readiness'}}
  path=self.root/'manifest.json';path.write_text(json.dumps(manifest));result=subprocess.run([str(ROOT/'target/debug/loom-host'),'runtime-register',str(self.host),'fixture-cpu',str(path),'one'],capture_output=True,text=True,check=True,timeout=15);pin=json.loads(result.stdout)
  exact=self.rpc('resolve_runtime',{'runtime_ref':pin});self.assertEqual(exact['manifest'],manifest);self.assertEqual(exact['sha256'],pin['sha256']);self.assertEqual((self.host/'runtimes.sqlite3').stat().st_mode&0o777,0o600)
 def test_full_log_ranges_preserve_bytes_beyond_the_control_reply_bound(self):
  accepted=self.rpc('start',self.request(['python','-c',"import sys;sys.stdout.write('x'*(5*1024*1024)+'TAIL')"]));self.assertEqual(self.wait(accepted['ticket'])['exit_code'],0)
  first=self.rpc('log_range',{'ticket':accepted['ticket'],'offset':0,'limit':16});self.assertEqual(first['text'],'x'*16);self.assertEqual(first['total_bytes'],5*1024*1024+4)
  last=self.rpc('log_range',{'ticket':accepted['ticket'],'offset':5*1024*1024,'limit':16});self.assertEqual(last['text'],'TAIL');self.assertTrue(last['eof'])

 def test_live_job_logs_are_read_through_native_ops_before_container_return(self):
  from urllib.request import Request,urlopen
  (self.data/'.loom-storage.json').write_text('{"storage_id":"fixture-data"}')
  credential=self.root/'credential';credential.write_text(self.token);credential.chmod(0o600)
  profile={'schema':'loom-execution/v2','revision':'fixture.live.r1','sha256':'a'*64,'params_schema':{'type':'object','additionalProperties':False},'runtime':{'executor':'container','runtime_id':'cpu','lane':'cpu','command':['/bin/sh','-c','echo LIVE-RECEIPT; sleep 20']},'inputs':{},'outputs':None,'limits':{'timeout_seconds':30,'max_attempts':1}}
  cfg=self.root/'api.json';cfg.write_text(json.dumps({'machine_id':'fixture','storage_id':'fixture-data','profiles':{'fixture.live.r1':profile},'actors':[{'id':'owner','scopes':['read','submit','cancel','events'],'owners':['owner'],'notify':['owner'],'token_sha256':hashlib.sha256(b'http-only').hexdigest()}],'executor':{'socket':str(self.host/'socket/executor.sock'),'credential':str(credential),'host_data':str(self.data),'storage':{'data/0':str(self.data)},'host_components':str(self.components)}}))
  with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
  process=subprocess.Popen([str(ROOT/'target/debug/loom-ops'),'--config',str(cfg),'--state',str(self.root/'ops'),'--data',str(self.data),'--components',str(self.components),'--listen',f'127.0.0.1:{port}'],stdout=self.log,stderr=self.log)
  def stop():
   if process.poll() is None:process.terminate();process.wait(timeout=10)
  self.addCleanup(stop)
  def call(action,args):
   with urlopen(Request(f'http://127.0.0.1:{port}/api/actions',data=json.dumps({'group':'jobs','action':action,'arguments':args}).encode(),headers={'Authorization':'Bearer http-only','Content-Type':'application/json'}),timeout=8) as response:return json.load(response)
  for _ in range(100):
   try:call('queue',{});break
   except OSError:time.sleep(.05)
  job=call('submit',{'key':'live','kind':'fixture.live.r1','owner':'owner','notify':'owner','params':{}})['id']
  last_log=None
  try:
   for _ in range(100):
    record=call('get',{'id':job})
    if record['state']=='running':
     try:
      logs=call('logs',{'id':job,'attempt':record['attempt'],'offset':0,'limit':65536});last_log=logs
      if 'LIVE-RECEIPT' in logs['text']:break
     except OSError as error:last_log=error.read().decode() if hasattr(error,'read') else str(error)
    time.sleep(.1)
   else:self.fail((record,last_log,(self.root/'host.log').read_text()))
   self.assertEqual(call('get',{'id':job})['state'],'running');self.assertIn('LIVE-RECEIPT',logs['text'])
  finally:call('cancel',{'id':job})
  for _ in range(100):
   if call('get',{'id':job})['state']=='cancelled':break
   time.sleep(.1)
  self.assertEqual(call('get',{'id':job})['state'],'cancelled')

 def test_slow_log_sealing_keeps_executor_cancellation_available(self):
  import concurrent.futures,shutil
  first=self.rpc('start',self.request(['/bin/true']));self.wait(first['ticket'])
  command=['python','-c',"import signal,time,sys; signal.signal(signal.SIGTERM,lambda *a:sys.exit(0)); open('/work/signal-ready','w').write('ready'); time.sleep(30)"]
  request=self.request(command);request['ticket']='second.attempt.run';request['job']='second';second=self.rpc('start',request)
  for _ in range(100):
   if (self.data/'signal-ready').exists():break
   time.sleep(.05)
  self.assertTrue((self.data/'signal-ready').exists())
  self.stop();bin=self.root/'bin';bin.mkdir();docker=shutil.which('docker');wrapper=bin/'docker'
  wrapper.write_text('#!/bin/sh\nif [ "$1" = logs ]; then sleep 3; fi\nexec '+docker+' "$@"\n');wrapper.chmod(0o700)
  self.process=subprocess.Popen([str(ROOT/'target/debug/loom-host'),'serve',str(self.host)],env={**os.environ,'PATH':str(bin)+':'+os.environ['PATH']},stdout=self.log,stderr=self.log)
  for _ in range(100):
   try:self.rpc('status',{'ticket':second['ticket']});break
   except OSError:time.sleep(.05)
  with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
   pending=pool.submit(self.rpc,'log_range',{'ticket':first['ticket'],'offset':0,'limit':16});time.sleep(.3)
   started=time.monotonic();returned=self.rpc('cancel',{'ticket':second['ticket']});elapsed=time.monotonic()-started
   self.assertTrue(returned['returned']);self.assertLess(elapsed,2.0);self.assertTrue(pending.result(timeout=15)['closed'])

 def test_capture_seals_after_process_return_without_another_log_read(self):
  accepted=self.rpc('start',self.request(['/bin/sh','-c','echo BEFORE-RETURN; sleep 1']))
  live=self.rpc('log_range',{'ticket':accepted['ticket'],'offset':0,'limit':64})
  generation=live['source_generation'];self.assertEqual(self.wait(accepted['ticket'])['exit_code'],0)
  pointer=self.host/'logs'/(hashlib.sha256(accepted['ticket'].encode()).hexdigest()+'.source.json')
  for _ in range(100):
   if pointer.exists() and json.loads(pointer.read_text()).get('closed'):break
   time.sleep(.05)
  self.assertTrue(json.loads(pointer.read_text())['closed'])
  self.stop();self.start();sealed=self.rpc('log_range',{'ticket':accepted['ticket'],'limit':64})
  self.assertEqual(sealed['source_generation'],generation);self.assertIn('BEFORE-RETURN',sealed['text'])

 def test_capture_budget_keeps_prefix_and_explicit_gap_without_stopping_job(self):
  import shutil
  accepted=self.rpc('start',self.request(['/bin/true']));self.assertEqual(self.wait(accepted['ticket'])['exit_code'],0)
  self.stop();binary=shutil.which('docker');folder=self.root/'limited-source';folder.mkdir();wrapper=folder/'docker'
  wrapper.write_text('#!/bin/sh\nif [ "$1" = logs ]; then exec head -c 270000000 /dev/zero; fi\nexec '+binary+' "$@"\n');wrapper.chmod(0o700)
  self.process=subprocess.Popen([str(ROOT/'target/debug/loom-host'),'serve',str(self.host)],env={**os.environ,'PATH':str(folder)+':'+os.environ['PATH']},stdout=self.log,stderr=self.log)
  for _ in range(100):
   try:self.rpc('drain_status',{});break
   except OSError:time.sleep(.05)
  page=self.rpc('log_range',{'ticket':accepted['ticket'],'offset':0,'limit':16})
  self.assertEqual(page['total_bytes'],256*1024*1024);self.assertTrue(page['closed']);self.assertTrue(page['gap'])
  self.assertEqual(page['data_base64'],'AAAAAAAAAAAAAAAAAAAAAA==');self.assertTrue(self.rpc('drain_status',{})['ready'])
  self.assertEqual(self.rpc('log_range',{'ticket':accepted['ticket'],'limit':16})['source_generation'],page['source_generation'])

 def test_expired_provider_generation_never_recaptures_container_bytes(self):
  accepted=self.rpc('start',self.request(['/bin/sh','-c','echo EXPIRE-SECRET']))
  self.assertEqual(self.wait(accepted['ticket'])['exit_code'],0)
  page=self.rpc('log_range',{'ticket':accepted['ticket'],'limit':64})
  self.assertTrue(page['closed'])
  receipt=self.rpc('log_expire',{'ticket':accepted['ticket'],'source_generation':page['source_generation']})
  self.assertTrue(receipt['expired'])
  error=self.rpc('log_range',{'ticket':accepted['ticket'],'source_generation':page['source_generation'],'limit':64},status=409)
  self.assertEqual(error['code'],'retention_gap')
  raw=self.host/'logs'/(hashlib.sha256(accepted['ticket'].encode()).hexdigest()+'.'+page['source_generation']+'.log')
  self.assertFalse(raw.exists())
  self.assertEqual(self.rpc('status',{'ticket':accepted['ticket']})['state'],'exited')
