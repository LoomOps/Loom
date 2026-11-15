"""Native helper bytes must be bound to the same immutable Ops image as the release."""
import hashlib,json,tempfile,unittest
from pathlib import Path
import native_artifacts
class NativeArtifactTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name);(self.root/'source').mkdir();(self.root/'source/Cargo.toml').write_text('[workspace]\n');native=self.root/'native';native.mkdir();files={}
  for name in native_artifacts.BINARIES:(native/name).write_bytes(name.encode());files[name]=hashlib.sha256(name.encode()).hexdigest()
  (native/'manifest.json').write_text(json.dumps({'schema':'loom-native-artifacts/v1','image':'sha256:fixture','files':files}));(self.root/'result.json').write_text(json.dumps({'native':files,'image_ids':{'ops':'sha256:fixture'}}))
 def test_native_helper_matches_the_verified_image_and_result_receipt(self):
  self.assertEqual(native_artifacts.verify_release(self.root)['image'],'sha256:fixture')
  (self.root/'native/loom-host').write_bytes(b'changed');self.assertRaises(ValueError,native_artifacts.verify_release,self.root)
 def test_resealing_a_helper_manifest_cannot_change_the_release_receipt(self):
  path=self.root/'native/manifest.json';manifest=json.loads(path.read_text());manifest['image']='another-image';path.write_text(json.dumps(manifest));self.assertRaisesRegex(ValueError,'immutable Ops',native_artifacts.verify_release,self.root)

 def test_schema_change_without_a_cutover_receipt_is_not_a_completed_migration(self):
  import importlib.util,sqlite3
  script=Path(__file__).resolve().parents[2]/'deploy/migrations/rust_authority.py'
  spec=importlib.util.spec_from_file_location('native_cutover_candidate',script);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
  instance=self.root/'instance';runtime=instance/'runtime';runtime.mkdir(parents=True)
  with sqlite3.connect(runtime/'ops.sqlite3') as db:db.execute('PRAGMA user_version=5')
  backup=runtime/'native-cutover-v1/before';backup.mkdir(parents=True);(backup/'intent.json').write_text('{}')
  self.assertEqual(module.run(instance,self.root/'platform','check')['status'],'needed')
  (backup/'receipt.json').write_text('{}')
  self.assertRaisesRegex(ValueError,'receipt',module.run,instance,self.root/'platform','check')
  (backup/'receipt.json').write_text(json.dumps({'schema':'loom-rust-migration/v1','state':str(runtime),'before':{'schema':4},'after':{'schema':5}}))
  self.assertEqual(module.run(instance,self.root/'platform','check')['status'],'done')
