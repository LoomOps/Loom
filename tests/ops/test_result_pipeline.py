import contextlib
import io
import json
from unittest.mock import MagicMock,patch
import ops
from test_support import IsolatedOps

class ResultPipelineTests(IsolatedOps):
    def fake_process(self,command,**kwargs):
        # The producer boundary is mocked; SQLite and output validation are real.
        folder=ops.relative(self.data,'runs/'+self.active['id']+'/'+self.active['attempt'])
        ops.cpu(folder,{'seconds':0,'fail':False})
        child=MagicMock();child.poll.return_value=0;child.returncode=0
        child.__enter__.return_value=child
        return child
    def execute_fixture(self):
        self.active=self.claim(self.submit())
        with patch.object(ops,'guard'),patch.object(ops.subprocess,'Popen',side_effect=self.fake_process),patch.object(ops,'wait_process_group',return_value=True):
            ops.execute(self.active,self.config)
        return self.row(self.active['id'])
    def test_execution_returns_before_result_indexing(self):
        row=self.execute_fixture()
        self.assertEqual(row['state'],'processing')
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT state FROM attempts').fetchone()[0],'complete')
            self.assertEqual(c.execute('SELECT COUNT(*) FROM entity_revisions').fetchone()[0],0)
        with patch.object(ops,'guard'):ops.process_result({**row,'retries':0},self.config)
        self.assertEqual(self.row(row['id'])['state'],'complete')
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT COUNT(*) FROM result_outbox').fetchone()[0],0)
            self.assertEqual(c.execute('SELECT COUNT(*) FROM entity_revisions').fetchone()[0],1)
    def test_index_failure_retries_without_reexecuting_producer(self):
        row=self.execute_fixture()
        with patch.object(ops,'guard'),patch.object(ops.registry,'index',side_effect=RuntimeError('fixture failure')),patch.object(ops.subprocess,'Popen') as launch,contextlib.redirect_stderr(io.StringIO()):
            for retries in range(3):ops.process_result({**self.row(row['id']),'retries':retries},self.config)
            launch.assert_not_called()
        self.assertEqual(self.row(row['id'])['state'],'result_failed')
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT COUNT(*) FROM attempts').fetchone()[0],1)
            self.assertEqual([r[0] for r in c.execute('SELECT code FROM events ORDER BY seq')],['result_processing_failed','result_retry_required'])
        ops.operate('reindex_job',{'id':row['id'],'attempt':row['attempt']},self.owner,self.config)
        with patch.object(ops,'guard'):ops.process_result({**self.row(row['id']),'retries':0},self.config)
        self.assertEqual(self.row(row['id'])['state'],'complete')
    def test_cancel_during_processing_does_not_publish(self):
        row=self.execute_fixture()
        with ops.db() as c:c.execute('UPDATE jobs SET cancel=1 WHERE id=?',(row['id'],))
        ops.process_result({**self.row(row['id']),'retries':0},self.config)
        self.assertEqual(self.row(row['id'])['state'],'cancelled')
        with ops.db() as c:self.assertEqual(c.execute('SELECT COUNT(*) FROM entity_revisions').fetchone()[0],0)
    def test_failed_producer_reports_to_owner_and_never_indexes(self):
        row=self.claim(self.submit())
        ops.finish(row,'failed',{'exit_code':3,'process_group_returned':True})
        events=ops.operate('list_events',{},self.owner,self.config)
        self.assertEqual([(e['job'],e['target'],e['code']) for e in events],[(row['id'],'owner','failed')])
        with ops.db() as c:self.assertEqual(c.execute('SELECT COUNT(*) FROM entity_revisions').fetchone()[0],0)
    def test_downstream_starts_after_artifact_return_while_indexing_is_pending(self):
        upstream=self.submit(self.spec('upstream'))
        with ops.db() as c:c.execute("UPDATE jobs SET state='processing',result=? WHERE id=?",(json.dumps({'artifact_ready':True}),upstream))
        downstream=self.submit({**self.spec('downstream'),'depends_on':[upstream]})
        launched=[]
        def execute(row,config,stage="run"):
            launched.append(row['id']);ops.STOP=True
        with patch.object(ops,'guard'),patch.object(ops,'execute',side_effect=execute),patch.object(ops.signal,'signal'):
            ops.worker(self.config)
        self.assertEqual(launched,[downstream]);self.assertEqual(self.row(upstream)['state'],'processing')
    def test_failed_dependency_blocks_and_notifies_without_launch(self):
        upstream=self.submit(self.spec('upstream'))
        with ops.db() as c:c.execute("UPDATE jobs SET state='failed' WHERE id=?",(upstream,))
        downstream=self.submit({**self.spec('downstream'),'depends_on':[upstream]})
        def stop(_):ops.STOP=True
        with patch.object(ops,'guard'),patch.object(ops,'execute') as launch,patch.object(ops.signal,'signal'),patch.object(ops.time,'sleep',side_effect=stop):
            ops.worker(self.config);launch.assert_not_called()
        self.assertEqual(self.row(downstream)['state'],'blocked')
        self.assertEqual(ops.operate('list_events',{},self.owner,self.config)[0]['code'],'dependency_failed')
