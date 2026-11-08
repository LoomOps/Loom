import copy,json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'deploy'))
import features

class FeatureCatalogTests(unittest.TestCase):
    def test_all_current_sources_tests_and_links_have_a_feature(self):
        import jsonschema
        c=features.load();schema=json.loads((ROOT/'docs/features/catalog.schema.json').read_text())
        jsonschema.Draft202012Validator(schema).validate(c)
        self.assertEqual(features.check(c),[])

    def test_invalid_dependency_duplicate_ownership_and_missing_file_fail(self):
        c=features.load();c['features'][0]['depends_on'].append('nonexistent')
        c['features'][1]['sources'].append(c['features'][0]['sources'][0])
        c['features'][0]['contracts'].append('missing-file')
        errors='\n'.join(features.check(c))
        self.assertIn('invalid dependency',errors);self.assertIn('Ambiguous ownership',errors);self.assertIn('missing/unsafe',errors)

    def test_identity_change_reaches_dashboard_api_network_and_tests(self):
        report=features.impact(['dashboard/sso.py'],features.load())
        self.assertIn('identity',report['direct'])
        for f in ['api','dashboard','network']:self.assertIn(f,report['review'])
        self.assertIn('tests/dashboard/test_sso.py',report['tests'])
        self.assertEqual(features.impact(['new_component.py'],features.load())['unmapped'],['new_component.py'])
