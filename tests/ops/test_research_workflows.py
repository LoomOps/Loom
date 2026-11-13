"""Logical outcome, operational isolation and additive reconciliation boundaries."""
import copy
import json
from unittest.mock import patch
import ops
import research
import workflows
import dashboard_read
from test_support import IsolatedOps


class ResearchWorkflowTests(IsolatedOps):
    def request(self):
        return {'key':'one-outcome','label':'Generate and validate','owner':'owner','notify':'owner','category':'sdg',
            'nodes':[{'id':'generate','label':'Generate','spec':{'kind':'cpu.smoke','params':{'seconds':0,'fail':False}}},
                     {'id':'verify','label':'Verify','depends_on':['generate'],'spec':{'kind':'cpu.smoke','params':{'seconds':0,'fail':False}}}],
            'outputs':{'receipt':{'node':'verify','entity':'observation'}}}

    def workflow(self,args=None):
        with patch.object(ops,'guard'):return ops.operate('submit_workflow',args or self.request(),self.owner,self.config)

    def catalog(self):
        with ops.db() as c:return dashboard_read.projection(c,self.reader,self.config,self.local,self.data)[0]

    def test_one_visible_job_pins_nodes_dependencies_and_output_aliases(self):
        value=self.workflow();root=value['id'];ids=value['nodes']
        self.assertEqual(self.workflow(),value)
        self.assertEqual([j['id'] for j in ops.operate('list_jobs',{},self.reader,self.config)],[root])
        self.assertEqual(len(ops.operate('list_jobs',{'view':'all'},self.reader,self.config)),3)
        self.assertEqual(json.loads(self.row(ids['verify'])['spec'])['depends_on'],[ids['generate']])
        with ops.db() as c:
            self.assertEqual(workflows.resolve_inputs(c,{'input':{'job':root,'entity':'receipt'}}),{'input':{'job':ids['verify'],'entity':'observation'}})
            self.assertEqual(c.execute('SELECT COUNT(*) FROM attempts').fetchone()[0],0)
        before=[self.row(i) for i in ids.values()]
        bad=self.request();bad['label']='Different outcome'
        with self.assertRaisesRegex(ValueError,'conflict'):self.workflow(bad)
        self.assertEqual([self.row(i) for i in ids.values()],before)

    def test_atomic_rejection_and_no_orphan_nodes(self):
        for mutate in [lambda a:a['nodes'][1]['spec']['params'].update(seconds=100),
                       lambda a:a['nodes'][0].update(depends_on=['verify']),
                       lambda a:a['outputs']['receipt'].update(entity='missing')]:
            a=self.request();mutate(a)
            with self.assertRaises((ValueError,PermissionError)):self.workflow(a)
            with ops.db() as c:
                self.assertEqual(c.execute('SELECT COUNT(*) FROM jobs').fetchone()[0],0)
                self.assertEqual(c.execute('SELECT COUNT(*) FROM job_workflows').fetchone()[0],0)

    def test_execution_readiness_failures_and_completion_are_not_fabricated(self):
        value=self.workflow();root=value['id'];ids=value['nodes']
        with ops.db() as c:
            row=c.execute('SELECT * FROM jobs WHERE id=?',(ids['verify'],)).fetchone()
            self.assertFalse(ops.queue_control.scheduling(c,row,ops.execution_lane)['eligible'])
            c.execute("UPDATE jobs SET state='failed' WHERE id=?",(ids['generate'],))
            self.assertEqual(workflows.get(c,root)['errors'],[ids['generate']])
            self.assertEqual(workflows.get(c,root)['state'],'failed')
            c.execute("UPDATE jobs SET state='processing',result=? WHERE id=?",(json.dumps({'artifact_ready':True}),ids['generate']))
            self.assertTrue(ops.queue_control.scheduling(c,row,ops.execution_lane)['eligible'])
            self.assertFalse(workflows.get(c,root)['result']['artifact_ready'])
            c.execute("UPDATE jobs SET state='complete',result=?",(json.dumps({'artifact_ready':True}),))
            workflows.refresh(c);workflows.refresh(c)
            self.assertEqual(workflows.get(c,root)['state'],'complete')
            self.assertEqual(c.execute('SELECT COUNT(*) FROM events WHERE job=?',(root,)).fetchone()[0],1)
            self.assertEqual(c.execute('SELECT COUNT(*) FROM attempts').fetchone()[0],0)

    def test_cancel_and_hold_delegate_to_real_executions_and_preserve_pins(self):
        value=self.workflow();root=value['id'];ids=list(value['nodes'].values())
        originals=[json.loads(self.row(i)['spec']) for i in ids]
        ops.operate('hold_job',{'id':root},self.owner,self.config)
        self.assertTrue(all(self.row(i)['queue_held'] for i in ids))
        ops.operate('resume_job',{'id':root},self.owner,self.config)
        ops.operate('cancel_job',{'id':root},self.owner,self.config)
        self.assertEqual([self.row(i)['state'] for i in ids],['cancelled','cancelled'])
        self.assertEqual([json.loads(self.row(i)['spec']) for i in ids],originals)
        with self.assertRaises(PermissionError):ops.operate('cancel_job',{'id':root},self.reader,self.config)

    def test_preview_apply_and_replay_preserve_original_records(self):
        main=self.submit(self.spec('science'));support=self.submit(self.spec('support'))
        before=[self.row(main),self.row(support)]
        args={'key':'review-1','reason':'Verified same requested outcome','changes':[{'kind':'job','id':support,'presentation':{'parent_job':main,'role':'support','scope':'research'}}]}
        with patch.object(ops,'guard'):
            self.assertTrue(ops.operate('curate_research',args,self.owner,self.config)['dry_run'])
            self.assertEqual(len(ops.operate('list_jobs',{},self.reader,self.config)),2)
            self.assertTrue(ops.operate('curate_research',{**args,'dry_run':False},self.owner,self.config)['applied'])
            self.assertFalse(ops.operate('curate_research',{**args,'dry_run':False},self.owner,self.config)['applied'])
            conflict=copy.deepcopy(args);conflict['reason']='Different review'
            with self.assertRaises(ValueError):ops.operate('curate_research',conflict,self.owner,self.config)
        self.assertEqual([j['id'] for j in ops.operate('list_jobs',{},self.reader,self.config)],[main])
        self.assertEqual([self.row(main),self.row(support)],before)
        self.assertEqual(next(e for e in self.catalog()['entities'] if e['id']==support)['presentation']['parent_job'],main)

    def test_operational_smoke_and_cycle_rejection(self):
        spec=self.spec();spec.pop('presentation');probe=self.submit(spec)
        self.assertFalse(ops.operate('list_jobs',{},self.reader,self.config))
        self.assertEqual(ops.operate('list_jobs',{'view':'operations'},self.reader,self.config)[0]['id'],probe)
        a=self.submit(self.spec('a'));b=self.submit(self.spec('b'))
        args={'key':'cycle','reason':'Invalid loop','changes':[{'kind':'job','id':a,'presentation':{'parent_job':b}},{'kind':'job','id':b,'presentation':{'parent_job':a}}],'dry_run':False}
        with patch.object(ops,'guard'),self.assertRaisesRegex(ValueError,'cycle'):ops.operate('curate_research',args,self.owner,self.config)
        with ops.db() as c:self.assertFalse(research.read(c))

    def test_partial_purpose_is_frozen_and_output_receipts_do_not_inflate_activity(self):
        spec={**self.spec(),'presentation':{'label':'A diagnostic','output_roles':{'observation':'support'}}}
        job=self.submit(spec)
        self.assertEqual(self.submit(spec),job)
        before=self.row(job)
        with self.assertRaises(ValueError):self.submit({**spec,'presentation':{'label':'Different'}})
        with ops.db() as c:
            values=[{'id':job+'.observation','kind':'experiment','loom_job':{'id':job},'loom_registry':{'id':job+'.observation'},'contribution':{'eligible':True}}]
            research.decorate(c,values)
        self.assertFalse(values[0]['presentation']['visible']);self.assertFalse(values[0]['contribution']['eligible'])
        self.assertEqual(self.row(job),before)

    def test_queue_keeps_real_consumers_and_reports_parallel_failure_without_false_completion(self):
        args=self.request();args['nodes'][1].pop('depends_on')
        value=self.workflow(args);ids=list(value['nodes'].values())
        with ops.db() as c:
            q=ops.queue_control.view(c,self.owner,ops.execution_lane)
            self.assertEqual({r['id'] for r in q['entries']},set(ids))
            self.assertEqual([r['id'] for r in q['logical_entries']],[value['id']])
            c.execute("UPDATE jobs SET state='failed' WHERE id=?",(ids[0],))
            c.execute("UPDATE jobs SET state='running' WHERE id=?",(ids[1],))
            workflows.refresh(c);workflows.refresh(c)
            main=workflows.get(c,value['id'])
            self.assertEqual(main['state'],'running');self.assertFalse(main['result']['artifact_ready'])
            self.assertEqual(main['errors'],[ids[0]])
            self.assertEqual(c.execute('SELECT count(*) FROM events WHERE job=?',(value['id'],)).fetchone()[0],1)
