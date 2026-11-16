"""Catch documentation omissions and broken navigation without production access."""
import importlib.util,json,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('loom_documentation',ROOT/'deploy/documentation.py');documentation=importlib.util.module_from_spec(spec);spec.loader.exec_module(documentation)

class DocumentationTests(unittest.TestCase):
 def test_current_inventory_and_all_local_links(self):
  self.assertEqual(documentation.check(),[])
 def test_unclassified_new_document_changes_the_inventory(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);(root/'README.md').write_text('# Entry\n')
   before=documentation.inventory(root)
   (root/'new-guide.md').write_text('# New task\n')
   self.assertNotEqual(before,documentation.inventory(root))
 def test_local_links_are_checked_but_code_and_external_urls_are_not_paths(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);(root/'README.md').write_text('[missing](guide.md)\n[external](https://example.test/path)\n```text\n[example](not-a-doc.md)\n```\n')
   self.assertEqual(documentation.local_link_errors(root,['README.md']),['README.md: missing local link guide.md'])
   (root/'guide.md').write_text('# Guide\n')
   self.assertEqual(documentation.local_link_errors(root,['README.md']),[])
 def test_all_agent_topics_resolve_from_one_catalog(self):
  c=json.loads((ROOT/'loom_use/catalog.json').read_text());ids={d['id'] for d in c['documents']}
  self.assertEqual(len(ids),len(c['documents']))
  for document in c['documents']:
   self.assertTrue((ROOT/document['source']).is_file())


 def test_action_heading_links_preserve_underscores_and_detect_missing_anchors(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);(root/'README.md').write_text('# API\n[workflow](#submit_workflow)\n## submit_workflow\n')
   self.assertEqual(documentation.local_link_errors(root,['README.md']),[])
   (root/'README.md').write_text('# API\n[workflow](#submit-workflow)\n## submit_workflow\n')
   self.assertTrue(documentation.local_link_errors(root,['README.md']))
