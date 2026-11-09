import json
from unittest.mock import patch
import ops,component_store,recovery
from test_support import IsolatedOps

class ComponentPlatformTests(IsolatedOps):
    def setUp(self):
        super().setUp();p=patch.object(component_store,'ROOT',self.root/'components');p.start();self.addCleanup(p.stop)
        self.config['runtime_ids']=['python']
        self.definition={'profiles':{'run':{'params_schema':{'type':'object'},'inputs':{},'outputs':None,
            'runtime':{'executor':'container','runtime_id':'python','command':['python3','/component/run.py'],'lane':'cpu'},
            'limits':{'timeout_seconds':3600,'max_attempts':4}}}}
    def register(self,owner=None):
        args={'action':'register','id':'demo','manifest':self.definition,'files':{'run.py':'print("hello")'}}
        if owner:args['owner']=owner
        return ops.operate('components',{'action':'register','arguments':{k:v for k,v in args.items() if k!='action'}},self.owner,self.config)
    def test_component_revision_and_job_freeze(self):
        first=self.register();kind=first['profiles'][0]
        job=self.submit({**self.spec(), 'kind':kind,'params':{'epochs':7}})
        second=self.register();self.assertNotEqual(kind,second['profiles'][0])
        frozen=json.loads(self.row(job)['spec']);self.assertEqual(frozen['kind'],kind)
        self.assertEqual(frozen['params']['epochs'],7)
        self.assertTrue(frozen['profile']['source_pins'])
        self.assertEqual(len(ops.operate('components',{'action':'list'},self.owner,self.config)),1)
    def test_user_sources_are_shared_but_runtime_policy_is_enforced(self):
        self.assertEqual(self.register('other')['owner'],'other')
        self.definition['profiles']['run']['runtime']['lane']='gpu'
        with self.assertRaises(PermissionError):self.register()
        self.definition['profiles']['run']['runtime']['lane']='cpu'
        self.definition['profiles']['run']['runtime']['runtime_id']='uninstalled'
        with self.assertRaises(ValueError):self.register()
    def test_failed_gpu_reservation_does_not_hide_ready_reason(self):
        job=self.submit();row=self.claim(job);ops.finish(row,'failed',{'exit_code':1})
        another=self.submit(self.spec('next'))
        result=ops.operate('jobs',{'action':'get','arguments':{'id':another}},self.owner,self.config)
        self.assertEqual(result['scheduling']['reason'],'recovery_reservation')
    def test_grouped_schema_and_event_defaults(self):
        schema=ops.operate('system',{'action':'describe','arguments':{'group':'jobs','action':'submit'}},self.owner,self.config)
        self.assertIn('params',schema['schema']['properties'])
        self.assertEqual(ops.operate('events',{'action':'list','arguments':{'kind':'all'}},self.owner,self.config),[])
    def test_callback_keeps_original_attempt_context_after_job_is_changed(self):
        import hooks
        row=self.claim(self.submit())
        spec=json.loads(row['spec']);spec['profile']={'on_error':{'command':['fixture']}}
        original={**row,'spec':json.dumps(spec),'result':json.dumps({'error':'first'})}
        with ops.db() as c:
            hooks.enqueue(c,original,'failed')
            c.execute('UPDATE jobs SET spec=?,result=? WHERE id=?',(json.dumps({'params':{'changed':True}}),json.dumps({'error':'second'}),row['id']))
            pending=hooks.pending(c)
        self.assertEqual(json.loads(pending['result']),{'error':'first'})
        self.assertEqual(json.loads(pending['spec']),spec)
        self.assertEqual(pending['attempt'],row['attempt'])

    def test_automatic_retries_consume_the_same_three_recovery_rounds(self):
        kind=self.register()['profiles'][0]
        job=self.submit({**self.spec(),'kind':kind,'params':{},'execution':{'max_attempts':4,'retry_on':['exit_nonzero'],'retry_backoff_seconds':0}})
        for index in range(4):
            attempt='try-'+str(index)
            with ops.db() as c:
                recovery.started(c,job)
                c.execute("UPDATE jobs SET state='running',attempt=? WHERE id=?",(attempt,job))
                c.execute("INSERT INTO attempts VALUES(?,?,'running',1,NULL)",(attempt,job))
            ops.finish(self.row(job),'failed',{'reason':'exit_nonzero','exit_code':1,'process_group_returned':True})
        with ops.db() as c:record=recovery.view(c,job)
        self.assertEqual(record['rounds'],3);self.assertEqual(record['state'],'exhausted')
        self.assertEqual(self.row(job)['state'],'failed')

    def test_finalize_preserves_primary_runtime_and_stage_tickets(self):
        import runtime_client
        folder=self.root/'output';folder.mkdir()
        request=folder/'request.json';request.write_text(json.dumps({'params':{},'output_dir':str(folder)}))
        config={'executor':{'host_data':str(self.root),'host_components':'/host/components/dev'}}
        runner={'runtime_id':'python','command':['python3'],'lane':'gpu'}
        spec={'profile':{'runtime':runner,'finalize':{**runner,'lane':'cpu'},'inputs':{},'component_path':'/loom/components/owner/demo/revisions/1'}}
        row={'id':'job','attempt':'attempt','owner':'owner'}
        def broker(config,action,payload):
            if action=='start':
                self.assertEqual(payload['component'],'/host/components/dev/owner/demo/revisions/1')
                self.assertNotIn('storage',payload)
                return {'ticket':payload['ticket'],'image_id':'sha256:fixture'}
            return {'returned':True,'exit_code':0}
        with patch.object(runtime_client,'call',side_effect=broker):
            runtime_client.Container(config,row,spec,'run',request,folder,self.root)
            runtime_client.Container(config,row,spec,'finalize',request,folder,self.root)
        primary=json.loads((folder/'container-ticket.json').read_text())
        self.assertEqual(primary['stage'],'run');self.assertEqual(primary['lane'],'gpu')
        self.assertEqual(json.loads((folder/'finalize-ticket.json').read_text())['lane'],'cpu')

    def test_storage_is_validated_frozen_and_forwarded_as_registered_ids(self):
        import runtime_client,profile_store
        self.config['executor']={'storage':{'data/1':'/host/external'}}
        self.definition['profiles']['run']['runtime']['storage']=['data/1']
        kind=self.register()['profiles'][0]
        job=self.submit({**self.spec(),'kind':kind,'params':{}})
        frozen=json.loads(self.row(job)['spec'])
        self.assertEqual(frozen['profile']['runtime']['storage'],['data/1'])
        self.assertIn('storage',profile_store.RUNNER_SCHEMA['properties'])
        self.definition['profiles']['run']['runtime']['storage']=['data/2']
        with self.assertRaisesRegex(ValueError,'Storage'):self.register()
        folder=self.data/'rw-job';folder.mkdir();request=folder/'request.json';request.write_text(json.dumps({'params':{}}))
        frozen['profile'].pop('component_path',None)
        config={'executor':{'host_data':str(self.data),'storage':{'data/1':'/host/external'}}}
        def broker(config,action,payload):
            if action=='start':
                self.assertEqual(payload['storage'],['data/1'])
                self.assertNotIn('/host/external',json.dumps(payload))
                return {'ticket':payload['ticket']}
            return {'returned':True,'exit_code':0}
        with patch.object(runtime_client,'call',side_effect=broker):
            runtime_client.Container(config,{'id':job,'attempt':'one','owner':'owner'},frozen,'run',request,folder,self.data)
        self.assertEqual(json.loads((folder/'run-request.json').read_text())['storage'],{'data/1':'/loom/data/1'})

    def test_owner_display_shared_discovery_and_builtin_are_distinct(self):
        import ownership
        with ops.db() as c:
            person=ownership.register(c,'person@example.test','Test Person')
        original=dict(self.owner)
        self.owner={**self.owner,'id':person['id']}
        user=self.register()
        self.assertEqual(user['owner_profile']['display_name'],'Test Person')
        self.assertEqual(user['owner'],person['id'])
        common=self.register('built-in')
        self.assertEqual(common['owner_name'],'Built-in')
        rows=ops.operate('components',{'action':'list'},original,self.config)
        self.assertEqual(len(rows),2)
        self.assertEqual({r['owner_kind'] for r in rows},{'built-in','user'})
        only=ops.operate('components',{'action':'list','arguments':{'owner':'built-in'}},original,self.config)
        self.assertEqual(len(only),1)
        discovered=ops.operate('list_execution_profiles',{},original,self.config)
        self.assertEqual(next(r for r in discovered if r['kind']==user['profiles'][0])['owner_name'],'Test Person')
        self.assertEqual(next(r for r in discovered if r['kind']=='cpu.smoke')['owner_name'],'Built-in')

    def test_existing_responsibility_does_not_rewrite_namespace_or_pins(self):
        import ownership
        first=self.register()
        with ops.db() as c:
            person=ownership.register(c,'responsible@example.test','Responsible')
            ownership.assign(c,'component','owner:demo',person['id'],'owner')
        result=ops.operate('components',{'action':'get','arguments':{'owner':'owner','id':'demo'}},self.owner,self.config)
        self.assertEqual(result['namespace'],'owner')
        self.assertEqual(result['owner_profile']['id'],person['id'])
        self.assertEqual(list(result['profiles']),first['profiles'])
        self.assertEqual(result['revision'],1)
