import json
from unittest.mock import patch
import ops
from test_support import IsolatedOps

class AdmissionTests(IsolatedOps):
    def test_repeated_submission_is_idempotent_but_changed_input_conflicts(self):
        first=self.submit();self.assertEqual(self.submit(),first)
        with self.assertRaises(ValueError):self.submit(self.spec(seconds=1))
    def test_shared_owner_notify_and_valid_dependencies(self):
        for field in ('owner','notify'):
            with self.subTest(field=field):
                self.assertTrue(self.submit({**self.spec(field),field:'other'}))
        with self.assertRaises(PermissionError):self.submit({**self.spec(),'depends_on':['missing']})
        upstream=self.submit(self.spec('upstream'))
        downstream=self.submit({**self.spec('downstream'),'depends_on':[upstream]})
        self.assertEqual(json.loads(self.row(downstream)['spec'])['depends_on'],[upstream])
    def test_admission_fence_rejects_new_and_replayed_submission(self):
        self.submit()
        with ops.db() as c:c.execute("UPDATE node_control SET mode='fenced'")
        for spec in [self.spec(),self.spec('new')]:
            with self.assertRaises(PermissionError):self.submit(spec)
    def test_invalid_parameters_do_not_create_jobs(self):
        for value in [-1,21,True,0.5]:
            with self.subTest(value=value),self.assertRaises(ValueError):self.submit(self.spec(seconds=value))
        with ops.db() as c:self.assertEqual(c.execute('SELECT COUNT(*) FROM jobs').fetchone()[0],0)
    def test_cancel_requires_cancel_scope(self):
        job=self.submit()
        with self.assertRaises(PermissionError):ops.operate('cancel_job',{'id':job},self.reader,self.config)
        ops.operate('cancel_job',{'id':job},self.owner,self.config)
        self.assertTrue(self.row(job)['cancel'])
    def test_invalid_job_identity_is_rejected_before_sql(self):
        for identity in [[],{},'../escape','',None]:
            with self.subTest(identity=identity),self.assertRaises(ValueError):
                ops.operate('get_job',{'id':identity},self.owner,self.config)
