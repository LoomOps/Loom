"""Resident setup stores only verified tokens and preserves unrelated Codex settings."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tomllib
from unittest.mock import patch
import test_service_tokens
from test_support import IsolatedOps
CLIENTS=Path(__file__).resolve().parents[2]/'loom_use/clients'
sys.path.insert(0,str(CLIENTS))
spec=importlib.util.spec_from_file_location('loom_client_setup',CLIENTS/'setup.py')
setup=importlib.util.module_from_spec(spec);spec.loader.exec_module(setup)

class ClientSetupTests(IsolatedOps):
    request=test_service_tokens.ServiceTokenTests.request
    close_server=test_service_tokens.ServiceTokenTests.close_server
    signed=test_service_tokens.ServiceTokenTests.signed
    issue=test_service_tokens.ServiceTokenTests.issue
    def setUp(self):
        test_service_tokens.ServiceTokenTests.setUp(self);self.folder=self.root/'private';self.configpath=self.root/'codex'/'config.toml'
        self.configpath.parent.mkdir();self.configpath.write_text('model = "example"\n[mcp_servers.other]\nurl = "https://other.test/mcp"\n')
    def test_real_mcp_setup_permissions_idempotence_and_revocation(self):
        issued=self.issue();before=tomllib.loads(self.configpath.read_text())
        result=setup.configure(self.folder,self.configpath,'loom-test',self.base+'/mcp',issued['token'])
        self.assertEqual(result['owner'],self.person['id']);self.assertEqual(len(result['tools']),6)
        self.assertNotIn(issued['token'],json.dumps(result));self.assertNotIn(issued['token'],self.configpath.read_text())
        self.assertEqual(self.folder.stat().st_mode & 0o777,0o700)
        self.assertEqual((self.folder/'credential.json').stat().st_mode & 0o777,0o600)
        current=tomllib.loads(self.configpath.read_text());self.assertEqual(current['model'],before['model']);self.assertEqual(current['mcp_servers']['other'],before['mcp_servers']['other'])
        setup.configure(self.folder,self.configpath,'loom-test',self.base+'/mcp',issued['token'])
        self.assertEqual(tomllib.loads(self.configpath.read_text()),current)
        self.assertEqual(setup.verify(self.folder)['status'],'verified')
        self.request('/api/admin/tokens/revoke',{'id':issued['credential_id']},token=None,headers=self.signed())
        with self.assertRaises(OSError):setup.verify(self.folder)
    def test_invalid_token_and_bootstrap_never_written(self):
        before=self.configpath.read_bytes()
        for token in ['bad','fixture-owner']:
            with self.assertRaises((OSError,ValueError)):setup.configure(self.folder,self.configpath,'loom-test',self.base+'/mcp',token)
        self.assertFalse(self.folder.exists());self.assertEqual(self.configpath.read_bytes(),before)
    def test_config_collision_nested_headers_and_unrelated_values(self):
        expected={'url':'https://loom.test/mcp','http_headers_helper':'python3 helper'}
        before='model="model"\n[mcp_servers.loom]\nurl="https://loom.test/mcp"\n[mcp_servers.loom.http_headers]\nAuthorization="old"\n[mcp_servers.other]\nurl="https://other.test"\n'
        changed=setup.config_text(before,'loom',expected)
        self.assertEqual(tomllib.loads(changed)['mcp_servers']['loom'],expected)
        self.assertIn('url="https://other.test"',changed)
        with self.assertRaises(ValueError):setup.config_text(before,'other',expected)
    def test_unsafe_permissions_symlinks_and_http_endpoint_rejected(self):
        self.folder.mkdir(mode=0o755)
        with self.assertRaises(PermissionError):setup.private_directory(self.folder)
        secret=self.root/'secret';secret.write_text('{"token":"test"}');secret.chmod(0o644)
        with self.assertRaises(PermissionError):setup.read_token(secret)
        link=self.root/'link';link.symlink_to(secret)
        with self.assertRaises(OSError):setup.read_token(link)
        with self.assertRaises(ValueError):setup.configure(self.root/'safe',self.configpath,'loom-test','http://remote.test/mcp','test')
