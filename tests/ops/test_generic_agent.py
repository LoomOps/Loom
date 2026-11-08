"""Independent agent acceptance tests; temporary DBs, CPU processes, no services/GPU."""
import copy
import json
import multiprocessing as mp
from pathlib import Path
import sys
import time
from unittest.mock import patch

import ops
import recovery
from test_support import IsolatedOps
from test_worker_recovery import fixture_worker


RUNNER = r'''
import json,sys,time
from pathlib import Path
request=json.loads(Path(sys.argv[-1]).read_text())
p=request['params'];out=Path(request['output_dir']);root=Path(p['root'])
stage=sys.argv[1];label=p['label'];trace=root/'trace.jsonl'
def record(event):
 with trace.open('a') as f:f.write(json.dumps({'stage':stage,'label':label,'event':event,'at':time.monotonic()})+'\n')
record('start')
if stage=='prepare':
 (out/'prepared').write_text('prepared')
 time.sleep(p.get('prepare_seconds',0))
else:
 if p.get('needs_preparation') and not (out/'prepared').exists():raise RuntimeError('Preparation lost')
 if p.get('fail_once'):
  marker=root/(label+'-failed')
  if not marker.exists():marker.write_text('failed');sys.exit(7)
 if p.get('fail_always'):sys.exit(8)
 if 'input' in p:
  assert Path(p['input']).read_text()=='fixture-artifact',p['input']
 time.sleep(p.get('seconds',0))
 (out/'payload.txt').write_text('fixture-artifact')
 (out/'result.json').write_text(json.dumps({'entities':[{'id':'output','kind':'artifact','path':'payload.txt','provenance':{'profile':request['profile']['revision'],'producer_sha256':request['profile']['sha256']},'relations':[]}],'metrics':[]}))
record('end')
'''


class GenericAgentAcceptanceTests(IsolatedOps):
    def setUp(self):
        super().setUp()
        self.runner=self.root/'runner.py';self.runner.write_text(RUNNER)
        self.params_schema={'type':'object','properties':{
            'root':{'type':'string'},'label':{'type':'string'},'seconds':{'type':'number','minimum':0},
            'prepare_seconds':{'type':'number','minimum':0},'needs_preparation':{'type':'boolean'},
            'fail_once':{'type':'boolean'},'fail_always':{'type':'boolean'},'input':{'type':'object'}},
            'required':['root','label'],'additionalProperties':False}

    def profile(self,kind='generic.cpu',*,lane='cpu',prepare=False,inputs=None):
        profile={'schema':'loom-execution/v2','revision':'acceptance-v1','sha256':'a'*64,
            'submitters':['owner'],'owners':['owner'],'params_schema':copy.deepcopy(self.params_schema),
            'inputs':inputs or {},'outputs':{'output':'artifact'},
            'runtime':{'command':[sys.executable,str(self.runner),'run'],'lane':lane},
            'limits':{'timeout_seconds':172800,'max_attempts':3}}
        if prepare:profile['prepare']={'command':[sys.executable,str(self.runner),'prepare'],'timeout':60}
        self.config['profiles'][kind]=profile
        return kind

    def job(self,key,kind=None,policy=None,**params):
        kind=kind or self.profile()
        spec={'key':key,'kind':kind,'owner':'owner','notify':'owner',
              'params':{'root':str(self.root),'label':key,**params}}
        if policy is not None:spec['execution']=policy
        return self.submit(spec)

    def start_worker(self):
        child=mp.get_context('fork').Process(target=fixture_worker,args=(self.config,))
        child.start();self.addCleanup(self.stop_worker,child)
        return child

    @staticmethod
    def stop_worker(child):
        if child.is_alive():child.terminate()
        child.join(8)
        if child.is_alive():child.kill();child.join(2)

    def wait_for(self,job,states,seconds=8):
        until=time.monotonic()+seconds
        while time.monotonic()<until:
            row=self.row(job)
            if row['state'] in states:return row
            time.sleep(.02)
        self.fail('Unexpected terminal/progress state: '+str(self.row(job)))

    def traces(self):
        path=self.root/'trace.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def test_preparation_overlaps_busy_execution_and_same_attempt_is_reused(self):
        first=self.job('busy',self.profile('generic.busy',lane='gpu'),seconds=1.2)
        second=self.job('prepared',self.profile('generic.prepared',lane='gpu',prepare=True),needs_preparation=True)
        child=self.start_worker()
        self.wait_for(first,{'complete'});self.wait_for(second,{'complete'});self.stop_worker(child)
        times={(x['label'],x['stage'],x['event']):x['at'] for x in self.traces()}
        self.assertLess(times['prepared','prepare','start'],times['busy','run','end'])
        self.assertGreaterEqual(times['prepared','run','start'],times['busy','run','end'])
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(second,)).fetchone()[0],1)

    def test_prepared_job_survives_worker_restart_without_repeating_preparation(self):
        busy=self.job('busy-restart',self.profile('generic.busy',lane='gpu'),seconds=10)
        pending=self.job('ready-restart',self.profile('generic.prepared',lane='gpu',prepare=True),needs_preparation=True)
        first=self.start_worker()
        before=self.wait_for(pending,{'ready'});self.stop_worker(first)
        self.assertEqual(self.row(busy)['state'],'interrupted')
        ops.operate('recover_job',{'id':busy,'action':'defer'},self.owner,self.config)
        second=self.start_worker();after=self.wait_for(pending,{'complete'});self.stop_worker(second)
        self.assertEqual(after['attempt'],before['attempt'])
        self.assertEqual(sum(x['label']=='ready-restart' and x['stage']=='prepare' and x['event']=='start' for x in self.traces()),1)

    def test_pending_upstream_entity_reference_requires_no_preknown_hash(self):
        upstream=self.job('source')
        kind=self.profile('generic.consumer',inputs={'input':'runs'})
        downstream=self.job('consumer',kind,input={'job':upstream,'entity':'output'})
        self.assertIn(upstream,json.loads(self.row(downstream)['spec'])['depends_on'])
        child=self.start_worker();self.wait_for(downstream,{'complete'});self.stop_worker(child)
        rows=self.traces();starts={x['label']:x['at'] for x in rows if x['stage']=='run' and x['event']=='start'}
        self.assertGreater(starts['consumer'],starts['source'])

    def test_durable_long_backoff_survives_restart_and_does_not_block_other_jobs(self):
        policy={'max_attempts':2,'retry_on':['exit_nonzero'],'retry_backoff_seconds':86400}
        job=self.job('retry-later',policy=policy,fail_once=True)
        first=self.start_worker()
        until=time.monotonic()+8
        while time.monotonic()<until:
            row=self.row(job)
            if row['state']=='queued' and row['available_at']>time.time()+86000:break
            time.sleep(.02)
        self.assertEqual(row['state'],'queued');self.assertGreater(row['available_at'],time.time()+86000)
        self.stop_worker(first)
        other=self.job('independent')
        second=self.start_worker();self.wait_for(other,{'complete'})
        self.assertEqual(self.row(job)['state'],'queued')
        # Advance only the temporary persisted eligibility time, without waiting one day.
        with ops.db() as c:c.execute('UPDATE jobs SET available_at=? WHERE id=?',(time.time()-1,job))
        self.wait_for(job,{'complete'});self.stop_worker(second)
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(job,)).fetchone()[0],2)
        events=ops.operate('list_events',{'kind':'all'},self.owner,self.config)
        self.assertEqual(sum(e['job']==job and e['code']=='complete' for e in events),1)

    def test_retry_budget_is_enforced_and_default_does_not_retry(self):
        repeated=self.job('bounded',policy={'max_attempts':2,'retry_on':['exit_nonzero'],'retry_backoff_seconds':0},fail_always=True)
        ordinary=self.job('default-stop',fail_always=True)
        child=self.start_worker();self.wait_for(repeated,{'failed'})
        ops.operate('recover_job',{'id':repeated,'action':'defer'},self.owner,self.config)
        self.wait_for(ordinary,{'failed'});self.stop_worker(child)
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(repeated,)).fetchone()[0],2)
            self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(ordinary,)).fetchone()[0],1)

    def test_real_timeout_returns_process_and_emits_owner_error(self):
        job=self.job('timeout',policy={'timeout_seconds':1},seconds=20)
        child=self.start_worker();row=self.wait_for(job,{'failed'});self.stop_worker(child)
        result=json.loads(row['result']);self.assertEqual(result['reason'],'timeout')
        self.assertTrue(result['process_group_returned'])
        events=ops.operate('list_events',{},self.owner,self.config)
        self.assertIn((job,'failed'),[(e['job'],e['code']) for e in events])

    def test_completion_is_durable_scoped_and_default_error_consumer_unchanged(self):
        job=self.job('lifecycle');child=self.start_worker();row=self.wait_for(job,{'complete'});self.stop_worker(child)
        self.assertEqual(ops.operate('list_events',{},self.owner,self.config),[])
        events=ops.operate('list_events',{'kind':'all'},self.owner,self.config)
        complete=[e for e in events if e['job']==job and e['code']=='complete'];self.assertEqual(len(complete),1)
        self.assertEqual(complete[0]['attempt'],row['attempt'])
        other={**self.owner,'id':'other','owners':['other'],'notify':['other']}
        self.assertEqual(ops.operate('list_events',{'kind':'all'},other,self.config),[])
        with self.assertRaises(PermissionError):ops.operate('ack_event',{'seq':complete[0]['seq']},other,self.config)
        ops.operate('ack_event',{'seq':complete[0]['seq']},self.owner,self.config)
        self.assertEqual(ops.operate('list_events',{'kind':'all'},self.owner,self.config),[])

    def test_schema_and_budget_rejections_create_no_jobs(self):
        kind=self.profile()
        for policy in [{'timeout_seconds':172801},{'max_attempts':4},{'retry_on':['any_error']},{'timeout_seconds':True}]:
            with self.subTest(policy=policy),self.assertRaises(ValueError):self.job('invalid',kind,policy=policy)
        with self.assertRaises(ValueError):self.job('bad-input',kind,seconds='not-a-number')
        with self.assertRaises(ValueError):self.job('undeclared',kind,unregistered='parameter')
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM jobs').fetchone()[0],0)

    def test_cancel_ready_prepared_job_finishes_attempt_without_starting_runtime(self):
        self.job('busy-cancel',self.profile('generic.busy',lane='gpu'),seconds=10)
        pending=self.job('ready-cancel',self.profile('generic.prepared',lane='gpu',prepare=True),needs_preparation=True)
        child=self.start_worker();before=self.wait_for(pending,{'ready'})
        ops.operate('cancel_job',{'id':pending},self.owner,self.config)
        self.assertEqual(self.row(pending)['state'],'cancelled')
        self.stop_worker(child)
        with ops.db() as c:
            attempt=c.execute('SELECT state,ended FROM attempts WHERE id=?',(before['attempt'],)).fetchone()
        self.assertEqual(attempt['state'],'cancelled')
        self.assertIsNotNone(attempt['ended'])
        self.assertFalse(any(x['label']=='ready-cancel' and x['stage']=='run' for x in self.traces()))

    def test_cancel_preparation_returns_process_and_never_starts_runtime(self):
        job=self.job('preparing-cancel',self.profile('generic.prepared',lane='gpu',prepare=True),prepare_seconds=20,needs_preparation=True)
        child=self.start_worker();self.wait_for(job,{'preparing'})
        ops.operate('cancel_job',{'id':job},self.owner,self.config)
        row=self.wait_for(job,{'cancelled'});self.stop_worker(child)
        self.assertTrue(json.loads(row['result'])['process_group_returned'])
        self.assertFalse(any(x['label']=='preparing-cancel' and x['stage']=='run' for x in self.traces()))

    def failed_incident(self,key='incident',lane='cpu'):
        job=self.job(key,self.profile('generic.'+key,lane=lane),fail_always=True)
        row=self.claim(job)
        ops.finish(row,'failed',{'reason':'exit_nonzero','exit_code':8,'process_group_returned':True})
        return job

    def recover(self,job,action,actor=None,**kwargs):
        return ops.operate('recover_job',{'id':job,'action':action,**kwargs},actor or self.owner,self.config)

    def test_unanswered_reservation_expires_and_releases_only_its_resource_lane(self):
        failed=self.failed_incident('no-response',lane='gpu')
        cpu=self.job('other-lane',self.profile('generic.other-lane',lane='cpu'))
        gpu=self.job('same-lane',self.profile('generic.same-lane',lane='gpu'))
        child=self.start_worker();self.wait_for(cpu,{'complete'})
        self.assertEqual(self.row(gpu)['state'],'queued')
        with ops.db() as c:
            rec=recovery.view(c,failed)
            self.assertEqual(rec['state'],'waiting')
            self.assertAlmostEqual(rec['deadline']-rec['updated'],900,delta=.01)
            c.execute('UPDATE recovery SET deadline=? WHERE job=?',(time.time()-1,failed))
        self.wait_for(gpu,{'complete'});self.stop_worker(child)
        with ops.db() as c:self.assertEqual(recovery.view(c,failed)['state'],'deferred')

    def test_claim_is_idempotent_and_other_agents_cannot_take_lease_or_progress(self):
        job=self.failed_incident()
        other={**self.owner,'id':'agent-b'}
        unrelated={**self.owner,'id':'unrelated','owners':['unrelated'],'notify':['unrelated']}
        lease=self.recover(job,'claim')
        self.assertEqual(self.recover(job,'claim')['lease'],lease['lease'])
        with self.assertRaises(ValueError):self.recover(job,'claim',other)
        with self.assertRaises(PermissionError):self.recover(job,'progress',other,lease=lease['lease'])
        with self.assertRaises(ValueError):self.recover(job,'claim',unrelated)
        with ops.db() as c:self.assertNotIn('token',recovery.view(c,job))

    def test_claim_progress_never_extends_hard_deadline(self):
        with patch.object(recovery.time,'time',return_value=1000):
            job=self.failed_incident();lease=self.recover(job,'claim')['lease']
        with patch.object(recovery.time,'time',return_value=1800):
            first=self.recover(job,'progress',lease=lease)
            self.assertEqual(first['deadline'],2700)
        with patch.object(recovery.time,'time',return_value=2600):
            last=self.recover(job,'progress',lease=lease)
            self.assertEqual(last['deadline'],2800)
        with ops.db() as c:recovery.expire(c,now=2801)
        with patch.object(recovery.time,'time',return_value=2801),self.assertRaises(PermissionError):
            self.recover(job,'progress',lease=lease)

    def test_late_claim_requires_reactivation_at_back_of_queue(self):
        with patch.object(recovery.time,'time',return_value=1000):job=self.failed_incident()
        later=self.job('arrived-later')
        with patch.object(recovery.time,'time',return_value=1901),self.assertRaises(ValueError):self.recover(job,'claim')
        with ops.db() as c:recovery.expire(c,now=1901)
        future=time.time()+1
        with patch.object(recovery.time,'time',return_value=future):
            self.recover(job,'reactivate');lease=self.recover(job,'claim')['lease']
        self.assertGreater(self.row(job)['queue_order'],self.row(later)['queue_order'])
        self.assertTrue(lease)

    def test_three_recovery_executions_exhaust_budget_not_claim_or_progress(self):
        job=self.failed_incident('three-rounds')
        child=self.start_worker()
        for index in range(3):
            claimed=self.recover(job,'claim');self.assertEqual(claimed['rounds'],index)
            self.recover(job,'progress',lease=claimed['lease'])
            pending=self.recover(job,'resume',lease=claimed['lease'],reason='fixture repair')
            self.assertEqual(pending['rounds'],index)
            self.wait_for(job,{'failed'})
            with ops.db() as c:
                rec=recovery.view(c,job)
                self.assertEqual(rec['rounds'],index+1)
                self.assertEqual(rec['state'],'waiting' if index<2 else 'exhausted')
        with self.assertRaises(ValueError):self.recover(job,'claim')
        self.stop_worker(child)
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(job,)).fetchone()[0],4)

    def test_gpu_reservation_allows_preparing_next_gpu_job_and_independent_cpu(self):
        failed=self.failed_incident('reserved-gpu',lane='gpu')
        next_gpu=self.job('prepare-next',self.profile('generic.prepared',lane='gpu',prepare=True),needs_preparation=True)
        cpu=self.job('free-cpu',self.profile('generic.free-cpu',lane='cpu'))
        child=self.start_worker();self.wait_for(next_gpu,{'ready'});self.wait_for(cpu,{'complete'})
        self.assertEqual(self.row(next_gpu)['state'],'ready')
        schedule=ops.operate('get_job',{'id':next_gpu},self.owner,self.config)['scheduling']
        self.assertFalse(schedule['eligible'],'A reserved execution lane must not be reported as schedulable')
        self.assertFalse(any(x['label']=='prepare-next' and x['stage']=='run' for x in self.traces()))
        self.recover(failed,'defer');self.wait_for(next_gpu,{'complete'});self.stop_worker(child)

    def test_dependency_recovery_does_not_skip_blocked_jobs_preparation(self):
        upstream=self.failed_incident('recover-source')
        kind=self.profile('generic.needs-prep',prepare=True)
        params={'root':str(self.root),'label':'unblocked-consumer','needs_preparation':True}
        downstream=self.submit({'key':'unblocked-consumer','kind':kind,'owner':'owner','notify':'owner','params':params,'depends_on':[upstream]})
        child=self.start_worker();self.wait_for(downstream,{'blocked'})
        old=json.loads(self.row(upstream)['spec'])['params']
        lease=self.recover(upstream,'claim')['lease']
        self.recover(upstream,'resume',lease=lease,patch={'params':{**old,'fail_always':False}})
        self.wait_for(upstream,{'complete'})
        row=self.wait_for(downstream,{'complete','failed'});self.stop_worker(child)
        self.assertEqual(row['state'],'complete','Dependency recovery must still execute preparation')
        stages=[x['stage'] for x in self.traces() if x['label']=='unblocked-consumer' and x['event']=='start']
        self.assertEqual(stages,['prepare','run'])

    def test_queue_hold_preparing_finishes_then_resume_reuses_attempt_after_restart(self):
        kind=self.profile('generic.held-prepare',lane='gpu',prepare=True)
        job=self.job('held-prepare',kind,prepare_seconds=.8,needs_preparation=True)
        child=self.start_worker();self.wait_for(job,{'preparing'})
        held=ops.operate('hold_job',{'id':job},self.owner,self.config);self.assertTrue(held['changed'])
        ready=self.wait_for(job,{'ready'});self.stop_worker(child)
        self.assertFalse(any(x['stage']=='run' for x in self.traces()))
        cpu_kind=self.profile()
        child=self.start_worker()
        other=self.job('free-cpu-while-held',cpu_kind);self.wait_for(other,{'complete'})
        self.assertEqual(self.row(job)['state'],'ready')
        ops.operate('resume_job',{'id':job},self.owner,self.config)
        final=self.wait_for(job,{'complete'});self.stop_worker(child)
        self.assertEqual(final['attempt'],ready['attempt'])
        self.assertEqual(sum(x['stage']=='prepare' and x['event']=='start' for x in self.traces()),1)

    def test_queue_priority_passes_recovery_but_dependency_chain_stays_blocked(self):
        failed=self.failed_incident('priority-reserved')
        kind=self.profile('generic.consumer')
        dependent=self.submit({'key':'dependent-priority','kind':kind,'owner':'owner','notify':'owner',
            'params':{'root':str(self.root),'label':'dependent-priority'},'depends_on':[failed]})
        independent=self.job('priority-independent')
        for job in (independent,dependent):ops.operate('reorder_job',{'id':job,'position':'next'},self.owner,self.config)
        child=self.start_worker();self.wait_for(independent,{'complete'})
        self.wait_for(dependent,{'blocked'});self.stop_worker(child)
        self.assertFalse(any(x['label']=='dependent-priority' for x in self.traces()))
        self.assertEqual(self.row(failed)['state'],'failed')

    def test_queue_priority_never_interrupts_running_and_hold_does_not_block_following(self):
        first=self.job('already-running',seconds=1)
        child=self.start_worker();running=self.wait_for(first,{'running'})
        held=self.job('held-next');last=self.job('urgent-next')
        ops.operate('hold_job',{'id':held},self.owner,self.config)
        ops.operate('reorder_job',{'id':last,'position':'next'},self.owner,self.config)
        self.wait_for(last,{'complete'});self.assertEqual(self.row(held)['state'],'queued')
        ops.operate('resume_job',{'id':held},self.owner,self.config);self.wait_for(held,{'complete'});self.stop_worker(child)
        starts=[x['label'] for x in self.traces() if x['stage']=='run' and x['event']=='start']
        self.assertEqual(starts,['already-running','urgent-next','held-next'])
        self.assertEqual(self.row(first)['attempt'],running['attempt'])

    def test_retry_after_prepared_run_failure_prepares_new_attempt(self):
        job=self.job('retry-prepared',self.profile('generic.retry-prepared',prepare=True),
            policy={'max_attempts':2,'retry_on':['exit_nonzero'],'retry_backoff_seconds':0},needs_preparation=True,fail_once=True)
        child=self.start_worker();self.wait_for(job,{'complete'});self.stop_worker(child)
        self.assertEqual(sum(x['stage']=='prepare' and x['event']=='start' for x in self.traces()),2)
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(job,)).fetchone()[0],2)
