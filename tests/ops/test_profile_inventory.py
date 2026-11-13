"""Profile combinations and attribution use real catalogs; no research execution."""
import copy
import json
from pathlib import Path
import sys
from unittest.mock import patch

from test_support import IsolatedOps,ops
import component_store,profile_store,profile_inventory,ownership

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'executor'))
import runtime_store


class ProfileCatalogFixture(IsolatedOps):
    def setUp(self):
        super().setUp()
        for module,name in ((component_store,'components'),(profile_store,'profiles')):
            p=patch.object(module,'ROOT',self.root/name);p.start();self.addCleanup(p.stop)
        self.catalog=self.root/'catalog';self.catalog.mkdir()
        self.manifest={'label':'Sim 6','dependencies':[{'name':'sim','version':'6.0'}],
            'runtime':{'image':'sha256:'+'a'*64},'validation':{'status':'passed'}}
        self.runtime=runtime_store.register(self.catalog,'sim',self.manifest,['alpha'])
        self.config.update(executor={'socket':'fixture'},runtime_ids=['python'])
        patcher=patch('runtime_client.call',side_effect=self.host);patcher.start();self.addCleanup(patcher.stop)
        self.tool=self.action('register_tool',id='planner',manifest={'label':'Planner old','requires':{'sim':'6.0'}},files={'plan.py':'pass'})
        self.definition={'usage':{'label':'Generate and evaluate','categories':['sdg','evaluation'],'execution_modes':['pilot','full']},
            'tools':{'planner':{'id':'planner','revision':1}},'params_schema':{'type':'object'},'inputs':{},'outputs':{},
            'runtime':{'executor':'container','runtime_ref':{'id':'sim','revision':1},'command':['python3','/component/run.py'],'env':{'PRIVATE':'DO_NOT_EXPOSE'}},
            'prepare':{'executor':'container','runtime_id':'python','command':['python3','/component/run.py']},
            'limits':{'timeout_seconds':30,'max_attempts':1}}

    def host(self,config,action,data,**kw):
        if action=='list_runtimes':return runtime_store.listing(self.catalog,{'instances':{'alpha':{'runtimes':[]}},'runtimes':{}},'alpha')
        if action!='resolve_runtime':raise AssertionError(action)
        value=runtime_store.detail(self.catalog,'alpha',data['runtime_ref'])
        return {'runtime_ref':{k:value[k] for k in ('id','revision','sha256')},'manifest':value['manifest']}

    def action(self,action,**arguments):
        return ops.operate('components',{'action':action,'arguments':arguments},self.owner,self.config)

    def register(self):
        return self.action('register',id='producer',manifest={'profiles':{'run':self.definition}},files={'run.py':'pass'})['profiles'][0]

    def inventory(self):
        with ops.db() as c:return profile_inventory.inventory(c,self.config,self.reader)


class ProfileInventoryTests(ProfileCatalogFixture):
    def test_exact_combinations_mixed_stages_and_multi_purpose_remain_readonly(self):
        original=copy.deepcopy(self.definition)
        kind=self.register()
        self.assertEqual(self.definition,original)
        registered=self.action('get',owner='owner',id='producer')
        self.assertNotIn('runtime_pin',registered['manifest']['profiles']['run']['runtime'])
        second=self.action('register',id='producer',manifest=registered['manifest'],files={'run.py':'pass'})
        self.assertNotEqual(second['profiles'][0],kind)
        self.action('register_tool',id='planner',manifest={'label':'Planner newer'},files={'plan.py':'print(2)'})
        with ops.db() as c:
            before=list(c.iterdump());rows=profile_inventory.inventory(c,self.config,self.reader)
            self.assertEqual(list(c.iterdump()),before)
        builtin=next(p for p in rows if p['kind']=='cpu.smoke')
        self.assertEqual(builtin['stages'][0]['environment']['type'],'builtin')
        self.assertEqual(builtin['stages'][0]['lane'],'cpu')
        self.assertIsNone(builtin['component'])
        self.assertEqual(builtin['tools'],[])
        row=next(p for p in rows if p['kind']==kind)
        self.assertEqual(row['usage']['categories'],['sdg','evaluation'])
        self.assertEqual(row['component']['revision'],1)
        self.assertEqual(row['tools'][0]['revision'],1)
        self.assertEqual(row['tools'][0]['label'],'Planner old')
        self.assertEqual(row['tools'][0]['invocation'],'runner_defined')
        stages={s['stage']:s for s in row['stages']}
        self.assertEqual(stages['runtime']['environment']['sha256'],self.runtime['sha256'])
        self.assertEqual(stages['prepare']['environment']['type'],'legacy')
        self.assertIsNone(stages['prepare']['environment']['dependencies'])
        self.assertNotIn('DO_NOT_EXPOSE',json.dumps(row));self.assertNotIn('/component/run.py',json.dumps(row))

    def test_observed_job_labels_are_separate_from_unrecorded_purpose_and_frozen_pins(self):
        self.definition.pop('usage');kind=self.register()
        job=self.submit({**self.spec(),'kind':kind,'params':{},'category':'evaluation','execution_mode':'pilot'})
        first=json.loads(self.row(job)['spec'])
        row=next(p for p in self.inventory() if p['kind']==kind)
        self.assertIsNone(row['usage'])
        self.assertEqual(row['observed_usage'],{'categories':['evaluation'],'execution_modes':['pilot']})
        self.assertTrue(row['jobs'][0]['pin_matches'])
        self.assertEqual(json.loads(self.row(job)['spec']),first)
        with ops.db() as c:
            spec=copy.deepcopy(first);spec.pop('category');spec.pop('execution_mode');spec['profile']['sha256']='b'*64
            c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(spec),job))
        row=next(p for p in self.inventory() if p['kind']==kind)
        self.assertFalse(row['jobs'][0]['pin_matches'])
        self.assertEqual(row['observed_usage'],{'categories':[],'execution_modes':[]})
        with ops.db() as c:
            spec['category']=[];spec['execution_mode']={}
            c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(spec),job))
        row=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(row['observed_usage'],{'categories':[],'execution_modes':[]})

    def test_stages_can_select_distinct_exact_runtime_revisions(self):
        changed=copy.deepcopy(self.manifest);changed['runtime']['image']='sha256:'+'b'*64
        changed['dependencies'][0]['version']='7.0'
        second=runtime_store.register(self.catalog,'sim',changed,['alpha'])
        self.definition['prepare'].pop('runtime_id')
        self.definition['prepare']['runtime_ref']={'id':'sim','revision':2}
        kind=self.register();p=next(p for p in self.inventory() if p['kind']==kind)
        stages={s['stage']:s['environment'] for s in p['stages']}
        self.assertEqual(stages['prepare']['sha256'],second['sha256'])
        self.assertEqual(stages['prepare']['dependencies'][0]['version'],'7.0')
        self.assertEqual(stages['runtime']['sha256'],self.runtime['sha256'])
        self.assertEqual(stages['runtime']['dependencies'][0]['version'],'6.0')

    def test_large_dependency_inventory_keeps_profile_discovery_within_client_bound(self):
        manifest=copy.deepcopy(self.manifest)
        manifest['dependencies'] += [{'name':'package-'+str(i),'version':'1.0','kind':'python','scope':'environment','source':'/metadata/'+str(i)} for i in range(995)]
        runtime_store.register(self.catalog,'sim',manifest,['alpha'])
        self.definition['runtime']['runtime_ref']['revision']=2
        result=self.action('register',id='many',manifest={'profiles':{'run'+str(i):self.definition for i in range(60)}},files={'run.py':'pass'})
        with ops.db() as c:before=list(c.iterdump())
        rows=self.action('profiles')
        wire=json.dumps({'jsonrpc':'2.0','id':1,'result':{'content':[{'type':'text','text':json.dumps(rows,indent=2)}]}}).encode()
        self.assertLess(len(wire),2*1024*1024)
        row=next(p for p in rows if p['kind']==result['profiles'][0])
        for env in (row['runtime_bindings']['runtime'],next(s['environment'] for s in row['combination']['stages'] if s['stage']=='runtime')):
            self.assertEqual(env['dependency_count'],996);self.assertFalse(env['dependencies_complete'])
            self.assertLessEqual(len(env['dependencies']),16)
            self.assertNotIn('source',env['dependencies'][0])
        with ops.db() as c:self.assertEqual(before,list(c.iterdump()))
        full=self.action('get_runtime',id='sim',revision=2)
        self.assertEqual(len(full['manifest']['dependencies']),996)

    def test_standalone_component_binding_and_all_revisions_are_discoverable_by_reader(self):
        self.action('register',id='source',manifest={},files={'run.py':'pass'})
        first=self.action('register_profile',id='combo',profile=self.definition,component={'id':'source','revision':1})
        second=self.action('register_profile',id='combo',profile=self.definition,component={'id':'source','revision':1})
        rows=ops.operate('components',{'action':'profiles'},self.reader,self.config)
        for kind in (first['kind'],second['kind']):
            p=next(p for p in rows if p['kind']==kind)['combination']
            self.assertEqual(p['origin']['type'],'standalone');self.assertEqual(p['component']['id'],'source')
        with self.assertRaises(PermissionError):
            ops.operate('components',{'action':'register_profile','arguments':{'id':'denied','profile':self.definition}},self.reader,self.config)

    def test_attribution_preserves_namespaces_source_revisions_jobs_and_shared_access(self):
        kind=self.register();job=self.submit({**self.spec(),'kind':kind,'params':{}})
        frozen=self.row(job)['spec']
        with ops.db() as c:person=ownership.register(c,'person@example.test','Test Person')
        for entry in (('component','producer'),('tool','planner')):
            result=self.action('assign_owner',kind=entry[0],namespace='owner',id=entry[1],owner=person['id'])
            self.assertEqual(result['owner_name'],'Test Person');self.assertEqual(result['namespace'],'owner')
            self.assertFalse(self.action('assign_owner',kind=entry[0],namespace='owner',id=entry[1],owner=person['id'])['changed'])
        self.assertEqual(self.row(job)['spec'],frozen)
        row=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(row['owner_profile']['id'],person['id']);self.assertEqual(row['component']['owner'],'owner')
        self.action('assign_owner',kind='component',namespace='owner',id='producer',owner='built-in')
        self.assertEqual(next(p for p in self.inventory() if p['kind']==kind)['owner_kind'],'built-in')
        with ops.db() as c:
            self.assertIsNone(c.execute("SELECT 1 FROM user_profiles WHERE id='built-in'").fetchone())
            with self.assertRaises(ValueError):ownership.assign(c,'job',job,'built-in','owner')
            c.execute("UPDATE node_control SET mode='maintenance' WHERE id=1")
        with self.assertRaises(PermissionError):self.action('assign_owner',kind='tool',namespace='owner',id='planner',owner=person['id'])

    def test_invalid_purpose_rejected_and_declared_purpose_does_not_gate_jobs(self):
        for usage in ({'categories':['invented']},{'execution_modes':['pilot','pilot']},{'label':[]},{'categories':'evaluation'}):
            self.definition['usage']=usage
            with self.assertRaises(ValueError):self.register()
        self.definition['usage']={'categories':['sdg'],'execution_modes':['full']}
        kind=self.register()
        self.submit({**self.spec(),'kind':kind,'params':{},'category':'evaluation','execution_mode':'pilot'})
        with ops.db() as c:
            row=c.execute("SELECT profiles FROM components WHERE id='producer'").fetchone()
            saved=json.loads(row[0]);saved[kind]['usage']=[]
            c.execute("UPDATE components SET profiles=? WHERE id='producer'",(json.dumps(saved),))
        row=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(row['usage_status'],'invalid');self.assertIsNone(row['usage'])

    def test_revision_tags_do_not_change_sources_or_frozen_jobs(self):
        kind=self.register();job=self.submit({**self.spec(),'kind':kind,'params':{}})
        before=self.row(job)['spec']
        original=self.action('get',owner='owner',id='producer')
        args={'kind':'component','owner':'owner','id':'producer','revision':1,'tags':['sdg','evaluation'],'label':'Generate + evaluate','evidence':'Inspected the run source and output contract.'}
        self.action('annotate',**args)
        row=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(row['component']['classification']['tags'],['sdg','evaluation'])
        self.assertEqual(self.action('get',owner='owner',id='producer')['classification']['tags'],['sdg','evaluation'])
        tool_args={'kind':'tool','owner':'owner','id':'planner','revision':1,'tags':['motion-planning'],'evidence':'Inspected planner interface.'}
        self.action('annotate',**tool_args)
        self.assertEqual(self.action('get_tool',owner='owner',id='planner',revision=1)['classification']['tags'],['motion-planning'])
        self.assertEqual(self.row(job)['spec'],before)
        self.assertEqual(self.action('get',owner='owner',id='producer')['manifest'],original['manifest'])
        self.assertTrue(self.action('annotate',**args)['unchanged'])
        with self.assertRaises(PermissionError):
            ops.operate('components',{'action':'annotate','arguments':args},self.reader,self.config)
        with self.assertRaises(ValueError):self.action('annotate',**{**args,'revision':99})
        with self.assertRaises(ValueError):self.action('annotate',**{**args,'tags':['made-up-category']})
        with ops.db() as c:c.execute("UPDATE node_control SET mode='maintenance'")
        with self.assertRaises(PermissionError):self.action('annotate',**{**args,'tags':['evaluation']})

    def test_reuse_counts_executed_jobs_not_attempts_or_consolidated_sources(self):
        kind=self.register()
        first=self.submit({**self.spec('use-one'),'kind':kind,'params':{}})
        second=self.submit({**self.spec('use-two'),'kind':kind,'params':{}})
        with ops.db() as c:
            c.executemany('INSERT INTO attempts(id,job,state,started,ended) VALUES(?,?,?,?,?)',
                [('a1',first,'complete',1,2),('a2',first,'complete',3,4),('a3',second,'complete',5,6)])
        current=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(current['usage_stats']['executed_jobs'],2)
        self.assertEqual(current['usage_stats']['reuses'],1)
        with ops.db() as c:
            spec=json.loads(self.row(second)['spec']);spec['consolidates']=[first]
            c.execute("UPDATE jobs SET state='complete',result=?,spec=? WHERE id=?",(json.dumps({'artifact_ready':True}),json.dumps(spec),second))
        current=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(current['usage_stats']['executed_jobs'],1)
        self.assertEqual(current['usage_stats']['reuses'],0)
        self.assertEqual(current['usage_stats']['consolidated_sources'],1)

    def test_consolidated_profile_remains_linked_to_canonical_job_with_another_kind(self):
        kind=self.register();first=self.submit({**self.spec('source-use'),'kind':kind,'params':{}})
        canonical=self.submit({**self.spec('canonical-use'),'kind':kind,'params':{}})
        with ops.db() as c:
            c.execute("INSERT INTO attempts(id,job,state,started,ended) VALUES('source-attempt',?,'complete',1,2)",(first,))
            spec=json.loads(self.row(canonical)['spec']);spec.update(kind='cpu.smoke',consolidates=[first])
            c.execute("UPDATE jobs SET state='complete',result=?,spec=? WHERE id=?",(json.dumps({'artifact_ready':True}),json.dumps(spec),canonical))
        current=next(p for p in self.inventory() if p['kind']==kind)
        self.assertEqual(current['usage_stats']['executed_jobs'],1)
        self.assertEqual(current['usage_stats']['recorded_executions'],1)
        self.assertEqual(current['usage_stats']['reuses'],0)
        self.assertEqual(current['jobs'][0]['logical_job'],canonical)
