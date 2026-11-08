import json
import sqlite3
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from dashboard_read import resources
from ops import execution_lane

class QueueResourceTests(unittest.TestCase):
    def test_all_running_jobs_reserve_execution_slots(self):
        with sqlite3.connect(':memory:') as db, TemporaryDirectory() as directory:
            db.row_factory=sqlite3.Row
            db.execute('CREATE TABLE jobs(id,state,spec,attempt,owner,submitter,created)')
            for i,state,owner,kind in [('cpu','running','owner','cpu.smoke'),('gpu','running','owner','cyclo.smoke'),
                ('waiting','queued','owner','cyclo.smoke'),('private','running','other','cyclo.smoke'),
                ('outbox','processing','owner','cyclo.smoke'),('done','complete','owner','cpu.smoke')]:
                db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)',(i,state,json.dumps({'kind':kind,'profile':{'runtime':{'gpu':int(kind=='cyclo.smoke')}}}),'a-'+i,owner,owner,1))
            result=resources({'machine_id':'test'},Path(directory),Path(directory),connection=db,
                actor={'id':'owner'},lane_for=execution_lane)
            lease=result['sections']['leases']
            self.assertEqual(lease['status'],'ok')
            self.assertEqual({r['job_id'] for r in lease['data']['registered']},{'cpu','gpu','private'})
            self.assertEqual(lease['data']['queued_jobs'],1)
            self.assertEqual({r['resource'] for r in lease['data']['registered']},{'CPU 실행 슬롯','GPU 실행 슬롯'})
            db.execute("UPDATE jobs SET state='complete' WHERE state='running'")
            idle=resources({'machine_id':'test'},Path(directory),Path(directory),connection=db,
                actor={'id':'owner'},lane_for=execution_lane)['sections']['leases']
            self.assertEqual(idle['data']['registered'],[])


class StorageResourceTests(unittest.TestCase):
    def test_configured_stores_and_missing_store_are_not_fabricated(self):
        from unittest.mock import patch
        with TemporaryDirectory() as directory:
            root=Path(directory)
            first=root/'first';second=root/'second'
            first.mkdir();second.mkdir()
            with patch.dict('os.environ',{'LOOM_DATA_PATHS':str(first)+':'+str(second)}):
                disks=resources({'machine_id':'test'},first,root)['sections']['storage']['data']
                self.assertEqual([d['path'] for d in disks],['data/0','data/1'])
                self.assertTrue(all(d['status']=='ok' and d['total_bytes']>0 for d in disks))
                second.rmdir()
                disks=resources({'machine_id':'test'},first,root)['sections']['storage']['data']
                self.assertEqual(disks[0]['status'],'ok')
                self.assertEqual(disks[1]['status'],'unavailable')
                self.assertNotIn('total_bytes',disks[1])
            with patch.dict('os.environ',{'LOOM_DATA_PATHS':str(first)}):
                self.assertEqual(len(resources({'machine_id':'test'},first,root)['sections']['storage']['data']),1)

if __name__=='__main__':unittest.main()
