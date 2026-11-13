"""Cumulative scripts use real SQLite/files; all Docker/systemd stay mocked."""
import hashlib
import json
import os
import shutil
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import migrations,state_snapshot,update
import test_release_lifecycle as lifecycle


class MigrationPlanTests(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup);self.p=Path(temp.name)
        self.root=self.p/'ops';(self.root/'runtime').mkdir(parents=True)
        self.db=self.root/'runtime/ops.sqlite3'
        with sqlite3.connect(self.db) as c:c.execute('CREATE TABLE credentials(id TEXT,digest TEXT)');c.execute("INSERT INTO credentials VALUES('owner','unchanged')")
        (self.root/'build.env').write_text('LOOM_BUILD_ID=0.1.0-alpha.40\n')
        self.release=self.p/'release';self.source=self.release/'source';(self.source/'deploy/migrations').mkdir(parents=True)
        (self.release/'result.json').write_text(json.dumps({'build_id':'0.1.0-alpha.45'}))
        self.steps=[]
        for key,introduced,requires in [('one','0.1.0-alpha.41',[]),('two','0.1.0-alpha.44',['one'])]:
            script=self.source/('deploy/migrations/'+key+'.py')
            script.write_text('''import argparse,json,sqlite3
p=argparse.ArgumentParser();p.add_argument('phase');p.add_argument('--instance');p.add_argument('--platform');a=p.parse_args()
with sqlite3.connect(a.instance+'/runtime/ops.sqlite3') as c:
 done=bool(c.execute("SELECT 1 FROM sqlite_master WHERE name='__TABLE__'").fetchone())
 if a.phase=='apply' and not done:c.execute('CREATE TABLE __TABLE__(id TEXT)');done=True
print(json.dumps({'status':'done' if done else 'needed'}))
'''.replace('__TABLE__',key))
            self.steps.append({'id':key,'introduced':introduced,'requires':requires,'script':str(script.relative_to(self.source)),'timeout_seconds':5})
        self.catalog={'schema':'loom-migration-catalog/v1','minimum_version':'0.1.0-alpha.38','steps':list(reversed(self.steps))}
        self.seal()

    def seal(self):
        (self.source/'deploy/migrations/catalog.json').write_text(json.dumps(self.catalog))
        (self.release/'manifest.json').write_text(json.dumps({'files':{str(p.relative_to(self.source)):hashlib.sha256(p.read_bytes()).hexdigest() for p in self.source.rglob('*') if p.is_file()}}))

    def test_skipped_versions_apply_dependencies_once_and_keep_credentials(self):
        plan=migrations.plan(self.root,self.release)
        self.assertEqual([s['id'] for s in plan['steps']],['one','two'])
        self.assertFalse((self.root/'runtime/migrations.json').exists())
        migrations.apply(plan,self.root,self.release,self.p)
        with sqlite3.connect(self.db) as c:self.assertEqual(c.execute('SELECT * FROM credentials').fetchall(),[('owner','unchanged')])
        again=migrations.plan(self.root,self.release);self.assertTrue(all(s['completed'] for s in again['steps']))
        migrations.apply(again,self.root,self.release,self.p)
        self.assertEqual(set(migrations.ledger(self.root)['completed']),{'one','two'})

    def test_interrupted_effect_before_receipt_is_verified_without_replay(self):
        plan=migrations.plan(self.root,self.release)
        migrations.invoke(plan['steps'][0],'apply',self.root,self.release,self.p)
        with patch.object(migrations,'invoke',wraps=migrations.invoke) as call:
            migrations.apply(plan,self.root,self.release,self.p)
        self.assertNotIn(('one','apply'),[(args.args[0]['id'],args.args[1]) for args in call.call_args_list])

    def test_missing_dependency_cycle_old_origin_and_script_tamper_rejected(self):
        self.catalog['steps'][0]['requires']=['missing'];self.seal()
        with self.assertRaisesRegex(ValueError,'dependency'):migrations.plan(self.root,self.release)
        self.catalog['steps'][0]['requires']=['one'];self.catalog['steps'][1]['requires']=['two'];self.seal()
        with self.assertRaisesRegex(ValueError,'cycle'):migrations.plan(self.root,self.release)
        self.catalog['steps'][1]['requires']=[];self.seal()
        (self.source/self.steps[0]['script']).write_text('tamper')
        with self.assertRaisesRegex(ValueError,'checksum'):migrations.plan(self.root,self.release)
        (self.root/'build.env').write_text('LOOM_BUILD_ID=0.1.0-alpha.1\n')
        with self.assertRaisesRegex(ValueError,'bridge'):migrations.plan(self.root,self.release)

    def test_completed_script_is_immutable_and_probe_is_bounded(self):
        plan=migrations.plan(self.root,self.release);migrations.apply(plan,self.root,self.release,self.p)
        script=self.source/self.steps[0]['script'];script.write_text(script.read_text()+'\n# changed\n');self.seal()
        with self.assertRaisesRegex(ValueError,'code changed'):migrations.plan(self.root,self.release)
        import subprocess
        with patch.object(migrations.subprocess,'run',side_effect=subprocess.TimeoutExpired('fixture',5)),self.assertRaises(subprocess.TimeoutExpired):
            migrations.invoke(plan['steps'][1],'check',self.root,self.release,self.p)


class PreservedRebuildTests(lifecycle.ReleaseLifecycleTests):
    def identity_state(self):
        (self.root/'config').mkdir();(self.root/'secrets').mkdir()
        (self.root/'config/api.json').write_text('{"installation_id":"original-install","machine_id":"original-machine"}')
        (self.root/'secrets/session-key').write_text('fixture-session-secret');(self.root/'secrets/session-key').chmod(0o600)
        with sqlite3.connect(self.db) as c:
            c.executescript("CREATE TABLE resident_tokens(id TEXT,digest TEXT,owner TEXT,scopes TEXT,revoked REAL); INSERT INTO resident_tokens VALUES('live','digest-live','person','[\"read\"]',NULL); INSERT INTO resident_tokens VALUES('revoked','digest-revoked','person','[\"read\"]',1); CREATE TABLE user_profiles(id TEXT,login TEXT); INSERT INTO user_profiles VALUES('person','person@example.test'); CREATE TABLE entity_revisions(id TEXT,revision INTEGER,locator TEXT); INSERT INTO entity_revisions VALUES('model',1,'original-store/path');")
        admission=self.root/'admission';admission.mkdir()
        with sqlite3.connect(admission/'requests.sqlite3') as c:c.executescript("CREATE TABLE shares(id TEXT,user TEXT);INSERT INTO shares VALUES('grant','person')")

    def test_rebuild_preserves_tokens_accounts_registry_config_and_secondary_database(self):
        self.identity_state();original=state_snapshot.db_inventory(self.db)
        with self.contexts(),patch.object(update,'compose',return_value='') as compose:
            receipt=update.apply(self.root,self.rel,'fixture',self.p,mode='rebuild')
            self.assertTrue(any('--force-recreate' in c.args for c in compose.call_args_list))
        self.assertEqual(state_snapshot.db_inventory(self.db),original)
        self.assertEqual((self.root/'secrets/session-key').read_text(),'fixture-session-secret')
        record=json.loads(Path(receipt['record']).read_text());self.assertEqual(len(record['databases']),2)
        self.assertTrue(record['preservation']['preserved'])
        # Authenticate both old active/revoked digests through the actual issuer
        # implementation, not just an aggregate row count.
        import importlib.util
        spec=importlib.util.spec_from_file_location('preserved_tokens_fixture',Path(__file__).resolve().parents[2]/'ops/service_tokens.py')
        tokens=importlib.util.module_from_spec(spec);spec.loader.exec_module(tokens)
        with sqlite3.connect(self.db) as c:
            self.assertEqual(tokens.authenticate(c,'digest-live')['id'],'person')
            self.assertEqual(tokens.authenticate(c,'digest-live')['scopes'],['read'])
            self.assertIsNone(tokens.authenticate(c,'digest-revoked'))
        with self.contexts():update.rollback(receipt['record'])
        self.assertEqual(state_snapshot.db_inventory(self.db),original)

    def test_missing_token_keeps_maintenance_and_allows_rollback(self):
        self.identity_state()
        def corrupt(*args):
            with sqlite3.connect(self.db) as c:c.execute("DELETE FROM resident_tokens WHERE id='live'")
        with self.contexts(),patch.object(update,'check_health',side_effect=corrupt),self.assertRaisesRegex(ValueError,'Preserved state'):
            update.apply(self.root,self.rel,'fixture',self.p)
        self.assertEqual(self.mode(),'maintenance')
        record=next((self.p/'updates').glob('*/record.json'))
        with self.contexts():update.rollback(record)
        with sqlite3.connect(self.db) as c:self.assertEqual(c.execute('SELECT count(*) FROM resident_tokens').fetchone()[0],2)

    def test_failed_start_resumes_same_snapshot_without_new_identity(self):
        self.identity_state()
        with self.contexts(),patch.object(update,'check_health',side_effect=RuntimeError('fixture')),self.assertRaises(RuntimeError):
            update.apply(self.root,self.rel,'fixture',self.p,mode='rebuild')
        record=next((self.p/'updates').glob('*/record.json'));identity=(self.root/'config/api.json').read_bytes()
        with self.contexts():result=update.resume(record)
        self.assertTrue(result['resumed']);self.assertEqual(self.mode(),'active')
        self.assertEqual((self.root/'config/api.json').read_bytes(),identity)

    def test_resume_stops_shared_helpers_before_replacing_code(self):
        helper=self.p/'shared/services/fixture.py';helper.parent.mkdir(parents=True);helper.write_text('old helper')
        candidate=self.src/'fixture.py';candidate.write_text('new helper')
        with self.contexts(),patch.object(update,'helper_files',return_value={helper:candidate}),patch.object(update,'check_health',side_effect=RuntimeError('fixture')):
            with self.assertRaises(RuntimeError):update.apply(self.root,self.rel,'fixture',self.p)
        record=next((self.p/'updates').glob('*/record.json'));events=[]
        original=update.shutil.copyfile
        def copy(source,target,*args,**kwargs):
            if Path(target)==helper:events.append('replace')
            return original(source,target,*args,**kwargs)
        def run(*args,**kwargs):
            if args[:2]==('systemctl','stop'):events.append('stop')
            return ''
        with self.contexts(),patch.object(update,'helper_files',return_value={helper:candidate}),patch.object(update,'run',side_effect=run),patch.object(update.shutil,'copyfile',side_effect=copy):
            self.assertTrue(update.resume(record)['resumed'])
        self.assertEqual(events[:2],['stop','replace'])

    def test_rollback_refuses_new_secondary_database_writes_and_changed_credentials(self):
        self.identity_state()
        with self.contexts():receipt=update.apply(self.root,self.rel,'fixture',self.p)
        with sqlite3.connect(self.root/'admission/requests.sqlite3') as c:c.execute("INSERT INTO shares VALUES('new','other')")
        with self.contexts(),self.assertRaisesRegex(ValueError,'lose new records'):update.rollback(receipt['record'])

    def test_job_accepted_immediately_after_ungate_is_not_a_rollback_baseline(self):
        normal=update.ungate
        def accept(path):
            normal(path)
            with sqlite3.connect(self.db) as c:c.execute("INSERT INTO jobs VALUES('accepted-after-release','queued','{}')")
        with self.contexts(),patch.object(update,'ungate',side_effect=accept):receipt=update.apply(self.root,self.rel,'fixture',self.p)
        with self.contexts(),self.assertRaisesRegex(ValueError,'lose new records'):update.rollback(receipt['record'])

    def test_failed_source_migration_restores_revisions_but_never_overwrites_later_edits(self):
        original=self.p/'components/custom.json'
        with self.contexts(),patch.object(update,'check_health',side_effect=lambda *args:original.unlink()),self.assertRaisesRegex(ValueError,'Preserved state'):
            update.apply(self.root,self.rel,'fixture',self.p)
        record=next((self.p/'updates').glob('*/record.json'))
        with self.contexts():update.rollback(record)
        self.assertEqual(original.read_text(),'user revision')

    def test_rollback_refuses_later_unregistered_source_edits(self):
        with self.contexts():receipt=update.apply(self.root,self.rel,'fixture',self.p)
        (self.p/'components/custom.json').write_text('new personal work')
        with self.contexts(),self.assertRaisesRegex(ValueError,'Extensions changed'):update.rollback(receipt['record'])

    def test_executor_upgrade_preserves_custom_graphics_network_and_mount_policy(self):
        path=self.p/'shared/executor/config.json';path.parent.mkdir(parents=True)
        original=json.dumps({'runtimes':{'sim':{'image':'sha256:pinned','driver_capabilities':['compute','utility','graphics'],
            'network':'bridge','mounts':[{'source':'/preserved/assets','target':'/assets','readonly':True}]}},
            'instances':{'alpha':{'declared_runtime':{'enabled':True,'driver_capabilities':['compute','utility','graphics'],'network':'bridge'}}}})
        path.write_text(original);path.chmod(0o600)
        with self.contexts():receipt=update.apply(self.root,self.rel,'fixture',self.p)
        self.assertEqual(path.read_text(),original);self.assertEqual(path.stat().st_mode&0o777,0o600)
        record=json.loads(Path(receipt['record']).read_text());self.assertEqual(record['executor_settings']['sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        path.write_text(original+'\n')
        with self.contexts(),self.assertRaisesRegex(ValueError,'executor configuration changed'):update.rollback(receipt['record'])

    def test_snapshot_restores_database_permissions_and_symlink_owner(self):
        self.identity_state();self.db.chmod(0o640)
        link=self.root/'config/session-key';link.symlink_to('../secrets/session-key')
        archive=self.p/'private-snapshot';archive.mkdir()
        saved=state_snapshot.capture(self.root,archive,self.p)
        self.db.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'database-permissions'):state_snapshot.verify(self.root,saved)
        state_snapshot.restore(self.root,saved)
        self.assertEqual(self.db.stat().st_mode&0o777,0o640)
        self.assertEqual(os.readlink(link),'../secrets/session-key')
        source=saved/'instance/config/session-key'
        original=state_snapshot.os.chown
        with patch.object(state_snapshot.os,'geteuid',return_value=0),patch.object(state_snapshot.os,'chown',wraps=original) as chown:
            state_snapshot.restore_file(source,link,self.root)
        info=source.lstat()
        chown.assert_called_once_with(link,info.st_uid,info.st_gid,follow_symlinks=False)
        self.assertTrue(state_snapshot.verify(self.root,saved)['preserved'])


class AdmissionPreservationTests(unittest.TestCase):
    def test_reconcile_clocks_can_progress_but_grants_and_tokens_remain_exact(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)/'instance';(root/'admission').mkdir(parents=True);platform=Path(tmp)
            (root/'compose.env').write_text('')
            (platform/'backup').mkdir()
            database=root/'admission/requests.sqlite3'
            with sqlite3.connect(database) as c:
                c.executescript("CREATE TABLE share_requests(id TEXT,state TEXT,approved_by TEXT,updated REAL,next_attempt REAL); INSERT INTO share_requests VALUES('request','ready','admin',1,2); CREATE TABLE tokens(id TEXT,revoked INTEGER); INSERT INTO tokens VALUES('existing',0)")
            saved=state_snapshot.capture(root,platform/'backup',platform)
            with sqlite3.connect(database) as c:c.execute('UPDATE share_requests SET updated=100,next_attempt=200')
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
            with sqlite3.connect(database) as c:c.execute("UPDATE share_requests SET approved_by='another'")
            with self.assertRaisesRegex(ValueError,'share_requests'):state_snapshot.verify(root,saved)
            with sqlite3.connect(database) as c:c.execute("UPDATE share_requests SET approved_by='admin'");c.execute('UPDATE tokens SET revoked=1')
            with self.assertRaisesRegex(ValueError,'tokens'):state_snapshot.verify(root,saved)
