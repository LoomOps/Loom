"""Exact environment pins, real separate SQLite and existing Job integration."""
import copy
import json
from pathlib import Path
import sys
from unittest.mock import patch
from test_support import IsolatedOps, ops

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'executor'))
import runtime_store
import runtime_catalog


class RuntimeCatalogTests(IsolatedOps):
    def setUp(self):
        super().setUp()
        import component_store,profile_store
        for module,name in ((component_store,'components'),(profile_store,'profiles')):
            p=patch.object(module,'ROOT',self.root/name);p.start();self.addCleanup(p.stop)
        self.catalog=self.root/'runtime-catalog';self.catalog.mkdir()
        self.manifest={'label':'Simulation','dependencies':[{'name':'sim','version':'6.0','kind':'python'}],
            'runtime':{'image':'sha256:'+'a'*64,'network':'none'},'validation':{'status':'passed','command':['python3','smoke.py'],'gpu':True}}
        self.ref=runtime_store.register(self.catalog,'simulation',self.manifest,['alpha'])
        self.config['executor']={'socket':'fixture'}
        self.config['runtime_ids']=['python']
        self.definition={'params_schema':{'type':'object'},'inputs':{},'outputs':{},
            'runtime':{'executor':'container','runtime_ref':{'id':'simulation','revision':1},'command':['python3','/component/run.py']},
            'limits':{'timeout_seconds':30,'max_attempts':1}}

    def host_call(self,config,action,data,**kw):
        if action=='resolve_runtime':
            item=runtime_store.detail(self.catalog,'alpha',data['runtime_ref'])
            return {'runtime_ref':{k:item[k] for k in ('id','revision','sha256')},'manifest':item['manifest']}
        if action=='list_runtimes':
            return runtime_store.listing(self.catalog,{'instances':{'alpha':{'runtimes':[]}},'runtimes':{}},'alpha')
        raise AssertionError(action)

    def action(self,action,**args):
        return ops.operate('components',{'action':action,'arguments':args},self.owner,self.config)

    def test_separate_database_grants_integrity_and_idempotency(self):
        self.assertTrue((self.catalog/'runtimes.sqlite3').is_file())
        self.assertEqual(runtime_store.register(self.catalog,'simulation',self.manifest,['alpha']),self.ref)
        with self.assertRaises(PermissionError):runtime_store.detail(self.catalog,'other',self.ref)
        changed=copy.deepcopy(self.manifest);changed['runtime']['image']='sha256:'+'b'*64
        second=runtime_store.register(self.catalog,'simulation',changed,['alpha'])
        self.assertEqual(second['revision'],2)
        self.assertEqual(runtime_store.detail(self.catalog,'alpha',self.ref)['manifest']['runtime']['image'],'sha256:'+'a'*64)
        with runtime_store.connect(self.catalog,True) as c:c.execute("UPDATE runtime_revisions SET manifest='{}' WHERE revision=1")
        with self.assertRaisesRegex(ValueError,'integrity'):runtime_store.detail(self.catalog,'alpha',self.ref)

    def test_profile_and_job_freeze_runtime_and_usage_is_bidirectional(self):
        with patch('runtime_client.call',side_effect=self.host_call):
            registered=self.action('register_profile',id='sim-run',profile=self.definition)
            job=self.submit({**self.spec(),'kind':registered['kind'],'params':{}})
            changed=copy.deepcopy(self.manifest);changed['runtime']['image']='sha256:'+'b'*64
            runtime_store.register(self.catalog,'simulation',changed,['alpha'])
            saved=json.loads(self.row(job)['spec'])['profile']['runtime']['runtime_pin']
            self.assertEqual(saved['sha256'],self.ref['sha256'])
            self.assertEqual(saved['image'],'sha256:'+'a'*64)
            listing=self.action('runtimes')
            old=next(r for r in listing['runtimes'] if r['revision']==1)
            self.assertEqual(old['used_by_jobs'],[job])
            self.assertEqual(old['used_by_profiles'][0]['kind'],registered['kind'])
            item=self.action('get_runtime',id='simulation',revision=1)
            self.assertEqual(item['sha256'],saved['sha256'])
            with ops.db() as c:
                import dashboard_read
                projection,_=dashboard_read.projection(c,self.owner,self.config,self.local)
            self.assertTrue(any(e['id']=='runtime:simulation:1' for e in projection['entities']))

    def test_unknown_runtime_spoofed_pin_and_dependency_mismatch_rejected(self):
        with patch('runtime_client.call',side_effect=self.host_call):
            bad=copy.deepcopy(self.definition);bad['runtime']['runtime_ref']['revision']=99
            with self.assertRaises(PermissionError):self.action('register_profile',id='bad',profile=bad)
            bad=copy.deepcopy(self.definition);bad['runtime']['runtime_pin']=self.ref
            with self.assertRaises(ValueError):self.action('register_profile',id='bad',profile=bad)
            tool=self.action('register_tool',id='planner',manifest={'requires':{'sim':'5.1'}},files={'planner.py':'pass'})
            bad=copy.deepcopy(self.definition);bad['tools']={'planner':{'id':'planner','revision':tool['revision']}}
            with self.assertRaisesRegex(ValueError,'dependency'):self.action('register_profile',id='bad',profile=bad)
            self.assertEqual([p['kind'] for p in self.action('profiles')],['cpu.smoke'])

    def test_discovery_is_read_only_and_legacy_is_not_verified(self):
        cfg={'instances':{'alpha':{'runtimes':['python']}},'runtimes':{'python':{'image':'python:3.12'}}}
        rows=runtime_store.listing(self.catalog,cfg,'alpha')['runtimes']
        legacy=next(x for x in rows if x['legacy'])
        self.assertIsNone(legacy['revision']);self.assertEqual(legacy['manifest']['validation']['status'],'unrecorded')
        missing=self.root/'missing';missing.mkdir()
        runtime_store.listing(missing,cfg,'alpha')
        self.assertFalse((missing/'runtimes.sqlite3').exists())
        with patch('runtime_client.call',side_effect=OSError('offline')):
            self.assertEqual(self.action('runtimes')['status'],'unavailable')

    def test_failed_installation_changes_dashboard_revision_without_creating_a_job(self):
        import dashboard_read
        def projection():
            with ops.db() as c:return dashboard_read.projection(c,self.owner,self.config,self.local)[0]
        with patch('runtime_client.call',side_effect=self.host_call):
            runtime_store.installation(self.catalog,'install','simulation','building',{'instance':'alpha','path':'/chosen/path'})
            before=projection()
            runtime_store.installation(self.catalog,'install','simulation','failed',{'instance':'alpha','path':'/chosen/path'})
            after=projection()
        self.assertNotEqual(before['revision'],after['revision'])
        self.assertEqual(after['runtime_catalog']['installations'][0]['state'],'failed')
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM jobs').fetchone()[0],0)

    def test_reconciled_alias_is_readonly_and_metadata_drift_cannot_bind(self):
        policy={'image':'python:tag','network':'none'}
        metadata=self.root/'METADATA';metadata.write_text('Version: 1.0')
        observed={**self.manifest,'reconciliation':{'runtime_id':'python','policy_sha256':runtime_store.digest(policy),
            'image_id':'sha256:'+'a'*64,'source_pins':{str(metadata):__import__('hashlib').sha256(metadata.read_bytes()).hexdigest()}}}
        ref=runtime_store.register(self.catalog,'inventory-python',observed,['alpha'])
        config={'instances':{'alpha':{'runtimes':['python']}},'runtimes':{'python':policy}}
        listing=runtime_store.listing(self.catalog,config,'alpha',image_identity=lambda _:observed['runtime']['image'])
        item=next(i for i in listing['runtimes'] if i['id']=='inventory-python')
        self.assertEqual(item['aliases'],['python'])
        env=runtime_catalog.observed_environment({'runtime_id':'python'},listing)
        self.assertEqual(env['type'],'observed');self.assertEqual(env['dependencies'][0]['version'],'6.0')
        self.assertEqual(policy,{'image':'python:tag','network':'none'})
        extra=self.root/'EXTRA';extra.write_text('Version: 1')
        broader=copy.deepcopy(observed);broader['reconciliation']['source_pins'][str(extra)]=__import__('hashlib').sha256(extra.read_bytes()).hexdigest()
        runtime_store.register(self.catalog,'inventory-python',broader,['alpha'])
        extra.write_text('Version: 2')
        changed=runtime_store.listing(self.catalog,config,'alpha',image_identity=lambda _:observed['runtime']['image'])
        self.assertIsNone(runtime_catalog.observed_environment({'runtime_id':'python'},changed))
        metadata.write_text('Version: 2.0')
        with self.assertRaisesRegex(ValueError,'metadata changed'):runtime_store.detail(self.catalog,'alpha',ref,verify_inventory=True)
        listing=runtime_store.listing(self.catalog,config,'alpha',image_identity=lambda _:observed['runtime']['image'])
        self.assertTrue(any(i.get('legacy') and i['id']=='python' for i in listing['runtimes']))
        self.assertIsNone(runtime_catalog.observed_environment({'runtime_id':'python'},listing))
