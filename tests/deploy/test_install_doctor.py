"""Installation regressions with real configuration/files and mocked machine boundaries."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
sys.path[:0]=[str(ROOT/'deploy/installer'),str(ROOT/'deploy'),str(ROOT/'dashboard')]
import doctor
import smoke
import source_install
spec=importlib.util.spec_from_file_location('full_install_setup',ROOT/'deploy/installer/setup.py')
setup=importlib.util.module_from_spec(spec);sys.modules[spec.name]=setup;spec.loader.exec_module(setup)


class InstallationDoctorTests(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
  self.plan=json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text())
  self.plan['data']=str(self.root/'data');self.plan['dependencies']=str(self.root/'deps')
  self.access=json.loads((ROOT/'deploy/installer/access-plan.example.json').read_text())
  self.access.update(machine_id='loom-test',device_id='device-test')
  self.access_file=self.root/'access.json';self.plan['tailscale']['access_plan']=str(self.access_file)
  self.save_access()
  self.patch=patch.object(setup,'ROOT',self.root/'instance');self.patch.start();self.addCleanup(self.patch.stop)
 def save_access(self):self.access_file.write_text(json.dumps(self.access))
 def write(self,name,value):
  p=setup.ROOT/name;p.parent.mkdir(parents=True,exist_ok=True)
  p.write_text(value if isinstance(value,str) else json.dumps(value));return p
 def configured(self):
  s=self.plan['tailscale']['sso'];issuer=self.access['admins'][0]['issuer']
  self.write('config/api.json',{'machine_id':self.access['machine_id'],'machine_name':self.access['name'],
   'dashboard_auth':{'enabled':True,'issuer':issuer},'tailscale_auth':{'enabled':True,'trusted_proxies':['10.203.41.1','10.203.41.3']},'executor':{'host_data':self.plan['data'],'storage':{'data/0':self.plan['data']}},'data_stores':{'data/0':{'path':'/loom/data/0','mode':'rw','storage_id':'primary'}}})
  self.write('secrets/sso/config.json',{**s,'mode':'network','issuer':issuer,'origin':self.access['dashboard_origin']})
  self.write('secrets/admission/config.json',{**self.access,'separate_gateway':True})
  self.write('secrets/admission/gateway.json',{'credential_file':'/not-mounted/management.json'})
  self.write('build.env','LOOM_DEV_NO_AUTH=0\n')
  self.write('secrets/executor-token','fixture-executor')
  executor=setup.ROOT.parent/'shared/executor';executor.mkdir(parents=True,exist_ok=True)
  (executor/'config.json').write_text(json.dumps({'instances':{'alpha':{'token_sha256':hashlib.sha256(b'fixture-executor').hexdigest(),'declared_runtime':self.plan.get('runtime_options',{})}}}))
  Path(self.plan['data']).mkdir();(Path(self.plan['data'])/'.loom-storage.json').write_text(json.dumps({'storage_id':'primary'}));self.write('config/storage-plan.json',{'primary':self.plan['data'],'read_write':[self.plan['data']]})
 def test_full_plan_accepts_and_reduced_modes_fail_before_mutation(self):
  setup.validate(self.plan);doctor.service_plan(self.plan)
  for value in [True,False,None]:
   p={**self.plan,'tailscale':value}
   with self.subTest(value=value),self.assertRaises(ValueError):setup.validate(p)
  for key,value in [('sso',None),('access_plan',None)]:
   p=copy.deepcopy(self.plan);p['tailscale'][key]=value
   with self.subTest(key=key),self.assertRaises(ValueError):setup.validate(p)
 def test_unbound_admin_and_public_private_port_collision_rejected(self):
  self.access['admins'][0]['subject']='tailscale:unbound@example.org';self.save_access()
  with self.assertRaisesRegex(ValueError,'Admin'):doctor.service_plan(self.plan)
  self.access['admins'][0]['subject']='tailscale:admin@example.com'
  self.access['origin']=self.access['dashboard_origin'];self.save_access()
  with self.assertRaises(ValueError):doctor.service_plan(self.plan)
 def test_doctor_does_not_accept_progress_receipt_or_token_only_service(self):
  self.write('install-progress.json',{'completed':['verified']})
  with patch.object(setup,'run') as run,patch.object(setup,'read_api') as api:
   result=doctor.doctor(setup,self.plan,'configuration')
  self.assertFalse(result['ready']);self.assertTrue(all(c['status']=='fail' for c in result['checks'] if c['check']!='managed_resource_journal'))
  run.assert_not_called();api.assert_not_called()
 def test_config_checks_actual_identity_and_storage_and_detects_regression(self):
  self.configured();self.assertTrue(doctor.doctor(setup,self.plan,'configuration')['ready'])
  self.write('build.env','LOOM_DEV_NO_AUTH=1\n')
  self.assertFalse(doctor.doctor(setup,self.plan,'configuration')['ready'])
  self.write('build.env','LOOM_DEV_NO_AUTH=0\n');self.access['admins'][0]['subject']='changed';self.save_access()
  self.assertFalse(doctor.doctor(setup,self.plan,'configuration')['ready'])
 def test_storage_list_is_explicit_read_write_and_nonoverlapping(self):
  other=self.root/'archive';other.mkdir();self.plan['additional_data']=[str(other)]
  setup.validate(self.plan);setup.ROOT.mkdir()
  self.write('config/api.json',{})
  with patch('tailnet.os.chown'),patch.object(setup,'run') as commands:setup.configure_identity_storage(self.plan,self.access)
  mounts=json.loads((setup.ROOT/'compose-storage.yaml').read_text())['services']['loom-ops']['volumes']
  self.assertEqual([m['target'] for m in mounts],['/loom/data/0','/loom/data/1'])
  self.assertTrue(all(m['read_only'] is False for m in mounts))
  self.assertTrue(any(c.args[:4]==('setfacl','-R','-P','-m') for c in commands.call_args_list))
  self.plan['additional_data']=[str(other),str(other)]
  with self.assertRaisesRegex(ValueError,'overlap'):setup.validate(self.plan)

 def test_doctor_compares_explicit_administrator_runtime_policy(self):
  self.configured();self.plan['runtime_options']={'network':'bridge','driver_capabilities':['compute','utility','graphics']}
  result=doctor.doctor(setup,self.plan,'configuration')
  self.assertFalse(next(c for c in result['checks'] if c['check']=='administrator_runtime_options')['status']=='pass')
  path=setup.ROOT.parent/'shared/executor/config.json';value=json.loads(path.read_text())
  value['instances']['alpha']['declared_runtime']=self.plan['runtime_options'];path.write_text(json.dumps(value))
  self.assertTrue(doctor.doctor(setup,self.plan,'configuration')['ready'])
 def test_doctor_reports_multiple_missing_services_without_secret_text(self):
  secret='tskey-api-DO-NOT-PRINT'
  with patch.object(setup,'run',side_effect=RuntimeError(secret)),patch.object(setup,'verify_core',side_effect=RuntimeError(secret)),patch.object(doctor,'request',side_effect=RuntimeError(secret)):
   result=doctor.doctor(setup,self.plan,'services')
  self.assertFalse(result['ready']);self.assertGreaterEqual(len(result['checks']),6)
  self.assertNotIn(secret,json.dumps(result));self.assertEqual(result['re_read'],'INSTALL.md')
 def test_network_refuses_funnel_exposure_of_private_dashboard(self):
  from tailnet import expected_routes
  self.write('compose.env','OPS_PORT=18892\nOPS_DASHBOARD_PORT=18893\n');self.write('network.env','')
  host='loom-server.your-tailnet.ts.net';gateway=host+':443';private=host+':8443'
  state={'Web':{gateway:{'Handlers':{'/':{'Proxy':'http://127.0.0.1:18904'}}},private:{'Handlers':{k:{'Proxy':v} for k,v in expected_routes(setup.ROOT).items()}}},'AllowFunnel':{gateway:True,private:True}}
  with patch.object(setup,'run',return_value=json.dumps(state)),patch.object(doctor,'request',return_value=(404,None)),patch('access_setup.TailnetAPI',side_effect=RuntimeError('unavailable')):
   result=doctor.doctor(setup,self.plan,'network')
  check=next(c for c in result['checks'] if c['check']=='serve_funnel_isolation')
  self.assertEqual(check['status'],'fail')
 def test_handoff_never_recommends_token_as_substitute(self):
  with patch.object(doctor,'doctor',return_value={'ready':False,'checks':[]}):
   result=doctor.handoff(setup,self.plan)
  self.assertEqual(result['status'],'incomplete');self.assertNotIn('gateway',result)
 def test_packaged_smoke_source_has_prepare_and_complete_result_contract(self):
  out=self.root/'output';out.mkdir();req=self.root/'request.json';source=self.root/'run.py';source.write_text(smoke.SOURCE)
  req.write_text(json.dumps({'output_dir':str(out),'profile':{'revision':'fixture','sha256':'a'*64}}))
  for stage in ('prepare','run'):subprocess.run([sys.executable,str(source),stage,str(req)],check=True)
  result=json.loads((out/'result.json').read_text())
  self.assertEqual(result['metrics'],[]);self.assertEqual(result['entities'][0]['relations'],[])
  self.assertEqual(result['entities'][0]['provenance']['profile'],'fixture')
  self.assertEqual((out/'installation.txt').read_text(),'Loom installation smoke\n')
 def test_missing_runtime_image_is_pulled_before_build(self):
  with patch.object(source_install.subprocess,'run',side_effect=[subprocess.CompletedProcess([],1),subprocess.CompletedProcess([],0)]) as run,patch.object(source_install.subprocess,'check_output',return_value='sha256:runtime\n'):
   self.assertEqual(source_install.ensure_runtime(),'sha256:runtime')
  self.assertEqual(run.call_args_list[1].args[0],['docker','pull','python:3.12-slim'])
 def test_plan_generator_keeps_explicit_admin_and_does_not_copy_host_accounts(self):
  state={'state':'Running','dns':'fixture.tail.test','tailnet':'example.test','device_id':'d123'}
  raw={'Self':{'HostName':'Fixture RTX','TailscaleIPs':['100.64.0.2']}}
  output=self.root/'generated.json'
  with patch('tailnet.status',return_value=state),patch.object(setup,'run',return_value=json.dumps(raw)):
   result=setup.make_plan(output,'human@example.com',self.plan['data'],[],False,self.root/'secret.json')
   with self.assertRaises(ValueError):setup.make_plan(output,'human@example.com',self.plan['data'],[],False,self.root/'secret.json')
  generated=json.loads(output.read_text());a=doctor.service_plan(generated)
  self.assertEqual(a['name'],'Fixture RTX');self.assertEqual(a['admins'][0]['subject'],'tailscale:human@example.com')
  self.assertEqual(result['gateway'],'https://fixture.tail.test');self.assertEqual(output.stat().st_mode&0o777,0o600)

 def test_source_kit_contains_all_steps_and_runtime_helpers(self):
  import hashlib
  snapshot=self.root/'release';source=snapshot/'source'
  import shutil
  shutil.copytree(ROOT/'deploy/installer',source/'deploy/installer',ignore=shutil.ignore_patterns('__pycache__'))
  result={'snapshot':str(snapshot),'images':{'ops':'ops:test','dashboard':'dashboard:test'},'image_ids':{'ops':'sha256:o','dashboard':'sha256:d'},'git_commit':'fixture'}
  dest=self.root/'stage';dest.mkdir()
  def output(args,**kwargs):
   return 'sha256:p\n' if '{{.Id}}' in args else '[{"Architecture":"amd64"}]'
  with patch.object(source_install,'ensure_runtime'),patch.object(source_install.release,'build',return_value=result),patch.object(source_install,'guide',side_effect=lambda image,dest:dest.write_bytes(b'guide')),patch.object(source_install.subprocess,'run'),patch.object(source_install.subprocess,'check_output',side_effect=output):
   prepared=source_install.prepare('0.1.0-alpha.99',dest)
  kit=Path(prepared['kit']);manifest=json.loads((kit/'manifest.json').read_text())
  for name in ['setup.py','doctor.py','smoke.py',*[str(p.relative_to(ROOT/'deploy/installer')) for p in (ROOT/'deploy/installer/steps').glob('*.md')]]:
   self.assertEqual(hashlib.sha256((kit/name).read_bytes()).hexdigest(),manifest['files'][name])

 def test_smoke_reuses_existing_job_and_reports_failed_state_without_recovery(self):
  import hashlib
  self.write('config/install-smoke.json',{'build':'test','source_sha256':hashlib.sha256(smoke.SOURCE.encode()).hexdigest(),'job':'existing','kind':'pinned'})
  with patch.object(setup,'manifest',return_value={'build_id':'test'}),patch.object(smoke,'call',return_value={'state':'result_failed'}) as call:
   result=smoke.run(setup,self.plan)
  self.assertEqual(result['status'],'incomplete');self.assertEqual(result['job'],'existing')
  self.assertEqual(call.call_args.args[2:4],('jobs','get'));self.assertEqual(call.call_count,1)

 def test_plan_diagnosis_does_not_read_credentials_or_contact_provider(self):
  with patch.object(setup,'run') as run,patch('access_setup.TailnetAPI') as api:
   result=doctor.doctor(setup,self.plan,'plan')
  self.assertTrue(result['ready']);run.assert_not_called();api.assert_not_called()

 def test_services_check_running_images_mounts_auth_and_ports(self):
  self.configured()
  folder=setup.ROOT.parent/'shared/observations/loom';folder.mkdir(parents=True)
  for name in ['docker-status.json','dashboard-gpus.json','dashboard-host-metrics.json']:(folder/name).write_text('{}')
  image_ids={'ops':{'id':'sha256:o'},'dashboard':{'id':'sha256:d'}}
  def container(service):
   mounts={
    'loom-ops':{'/etc/loom/api.json':setup.ROOT/'config/api.json','/etc/loom/sso/relay-key':setup.ROOT/'secrets/sso/relay-key','/loom/data/0':Path(self.plan['data']),'/run/loom-observations':setup.ROOT.parent/'shared/observations/loom'},
    'loom-dashboard':{'/etc/loom/sso':setup.ROOT/'secrets/sso','/etc/loom/admission':setup.ROOT/'secrets/admission'},
    'loom-gateway':{'/etc/loom/gateway.json':setup.ROOT/'secrets/admission/gateway.json','/var/lib/loom-admission':setup.ROOT/'admission'}}[service]
   ports={'loom-ops':{'8080/tcp':18892,'8443/tcp':18894},'loom-dashboard':{'8080/tcp':18893},'loom-gateway':{'8081/tcp':18904}}[service]
   return {'State':{'Running':True,'Health':{'Status':'healthy'}},'Image':'sha256:o' if service=='loom-ops' else 'sha256:d',
    'NetworkSettings':{'Networks':{'loom_default':{'Gateway':'10.203.41.1','IPAddress':'10.203.41.3' if service=='loom-dashboard' else '10.203.41.2'}}},
    'Config':{'Env':['LOOM_DEV_NO_AUTH=0','LOOM_TAILSCALE_TRUSTED_PROXIES=10.203.41.1']},'Mounts':[{'Destination':k,'Source':str(v),'RW':k.startswith('/loom/data/')} for k,v in mounts.items()],
    'HostConfig':{'PortBindings':{k:[{'HostIp':'127.0.0.1','HostPort':str(v)}] for k,v in ports.items()}}}
  self.plan['host_ssh']=False # This fixture exercises existing installations without optional SSH.
  containers={s:container(s) for s in doctor.SERVICES}
  def run(*args):
   if args[:2]==('docker','ps'):return args[-1].split('=')[-1]
   if args[:2]==('docker','inspect'):return json.dumps([containers[args[2]]])
   if args[:2]==('docker','exec'):return ''
   if args[:2]==('systemctl','is-enabled'):return 'enabled'
   if args[:2]==('systemctl','is-active'):return 'active'
   raise AssertionError(args)
  with patch.object(setup,'run',side_effect=run),patch.object(setup,'manifest',return_value={'build_id':'test','images':image_ids}),patch.object(setup,'verify_core',return_value={'build':'test'}),patch.object(doctor,'request',return_value=(200,{'sso':True})),patch('storage_parity.inspect',return_value={'status':'passed'}) as parity:
   self.assertTrue(doctor.doctor(setup,self.plan,'services')['ready'])
   parity.assert_called_with(setup.ROOT)
   with patch('storage_parity.inspect',side_effect=ValueError('fixture storage mismatch')):
    failed=doctor.doctor(setup,self.plan,'services')
    self.assertFalse(failed['ready'])
    self.assertEqual(next(c for c in failed['checks'] if c['check']=='effective_storage_parity')['status'],'fail')
   store=next(m for m in containers['loom-ops']['Mounts'] if m['Destination']=='/loom/data/0')
   store['RW']=False
   self.assertFalse(doctor.doctor(setup,self.plan,'services')['ready'])
   store['RW']=True
   with patch.object(setup,'run',side_effect=lambda *args: (_ for _ in ()).throw(PermissionError('No write access')) if args[:2]==('docker','exec') else run(*args)):
    self.assertFalse(doctor.doctor(setup,self.plan,'services')['ready'])
   containers['loom-dashboard']['NetworkSettings']['Networks']['loom_default']['Gateway']='10.203.41.128'
   failed=doctor.doctor(setup,self.plan,'services')
   self.assertFalse(failed['ready'])
   self.assertEqual(next(c for c in failed['checks'] if c['check']=='trusted_ingress_matches_docker')['status'],'fail')
   containers['loom-dashboard']['NetworkSettings']['Networks']['loom_default']['Gateway']='10.203.41.1'
   containers['loom-gateway']['Image']='sha256:old'
   self.assertFalse(doctor.doctor(setup,self.plan,'services')['ready'])
   containers['loom-gateway']['Image']='sha256:d'
   containers['loom-dashboard']['HostConfig']['PortBindings']['8080/tcp'][0]['HostIp']='0.0.0.0'
   self.assertFalse(doctor.doctor(setup,self.plan,'services')['ready'])

 def test_network_pass_requires_gateway_and_scoped_private_grant(self):
  from tailnet import expected_routes
  self.write('compose.env','OPS_PORT=18892\nOPS_DASHBOARD_PORT=18893\n');self.write('network.env','')
  host='loom-server.your-tailnet.ts.net';gateway=host+':443';private=host+':8443'
  state={'Web':{gateway:{'Handlers':{'/':{'Proxy':'http://127.0.0.1:18904'}}},private:{'Handlers':{k:{'Proxy':v} for k,v in expected_routes(setup.ROOT).items()}}},'AllowFunnel':{gateway:True}}
  grants=[{'src':['autogroup:member','autogroup:shared'],'dst':self.access['addresses'],'ip':['tcp:443']},
          {'src':['admin@example.com'],'dst':self.access['addresses'],'ip':['tcp:8443']}]
  def api(method,path):
   if path.startswith('/device/'):return {'addresses':self.access['addresses'],'tags':['tag:loom']},{}
   return {'grants':grants},{}
  def http(url):return (200,{'machine':'loom-test'}) if url.endswith('/api/session') else (404,None)
  with patch.object(setup,'run',return_value=json.dumps(state)),patch.object(doctor,'request',side_effect=http),patch('access_setup.TailnetAPI') as factory:
   factory.return_value.call.side_effect=api
   self.assertTrue(doctor.doctor(setup,self.plan,'network')['ready'])
   grants.pop()
   self.assertFalse(doctor.doctor(setup,self.plan,'network')['ready'])

 def test_host_identity_rejects_plan_from_another_machine_before_policy_changes(self):
  raw={'BackendState':'Running','CurrentTailnet':{'Name':self.plan['tailscale']['tailnet']},'Self':{
   'ID':self.access['device_id'],'DNSName':'loom-server.your-tailnet.ts.net.','TailscaleIPs':self.access['addresses']}}
  self.assertTrue(doctor.machine_matches(self.plan,raw))
  for field,value in [('ID','another-device'),('DNSName','another.tail.ts.net.'),('TailscaleIPs',['100.64.0.99'])]:
   other=copy.deepcopy(raw);other['Self'][field]=value
   self.assertFalse(doctor.machine_matches(self.plan,other))

 def test_handoff_offers_user_scoped_skill_and_private_mcp_registration(self):
  with patch.object(doctor,'doctor',return_value={'ready':True,'checks':[],'live_validation':[]}),patch.object(setup,'manifest',return_value={'build_id':'fixture'}),patch.object(setup,'read_api',return_value={'updates':{}}):
   result=doctor.handoff(setup,self.plan)
  client=result['client_setup']
  self.assertIsNone(client['authentication']['expires'])
  self.assertEqual(client['authentication']['without_token'],'server complete / client authentication pending')
  self.assertEqual(client['repository'],'https://github.com/LoomOps/Loom-Use')
  self.assertEqual(client['connect_argv'],['python3','loom_use.py','connect',self.access['dashboard_origin']+'/mcp'])
  self.assertIn('Current agent',client['target'])
  self.assertTrue(client['release']['sha256'])
  self.assertEqual(result['status'],'services_ready')
  self.assertEqual(client['status'],'offer_to_user')
  self.assertEqual(client['skill'],'loom-use')
  self.assertEqual(client['mcp_endpoint'],self.access['dashboard_origin']+'/mcp')
  self.assertNotEqual(client['mcp_endpoint'],self.access['origin']+'/mcp')
  self.assertEqual(client['bundle_url'],self.access['dashboard_origin']+'/api/guide/bundle')
  self.assertEqual(client['transport'],'streamable_http')
  self.assertIn('system.status',client['verification'])
  self.assertNotIn('token',client)

 def test_new_plan_requires_ownership_journal_even_with_healthy_config(self):
  self.configured();self.access['managed_resources_version']=1;self.save_access()
  result=doctor.doctor(setup,self.plan,'configuration')
  entry=next(c for c in result['checks'] if c['check']=='managed_resource_journal')
  self.assertEqual(entry['status'],'fail');self.assertFalse(result['ready'])

class InstallationOrderingTests(unittest.TestCase):
 def test_ssh_requires_final_storage_and_apply_orders_executor_first(self):
  import ast
  tree=ast.parse((ROOT/'deploy/installer/setup.py').read_text())
  fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='apply')
  markers=[(n.lineno,n.args[0].value) for n in ast.walk(fn) if isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id=='step' and n.args and isinstance(n.args[0],ast.Constant)]
  ssh=next(n.lineno for n in ast.walk(fn) if isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id=='configure_host_ssh')
  self.assertLess(next(line for line,label in markers if label=='executor'),ssh)
  self.assertLess(ssh,next(line for line,label in markers if label=='compose'))
  with tempfile.TemporaryDirectory() as t:
   root=Path(t);(root/'config').mkdir();p=root/'config/api.json'
   plan={'host_ssh':True,'data':'/primary','additional_data':['/secondary']}
   p.write_text(json.dumps({'executor':{'storage':{'data/0':'/primary'}}}))
   with patch.object(setup,'ROOT',root),patch('ssh_setup.install') as install,patch.object(setup.shutil,'which',return_value='/bin/setfacl'):
    with self.assertRaisesRegex(ValueError,'Finalize'):setup.configure_host_ssh(plan)
    install.assert_not_called()
    p.write_text(json.dumps({'executor':{'storage':{'data/0':'/primary','data/1':'/secondary'}}}))
    setup.configure_host_ssh(plan);install.assert_called_once_with(root)
 def test_doctor_starting_and_unrun_smoke_are_not_failures_or_passes(self):
  with self.assertRaises(doctor.PendingCheck) as pending:doctor.health_ready({'Running':True,'Health':{'Status':'starting'}})
  self.assertEqual(pending.exception.status,'waiting')
  self.assertFalse(doctor.health_ready({'Running':True,'Health':{'Status':'unhealthy'}}))
  self.assertTrue(doctor.health_ready({'Running':True,'Health':{'Status':'healthy'}}))
  with tempfile.TemporaryDirectory() as t,patch.object(setup,'ROOT',Path(t)):
   result=doctor.doctor(setup,{},'acceptance')
   self.assertEqual(result['stages']['acceptance'],'not_run');self.assertFalse(result['ready'])
   self.assertEqual(result['next_document'],'steps/07-handoff.md')
 def test_server_doctor_requires_sudo_before_opening_protected_plan(self):
  if os.geteuid()==0:self.skipTest('Requires a nonroot caller')
  r=subprocess.run([sys.executable,str(ROOT/'deploy/installer/setup.py'),'doctor','/missing-protected-plan'],capture_output=True,text=True)
  self.assertNotEqual(r.returncode,0);self.assertIn('sudo',r.stderr);self.assertNotIn('FileNotFoundError',r.stderr)
