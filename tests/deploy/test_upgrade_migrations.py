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

    def test_latest_schema_replaces_older_incompatible_checks_during_resume(self):
        third={**self.steps[-1],'id':'three','introduced':'0.1.0-alpha.45','requires':['two'],'supersedes_checks':['one','two'],'script':'deploy/migrations/three.py'}
        third_path=self.source/third['script'];third_path.write_text((self.source/self.steps[0]['script']).read_text().replace("name='one'","name='three'").replace('TABLE one(', 'TABLE three('))
        self.steps[-1]['supersedes_checks']=['one']
        for step in self.steps:
            path=self.source/step['script'];text=path.read_text();text=text.replace(' done=bool(', " if a.phase in ('check','verify') and c.execute(\"SELECT 1 FROM sqlite_master WHERE name='three'\").fetchone():raise RuntimeError('Older schema check cannot inspect schema three')\n done=bool(")
            path.write_text(text)
        self.catalog['steps']=[*self.steps,third];self.seal()
        with sqlite3.connect(self.db) as db:db.executescript('CREATE TABLE one(id);CREATE TABLE two(id);CREATE TABLE three(id);')
        migrations.apply(migrations.plan(self.root,self.release),self.root,self.release,self.p)
        self.assertEqual(set(migrations.ledger(self.root)['completed']),{'one','two','three'})
        with sqlite3.connect(self.db) as db:self.assertEqual(db.execute('SELECT * FROM credentials').fetchall(),[('owner','unchanged')])

    def test_release_checks_accept_schema_eight_with_existing_migration_receipts(self):
        checkout = Path(__file__).resolve().parents[2]
        catalog = json.loads((checkout / "deploy/migrations/catalog.json").read_text())
        self.catalog = catalog
        for step in catalog["steps"]:
            rel = step["script"]
            shutil.copy2(checkout / rel, self.source / rel)
        (self.release / "result.json").write_text('{"build_id":"0.2.0-alpha.72"}')
        (self.root / "build.env").write_text("LOOM_BUILD_ID=0.2.0-alpha.69\n")
        self.seal()
        plan = migrations.plan(self.root, self.release)
        step = plan["steps"][-1]
        self.assertEqual(step["id"], "reconciliation-compatibility-v1")
        # Scope the cumulative check to migrations owning these database schemas.
        plan["steps"] = [s for s in plan["steps"]
                         if s["id"] not in ("research-tools-v1", "browser-relay-routes-v1")]
        prior = next(s for s in plan["steps"] if s["id"] == "common-reconciliation-v1")
        migrations.save(self.root / "runtime/migrations.json", {
            "schema": "loom-migrations/v1",
            "completed": {prior["id"]: {"sha256": prior["sha256"]}},
        })
        with sqlite3.connect(self.db) as db:
            db.executescript("""
                PRAGMA user_version=6;
                CREATE TABLE catalog_meta(id);
                CREATE TABLE reconcile_plans(id);
                CREATE TABLE asset_imports(id);
                CREATE TABLE file_bindings(id);
                CREATE TABLE log_retention(id);
                CREATE TABLE catalog_tasks(id);
                CREATE TABLE asset_revisions(id);
                CREATE TABLE asset_locations(id);
                CREATE TABLE research_jobs(id);
                CREATE TABLE execution_units(id);
                CREATE VIEW execution_memberships AS SELECT id FROM execution_units;
                CREATE TABLE rust_authority(id, minimum_writer);
                INSERT INTO rust_authority VALUES(1, 2);
            """)
        self.assertEqual(migrations.invoke(step, "check", self.root, self.release, self.p), "needed")
        with sqlite3.connect(self.db) as db:
            db.execute("PRAGMA user_version=8")
        # Exercise the real cumulative order and supersession with the real schema probes.
        original = self.db.read_bytes()
        migrations.apply(plan, self.root, self.release, self.p)
        self.assertEqual(self.db.read_bytes(), original)
        with sqlite3.connect(self.db) as db:
            db.execute("DROP VIEW execution_memberships")
        with self.assertRaisesRegex(RuntimeError, "reconciliation-compatibility-v1"):
            migrations.invoke(step, "check", self.root, self.release, self.p)


class PreservedRebuildTests(lifecycle.ReleaseLifecycleFixture):
    def test_preflight_failure_restores_active_admission_without_resuming_dispatch(self):
        with sqlite3.connect(self.db) as db:
            db.executescript('CREATE TABLE dispatch_control(id INTEGER PRIMARY KEY, paused INTEGER); INSERT INTO dispatch_control VALUES(1,1);')
        executor = self.p/'shared/executor/executor.sqlite3'
        executor.parent.mkdir(parents=True)
        with sqlite3.connect(executor) as db:
            db.execute('CREATE TABLE invalid_fixture(id)')
        with self.contexts(), self.assertRaises(sqlite3.OperationalError):
            update.apply(self.root,self.rel,'fixture',self.p)
        self.assertEqual(self.mode(),'active')
        with sqlite3.connect(self.db) as db:
            self.assertEqual(db.execute('SELECT paused FROM dispatch_control').fetchone(),(1,))

    def test_rollback_restores_active_admission_with_a_paused_dispatcher(self):
        with sqlite3.connect(self.db) as db:
            db.executescript('CREATE TABLE dispatch_control(id INTEGER PRIMARY KEY, paused INTEGER); INSERT INTO dispatch_control VALUES(1,1);')
        with self.contexts():
            receipt=update.apply(self.root,self.rel,'fixture',self.p)
            update.rollback(receipt['record'])
        self.assertEqual(self.mode(),'active')
        with sqlite3.connect(self.db) as db:
            self.assertEqual(db.execute('SELECT paused FROM dispatch_control').fetchone(),(1,))

    def test_update_preserves_preexisting_admission_fence_and_dispatch_pause(self):
        with sqlite3.connect(self.db) as db:
            db.execute("UPDATE node_control SET mode='maintenance' WHERE id=1")
            db.execute("CREATE TABLE dispatch_control(id INTEGER PRIMARY KEY, paused INTEGER)")
            db.execute("INSERT INTO dispatch_control VALUES(1,1)")
        with self.contexts(),patch.object(update,'ungate',wraps=update.ungate) as ungate:
            receipt=update.apply(self.root,self.rel,'fixture',self.p)
            ungate.assert_not_called()
        self.assertEqual(self.mode(),'maintenance')
        self.assertEqual(json.loads(Path(receipt['record']).read_text())['pre_update_admission'][str(self.db)],'maintenance')
        with sqlite3.connect(self.db) as db:self.assertEqual(db.execute('SELECT paused FROM dispatch_control').fetchone(),(1,))

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

    def test_snapshot_failure_rollback_restarts_helpers_stopped_before_backup(self):
        helper=self.p/'shared/services/fixture.py';helper.parent.mkdir(parents=True);helper.write_text('preserved helper')
        candidate=self.src/'fixture.py';candidate.write_text('candidate')
        with self.contexts(),patch.object(update,'helper_files',return_value={helper:candidate}),patch.object(state_snapshot,'capture',side_effect=OSError('transient SQLite sidecar')):
            with self.assertRaises(OSError):update.apply(self.root,self.rel,'fixture',self.p)
        record=next((self.p/'updates').glob('*/record.json'));saved=json.loads(record.read_text());self.assertTrue(saved['helpers_stop_requested']);self.assertEqual(saved['helpers'],{})
        with self.contexts(),patch.object(update,'run',wraps=update.run) as calls:update.rollback(record)
        self.assertIn(('systemctl','start','loom-executor.service','loom-observations.timer'),[c.args for c in calls.call_args_list]);self.assertEqual(helper.read_text(),'preserved helper');self.assertEqual(self.mode(),'active')

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
    def test_expired_dashboard_receipts_can_be_pruned_during_health_check(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'ops';admission=root/'admission';admission.mkdir(parents=True)
            (root/'compose.env').touch()
            path=admission/'requests.sqlite3'
            with sqlite3.connect(path) as c:
                c.execute('CREATE TABLE receipts(id TEXT PRIMARY KEY, subject TEXT, expires REAL)')
                c.executemany('INSERT INTO receipts VALUES(?,?,?)',[('expired','a',9),('live','b',11)])
            archive=Path(directory)/'snapshot';archive.mkdir()
            saved=state_snapshot.capture(root,archive,Path(directory))
            with sqlite3.connect(path) as c:c.execute("DELETE FROM receipts WHERE id='expired'")
            with patch.object(state_snapshot.time,'time',return_value=10):
                self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
                with sqlite3.connect(path) as c:c.execute("UPDATE receipts SET subject='c' WHERE id='live'")
                with self.assertRaisesRegex(ValueError,'rows:receipts'):state_snapshot.verify(root,saved)

    def test_profile_access_clock_can_advance_but_identity_and_display_fields_stay_exact(self):
        with tempfile.TemporaryDirectory() as tmp:
            platform=Path(tmp);root=platform/'instance';(root/'runtime').mkdir(parents=True)
            (root/'compose.env').write_text('');(platform/'backup').mkdir()
            database=root/'runtime/ops.sqlite3'
            with sqlite3.connect(database) as c:
                c.executescript("CREATE TABLE user_profiles(id TEXT,provider TEXT,login TEXT,display_name TEXT,updated REAL); INSERT INTO user_profiles VALUES('person','sso','person@example.test','Person',1)")
            saved=state_snapshot.capture(root,platform/'backup',platform)
            with sqlite3.connect(database) as c:c.execute('UPDATE user_profiles SET updated=200')
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
            for column in ('id','provider','login','display_name'):
                with self.subTest(column=column):
                    with sqlite3.connect(database) as c:
                        before=c.execute('SELECT '+column+' FROM user_profiles').fetchone()[0]
                        c.execute('UPDATE user_profiles SET '+column+'=?',('changed',))
                    with self.assertRaisesRegex(ValueError,'user_profiles'):state_snapshot.verify(root,saved)
                    with sqlite3.connect(database) as c:c.execute('UPDATE user_profiles SET '+column+'=?',(before,))

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


class SQLiteEvidenceTests(unittest.TestCase):
    def test_reader_pins_one_wal_snapshot_and_closes_before_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'state.sqlite3'
            with sqlite3.connect(path) as writer:
                writer.execute('PRAGMA journal_mode=WAL')
                writer.execute('CREATE TABLE evidence(id INTEGER)');writer.execute('INSERT INTO evidence VALUES(1)')
            with state_snapshot.database(path,readonly=True) as reader:
                self.assertEqual(reader.execute('SELECT count(*) FROM evidence').fetchone()[0],1)
                with sqlite3.connect(path) as writer:writer.execute('INSERT INTO evidence VALUES(2)')
                self.assertEqual(reader.execute('SELECT count(*) FROM evidence').fetchone()[0],1)
            with self.assertRaises(sqlite3.ProgrammingError):reader.execute('SELECT 1')
            with state_snapshot.database(path,readonly=True) as current:
                self.assertEqual(current.execute('SELECT count(*) FROM evidence').fetchone()[0],2)

    def test_bad_db_fingerprint_does_not_mask_primary_failure_or_lose_record(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);good=root/'good.sqlite3';bad=root/'bad.sqlite3';bad.write_bytes(b'not a database')
            with sqlite3.connect(good) as c:c.execute('CREATE TABLE credentials(id TEXT)');c.execute("INSERT INTO credentials VALUES('preserved')")
            record={'state':'failed','databases':{str(good):'good-backup',str(bad):'bad-backup'},'files':{},'helpers':{}}
            target=root/'record.json'
            with self.assertRaisesRegex(RuntimeError,'primary preservation failure'):
                try:raise RuntimeError('primary preservation failure')
                finally:update.failure_fingerprints(record,target,root)
            saved=json.loads(target.read_text())
            self.assertEqual(saved['state'],'failed');self.assertIn(str(good),saved['post_databases'])
            self.assertEqual(saved['post_evidence_errors'][str(bad)],'DatabaseError')
            self.assertNotIn(str(bad),saved['post_databases'])
            with sqlite3.connect(good) as c:self.assertEqual(c.execute('SELECT * FROM credentials').fetchall(),[('preserved',)])


class ItemManifestPreservationTests(unittest.TestCase):
    def test_dashboard_projection_counters_can_refresh_without_losing_domains(self):
        with tempfile.TemporaryDirectory() as temp:
            platform=Path(temp);root=platform/'ops';(root/'runtime').mkdir(parents=True)
            (root/'compose.env').write_text('');db=root/'runtime/ops.sqlite3'
            with sqlite3.connect(db) as c:
                c.executescript("CREATE TABLE rust_dashboard_versions(domain TEXT PRIMARY KEY,version INTEGER NOT NULL);"
                                "INSERT INTO rust_dashboard_versions VALUES('software',1),('outputs',1);")
            archive=platform/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,platform)
            with sqlite3.connect(db) as c:c.execute("UPDATE rust_dashboard_versions SET version=9 WHERE domain='software'")
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
            with sqlite3.connect(db) as c:c.execute("DELETE FROM rust_dashboard_versions WHERE domain='software'")
            with self.assertRaisesRegex(ValueError,'rust_dashboard_versions'):state_snapshot.verify(root,saved)

    def test_native_projection_progress_keeps_original_identities_and_revisions(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'ops';(root/'runtime').mkdir(parents=True);(root/'compose.env').write_text('');db=root/'runtime/ops.sqlite3'
            with sqlite3.connect(db) as c:
                c.executescript("CREATE TABLE original(id,value);INSERT INTO original VALUES('science','preserved');CREATE TABLE rust_runtime_observation(id,value,observed);INSERT INTO rust_runtime_observation VALUES(1,'old',1);CREATE TABLE rust_projection_version(id,version);INSERT INTO rust_projection_version VALUES(1,1);CREATE TABLE catalog_items(id,kind,native_kind,native_key,current_revision,created,deleted);INSERT INTO catalog_items VALUES('item','profile','profile','builtin','r1',1,0);CREATE TABLE catalog_revisions(id,item,body);INSERT INTO catalog_revisions VALUES('r1','item','original revision');CREATE TABLE catalog_aliases(kind,identity,item,revision);INSERT INTO catalog_aliases VALUES('profile','builtin','item','r1');CREATE TABLE catalog_sources(kind,identity,body);INSERT INTO catalog_sources VALUES('profile','builtin','old');")
            archive=Path(temp)/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,Path(temp))
            with sqlite3.connect(db) as c:
                c.executescript("UPDATE rust_runtime_observation SET value='fresh',observed=2;UPDATE rust_projection_version SET version=2;INSERT INTO catalog_revisions VALUES('r2','item','new version');UPDATE catalog_items SET current_revision='r2';UPDATE catalog_aliases SET revision='r2';UPDATE catalog_sources SET body='fresh';")
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
            with sqlite3.connect(db) as c:c.execute("DELETE FROM catalog_revisions WHERE id='r1'")
            with self.assertRaisesRegex(ValueError,'catalog_revisions'):state_snapshot.verify(root,saved)

    def test_observed_runtime_retirement_preserves_identity_but_scientific_retirement_does_not(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'ops';(root/'runtime').mkdir(parents=True);(root/'compose.env').write_text('');db=root/'runtime/ops.sqlite3'
            with sqlite3.connect(db) as c:
                c.executescript("CREATE TABLE catalog_items(id,kind,native_kind,native_key,current_revision,created,deleted);INSERT INTO catalog_items VALUES('dep','dependency','dependency','original-package',NULL,1,0);INSERT INTO catalog_items VALUES('science','model','entity','original-model',NULL,1,0);")
            archive=Path(temp)/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,Path(temp))
            with sqlite3.connect(db) as c:c.execute("UPDATE catalog_items SET deleted=1 WHERE id='dep'")
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
            with sqlite3.connect(db) as c:c.execute("UPDATE catalog_items SET deleted=1 WHERE id='science'")
            with self.assertRaisesRegex(ValueError,'catalog_items'):state_snapshot.verify(root,saved)
            with sqlite3.connect(db) as c:c.execute("UPDATE catalog_items SET deleted=0 WHERE id='science'");c.execute("UPDATE catalog_items SET native_key='changed' WHERE id='dep'")
            with self.assertRaisesRegex(ValueError,'catalog_items'):state_snapshot.verify(root,saved)

    def test_catalog_epoch_only_advances_while_origin_and_cursor_key_stay_exact(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'ops';(root/'runtime').mkdir(parents=True);(root/'compose.env').write_text('');db=root/'runtime/ops.sqlite3'
            with sqlite3.connect(db) as c:c.executescript("CREATE TABLE catalog_meta(id,origin,cursor_key,access_epoch);INSERT INTO catalog_meta VALUES(1,'original-origin','original-key',4);")
            archive=Path(temp)/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,Path(temp))
            with sqlite3.connect(db) as c:c.execute('UPDATE catalog_meta SET access_epoch=5')
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])
            for column,value in [('access_epoch',3),('origin','changed'),('cursor_key','changed')]:
                with sqlite3.connect(db) as c:c.execute("UPDATE catalog_meta SET origin='original-origin',cursor_key='original-key',access_epoch=5");c.execute('UPDATE catalog_meta SET '+column+'=?',(value,))
                with self.assertRaises(ValueError):state_snapshot.verify(root,saved)

    def test_snapshot_omits_sqlite_sidecars_and_keeps_committed_wal_rows(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'ops';(root/'runtime').mkdir(parents=True);(root/'compose.env').write_text('')
            db=root/'runtime/ops.sqlite3';connection=sqlite3.connect(db)
            try:
                connection.execute('PRAGMA journal_mode=WAL');connection.execute('CREATE TABLE original(id)');connection.execute("INSERT INTO original VALUES('committed only in WAL')");connection.commit()
                self.assertTrue(Path(str(db)+'-wal').is_file());userfile=root/'notes-wal';userfile.write_text('user source')
                archive=Path(temp)/'backup';archive.mkdir()
                original=state_snapshot.copy_file
                def copy(source,dest):
                    self.assertFalse(state_snapshot.sqlite_sidecar(source),'Volatile sidecar must not be copied')
                    return original(source,dest)
                with patch.object(state_snapshot,'copy_file',side_effect=copy):saved=state_snapshot.capture(root,archive,Path(temp))
                self.assertFalse((saved/'instance/runtime/ops.sqlite3-wal').exists())
                with state_snapshot.database(saved/'instance/runtime/ops.sqlite3',readonly=True) as backup:self.assertEqual(backup.execute('SELECT * FROM original').fetchall(),[('committed only in WAL',)])
                self.assertEqual((saved/'instance/notes-wal').read_text(),'user source');self.assertIn('notes-wal',json.loads((saved/'state.json').read_text())['files'])
            finally:connection.close()

    def test_private_snapshot_preserves_authority_manifests_and_omits_search_projection(self):
        with tempfile.TemporaryDirectory() as temp:
            platform=Path(temp);root=platform/'instance';(root/'runtime/catalog-manifests/rev').mkdir(parents=True);(root/'runtime/search').mkdir()
            (root/'compose.env').write_text('OPS_BULK='+str(platform/'data')+'\n')
            chunk=root/'runtime/catalog-manifests/rev/chunk.jsonl';chunk.write_text('{"item_id":"123"}\n')
            with sqlite3.connect(root/'runtime/search/catalog.sqlite3') as c:c.execute('CREATE TABLE derived(v)');c.execute('INSERT INTO derived VALUES(1)')
            archive=platform/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,platform)
            self.assertFalse((saved/'instance/runtime/search').exists())
            chunk.write_text('changed')
            with self.assertRaisesRegex(ValueError,'catalog-manifests'):state_snapshot.verify(root,saved)
            state_snapshot.restore(root,saved);self.assertEqual(chunk.read_text(),'{"item_id":"123"}\n')
            self.assertTrue(state_snapshot.verify(root,saved)['preserved'])

    def test_rollback_cannot_drop_ids_assigned_during_a_failed_upgrade(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'instance';(root/'runtime').mkdir(parents=True);(root/'compose.env').write_text('')
            with sqlite3.connect(root/'runtime/ops.sqlite3') as c:c.execute('CREATE TABLE original(v)')
            archive=Path(temp)/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,Path(temp))
            with sqlite3.connect(root/'runtime/ops.sqlite3') as c:c.execute('CREATE TABLE catalog_items(id BLOB PRIMARY KEY)');c.execute('INSERT INTO catalog_items VALUES(?)',(b'i'*16,))
            with self.assertRaisesRegex(ValueError,'Item identities'):state_snapshot.guard_catalog_rollback(root,saved)
            with sqlite3.connect(root/'runtime/ops.sqlite3') as c:self.assertEqual(c.execute('SELECT count(*) FROM catalog_items').fetchone()[0],1)

    def test_snapshot_maps_custom_container_manifest_path_and_refuses_unknown_mount(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'instance';(root/'runtime/manifests/custom').mkdir(parents=True);(root/'config').mkdir()
            (root/'compose.env').write_text('')
            config=root/'config/api.json';config.write_text(json.dumps({'catalog':{'manifest_directory':'/loom/ops/manifests/custom'}}))
            chunk=root/'runtime/manifests/custom/chunk';chunk.write_text('sealed identities')
            archive=Path(temp)/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,Path(temp))
            chunk.write_text('corrupt');state_snapshot.restore(root,saved);self.assertEqual(chunk.read_text(),'sealed identities')
            config.write_text(json.dumps({'catalog':{'manifest_directory':'/unmapped/manifest'}}))
            with self.assertRaisesRegex(ValueError,'preservation plan'):state_snapshot.external_roots(root,Path(temp))

    def test_rollback_guard_compares_copy_identity_and_mutated_plan_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'instance';(root/'runtime').mkdir(parents=True);(root/'compose.env').write_text('')
            with sqlite3.connect(root/'runtime/ops.sqlite3') as db:
                db.execute('CREATE TABLE file_bindings(file BLOB,locator TEXT,evidence TEXT,PRIMARY KEY(file,locator))')
                db.execute('INSERT INTO file_bindings VALUES(?,?,?)',(b'i'*16,'one','proof-one'))
            archive=Path(temp)/'backup';archive.mkdir();saved=state_snapshot.capture(root,archive,Path(temp))
            with sqlite3.connect(root/'runtime/ops.sqlite3') as db:db.execute('INSERT INTO file_bindings VALUES(?,?,?)',(b'i'*16,'two','proof-two'))
            with self.assertRaisesRegex(ValueError,'Item identities'):state_snapshot.guard_catalog_rollback(root,saved)
