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

    def test_empty_test_reference_and_invalid_interface_are_detected(self):
        c=features.load();c['features'][0]['tests'].append('tests/ops/client_source.py')
        c['features'][0]['interfaces'][0]['entrypoint']='dashboard/sso.py#missing_function'
        errors='\n'.join(features.check(c))
        self.assertIn('no discoverable tests',errors);self.assertIn('missing interface symbol',errors)

    def test_audit_rejects_missing_execution_stale_source_and_skips(self):
        import tempfile,hashlib
        with tempfile.TemporaryDirectory() as t:
            evidence=Path(t);c=features.load();f=next(f for f in c['features'] if f['id']=='ssh')
            empty=features.audit(c,evidence);block=next(b for b in empty['blocks'] if b['id']=='ssh')
            self.assertFalse(block['passed']);self.assertTrue(block['missing'])
            paths=f['sources']+f['tests']+f['contracts']+[f['spec']]
            sources={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in paths}
            # A green summary with no actual test cases cannot satisfy a block.
            (evidence/'packaging.json').write_text(json.dumps({'successful':True,'sources':sources,'cases':[]}))
            self.assertFalse(next(b for b in features.audit(c,evidence)['blocks'] if b['id']=='ssh')['passed'])
            (evidence/'ops.json').write_text(json.dumps({'sources':{},'cases':[{'name':'test_ssh_info.SSHInfoTests.test_per_principal_and_no_fake_permission_on_missing_data','status':'skipped'}]}))
            block=next(b for b in features.audit(c,evidence)['blocks'] if b['id']=='ssh')
            self.assertTrue(block['not_passed']);self.assertTrue(block['stale_or_unrecorded'])
