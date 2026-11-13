"""Business-boundary regressions: real temporary DBs/files, no installed services."""
import contextlib
import copy
import io
import json
import sqlite3
from unittest.mock import patch

import dashboard_read
import job_history
import job_work
import ops
import recovery
import runtime_client
from test_support import IsolatedOps


class JobLifecycleTests(IsolatedOps):
    def get(self, job):
        return ops.operate('get_job', {'id':job}, self.owner, self.config)

    def recover(self, job, action, **args):
        return ops.operate('recover_job', {'id':job, 'action':action, **args}, self.owner, self.config)

    def returned(self, key):
        row=self.processing(self.claim(self.submit(self.spec(key))))
        folder=self.data/'runs'/row['id']/row['attempt'];folder.mkdir(parents=True)
        ops.cpu(folder, {'seconds':0, 'fail':False})
        return row,folder

    def process(self, row):
        with patch.object(ops,'guard'),contextlib.redirect_stderr(io.StringIO()):
            ops.process_result(row,self.config)

    def test_cancelled_verified_upstream_never_admits_downstream(self):
        row,_=self.returned('source')
        with ops.db() as c:
            c.execute("UPDATE jobs SET state='result_failed',result=? WHERE id=?", (json.dumps({'artifact_ready':True}),row['id']))
        child=self.submit({**self.spec('consumer'),'depends_on':[row['id']]})
        ops.operate('cancel_job',{'id':row['id']},self.owner,self.config)
        schedule=self.get(child)['scheduling']
        self.assertFalse(schedule['eligible'])
        self.assertTrue(schedule['dependency_failed'])

    def test_cancel_wins_even_when_finalization_raises_on_last_retry(self):
        row,_=self.returned('cancel-race')
        def fail(*_):
            ops.operate('cancel_job',{'id':row['id']},self.owner,self.config)
            raise ValueError('Invalid result after cancellation')
        with patch.object(ops,'guard',side_effect=fail),contextlib.redirect_stderr(io.StringIO()):
            ops.process_result({**row,'retries':2},self.config)
        self.assertEqual(self.row(row['id'])['state'],'cancelled')
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM result_outbox').fetchone()[0],0)
            self.assertEqual(c.execute('SELECT count(*) FROM entity_revisions').fetchone()[0],0)

    def test_changed_published_bytes_cannot_be_reindexed_as_success(self):
        row,folder=self.returned('immutable');self.process(row)
        with ops.db() as c:before=tuple(c.execute('SELECT * FROM entity_revisions').fetchone())
        (folder/'observation.json').write_text('{"synthetic":true,"changed":true}')
        ops.operate('reindex_job',{'id':row['id'],'attempt':row['attempt']},self.owner,self.config)
        self.process({**self.row(row['id']),'retries':2})
        self.assertEqual(self.row(row['id'])['state'],'result_failed')
        with ops.db() as c:self.assertEqual(tuple(c.execute('SELECT * FROM entity_revisions').fetchone()),before)
        self.assertFalse(json.loads(self.row(row['id'])['result']).get('artifact_ready',False))

    def test_identical_reindex_is_idempotent_and_does_not_duplicate_completion(self):
        row,_=self.returned('identical');self.process(row)
        ops.operate('reindex_job',{'id':row['id'],'attempt':row['attempt']},self.owner,self.config)
        self.process({**self.row(row['id']),'retries':0})
        self.assertEqual(self.row(row['id'])['state'],'complete')
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM entity_revisions').fetchone()[0],1)
            self.assertEqual(c.execute("SELECT count(*) FROM events WHERE code='complete'").fetchone()[0],1)

    def test_original_submission_replay_survives_recovery_patch(self):
        request=self.spec('replay');job=self.submit(request);old=self.claim(job)
        failure={'reason':'exit_nonzero','stage':'prepare','exit_code':7,'process_group_returned':True}
        ops.finish(old,'failed',failure)
        lease=self.recover(job,'claim')['lease']
        self.recover(job,'resume',lease=lease,patch={'params':{'seconds':1,'fail':False}})
        self.assertEqual(self.submit(request),job)
        with self.assertRaises(ValueError):self.submit({**request,'params':{'seconds':1,'fail':False}})
        detail=self.get(job)
        self.assertIsNone(detail['result'])
        self.assertEqual(detail['attempts'][0]['result'],failure)
        self.assertEqual(json.loads(self.row(job)['spec'])['params']['seconds'],1)

    def test_finalization_recovery_keeps_frozen_inputs_without_readmitting_execution(self):
        row,_=self.returned('finalization-only')
        with ops.db() as c:
            c.execute("UPDATE jobs SET state='result_failed' WHERE id=?",(row['id'],))
            recovery.open_incident(c,row,'finalization_failed')
        lease=self.recover(row['id'],'claim')['lease']
        with patch.object(ops.execution,'policy',side_effect=ValueError('Execution admission must not run')):
            self.recover(row['id'],'resume',lease=lease)
        self.assertEqual(self.row(row['id'])['state'],'processing')
        self.assertEqual(self.row(row['id'])['attempt'],row['attempt'])

    def test_old_attempt_cannot_finish_or_mark_returned_over_a_new_attempt(self):
        old=self.claim(self.submit(self.spec('stale')))
        with ops.db() as c:
            c.execute("INSERT INTO attempts VALUES('new-attempt',?,'running',10,NULL)",(old['id'],))
            c.execute("UPDATE jobs SET attempt='new-attempt' WHERE id=?",(old['id'],))
        ops.finish(old,'failed',{'reason':'late_old_failure'})
        with ops.db() as c:ops.mark_returned(c,old,{'ended':20,'execution':{'exit_code':0,'process_group_returned':True}})
        row=self.row(old['id'])
        self.assertEqual((row['attempt'],row['state'],row['result']),('new-attempt','running',None))
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM result_outbox').fetchone()[0],0)

    def test_ready_attempt_cancellation_enqueues_its_cancel_hook(self):
        row=self.claim(self.submit(self.spec('prepared-cancel')))
        spec=json.loads(row['spec']);spec['profile']={'on_cancel':{'command':['fixture']}}
        with ops.db() as c:c.execute("UPDATE jobs SET state='ready',spec=? WHERE id=?",(json.dumps(spec),row['id']))
        ops.operate('cancel_job',{'id':row['id']},self.owner,self.config)
        with ops.db() as c:
            hook=c.execute('SELECT attempt,stage,state FROM hooks').fetchone()
        self.assertIsNotNone(hook)
        self.assertEqual(tuple(hook),(row['attempt'],'on_cancel','pending'))

    def test_runtime_intervals_use_stage_receipts_and_exclude_ready_wait(self):
        row=self.claim(self.submit(self.spec('stage-times')))
        with ops.db() as c:
            c.execute("UPDATE jobs SET state='complete' WHERE id=?",(row['id'],))
            c.execute("UPDATE attempts SET state='complete',started=100,ended=220 WHERE job=?",(row['id'],))
            c.executemany('INSERT INTO stage_runs VALUES(?,?,?,?,?,?)',[
                (row['id'],row['attempt'],'prepare',100,110,'returned'),
                (row['id'],row['attempt'],'run',200,220,'returned')])
            catalog=dashboard_read.projection(c,self.reader,self.config,self.local,self.data)[0]
        item=next(e for e in catalog['entities'] if e['id']==row['id'])
        self.assertEqual(sum(x['end']-x['start'] for x in item['execution_intervals']),30)

    def test_native_output_activity_has_registration_time_without_inventing_creation(self):
        row,_=self.returned('registered');self.process(row)
        with ops.db() as c:
            c.execute("UPDATE entity_revisions SET kind='dataset',manifest=json_set(manifest,'$.kind','dataset')")
            catalog=dashboard_read.projection(c,self.reader,self.config,self.local,self.data)[0]
            registered=c.execute("SELECT created FROM events WHERE code='complete'").fetchone()[0]
        item=next(e for e in catalog['entities'] if e.get('loom_registry'))
        self.assertEqual(item['registered_at'],registered)
        self.assertNotIn('created_at',item)
        self.assertTrue(item['contribution']['eligible'])

    def test_long_local_entity_id_remains_addressable_after_publication(self):
        row,folder=self.returned('long-output');manifest=json.loads((folder/'result.json').read_text())
        manifest['entities'][0]['id']='x'*96;manifest['metrics'][0]['entity_id']='x'*96
        (folder/'result.json').write_text(json.dumps(manifest));self.process(row)
        item=ops.operate('get_entity',{'id':row['id']+'.'+'x'*96,'revision':1},self.owner,self.config)
        self.assertEqual(item['id'],row['id']+'.'+'x'*96)

    def test_registry_listing_does_not_silently_drop_the_251st_output(self):
        with ops.db() as c:
            c.executemany('INSERT INTO entity_revisions VALUES(?,1,?,\'artifact\',\'owner\',\'owner\',\'{}\')',[(str(i),'j') for i in range(251)])
            self.assertEqual(len(ops.registry.view(c,self.owner)),251)

    def test_recovery_refreshes_input_dependencies_but_rejects_cycles(self):
        a=self.submit(self.spec('input-a'));b=self.submit(self.spec('input-b'))
        profile={'schema':'loom-execution/v2','sha256':'a'*64,'revision':'input-profile',
            'runtime':{'command':['fixture']},'inputs':{'input':'runs'},'outputs':None,
            'params_schema':{'type':'object','required':['input']},'limits':{'timeout_seconds':30,'max_attempts':1}}
        self.config['profiles']['input-profile']=profile
        job=self.submit({'key':'consumer-repair','kind':'input-profile','owner':'owner','notify':'owner','params':{'input':{'job':a,'entity':'out'}}})
        row=self.claim(job);ops.finish(row,'failed',{'reason':'exit_nonzero','process_group_returned':True})
        lease=self.recover(job,'claim')['lease']
        self.recover(job,'resume',lease=lease,patch={'params':{'input':{'job':b,'entity':'out'}}})
        self.assertEqual(json.loads(self.row(job)['spec'])['depends_on'],[b])
        self.assertEqual(self.get(job)['scheduling']['waiting_for'],[b])
        with ops.db() as c:
            c.execute("UPDATE jobs SET state='failed' WHERE id=?",(job,));recovery.open_incident(c,self.row(job),'exit_nonzero')
        descendant=self.submit({**self.spec('descendant'),'depends_on':[job]})
        lease=self.recover(job,'claim')['lease']
        with self.assertRaisesRegex(ValueError,'cycle'):
            self.recover(job,'resume',lease=lease,patch={'params':{'input':{'job':descendant,'entity':'out'}}})
        self.assertEqual(self.row(job)['state'],'failed')

    def test_result_and_callback_resource_uncertainty_is_not_swallowed(self):
        row,folder=self.returned('uncertain-finalize')
        spec=json.loads(row['spec']);spec['profile']={'finalize':{'command':['fixture']},'on_complete':{'command':['fixture']}}
        row['spec']=json.dumps(spec)
        with patch.object(ops,'guard'),patch.object(ops,'run_hook',side_effect=ops.UnreturnedProcessGroup('unknown')):
            with self.assertRaises(ops.UnreturnedProcessGroup):ops.process_result(row,self.config)
            item={**row,'job':row['id'],'stage':'on_complete','retries':0}
            with self.assertRaises(ops.UnreturnedProcessGroup):ops.process_hook(item,self.config)
        self.assertEqual(self.row(row['id'])['state'],'processing')

    def test_reindex_cannot_add_or_remove_published_outputs(self):
        row,folder=self.returned('output-set');self.process(row)
        manifest=json.loads((folder/'result.json').read_text());manifest['entities'][0]['id']='different'
        manifest['metrics'][0]['entity_id']='different';(folder/'result.json').write_text(json.dumps(manifest))
        ops.operate('reindex_job',{'id':row['id'],'attempt':row['attempt']},self.owner,self.config)
        self.process({**self.row(row['id']),'retries':2})
        self.assertEqual(self.row(row['id'])['state'],'result_failed')
        with ops.db() as c:self.assertEqual(c.execute('SELECT id FROM entity_revisions').fetchone()[0],row['id']+'.observation')

    def test_legacy_active_row_does_not_present_a_previous_failure_as_current(self):
        row=self.claim(self.submit(self.spec('legacy-active')))
        old={'reason':'exit_nonzero','stage':'prepare','exit_code':7}
        with ops.db() as c:c.execute('UPDATE jobs SET result=? WHERE id=?',(json.dumps(old),row['id']))
        detail=self.get(row['id'])
        self.assertIsNone(detail['result']);self.assertEqual(detail['previous_result'],old)
        self.assertIsNone(job_history.entity(detail,detail['spec'])['properties']['reason'])
        self.assertEqual(json.loads(self.row(row['id'])['result']),old)

    def test_replay_uses_frozen_defaults_even_when_current_installation_changes(self):
        profile={'schema':'loom-execution/v2','sha256':'a'*64,'revision':'default-profile',
            'runtime':{'command':['fixture']},'inputs':{},'outputs':None,
            'params_schema':{'type':'object'},'limits':{'timeout_seconds':30,'max_attempts':1},'defaults':{'size':1}}
        self.config['profiles']['default-profile']=profile
        request={**self.spec('frozen-defaults'),'kind':'default-profile','params':{}}
        job=self.submit(request)
        self.config['profiles']['default-profile']={**profile,'defaults':{'size':2}}
        self.assertEqual(self.submit(request),job)
        del self.config['profiles']['default-profile']
        self.assertEqual(self.submit(request),job)

    def test_generated_long_profile_kind_can_be_submitted(self):
        kind='namespace.'+'profile-'*15+'.r1'
        self.config['profiles'][kind]={'schema':'loom-execution/v2','sha256':'a'*64,'revision':kind,
            'runtime':{'command':['fixture']},'inputs':{},'outputs':None,
            'params_schema':{'type':'object'},'limits':{'timeout_seconds':30,'max_attempts':1}}
        self.assertTrue(self.submit({**self.spec('long-kind'),'kind':kind,'params':{}}))

    def test_verified_finalizer_is_not_reexecuted_for_index_retry_or_completed_reindex(self):
        row,folder=self.returned('finalizer-once')
        profile={'schema':'loom-execution/v2','sha256':'a'*64,'revision':'finalizer-profile',
            'runtime':{'command':['fixture']},'finalize':{'command':['fixture']},'inputs':{},'outputs':{'observation':'artifact'},
            'params_schema':{'type':'object'},'limits':{'timeout_seconds':30,'max_attempts':1}}
        spec=json.loads(row['spec']);spec['profile']=profile
        with ops.db() as c:c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(spec),row['id']))
        manifest=json.loads((folder/'result.json').read_text())
        manifest['metrics']=[];manifest['entities'][0].update(provenance={'profile':profile['revision'],'producer_sha256':profile['sha256']},relations=[])
        (folder/'result.json').write_text(json.dumps(manifest))
        with patch.object(ops,'run_hook') as finalize:
            with patch.object(ops.registry,'index',side_effect=RuntimeError('index offline')):
                self.process({**self.row(row['id']),'retries':0})
            self.process({**self.row(row['id']),'retries':1})
            self.assertEqual(self.row(row['id'])['state'],'complete')
            ops.operate('reindex_job',{'id':row['id'],'attempt':row['attempt']},self.owner,self.config)
            self.assertFalse(json.loads(self.row(row['id'])['result']).get('artifact_ready',False))
            self.process({**self.row(row['id']),'retries':0})
            self.assertEqual(self.row(row['id'])['state'],'complete')
            self.assertEqual(finalize.call_count,1)


class ContainerReturnTests(IsolatedOps):
    def container(self):
        child=object.__new__(runtime_client.Container)
        child.config={};child.ticket='fixture-ticket';child.returncode=None
        return child

    def test_unconfirmed_container_stop_is_not_an_ordinary_launch_failure(self):
        child=self.container()
        for effect in (RuntimeError('executor unreachable'),{'returned':False}):
            options={'side_effect':effect} if isinstance(effect,Exception) else {'return_value':effect}
            with self.subTest(effect=str(effect)),patch.object(runtime_client,'call',**options):
                with self.assertRaises(ops.UnreturnedProcessGroup):ops.stop_process_group(child)

    def test_lost_start_ack_keeps_job_running_until_executor_reconciliation(self):
        row=self.claim(self.submit(self.spec('uncertain-start')))
        profile={'schema':'loom-execution/v2','sha256':'a'*64,'revision':'fixture',
            'runtime':{'executor':'container','command':['fixture'],'image':'sha256:'+'a'*64},
            'inputs':{},'outputs':None,'params_schema':{'type':'object'},'limits':{'timeout_seconds':30,'max_attempts':1}}
        spec=json.loads(row['spec']);spec['profile']=profile;spec['kind']='fixture'
        with ops.db() as c:c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(spec),row['id']))
        row=self.row(row['id']);self.config.update(profiles={'fixture':profile},executor={'host_data':str(self.data)})
        with patch.object(ops,'guard'),patch.object(runtime_client,'call',side_effect=TimeoutError('ack lost')):
            with self.assertRaises(ops.UnreturnedProcessGroup):ops.execute(row,self.config)
        self.assertEqual(self.row(row['id'])['state'],'running')

    def test_monitor_error_cannot_publish_failure_before_container_return(self):
        row=self.claim(self.submit(self.spec('monitor-unknown')));child=self.container()
        with patch.object(child,'poll',side_effect=RuntimeError('status unavailable')),patch.object(runtime_client,'call',return_value={'returned':False}):
            with self.assertRaises(ops.UnreturnedProcessGroup):
                ops.monitor_child(row,self.config,json.loads(row['spec']),self.data,'run',child,None,30,False)
        self.assertEqual(self.row(row['id'])['state'],'running')

    def test_stage_record_failure_returns_the_already_started_container(self):
        row=self.claim(self.submit(self.spec('stage-commit')))
        profile={'schema':'loom-execution/v2','sha256':'a'*64,'revision':'fixture',
            'runtime':{'executor':'container','command':['fixture'],'image':'sha256:'+'a'*64},
            'inputs':{},'outputs':None,'params_schema':{'type':'object'},'limits':{'timeout_seconds':30,'max_attempts':1}}
        spec=json.loads(row['spec']);spec.update(kind='fixture',profile=profile)
        with ops.db() as c:
            c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(spec),row['id']))
            c.execute("CREATE TRIGGER reject_stage BEFORE INSERT ON stage_runs BEGIN SELECT RAISE(ABORT,'stage DB offline'); END")
        self.config.update(profiles={'fixture':profile},executor={'host_data':str(self.data)})
        calls=[]
        def executor(config,action,payload):
            calls.append(action)
            return {'ticket':'fixture-ticket','returned':action=='cancel','exit_code':137 if action=='cancel' else None,'text':''}
        with patch.object(ops,'guard'),patch.object(runtime_client,'call',side_effect=executor):
            with self.assertRaises(sqlite3.IntegrityError):ops.execute(self.row(row['id']),self.config)
        self.assertIn('start',calls);self.assertIn('cancel',calls)

    def test_transfer_grant_is_not_released_with_unreturned_preparation_or_callback(self):
        from unittest.mock import Mock
        row=self.claim(self.submit(self.spec('owned-prepare')))
        spec=json.loads(row['spec']);spec['profile']={'prepare':{'executor':'container'}}
        with ops.db() as c:c.execute("UPDATE jobs SET state='preparing',spec=? WHERE id=?",(json.dumps(spec),row['id']))
        self.config.update(executor={'socket':'fixture'},remote_fence={'fixture':True})
        for phase in ('preparing','callback'):
            if phase=='callback':
                with ops.db() as c:
                    c.execute("UPDATE jobs SET state='complete'")
                    c.execute("INSERT INTO hooks(job,attempt,stage,state) VALUES(?,?,'on_complete','pending')",(row['id'],row['attempt']))
            grant=Mock()
            with self.subTest(phase=phase),patch.object(ops,'WorkerGrant',return_value=grant),patch.object(ops.signal,'signal'),patch.object(runtime_client,'call',side_effect=ops.UnreturnedProcessGroup('ownership unknown')):
                with self.assertRaises(ops.UnreturnedProcessGroup):ops.worker(self.config)
                self.assertEqual([call.args[0] for call in grant.request.call_args_list],['claim'])
