"""Real catalogs and immutable definitions; no host installation or Job execution."""
import copy,json
from unittest.mock import patch
from test_support import IsolatedOps,ops
import test_profile_inventory as fixture
import runtime_catalog,runtime_hub,runtime_store

class RuntimeHubTests(fixture.ProfileCatalogFixture):

    def test_display_curation_preserves_source_job_pins_and_permissions(self):
        kind=self.register();job=self.submit({**self.spec(),'kind':kind,'params':{}})
        before=self.row(job)['spec'];source=self.action('get',owner='owner',id='producer')
        args={'type':'profile','id':kind,'label':'Evaluate','categories':['evaluation'],'execution_modes':['full'],'tool_roles':{'planner':{'role':'Planner','stages':['runtime']}},'evidence':'Reviewed the evaluation entrypoint'}
        self.action('curate',**args)
        p=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(p['label'],'Evaluate');self.assertEqual(p['usage']['categories'],['evaluation']);self.assertEqual(p['tools'][0]['role']['stages'],['runtime'])
        self.assertEqual(before,self.row(job)['spec']);self.assertEqual(source,self.action('get',owner='owner',id='producer'))
        with self.assertRaises(PermissionError):ops.operate('components',{'action':'curate','arguments':args},self.reader,self.config)
        with ops.db() as c:c.execute("UPDATE node_control SET mode='maintenance'")
        with self.assertRaises(PermissionError):self.action('curate',**{**args,'label':'Blocked'})

    def test_family_preference_archive_and_unknown_bindings(self):
        first=self.register();second=self.register();rows=self.inventory();a,b=[next(p for p in rows if p['kind']==k) for k in (first,second)]
        self.assertEqual(a['family'],b['family'])
        self.action('curate',type='profile',id=first,preferred=True,evidence='Preferred validated revision')
        with self.assertRaises(ValueError):self.action('curate',type='profile',id=second,preferred=True,evidence='Cannot have two preferred revisions')
        with self.assertRaises(ValueError):self.action('curate',type='profile',id=first,state='archived',evidence='Cannot archive the preferred version')
        with self.assertRaises(ValueError):self.action('curate',type='profile',id=first,tool_roles={'missing':{'role':'X','stages':['runtime']}},evidence='Unknown alias')
        with self.assertRaises(ValueError):self.action('curate',type='profile',id='absent',label='Missing',evidence='Unknown profile')

    def test_inspection_and_comparison_are_read_only_and_do_not_leak_env_values(self):
        self.definition['params_schema']={'type':'object','properties':{'episodes':{'type':'integer','minimum':1}},'required':['episodes']}
        first=self.register();self.definition['runtime']['env']['PRIVATE']='SECOND_SECRET';second=self.register()
        with ops.db() as c:before=list(c.iterdump())
        checked=self.action('inspect_profile',kind=first,params={})
        self.assertEqual(checked['params']['status'],'invalid')
        self.assertEqual(self.action('inspect_profile',kind=first,params={'episodes':2})['params']['status'],'valid')
        diff=self.action('compare_profiles',left=first,right=second)
        self.assertFalse(diff['execution_equal']);self.assertTrue(any(x['path']=='runtime.env.PRIVATE' for x in diff['changes']))
        self.assertNotIn('SECOND_SECRET',json.dumps(diff));self.assertNotIn('DO_NOT_EXPOSE',json.dumps(checked))
        with ops.db() as c:self.assertEqual(before,list(c.iterdump()))

    def test_representative_dependencies_are_scoped_and_do_not_list_os_packages(self):
        m={'dependencies':[{'name':'Python','version':'3.12','scope':'image'},{'name':'numpy','version':'1.26','scope':'sim'},{'name':'numpy','version':'2.2','scope':'training'},{'name':'old','version':'1','kind':'system'}]}
        self.assertEqual([x['name'] for x in runtime_catalog.primary_dependencies(m)],['Python'])
        with self.assertRaises(ValueError):runtime_catalog.primary_dependencies(m,[{'name':'numpy'}],strict=True)
        self.assertEqual(runtime_catalog.primary_dependencies(m,[{'name':'numpy','scope':'sim'}],strict=True)[0]['version'],'1.26')
        self.action('curate',type='runtime',id='sim',revision=1,label='Simulator',primary_dependencies=[{'name':'sim'}],evidence='Primary simulation dependency')
        runtime=self.action('runtimes')['runtimes'][0]
        self.assertEqual(runtime['primary_dependencies'][0]['version'],'6.0')
        with self.assertRaises(ValueError):self.action('curate',type='runtime',id='sim',revision=1,primary_dependencies=[{'name':'not-installed'}],evidence='Missing package')

    def test_scoped_stage_requirements_and_current_observation_are_not_execution_proof(self):
        p={'prepare':{'runtime_pin':{'dependencies':[{'name':'numpy','version':'1','scope':'cpu'},{'name':'numpy','version':'2','scope':'train'}]}},'runtime':{'runtime_pin':{'dependencies':[{'name':'sim','version':'6'}]}}}
        m={'stage_requires':[{'stage':'prepare','name':'numpy','version':'1','scope':'cpu'}]}
        self.assertEqual(runtime_catalog.requirement_report(m,p)[0]['status'],'matched')
        m['stage_requires'][0].pop('scope');self.assertEqual(runtime_catalog.requirement_report(m,p)[0]['status'],'ambiguous')
        with self.assertRaises(ValueError):runtime_catalog.check_requirements(m,p)
        kind=self.register();job=self.submit({**self.spec(),'kind':kind,'params':{}})
        item=self.action('runtimes')['runtimes'][0];self.assertIn(job,item['used_by_jobs']);self.assertEqual(item['executed_jobs'],[])
        self.claim(job);item=self.action('runtimes')['runtimes'][0];self.assertEqual(item['executed_jobs'],[job])

    def test_stage_representatives_are_display_only_and_require_an_exact_inventory(self):
        kind=self.register();before=self.action('get',owner='owner',id='producer')
        self.action('curate',type='profile',id=kind,stage_dependencies={'runtime':[{'name':'sim'}]},evidence='Simulation is the representative run dependency')
        profile=next(p for p in self.inventory() if p['kind']==kind)
        run=next(s for s in profile['stages'] if s['stage']=='runtime')
        self.assertEqual(run['environment']['primary_dependencies'][0]['version'],'6.0')
        self.assertEqual(before,self.action('get',owner='owner',id='producer'))
        for stage,name in [('absent','sim'),('prepare','unrecorded'),('runtime','missing')]:
            with self.assertRaises(ValueError):self.action('curate',type='profile',id=kind,stage_dependencies={stage:[{'name':name}]},evidence='Invalid representative')

    def test_inspection_reports_runtime_availability_and_expected_actual_versions(self):
        kind=self.register();original=self.host
        def boundary(config,action,data,**kw):
            if action=='resolve_runtime':raise OSError('executor unavailable')
            return original(config,action,data,**kw)
        with patch('runtime_client.call',side_effect=boundary):report=self.action('inspect_profile',kind=kind,params={})
        self.assertTrue(any(r['check']=='runtime' and r['status']=='unavailable' for r in report['checks']))
        check=next(r for r in report['checks'] if r['check']=='dependency')
        self.assertEqual(check['version'],'6.0');self.assertEqual(check['actual_versions'],['6.0']);self.assertEqual(check['basis'],'pinned')
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM jobs').fetchone()[0],0)
