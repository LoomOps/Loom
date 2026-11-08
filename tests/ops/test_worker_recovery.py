"""Real CPU children and durable restart behavior in isolated temporary storage."""
import json
import multiprocessing as mp
from pathlib import Path
import time
from unittest.mock import patch
import ops
from test_support import IsolatedOps


def fixture_worker(config, pause_after_receipt=False):
    ops.STOP=False
    ops.guard=lambda _: None
    if pause_after_receipt:
        def pause(*_):
            time.sleep(30)
        ops.mark_returned=pause
    ops.worker(config)


class WorkerRecoveryTests(IsolatedOps):
    def start_worker(self, pause=False):
        child=mp.get_context('fork').Process(target=fixture_worker,args=(self.config,pause))
        child.start()
        self.addCleanup(self.stop_worker,child)
        return child

    @staticmethod
    def stop_worker(child):
        if child.is_alive():child.terminate()
        child.join(8)
        if child.is_alive():child.kill();child.join(2)

    def wait_for(self,job,states,seconds=10):
        until=time.monotonic()+seconds
        while time.monotonic()<until:
            row=self.row(job)
            if row['state'] in states:return row
            time.sleep(.03)
        self.fail('Job did not reach expected state: '+str(self.row(job)))

    def test_real_producer_success_and_failure_report_results_and_events(self):
        success=self.submit(self.spec('real-success'))
        failure=self.submit(self.spec('real-failure',fail=True))
        child=self.start_worker()
        row=self.wait_for(success,{'complete'})
        self.assertTrue(json.loads(row['result'])['execution']['process_group_returned'])
        self.wait_for(failure,{'failed'})
        events=ops.operate('list_events',{},self.owner,self.config)
        self.assertIn((failure,'failed'),[(e['job'],e['code']) for e in events])
        self.stop_worker(child)
        self.assertEqual(child.exitcode,0)

    def test_cancel_running_real_process_returns_and_next_job_can_execute(self):
        slow=self.submit(self.spec('cancel-real',seconds=10))
        child=self.start_worker()
        self.wait_for(slow,{'running'})
        ops.operate('cancel_job',{'id':slow},self.owner,self.config)
        row=self.wait_for(slow,{'cancelled'})
        self.assertTrue(json.loads(row['result'])['process_group_returned'])
        next_job=self.submit(self.spec('after-cancel'))
        self.wait_for(next_job,{'complete'})
        self.stop_worker(child)
        self.assertEqual(child.exitcode,0)

    def test_crash_between_return_receipt_and_db_commit_does_not_rerun(self):
        job=self.submit(self.spec('receipt-gap'))
        first=self.start_worker(pause=True)
        row=self.wait_for(job,{'running'})
        marker=self.local/'returns'/(row['attempt']+'.json')
        until=time.monotonic()+10
        while not marker.exists() and time.monotonic()<until:time.sleep(.03)
        self.assertTrue(marker.is_file())
        # Only the controller is killed after the receipt proves child return.
        first.kill();first.join(2)
        second=self.start_worker()
        final=self.wait_for(job,{'complete'})
        self.assertEqual(final['attempt'],row['attempt'])
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM attempts WHERE job=?',(job,)).fetchone()[0],1)
            self.assertEqual(c.execute('SELECT count(*) FROM entity_revisions WHERE job=?',(job,)).fetchone()[0],1)
        self.stop_worker(second)
