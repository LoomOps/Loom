"""Validate every distributable file, including code outside imported test modules."""
import ast
import hashlib
import html
import json
from pathlib import Path
import re
import subprocess
import sys
import unittest
import yaml

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'deploy'))
import snapshot

class DistributionTests(unittest.TestCase):
    def test_all_distributed_sources_parse(self):
        files=list(snapshot.sources())
        self.assertGreater(len(files),180)
        for path in files:
            with self.subTest(path=str(path.relative_to(ROOT))):
                if path.suffix=='.py':ast.parse(path.read_text(),filename=str(path))
                elif path.suffix=='.json':json.loads(path.read_text())
                elif path.suffix in ('.yaml','.yml'):yaml.safe_load(path.read_text())
                elif path.suffix=='.sh':subprocess.run(['bash','-n',str(path)],check=True,capture_output=True)

    def test_licenses_travel_with_source_images_and_client_bundle(self):
        canonical=(ROOT/'LICENSE').read_bytes()
        self.assertIn(b'Version 3, 29 June 2007',canonical)
        for folder in ['dashboard','loom_use']:
            self.assertEqual((ROOT/folder/'LICENSE').read_bytes(),canonical)
            self.assertEqual((ROOT/folder/'THIRD_PARTY_NOTICES.md').read_bytes(),(ROOT/'THIRD_PARTY_NOTICES.md').read_bytes())
        paths={str(p.relative_to(ROOT)) for p in snapshot.sources()}
        self.assertTrue({'LICENSE','THIRD_PARTY_NOTICES.md','loom_use/LICENSE','dashboard/LICENSE'}<=paths)
        for dockerfile in ['ops/Dockerfile.loom','dashboard/Dockerfile']:
            self.assertIn('COPY LICENSE THIRD_PARTY_NOTICES.md /usr/share/doc/loom/',(ROOT/dockerfile).read_text())
        self.assertIn('GPL-3.0-only',(ROOT/'README.md').read_text())

    def test_current_ui_has_no_hidden_agent_application(self):
        for file in ['dashboard/workspace/template.html','dashboard/public/workspace.html']:
            s=html.unescape((ROOT/file).read_text())
            self.assertNotIn('function agentView(',s)
            self.assertNotIn('function agentDetail(',s)
        for file in ['dashboard/public/index.html','dashboard/public/access.html','dashboard/public/access-admin.html',
                     'deploy/native_identity.py','ops/install-profile.py','ops/delivery-adapter.py']:
            self.assertFalse((ROOT/file).exists(),file)

    def test_distributed_code_has_no_personal_operator_or_provider_integration(self):
        for path in snapshot.sources():
            if 'tests' in path.relative_to(ROOT).parts or path.name=='LICENSE':continue
            with self.subTest(path=str(path.relative_to(ROOT))):
                self.assertNotRegex(path.read_text(),r'(?i)\bpale(?:-dispatcher)?\b|register_oidc|import jwt|import PyJWT|external_oidc')

    def test_documentation_local_links_resolve(self):
        for path in snapshot.sources():
            if path.suffix!='.md':continue
            for target in re.findall(r'\]\(([^\s)]+)\)',path.read_text()):
                if ':' in target or target.startswith(('/', '#')) or '<' in target:continue
                ref=target.partition('#')[0]
                with self.subTest(document=str(path.relative_to(ROOT)),target=ref):
                    self.assertTrue((path.parent/ref).exists())

    def test_server_entry_delegates_to_external_client_installer(self):
        entry=(ROOT/'loom_use/entry.md').read_text()
        self.assertNotIn('Follow clients/README.md',entry)
        self.assertIn('https://github.com/LoomOps/Loom-Use/blob/main/INSTALL.md',entry)
