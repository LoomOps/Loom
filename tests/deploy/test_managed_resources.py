import copy,json,tempfile,unittest,sys
from pathlib import Path
sys.path[:0]=[str(Path(__file__).resolve().parents[2]/'dashboard'),str(Path(__file__).resolve().parents[2]/'deploy')]
import managed_resources as registry
from admission import RemoteError

class RegistryTests(unittest.TestCase):
 def setUp(self):
  t=tempfile.TemporaryDirectory();self.addCleanup(t.cleanup)
  self.c={'tailnet':'example.test','machine_id':'fixture','device_id':'device','database':str(Path(t.name)/'requests.sqlite3')}
 def write(self,before,after,send=lambda:None):return registry.mutation(self.c,registry.flatten(before),registry.flatten(after),send)
 def test_installer_and_runtime_updates_restore_only_owned_delta(self):
  base={'groups':{'group:existing':['original@example.test']},'grants':[{'src':['admin@example.test'],'dst':['100.64.0.8'],'ip':['*']}]}
  created=copy.deepcopy(base);created['groups']['group:fixture']=[];created['grants'].append({'src':['group:fixture'],'dst':['100.64.0.8'],'ip':['tcp:8443']})
  self.write(base,created)
  approved=copy.deepcopy(created);approved['groups']['group:fixture']=['member@example.test'];self.write(created,approved)
  self.assertEqual(registry.revert(self.c,approved),base)
  self.assertEqual(registry.path(self.c).stat().st_mode&0o777,0o600)
 def test_preexisting_group_is_restored_not_deleted(self):
  base={'groups':{'group:fixture':['original@example.test']}}
  changed=copy.deepcopy(base);changed['groups']['group:fixture'].append('new@example.test');self.write(base,changed)
  self.assertEqual(registry.revert(self.c,changed),base)
 def test_external_edit_and_reference_are_not_deleted(self):
  base={'groups':{}};new={'groups':{'group:fixture':[]}};self.write(base,new)
  edited=copy.deepcopy(new);edited['groups']['group:fixture']=['manual@example.test']
  with self.assertRaisesRegex(ValueError,'changed'):registry.revert(self.c,edited)
  edited=copy.deepcopy(new);edited['ssh']=[{'src':['group:fixture'],'dst':['*']}]
  with self.assertRaisesRegex(ValueError,'referenced'):registry.revert(self.c,edited)
 def test_pending_write_survives_lost_response_and_reject_is_not_owned(self):
  def lost():raise TimeoutError()
  with self.assertRaises(TimeoutError):self.write({}, {'groups':{'g':[]}},lost)
  with self.assertRaisesRegex(ValueError,'Unconfirmed'):registry.revert(self.c,{'groups':{'g':[]}})
  self.assertEqual(registry.summary(self.c)['pending'],1)
 def test_etag_rejection_allows_safe_retry(self):
  def reject():raise RemoteError(412)
  with self.assertRaises(RemoteError):self.write({}, {'groups':{'g':[]}},reject)
  self.assertEqual(registry.summary(self.c)['pending'],0)
  self.write({}, {'groups':{'g':[]}});self.assertEqual(registry.revert(self.c,{'groups':{'g':[]}}),{'groups':{}})
 def test_tag_shared_by_another_machine_or_manual_ssh_is_preserved(self):
  old={'tagOwners':{}};new={'tagOwners':{'tag:loom':['autogroup:admin']}};self.write(old,new)
  self.assertEqual(registry.revert(self.c,new,{'tag:loom'}),new)
  with_ssh={**new,'ssh':[{'src':['tag:loom'],'dst':['*']}]}
  self.assertEqual(registry.revert(self.c,with_ssh),with_ssh)
 def test_identity_reuse_fails(self):
  self.write({}, {'groups':{'g':[]}})
  with self.assertRaisesRegex(ValueError,'identity'):registry.read({**self.c,'machine_id':'another'})
 def test_uninstall_is_idempotent_and_preserves_new_unrelated_entries(self):
  self.write({}, {'groups':{'g':[]}})
  current={'groups':{'g':[],'other':['a']}}
  first=registry.revert(self.c,current);self.assertEqual(registry.revert(self.c,first),first)
 def test_explicit_reconcile_observes_without_replaying(self):
  new={'groups':{'g':[]}}
  def lost():raise TimeoutError()
  with self.assertRaises(TimeoutError):self.write({},new,lost)
  self.assertEqual(registry.reconcile(self.c,new,[])['pending'],0)
  self.assertEqual(registry.revert(self.c,new),{'groups':{}})
 def test_reused_invitation_is_not_claimed_as_created(self):
  registry.invitation(self.c,'existing','request',False,'invited')
  self.assertFalse(registry.read(self.c)['resources']['invitation/existing']['after']['value']['created'])
  self.assertEqual(registry.revert(self.c,{}),{})

class OrphanPolicyTests(unittest.TestCase):
 def setUp(self):
  import policy_cleanup
  self.cleanup=policy_cleanup
  self.group='group:loom-access-fixture'
  self.p={'groups':{self.group:[],'group:existing-users':['admin@example.test']},'grants':[
   {'src':[self.group],'dst':['100.64.0.2'],'ip':['tcp:8443']},
   {'src':['group:existing-users'],'dst':['tag:machine'],'ip':['*']},
   {'src':['admin@example.test'],'dst':['100.64.0.3'],'app':{'tailscale.com/cap/tsidp':[{'allow_admin_ui':True}]}}],
   'ssh':[{'src':['group:existing-users'],'dst':['tag:machine'],'users':['autogroup:nonroot'],'action':'accept'}]}
 def test_observed_orphan_shape_removes_only_reviewed_service_rules(self):
  result=self.cleanup.candidate(self.p,['100.64.0.2'],self.group,['100.64.0.3'])
  self.assertEqual(result['ssh'],self.p['ssh']);self.assertEqual(result['grants'],[self.p['grants'][1]])
  self.assertEqual(result['groups'],{'group:existing-users':['admin@example.test']})
 def test_nonempty_extended_or_referenced_group_is_not_an_orphan(self):
  for change in ('member','destination','ssh'):
   p=copy.deepcopy(self.p)
   if change=='member':p['groups'][self.group]=['user@example.test']
   elif change=='destination':p['grants'][0]['dst'].append('100.64.0.9')
   else:p['ssh'][0]['src'].append(self.group)
   with self.subTest(change=change),self.assertRaises(ValueError):self.cleanup.candidate(p,['100.64.0.2'],self.group)
 def test_apply_rejects_stale_or_broadened_plan_without_write(self):
  from unittest.mock import Mock
  after=self.cleanup.candidate(self.p,['100.64.0.2'],self.group)
  p={'schema':'loom-orphan-policy/v1','tailnet':'example.test','device':'fixture','addresses':['100.64.0.2'],'group':self.group,'before':self.p,'after':after}
  api=Mock();api.call.side_effect=[({'addresses':['100.64.0.2']},{}),({'groups':{}},{'ETag':'new'})]
  with self.assertRaisesRegex(ValueError,'changed'):self.cleanup.apply(api,p,'fixture')
  self.assertTrue(all(c.args[0]=='GET' for c in api.call.call_args_list))
  p['after']={};api.reset_mock()
  with self.assertRaisesRegex(ValueError,'selection'):self.cleanup.apply(api,p,'fixture')
  api.call.assert_not_called()
