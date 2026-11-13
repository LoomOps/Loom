import copy,json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'deploy'))
import features

class FeatureCatalogTests(unittest.TestCase):
    def test_expected_failure_and_unexpected_success_are_never_recorded_as_passes(self):
        import importlib.util,io
        spec=importlib.util.spec_from_file_location('loom_test_result',ROOT/'tests/suite.py')
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        class Fixture(unittest.TestCase):
            @unittest.expectedFailure
            def test_known_failure(self):self.fail('known fixture failure')
            @unittest.expectedFailure
            def test_unexpected_success(self):pass
        result=unittest.TextTestRunner(stream=io.StringIO(),resultclass=module.Result).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(Fixture))
        statuses={case['name'].split('.')[-1]:case['status'] for case in result.records.values()}
        self.assertEqual(statuses,{'test_known_failure':'expected_failure','test_unexpected_success':'unexpected_success'})
        self.assertFalse(result.wasSuccessful())

    def test_verification_matrix_and_catalog_fingerprint_cover_all_blocks(self):
        import re
        c=features.load();ids={f['id'] for f in c['features']}
        text=(ROOT/'docs/features/VERIFICATION.md').read_text()
        rows=set(re.findall(r'^\|\s*([a-z][a-z-]+)\s*\|',text,re.M))
        self.assertEqual(rows,ids)
        testing=next(f for f in c['features'] if f['id']=='testing')
        self.assertIn('docs/features/catalog.json',testing['contracts'])

    def test_audit_returns_invalid_for_a_broken_catalog_instead_of_crashing(self):
        import tempfile
        catalog=features.load();catalog['features'][0]['tests'].append('tests/missing.py')
        with tempfile.TemporaryDirectory() as temp:
            result=features.audit(catalog,Path(temp))
        self.assertFalse(result['valid']);self.assertTrue(result['errors'])

    def test_failed_suite_summary_cannot_be_overridden_by_passed_case_rows(self):
        import tempfile,hashlib,ast
        c=features.load();f=next(f for f in c['features'] if f['id']=='ssh')
        paths=f['sources']+f['tests']+f['contracts']+[f['spec']]
        sources={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in paths}
        cases={}
        for path in f['tests']:
            suite='packaging' if Path(path).parts[1]=='deploy' else Path(path).parts[1]
            tree=ast.parse((ROOT/path).read_text())
            for cls in tree.body:
                if isinstance(cls,ast.ClassDef):
                    for fn in cls.body:
                        if isinstance(fn,ast.FunctionDef) and fn.name.startswith('test_'):
                            cases.setdefault(suite,[]).append({'name':Path(path).stem+'.'+cls.name+'.'+fn.name,'status':'passed'})
        with tempfile.TemporaryDirectory() as temp:
            for suite,rows in cases.items():
                (Path(temp)/(suite+'.json')).write_text(json.dumps({'successful':False,'sources':sources,'cases':rows}))
            block=next(b for b in features.audit(c,Path(temp))['blocks'] if b['id']=='ssh')
        self.assertFalse(block['passed']);self.assertTrue(block['suite_errors'])

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
