"""Real native migration of disposable legacy databases; no system service actions."""
from native_binaries import binary as native_binary
import hashlib,json,os,sqlite3,subprocess,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
class NativeMigrationTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='loom-native-migration-');self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name);self.state=self.root/'state';self.state.mkdir();self.data=self.root/'data';self.data.mkdir();self.components=self.root/'components';self.components.mkdir();(self.data/'.loom-storage.json').write_text('{"storage_id":"preserved-store"}')
  self.cfg=self.root/'config.json';self.cfg.write_text(json.dumps({'machine_id':'preserved-machine','storage_id':'preserved-store','actors':[]}));self.db=self.state/'ops.sqlite3'
  with sqlite3.connect(self.db) as db:
   db.executescript((ROOT/'rust/contracts/legacy-schema.sql').read_text());db.execute('PRAGMA user_version=4');db.execute("INSERT INTO node_control VALUES(1,'maintenance')");db.execute("INSERT INTO dispatch_control VALUES(1,0,NULL,NULL,NULL)");db.execute("INSERT INTO user_profiles(id,provider,login,display_name,updated) VALUES('owner','test','owner@example.test','Owner',1)")
  self.before=self.rows();self.backup=self.root/'backup'
 def rows(self):
  with sqlite3.connect(self.db) as db:return {n:db.execute('SELECT * FROM "'+n+'" ORDER BY rowid').fetchall() for n, in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")}
 def invoke(self,phase,*more,ok=True):
  result=subprocess.run([str(native_binary('loom-migrate')),phase,'--state',str(self.state),*map(str,more)],capture_output=True,text=True,timeout=20)
  self.assertEqual(result.returncode==0,ok,result.stderr);return json.loads(result.stdout) if ok else result
 def apply(self):return self.invoke('apply','--config',self.cfg,'--data',self.data,'--components',self.components,'--backup',self.backup,'--executor-root',self.root/'stopped-executor')
 def test_native_cutover_preserves_every_legacy_table_and_can_rollback(self):
  self.apply();after=self.rows();self.assertTrue(all(after[k]==v for k,v in self.before.items()));self.assertEqual(self.invoke('inspect')['schema'],5)
  self.assertEqual((self.backup/'ops.sqlite3').stat().st_mode&0o777,0o600)
  self.invoke('rollback','--receipt',self.backup/'receipt.json');self.assertEqual(self.rows(),self.before);self.assertEqual(self.invoke('inspect')['schema'],4)
 def test_native_rollback_refuses_new_user_records(self):
  self.apply()
  with sqlite3.connect(self.db) as db:db.execute("INSERT INTO user_profiles(id,provider,login,display_name,updated) VALUES('new','test','new@example.test','New',2)")
  self.invoke('rollback','--receipt',self.backup/'receipt.json',ok=False);self.assertEqual(self.invoke('inspect')['schema'],5);self.assertEqual(len(self.rows()['user_profiles']),2)
 def test_native_cutover_rejects_storage_identity_mismatch(self):
  (self.data/'.loom-storage.json').write_text('{"storage_id":"wrong"}');self.invoke('apply','--config',self.cfg,'--data',self.data,'--components',self.components,'--backup',self.backup,'--executor-root',self.root/'stopped-executor',ok=False);self.assertEqual(self.rows(),self.before)

 def test_interrupted_receipt_write_can_resume_without_a_second_cutover(self):
  self.apply();after=self.rows();(self.backup/'receipt.json').unlink()
  self.assertTrue(self.apply()['applied']);self.assertEqual(self.rows(),after)
  with sqlite3.connect(self.db) as db:db.execute("INSERT INTO user_profiles(id,provider,login,display_name,updated) VALUES('later','test','later@example.test','Later',3)")
  self.invoke('apply','--config',self.cfg,'--data',self.data,'--components',self.components,'--backup',self.backup,'--executor-root',self.root/'stopped-executor',ok=False)
  self.assertEqual(len(self.rows()['user_profiles']),2)

 def test_catalog_boundary_preserves_native_rows_and_refuses_rollback_after_ids(self):
  self.apply();original=self.rows();backup=self.root/'catalog-backup'
  self.invoke('catalog-install','--config',self.cfg,'--data',self.data,'--components',self.components,'--backup',backup)
  after=self.rows();self.assertTrue(all(after[k]==v for k,v in original.items()));self.assertEqual(self.invoke('inspect')['schema'],6);self.assertEqual((backup/'ops.sqlite3').stat().st_mode&0o777,0o600)
  self.invoke('catalog-backfill','--config',self.cfg,'--data',self.data,'--components',self.components)
  self.invoke('rollback','--receipt',backup/'receipt.json',ok=False)
  self.assertEqual(self.invoke('inspect')['schema'],6)
  self.assertTrue((self.state/'search/catalog.sqlite3').is_file())
  self.invoke('catalog-rebuild','--config',self.cfg,'--data',self.data,'--components',self.components)
  rebuilt=self.rows();self.assertTrue(all(rebuilt[k]==v for k,v in original.items()))

 def test_reconciliation_schema_preserves_old_authority_and_resumes_completed_boundary(self):
  self.apply()
  self.invoke('catalog-install','--config',self.cfg,'--data',self.data,'--components',self.components,'--backup',self.root/'catalog-backup')
  self.invoke('catalog-backfill','--config',self.cfg,'--data',self.data,'--components',self.components)
  before=self.rows()
  args=('--config',self.cfg,'--data',self.data,'--components',self.components,'--backup',self.root/'reconcile-backup')
  self.invoke('reconcile-install',*args)
  self.assertEqual(self.invoke('inspect')['schema'],7)
  after=self.rows()
  for table,rows in before.items():
   self.assertTrue(all(row in after[table] for row in rows),table)
  self.assertFalse(self.invoke('reconcile-install',*args)['applied'])
  self.invoke('catalog-backfill','--config',self.cfg,'--data',self.data,'--components',self.components)
  self.assertEqual(self.invoke('inspect')['schema'],7)

  # A completed receipt remains idempotent after legitimate projection progress.
  self.assertFalse(self.invoke('reconcile-install',*args)['applied'])
