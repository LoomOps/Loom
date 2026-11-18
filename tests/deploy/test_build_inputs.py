"""Build inputs, inert entry points and target-qualified test selection."""
import importlib.util
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]

def module(path,name):
    spec=importlib.util.spec_from_file_location(name,ROOT/path)
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value)
    return value

class BuildInputTests(unittest.TestCase):
    def test_importing_image_builder_never_probes_or_changes_a_host(self):
        with patch('subprocess.run',side_effect=AssertionError('import side effect')):
            builder=module('deploy/build.py','isolated_image_builder')
        self.assertTrue(callable(builder.build))

    def test_snapshot_embeds_identity_without_mutating_the_checkout_or_selecting_an_instance(self):
        snapshot=module('deploy/snapshot.py','isolated_snapshot')
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'source';root.mkdir()
            for name in ('ops','dashboard'):
                (root/name).mkdir();(root/name/'build-info.json').write_text('existing local identity')
                (root/name/'source.py').write_text('# fixture\n')
            with patch.object(snapshot,'ROOT',root),patch.object(snapshot,'select',side_effect=AssertionError('implicit activation')):
                info=snapshot.capture(Path(directory)/'outputs')
            for name in ('ops','dashboard'):
                self.assertEqual((root/name/'build-info.json').read_text(),'existing local identity')
            with tarfile.open(Path(info['snapshot'])/'source.tar.gz') as archive:
                embedded=json.load(archive.extractfile('ops/build-info.json'))
            self.assertEqual(embedded['build_id'],info['build_id'])
            self.assertIn('source.py',embedded['files'])

    def test_dashboard_noop_preserves_mtime_and_failed_publication_keeps_previous_bytes(self):
        sys.path.insert(0,str(ROOT/'dashboard/workspace'))
        self.addCleanup(sys.path.pop,0)
        builder=module('dashboard/workspace/build.py','isolated_workspace_builder')
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'workspace.html'
            self.assertTrue(builder.write_output(path,'complete'))
            stamp=path.stat().st_mtime_ns
            self.assertFalse(builder.write_output(path,'complete'))
            self.assertEqual(path.stat().st_mtime_ns,stamp)
            with patch.object(builder.os,'replace',side_effect=OSError('publish failed')):
                with self.assertRaises(OSError):builder.write_output(path,'different')
            self.assertEqual(path.read_text(),'complete')
            self.assertEqual(list(path.parent.iterdir()),[path])

    def test_rust_mapping_allows_the_same_function_in_different_modules(self):
        runner=module('tests/rust/run.py','isolated_rust_runner')
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/'lib.rs').write_text('#[path="left.rs"] mod left;\n#[path="right.rs"] mod right;\n')
            for name in ('left','right'):(root/(name+'.rs')).write_text('#[test]\nfn same_name() {}\n')
            index=runner.test_index(root/'lib.rs')
            self.assertEqual(set(index),{'left::same_name','right::same_name'})
            self.assertNotEqual(index['left::same_name'],index['right::same_name'])
