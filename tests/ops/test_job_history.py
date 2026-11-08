import json
import sqlite3
import unittest
from unittest.mock import patch
import history_store
import job_history
import ops

class JobHistoryTests(unittest.TestCase):
    def setUp(self):
        self.db=sqlite3.connect(':memory:');self.db.row_factory=sqlite3.Row
        history_store.init(self.db)
        self.db.execute('CREATE TABLE jobs(id)')
        self.owner={'id':'owner','scopes':['read']}
        self.config={'history_readers':['owner']}
        e={'id':'work:old','label':'Old Job','properties':{'state':'paused','created_at':1,'record_role':'work'},'steps':[{'id':'step-a'}]}
        for snapshot in ['old','new']:
            self.db.execute('INSERT INTO history_records VALUES(?,?,?,?,?,?)',
                ('queue','job_view',e['id'],snapshot,snapshot,json.dumps({'entity':e})))
    def tearDown(self):self.db.close()
    def test_latest_view_once_and_permissions(self):
        records=job_history.imported(self.db,self.owner,self.config)
        self.assertEqual(len(records),1)
        self.assertEqual(records[0]['loom_history']['revision'],'new')
        self.assertEqual(len(job_history.imported(self.db,{'id':'other'},self.config)),1)
        self.assertEqual(job_history.get(self.db,'work:old',{'id':'other'},self.config)['id'],'work:old')
    def test_readable_without_worker_admission(self):
        record=job_history.get(self.db,'work:old',self.owner,self.config)
        self.assertEqual(record['state'],'paused')
        self.assertEqual(record['category'],'other')
        self.assertIsNone(record['execution_mode'])
        self.assertTrue(record['migration']['read_only'])
        self.assertEqual(self.db.execute('SELECT COUNT(*) FROM jobs').fetchone()[0],0)
        self.db.execute("INSERT INTO jobs VALUES('work:old')")
        self.assertEqual(job_history.imported(self.db,self.owner,self.config),[])
    def test_get_job_uses_same_read_model(self):
        from contextlib import contextmanager
        @contextmanager
        def db():yield self.db
        with patch.object(ops,'db',db):
            self.assertTrue(ops.operate('get_job',{'id':'work:old'},self.owner,self.config)['migration']['read_only'])
    def test_native_duration_and_missing_verification_preserved(self):
        e=job_history.entity({'id':'abc','state':'complete','created':1,'attempt':'a',
            'attempts':[{'id':'a','state':'complete','started':2,'ended':4}]},{'kind':'cpu.smoke'})
        self.assertEqual(e['execution_intervals'][0]['end'],4)
        self.assertIsNone(e['properties']['verification']['level'])

if __name__=='__main__':unittest.main()
