import json
import ops
import job_contract
from test_support import IsolatedOps


class JobClassificationTests(IsolatedOps):
    def test_classification_survives_reads_without_changing_runner_or_verification(self):
        job = self.submit({**self.spec(), 'category':'sdg', 'execution_mode':'pilot'})
        detail = ops.operate('get_job', {'id':job}, self.owner, self.config)
        listing = ops.operate('list_jobs', {'category':'sdg','execution_mode':'pilot'}, self.owner, self.config)
        self.assertEqual(detail['spec']['kind'], 'cpu.smoke')
        self.assertEqual((detail['category'],detail['execution_mode']), ('sdg','pilot'))
        self.assertEqual(listing[0]['id'], job)
        entity = ops.job_history.entity(detail, detail['spec'])
        self.assertEqual(entity['properties']['category'], 'sdg')
        self.assertIsNone(entity['properties']['verification']['level'])
        self.assertEqual(self.row(job)['state'], 'queued')

    def test_optional_defaults_preserve_old_idempotency_but_changed_metadata_conflicts(self):
        job = self.submit()
        # Simulate a pre-contract stored spec, without changing its execution.
        with ops.db() as c:
            c.execute('UPDATE jobs SET spec=? WHERE id=?', (json.dumps(self.spec()),job))
        self.assertEqual(self.submit({**self.spec(), 'category':'other','execution_mode':None}), job)
        with self.assertRaises(ValueError):
            self.submit({**self.spec(),'category':'evaluation'})
        self.assertIsNone(ops.operate('get_job', {'id':job}, self.owner, self.config)['execution_mode'])

    def test_invalid_metadata_is_rejected_before_admission(self):
        for field, value in [('category','SDG'),('category',None),('category',[]),
                             ('execution_mode','Pilot-test'),('execution_mode',True),('execution_mode',{})]:
            with self.subTest(field=field,value=value), self.assertRaises(ValueError):
                self.submit({**self.spec(),field:value})
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT COUNT(*) FROM jobs').fetchone()[0], 0)

    def test_shared_filters_and_paging_do_not_truncate_before_filter(self):
        ids = [self.submit({**self.spec('j'+str(i)), 'category':'sdg' if i==251 else 'other'}) for i in range(252)]
        filtered = ops.operate('list_jobs', {'category':'sdg'}, self.owner, self.config)
        self.assertEqual([x['id'] for x in filtered], [ids[-1]])
        full = ops.operate('list_jobs', {'limit':1000}, self.owner, self.config)
        first = ops.operate('list_jobs', {}, self.owner, self.config)
        second = ops.operate('list_jobs', {'offset':250}, self.owner, self.config)
        self.assertEqual(first+second, full)
        self.assertEqual(ops.operate('list_jobs', {'limit':1000}, self.reader, self.config), full)
        for args in [{'offset':-1},{'limit':True},{'limit':1001},{'category':[]},{'execution_mode':'unknown'}]:
            with self.subTest(args=args), self.assertRaises(ValueError):
                ops.operate('list_jobs', args, self.owner, self.config)

    def test_schema_and_runtime_share_the_same_enums(self):
        schema = ops.tool_schema('submit_job')
        self.assertNotIn('category',schema['required'])
        self.assertEqual(schema['properties']['category']['enum'],list(job_contract.CATEGORIES))
        for i, category in enumerate(job_contract.CATEGORIES):
            self.submit({**self.spec('category'+str(i)),'category':category})
        contract=ops.operate('get_job_contract',{},self.reader,self.config)
        self.assertEqual(contract['revision'],job_contract.REVISION)
        self.assertEqual(contract['submit_schema'],schema)
