"""Privacy regression for distributable template data, not a secret-audit guarantee."""
import html
import json
from pathlib import Path
import re
import unittest
ROOT=Path(__file__).resolve().parents[2]

class PublicSourceTests(unittest.TestCase):
    def payload(self,file,marker):
        s=html.unescape((ROOT/file).read_text())
        start=s.index(marker)+len(marker)
        return json.JSONDecoder().raw_decode(s[start:])[0]

    def test_template_and_generated_workspace_start_without_embedded_records(self):
        for file in ['dashboard/workspace/template.html','dashboard/public/workspace.html']:
            with self.subTest(file=file):
                text=html.unescape((ROOT/file).read_text())
                self.assertIn('let catalogData=catTitleSnapshot({entities:[],sources:[],revision:null});',text)
                self.assertNotIn('const metricSnapshot=',text)
                self.assertNotIn('window.openai',text)
                self.assertNotIn('FloatingUIDOM',text)
                self.assertNotIn('Example RGBD',text)

    def test_template_excludes_private_operational_documents(self):
        template=(ROOT/'dashboard/workspace/template.html').read_text()
        self.assertNotRegex(template,r'agent-doc:remote:')
        self.assertNotRegex(template,r'/home/(?!developer/)[^/]+/\.codex/')

    def test_distributable_documentation_is_english(self):
        # UI localization and multilingual test fixtures are intentionally outside this boundary.
        files = list(ROOT.glob('*.md'))
        for folder in ('docs', 'loom_use', 'deploy', 'ops', 'dashboard', 'executor', 'integrations', 'tests'):
            files.extend(p for p in (ROOT/folder).rglob('*.md')
                         if not {'evidence', '__pycache__'} & set(p.relative_to(ROOT).parts))
        files.append(ROOT/'docs/features/catalog.json')
        self.assertGreater(len(files), 40)
        for path in files:
            with self.subTest(path=str(path.relative_to(ROOT))):
                self.assertNotRegex(path.read_text(), r'[\uac00-\ud7a3]')
