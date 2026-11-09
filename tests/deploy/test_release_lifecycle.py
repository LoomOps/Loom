"""Real files/SQLite, mocked Docker/systemd: release boundaries and recovery."""
import hashlib,json,os,sqlite3,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import update,tailnet,release,guide_bundle
import io,zipfile

class EntryReadinessTests(unittest.TestCase):
 def test_entry_checks_running_identity_and_exact_public_private_routes(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t)
   def save(name,value):
    p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(value))
   access={'origin':'https://loom.tail123.ts.net','dashboard_origin':'https://loom.tail123.ts.net:8443','host_port':8003,'machine_id':'machine'}
   save('config/tailnet-plan.json',{'https_port':8443})
   save('config/access-plan.json',access)
   save('secrets/sso/config.json',{'origin':access['dashboard_origin'],'network_users':{'admin@example.test':{'subject':'admin'}}})
   save('secrets/admission/config.json',{**access,'separate_gateway':True})
   (root/'compose.env').write_text('OPS_PORT=8001\nOPS_DASHBOARD_PORT=8002\n');(root/'network.env').write_text('')
   state={'Web':{'loom.tail123.ts.net:443':{'Handlers':{'/':{'Proxy':'http://127.0.0.1:8003'}}},'loom.tail123.ts.net:8443':{'Handlers':{k:{'Proxy':v} for k,v in tailnet.expected_routes(root).items()}}},'AllowFunnel':{'loom.tail123.ts.net:443':True}}
   def response(req,**kwargs):
    return io.BytesIO(json.dumps({'machine':'machine'} if req.endswith('/api/session') else {'sso':True,'entry_url':access['origin']}).encode())
   with patch('urllib.request.urlopen',side_effect=response),patch.object(update,'run',side_effect=lambda *args:json.dumps(state)):
    update.check_entry(root)
    handlers=state['Web']['loom.tail123.ts.net:8443']['Handlers']
    route=handlers.pop('/api/admin')
    with self.assertRaisesRegex(OSError,'relay routes'):update.check_entry(root)
    handlers['/api/admin']=route
    state['AllowFunnel']['loom.tail123.ts.net:8443']=True
    with self.assertRaisesRegex(OSError,'Funnel boundary'):update.check_entry(root)
    state['AllowFunnel'].pop('loom.tail123.ts.net:8443')
    save('secrets/sso/config.json',{'origin':'https://loom.tail123.ts.net:18443','network_users':{'admin@example.test':{}}})
    with self.assertRaisesRegex(OSError,'native identity'):update.check_entry(root)
 def test_entry_rejects_missing_configuration_and_token_fallback(self):
  with tempfile.TemporaryDirectory() as t:
   with self.assertRaisesRegex(OSError,'Incomplete'):update.check_entry(Path(t))

def bundle(version,extra=None):
 b=io.BytesIO()
 with zipfile.ZipFile(b,'w') as z:
  z.writestr('manifest.json',json.dumps({'build_id':version}));z.writestr('skills/loom-use/SKILL.md','fixture')
  if extra:z.writestr(extra,'unsafe')
 return b.getvalue()

class ReleaseLifecycleTests(unittest.TestCase):
 def setUp(self):
  t=tempfile.TemporaryDirectory();self.addCleanup(t.cleanup);self.p=Path(t.name);self.root=self.p/'ops';self.root.mkdir()
  (self.root/'runtime').mkdir();self.db=self.root/'runtime/ops.sqlite3'
  with sqlite3.connect(self.db) as c:
   c.executescript("CREATE TABLE jobs(id TEXT,state TEXT,spec TEXT); INSERT INTO jobs VALUES('old','complete','{}'); CREATE TABLE node_control(id INTEGER,mode TEXT);INSERT INTO node_control VALUES(1,'active'); CREATE TABLE hooks(state TEXT);")
  for name,content in [('compose.env','OPS_PORT=1\n'),('network.env','OPS_DASHBOARD_PORT=2\n'),('build.env','LOOM_BUILD_ID=old\n'),('override.env','USER_VALUE=kept\n'),('compose.override.yaml','services: {}\n')]: (self.root/name).write_text(content)
  self.rel=self.p/'release';self.src=self.rel/'source';(self.src/'deploy').mkdir(parents=True);(self.src/'deploy/compose.sh').write_text('fixture')
  (self.rel/'build.env').write_text('LOOM_BUILD_ID=new\n');self.info={'build_id':'new','status':'built','images':{'ops':'new'},'image_ids':{'ops':'sha256:fixture'}}
  (self.rel/'result.json').write_text(json.dumps(self.info));(self.rel/'manifest.json').write_text(json.dumps({'build_id':'new','files':{'deploy/compose.sh':hashlib.sha256(b'fixture').hexdigest()}}))
  guides=self.p/'loom_use';guide_bundle.install(bundle('old'),guides,'old');guide_bundle.select(guides,'old')
  self.old=self.p/'checkout';(self.old/'deploy').mkdir(parents=True);(self.old/'deploy/compose.sh').write_text('old compose')
  for kind in ('components','profiles'):
   (self.p/kind).mkdir();(self.p/kind/'custom.json').write_text('user revision')
 def mode(self):
  with sqlite3.connect(self.db) as c:return c.execute('select mode from node_control').fetchone()[0]
 def contexts(self):
  from contextlib import ExitStack
  stack=ExitStack();stack.enter_context(patch.object(update,'previous_source',return_value=self.old))
  # A real installation may exist on the development host. Never touch its unit.
  stack.enter_context(patch.object(update,'Path',side_effect=lambda value: self.p/'loom-stack.service' if str(value)=='/etc/systemd/system/loom-stack.service' else Path(value)))
  stack.enter_context(patch.object(update,'inspect',return_value=self.info));stack.enter_context(patch.object(update,'instance_paths',return_value=[self.root]));stack.enter_context(patch.object(update,'helper_files',return_value={}))
  stack.enter_context(patch.object(guide_bundle,'generate',return_value=bundle('new')))
  stack.enter_context(patch.object(update,'run',return_value=''));stack.enter_context(patch.object(update,'compose',return_value=''));stack.enter_context(patch.object(update,'check_health'))
  return stack
 def test_gate_rejects_running_jobs_and_pending_hooks_without_state_change(self):
  with sqlite3.connect(self.db) as c:c.execute("update jobs set state='running'")
  with self.assertRaises(ValueError):update.idle_gate(self.db)
  self.assertEqual(self.mode(),'active')
  with sqlite3.connect(self.db) as c:c.execute("update jobs set state='complete'");c.execute("insert into hooks values('pending')")
  with self.assertRaises(ValueError):update.idle_gate(self.db)
  self.assertEqual(self.mode(),'active')
 def test_update_preserves_extensions_overrides_and_history_then_rolls_back(self):
  with self.contexts():
   receipt=update.apply(self.root,self.rel,'fixture',self.p)
   self.assertEqual(guide_bundle.selected(self.p/'loom_use'),'new')
   self.assertEqual(json.loads((self.p/'loom_use/current/manifest.json').read_text())['build_id'],'new')
   self.assertEqual(self.mode(),'active');self.assertEqual((self.root/'build.env').read_text(),'LOOM_BUILD_ID=new\n')
   self.assertEqual((self.root/'override.env').read_text(),'USER_VALUE=kept\n')
   self.assertEqual((self.root/'compose.override.yaml').read_text(),'services: {}\n')
   for kind in ('components','profiles'):self.assertEqual((self.p/kind/'custom.json').read_text(),'user revision')
   # Snapshot of previous Compose is independent from a subsequently changed checkout.
   (self.old/'deploy/compose.sh').write_text('later change')
   record=json.loads(Path(receipt['record']).read_text());self.assertEqual((Path(record['previous_source'])/'deploy/compose.sh').read_text(),'old compose')
   update.rollback(receipt['record']);self.assertEqual(self.mode(),'active');self.assertEqual((self.root/'build.env').read_text(),'LOOM_BUILD_ID=old\n')
   self.assertEqual(guide_bundle.selected(self.p/'loom_use'),'old')
   self.assertTrue((self.p/'loom_use/new/manifest.json').exists())
   with sqlite3.connect(self.db) as c:self.assertEqual(c.execute('select * from jobs').fetchall(),[('old','complete','{}')])
 def test_rollback_refuses_to_erase_new_work(self):
  with self.contexts():
   receipt=update.apply(self.root,self.rel,'fixture',self.p)
   with sqlite3.connect(self.db) as c:c.execute("insert into jobs values('new','queued','{}')")
   with self.assertRaisesRegex(ValueError,'lose new records'):update.rollback(receipt['record'])
   self.assertEqual(self.mode(),'active')
 def test_failed_start_leaves_recoverable_snapshot_and_dispatch_fenced(self):
  with self.contexts(),patch.object(update,'check_health',side_effect=RuntimeError('start failed')):
   with self.assertRaises(RuntimeError):update.apply(self.root,self.rel,'fixture',self.p)
  self.assertEqual(guide_bundle.selected(self.p/'loom_use'),'old')
  record=next((self.p/'updates').glob('*/record.json'));self.assertEqual(json.loads(record.read_text())['state'],'failed');self.assertEqual(self.mode(),'maintenance')
  with self.contexts():update.rollback(record)
  self.assertEqual(self.mode(),'active');self.assertEqual((self.root/'build.env').read_text(),'LOOM_BUILD_ID=old\n')
 def test_busy_executor_releases_temporary_gate_before_mutation(self):
  ex=self.p/'shared/executor';ex.mkdir(parents=True)
  with sqlite3.connect(ex/'executor.sqlite3') as c:c.executescript("create table tickets(state text);insert into tickets values('running')")
  with self.contexts(),self.assertRaisesRegex(ValueError,'unreturned'):update.apply(self.root,self.rel,'fixture',self.p)
  self.assertEqual(self.mode(),'active');self.assertEqual((self.root/'build.env').read_text(),'LOOM_BUILD_ID=old\n')
 def test_modified_release_and_retagged_image_are_rejected(self):
  with patch.object(update,'run',return_value='different'),self.assertRaisesRegex(ValueError,'image mismatch'):update.inspect(self.root,self.rel)
  (self.src/'deploy/compose.sh').write_text('tampered')
  with patch.object(update,'run') as run,self.assertRaisesRegex(ValueError,'source mismatch'):update.inspect(self.root,self.rel)
  run.assert_not_called()
 def test_inspect_rejects_pending_unsupported_profile_but_keeps_history(self):
  def docker(*args,**kwargs):
   if args[:3]==('docker','image','inspect'):return 'sha256:fixture'
   return json.dumps({'services':{'loom-ops':{'image':'new'}}})
  with patch.object(update,'run',side_effect=docker):
   self.assertEqual(update.inspect(self.root,self.rel),self.info)
   with sqlite3.connect(self.db) as c:c.execute("insert into jobs values('pending','queued',?)",(json.dumps({'kind':'old.train','profile':{'entry':'producer.py'}}),))
   original=update.digest_db(self.db)
   with self.assertRaisesRegex(ValueError,'unsupported profile'):update.inspect(self.root,self.rel)
   self.assertEqual(update.digest_db(self.db),original)
   with sqlite3.connect(self.db) as c:c.execute("update jobs set spec=? where id='pending'",(json.dumps({'kind':'new.train','profile':{'schema':'loom-execution/v2'}}),))
   self.assertEqual(update.inspect(self.root,self.rel),self.info)
   with sqlite3.connect(self.db) as c:self.assertEqual(c.execute("select * from jobs where id='old'").fetchone(),('old','complete','{}'))
 def test_final_user_override_is_last_in_actual_compose_arguments(self):
  # Execute the wrapper against a fake engine; no Docker mutation.
  import subprocess
  b=self.p/'bin';b.mkdir();docker=b/'docker';docker.write_text('#!/bin/sh\nif [ "$1" = info ]; then exit 0; fi\nprintf "%s\\n" "$@"\n');docker.chmod(0o755)
  script=Path(__file__).resolve().parents[2]/'deploy/compose.sh'
  output=subprocess.check_output(['bash',str(script),'config'],env={**os.environ,'PATH':str(b)+':'+os.environ['PATH'],'LOOM_INSTANCE':str(self.root),'LOOM_PROJECT':'test'},text=True).splitlines()
  self.assertEqual(output[-5:],['--env-file',str(self.root/'override.env'),'-f',str(self.root/'compose.override.yaml'),'config'])


class GuideBundleTests(unittest.TestCase):
 def test_release_guide_generator_requests_complete_usage_scopes(self):
  # Execute the exact bundle-generation script without Docker to inspect the distributable contract.
  import subprocess
  root=Path(__file__).resolve().parents[2]
  with patch.object(guide_bundle.subprocess,'check_output',return_value=b'fixture') as call:
   guide_bundle.generate('fixture-image')
   command=call.call_args.args[0]
  result=subprocess.run([sys.executable,'-c',command[-1]],cwd=root/'ops',capture_output=True,check=True)
  with zipfile.ZipFile(io.BytesIO(result.stdout)) as z:
   tools=json.loads(z.read('mcp-tools.json'))['tools']
   self.assertEqual({t['name'] for t in tools},{'jobs','recovery','events','components','results','system'})
   jobs=next(t for t in tools if t['name']=='jobs')
   self.assertIn('cancel',jobs['inputSchema']['properties']['action']['enum'])
 def test_bundle_rejects_wrong_version_and_path_escape_before_writing(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t)/'guides'
   for data in [bundle('wrong'),bundle('v1','../escape'),bundle('v1','./manifest.json'),bundle('v1','skills//extra.md')]:
    with self.assertRaises(ValueError):guide_bundle.install(data,root,'v1')
   self.assertFalse(root.exists())
 def test_reinstall_preserves_modified_version_and_current_directory(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t);guide_bundle.install(bundle('v1'),root,'v1')
   guide_bundle.install(bundle('v1'),root,'v1')
   file=root/'v1/skills/loom-use/SKILL.md';file.write_text('user change')
   with self.assertRaisesRegex(ValueError,'retained'):guide_bundle.install(bundle('v1'),root,'v1')
   self.assertEqual(file.read_text(),'user change')
   (root/'current').mkdir()
   with self.assertRaises(ValueError):guide_bundle.select(root,'v1')
   self.assertTrue((root/'current').is_dir())
 def test_current_can_be_removed_when_no_previous_bundle(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t);guide_bundle.install(bundle('v1'),root,'v1');guide_bundle.select(root,'v1')
   guide_bundle.select(root,None)
   self.assertFalse((root/'current').is_symlink());self.assertTrue((root/'v1').is_dir())
 def test_reinstall_rejects_directory_symlinks_even_when_files_match(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t);guide_bundle.install(bundle('v1'),root,'v1')
   (root/'v1/skills').rename(root/'external');(root/'v1/skills').symlink_to(root/'external',target_is_directory=True)
   with self.assertRaisesRegex(ValueError,'linked'):guide_bundle.install(bundle('v1'),root,'v1')
