import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import release_check_setup

class ReleaseSetupTests(unittest.TestCase):
    def test_private_configuration_is_protected_and_preserved(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);(root/'runtime').mkdir();(root/'secrets').mkdir();(root/'compose.env').write_text('fixture')
            result=release_check_setup.configure(root,token='fixture-secret')
            self.assertNotIn('fixture-secret',json.dumps(result));self.assertTrue(result['credential_present'])
            secret=root/'secrets/releases/github-token';self.assertEqual(secret.stat().st_mode & 0o777,0o600)
            self.assertNotIn('fixture-secret',(root/'release-check.env').read_text())
            release_check_setup.configure(root,enabled=False)
            self.assertEqual(secret.read_text(),'fixture-secret\n');self.assertIn('LOOM_RELEASE_CHECK_ENABLED=0',(root/'release-check.env').read_text())
            with self.assertRaises(ValueError):release_check_setup.configure(root,repository='x/y\nSECRET=1')
            self.assertEqual(secret.read_text(),'fixture-secret\n')

    def test_not_an_installed_instance_is_rejected(self):
        with tempfile.TemporaryDirectory() as t,self.assertRaises(ValueError):release_check_setup.configure(Path(t))
