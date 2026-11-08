import copy,importlib.util,json,subprocess,tempfile,unittest
from pathlib import Path
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('kit_setup',ROOT/'deploy/installer/setup.py');setup=importlib.util.module_from_spec(spec);spec.loader.exec_module(setup)
spec=importlib.util.spec_from_file_location('kit_builder',ROOT/'deploy/install-kit.py');builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)

class InstallKitTests(unittest.TestCase):
 def test_default_plan_needs_no_research_dependency(self):
  p=setup.validate(json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text()));self.assertEqual(p['repositories'],[]);self.assertFalse(p['gpu']);self.assertIsNone(p['mount'])

 def test_paths_cannot_hide_existing_data_or_inject_env(self):
  for name,value in [('data','/etc/new-data'),('data','/storage/../etc/new'),('data','/storage/a\nLOOM_DEV_NO_AUTH=1'),('dependencies','/loom/components'),('dependencies','/storage/loom/data/cache')]:
   with self.subTest(name=name,value=value):
    p=json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text());p[name]=value
    with self.assertRaises(ValueError):setup.validate(p)
  with tempfile.TemporaryDirectory() as t:
   target=Path(t)/'link';target.symlink_to('/tmp');p=json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text());p['data']=str(target/'data')
   with self.assertRaises(ValueError):setup.validate(p)

 def test_repository_must_be_pinned_and_credential_free(self):
  p=json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text());p['repositories']=[{'name':'example-library','url':'https://example.org/lab.git','commit':'a'*40}]
  setup.validate(p)
  for key,value in [('url','https://token@example.org/lab.git'),('url','file:///tmp/repo'),('commit','main'),('name','../escape')]:
   bad=copy.deepcopy(p);bad['repositories'][0][key]=value
   with self.subTest(key=key),self.assertRaises(ValueError):setup.validate(bad)

 def test_mount_is_existing_filesystem_with_separate_dependency_path(self):
  p=json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text());p['mount']={'uuid':'12345678-abcd','target':'/storage','fstype':'ext4'};setup.validate(p)
  p['mount']['target']='/dependencies'
  with self.assertRaises(ValueError):setup.validate(p)

 def test_bundle_tampering_and_path_escape_rejected(self):
  with tempfile.TemporaryDirectory() as t,patch.object(setup,'BUNDLE',Path(t)):
   p=Path(t)/'setup.py';p.write_text('fixture')
   m={'schema':'loom-install-kit/v1','files':{'setup.py':setup.sha(p)}}
   (Path(t)/'manifest.json').write_text(json.dumps(m));setup.manifest()
   p.write_text('changed')
   with self.assertRaises(ValueError):setup.manifest()
   m['files']={'../outside':'0'*64};(Path(t)/'manifest.json').write_text(json.dumps(m))
   with self.assertRaises(ValueError):setup.manifest()

 def test_mount_failure_restores_fstab_without_formatting(self):
  with tempfile.TemporaryDirectory() as t:
   f=Path(t)/'fstab';f.write_text('# existing\n');m={'uuid':'12345678-abcd','target':str(Path(t)/'volume'),'fstype':'ext4'}
   with patch.object(setup,'FSTAB',f),patch.object(setup,'mount_preflight'),patch.object(setup,'run',side_effect=subprocess.CalledProcessError(1,['mount'])) as run:
    with self.assertRaises(subprocess.CalledProcessError):setup.mount_volume(m)
   self.assertEqual(f.read_text(),'# existing\n');self.assertEqual(len(list(Path(t).glob('fstab.loom-*'))),1)
   run.assert_called_once_with('mount',m['target'])

 def test_mount_rejects_fstab_conflict_before_mutation(self):
  with tempfile.TemporaryDirectory() as t:
   f=Path(t)/'fstab';f.write_text('UUID=12345678-abcd /old ext4 defaults 0 2\n')
   m={'uuid':'12345678-abcd','target':str(Path(t)/'volume'),'fstype':'ext4'}
   with patch.object(setup,'FSTAB',f),patch.object(setup,'run',side_effect=['/dev/test','ext4']),patch.object(setup.subprocess,'run',return_value=subprocess.CompletedProcess([],1,'')):
    with self.assertRaisesRegex(ValueError,'fstab'):setup.mount_preflight(m)
   self.assertFalse(Path(m['target']).exists())

 def test_host_bundle_contains_local_import_dependencies(self):
  import ast
  paths={ROOT/d/n for d,names in builder.HOST_FILES.items() for n in names}
  names={p.stem for p in paths}
  local={p.stem for d in ['deploy','ops','executor'] for p in (ROOT/d).glob('*.py')}
  for p in paths:
   self.assertTrue(p.is_file(),str(p))
   if p.suffix!='.py':continue
   for node in ast.walk(ast.parse(p.read_text())):
    modules=[a.name.split('.')[0] for a in node.names] if isinstance(node,ast.Import) else [node.module.split('.')[0]] if isinstance(node,ast.ImportFrom) and node.module else []
    for m in modules:
     if m in local:self.assertIn(m,names,f'{p.name} requires {m}')

 def test_wrong_architecture_refused_before_docker_or_host_changes(self):
  with patch.object(setup.os,'geteuid',return_value=0),patch.object(setup.Path,'is_dir',return_value=True),patch.object(setup.platform,'machine',return_value='aarch64'),patch.object(setup,'run') as run:
   with self.assertRaisesRegex(ValueError,'architecture'):setup.preflight(json.loads((ROOT/'deploy/installer/install-plan.example.json').read_text()),{'architecture':'amd64'})
   run.assert_not_called()
