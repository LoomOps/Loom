"""Real temporary state, mocked remote policy and host process boundaries."""
import copy,json,sqlite3,tempfile,unittest,sys
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import uninstall
from admission import group_name,RemoteError

class API:
 def __init__(self,c):
  self.c=c;self.calls=[];self.fail=False;self.conflict=False
  g=group_name(c['machine_id'],c['device_id'])
  self.policy={'groups':{g:['member@example.test'],'group:other':['other@example.test']},'grants':[
    {'src':[g],'dst':c['addresses'],'ip':['tcp:8443']},
    {'src':['admin@example.test'],'dst':c['addresses'],'ip':['*']},
    {'src':['autogroup:member','autogroup:shared'],'dst':c['addresses'],'ip':['tcp:10000']},
    {'src':['group:other'],'dst':['100.64.0.99'],'ip':['*']}],'tagOwners':{'tag:loom':['autogroup:admin']}}
  self.invites=[{'id':'owned'},{'id':'manual'}]
 def call(self,method,path,value=None,headers=None):
  self.calls.append((method,path))
  if path.endswith('/validate'):return {},{}
  if path.endswith('/acl'):
   if method=='POST':
    if self.conflict:self.conflict=False;raise RemoteError(412)
    self.policy=copy.deepcopy(value)
   return copy.deepcopy(self.policy),{'ETag':'version'}
  if method=='DELETE':
   if self.fail:raise RemoteError(503)
   self.invites=[x for x in self.invites if x['id']!=path.rsplit('/',1)[1]];return {},{}
  if path.endswith('/device-invites'):return copy.deepcopy(self.invites),{}
  if path=='/device/device1':return {'addresses':self.c['addresses']},{}
  raise AssertionError((method,path))

class UninstallTests(unittest.TestCase):
 def setUp(self):
  t=tempfile.TemporaryDirectory();self.addCleanup(t.cleanup);self.base=Path(t.name);self.root=self.base/'ops'
  for x in ['config','runtime','admission','dashboard-auth','secrets/admission','secrets/sso','network-backups']:(self.root/x).mkdir(parents=True)
  self.c={'machine_id':'machine1','device_id':'device1','tailnet':'example.test','addresses':['100.64.0.1'],'ports':[8443],
   'origin':'https://machine.test:10000','dashboard_origin':'https://machine.test:8443','host_port':18904,
   'admins':[{'issuer':'https://identity.test','subject':'admin'}],'enrollment_mode':'sharing'}
  (self.root/'secrets/admission/config.json').write_text(json.dumps(self.c));(self.root/'secrets/admission/management.json').write_text('fixture secret')
  (self.root/'config/api.json').write_text(json.dumps({'tailscale_auth':{'enabled':True},'dashboard_auth':{'enabled':True},'storage_id':'keep'}))
  (self.root/'compose.env').write_text('OPS_PORT=18902\nOPS_DASHBOARD_PORT=18903\n')
  (self.root/'network.env').write_text('LOOM_DEV_NO_AUTH=0\nLOOM_TAILSCALE_TRUSTED_PROXIES=10.0.0.1\n')
  with sqlite3.connect(self.root/'admission/requests.sqlite3') as db:
   db.execute('create table share_requests(id text,machine text,invite_id text,send_started integer)')
   db.execute("insert into share_requests values('request','machine1','owned',1)")
  with sqlite3.connect(self.root/'runtime/ops.sqlite3') as db:
   db.executescript("create table jobs(state text);create table node_control(id integer,mode text);insert into node_control values(1,'active');")
  self.data=self.base/'data';self.data.mkdir();(self.data/'model').write_text('preserve')
  self.state={'Web':{'machine.test:8443':{'Handlers':{k:{'Proxy':v} for k,v in uninstall.expected_routes(self.root).items()}},'machine.test:10000':{'Handlers':{'/':{'Proxy':'http://127.0.0.1:18904'}}},'other.test:443':{'Handlers':{'/':{'Proxy':'http://127.0.0.1:9999'}}}}}
  self.api=API(self.c);self.commands=[];self.receipt=self.base/'uninstall-receipt.json'
 def host_run(self,*args):
  self.commands.append(args)
  if args==('tailscale','serve','status','--json'):return json.dumps(self.state)
  if args[:2] in [('tailscale','serve'),('tailscale','funnel')] and args[-1]=='off':
   port=next(a.split('=')[1] for a in args if a.startswith('--https='));self.state['Web'].pop('machine.test:'+port,None);return ''
  if args[:2]==('docker','ps'):return ''
  raise AssertionError(args)
 def plan(self):return uninstall.inspect(self.root,'fixture-uninstall',self.api,self.host_run)
 def apply(self,p):
  with patch.object(uninstall.update,'compose') as compose:
   r=uninstall.apply(p,'machine1',self.receipt,self.api,self.host_run)
   self.assertNotIn('--volumes',str(compose.call_args_list));return r
 def test_plan_is_read_only_and_contains_no_secrets(self):
  p=self.plan();self.assertEqual(p['invites'],['owned']);self.assertEqual(p['unmanaged_shares_preserved'],1)
  self.assertTrue(all(m=='GET' for m,_ in self.api.calls));self.assertNotIn('fixture secret',json.dumps(p));self.assertTrue((self.root/'admission').exists())
 def test_uninstall_cleans_only_owned_access_and_preserves_data_and_admin(self):
  p=self.plan();self.api.conflict=True;r=self.apply(p);self.assertEqual(r['state'],'complete')
  self.assertEqual(self.api.invites,[{'id':'manual'}]);self.assertEqual(len(self.api.policy['grants']),2)
  self.assertIn('group:other',self.api.policy['groups']);self.assertEqual(self.data.joinpath('model').read_text(),'preserve')
  self.assertTrue((self.root/'runtime/ops.sqlite3').exists());self.assertFalse((self.root/'admission').exists())
  self.assertFalse((self.root/'secrets/admission').exists());self.assertIn('other.test:443',self.state['Web'])
  self.assertNotIn('tailscale_auth',json.loads((self.root/'config/api.json').read_text()))
  count=len(self.api.calls);self.apply(p);self.assertEqual(count,len(self.api.calls))
 def test_api_failure_keeps_state_and_resume_finishes_without_new_invitations(self):
  p=self.plan();self.api.fail=True
  with self.assertRaises(RemoteError):self.apply(p)
  self.assertTrue((self.root/'admission/requests.sqlite3').exists());self.assertTrue((self.root/'secrets/admission/management.json').exists())
  self.assertEqual(json.loads(self.receipt.read_text())['state'],'incomplete')
  self.api.fail=False;self.assertEqual(self.apply(p)['state'],'complete')
  self.assertFalse(any(m=='POST' and p.endswith('device-invites') for m,p in self.api.calls))
 def test_wrong_identity_and_new_request_require_new_plan(self):
  p=self.plan()
  with self.assertRaises(ValueError):uninstall.apply(p,'wrong',self.receipt,self.api,self.host_run)
  with sqlite3.connect(self.root/'admission/requests.sqlite3') as db:db.execute("insert into share_requests values('new','machine1',NULL,0)")
  with self.assertRaisesRegex(ValueError,'New access'):self.apply(p)
  self.assertFalse(self.receipt.exists())
 def test_active_work_prevents_stop(self):
  p=self.plan()
  with sqlite3.connect(self.root/'runtime/ops.sqlite3') as db:db.execute("insert into jobs values('running')")
  with patch.object(uninstall.update,'compose') as compose,self.assertRaises(ValueError):uninstall.apply(p,'machine1',self.receipt,self.api,self.host_run)
  compose.assert_not_called()
 def test_uncertain_invitation_and_mixed_listener_are_not_forgotten(self):
  self.state['Web']['machine.test:8443']['Handlers']['/other']={'Proxy':'http://other:8888'}
  with self.assertRaisesRegex(ValueError,'unrelated'):self.plan()
  del self.state['Web']['machine.test:8443']['Handlers']['/other']
  with sqlite3.connect(self.root/'admission/requests.sqlite3') as db:db.execute('update share_requests set invite_id=NULL')
  with self.assertRaisesRegex(ValueError,'Unconfirmed'):self.plan()
 def test_extended_group_and_multiple_instance_ports_are_rejected(self):
  self.api.policy['grants'][0]['dst'].append('100.64.0.2')
  with self.assertRaisesRegex(ValueError,'extended'):self.plan()
  self.api.policy['grants'][0]['dst'].pop()
  c={**self.c,'ports':[443,8443]};(self.root/'secrets/admission/config.json').write_text(json.dumps(c))
  self.state['Web']['machine.test:443']={'Handlers':{'/':{'Proxy':'http://another-instance:8080'}}}
  with self.assertRaisesRegex(ValueError,'unrelated'):self.plan()
 def test_symlink_state_never_deletes_target(self):
  p=self.plan();(self.root/'dashboard-auth').rmdir();(self.root/'dashboard-auth').symlink_to(self.data)
  with self.assertRaisesRegex(ValueError,'symlink'):self.apply(p)
  self.assertTrue((self.data/'model').exists());self.assertTrue((self.root/'secrets/admission').exists())

 def test_wrong_compose_project_cannot_purge_a_running_instance(self):
  def wrong(*args):
   if args[:3]==('docker','ps','-aq') and 'label=com.docker.compose.service=loom-ops' in args:return 'container'
   if args[:2]==('docker','inspect'):return json.dumps([{'Mounts':[{'Source':str(self.root/'runtime'),'Destination':'/loom/ops'}],'Config':{'Labels':{'com.docker.compose.project':'actual','com.docker.compose.service':'loom-ops'}}}])
   return self.host_run(*args)
  with self.assertRaisesRegex(ValueError,'different Compose'):uninstall.inspect(self.root,'wrong',self.api,wrong)
 def test_executor_with_unreturned_ticket_blocks_uninstall(self):
  host=self.base/'executor';host.mkdir();(self.root/'executor.env').write_text('LOOM_EXECUTOR_HOST='+str(host)+'\n')
  (self.root/'secrets/executor-token').write_text('local-fixture')
  import hashlib
  (host/'config.json').write_text(json.dumps({'instances':{'test':{'token_sha256':hashlib.sha256(b'local-fixture').hexdigest()}}}))
  with sqlite3.connect(host/'executor.sqlite3') as c:
   c.executescript("CREATE TABLE tickets(instance TEXT,state TEXT);INSERT INTO tickets VALUES('test','running');")
  p=self.plan()
  with self.assertRaisesRegex(ValueError,'not returned'):self.apply(p)
  self.assertFalse(self.receipt.exists())

 def test_resume_rechecks_active_work_even_with_existing_receipt(self):
  p=self.plan();self.api.fail=True
  with self.assertRaises(RemoteError):self.apply(p)
  self.api.fail=False
  with sqlite3.connect(self.root/'runtime/ops.sqlite3') as db:db.execute("insert into jobs values('running')")
  before=len(self.api.calls)
  with self.assertRaisesRegex(ValueError,'Active Jobs'):self.apply(p)
  self.assertTrue(all(m=='GET' for m,_ in self.api.calls[before:]))
  self.assertTrue((self.root/'secrets/admission/management.json').exists())
