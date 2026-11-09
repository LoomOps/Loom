"""Disposable host state; mock only the private executor transport."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
import runtime_doctor


class RuntimeDoctorTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.base=Path(self.temp.name)
        self.root=self.base/'ops';self.executor=self.base/'executor';self.data=self.base/'data'
        (self.root/'secrets').mkdir(parents=True);self.executor.mkdir();self.data.mkdir()
        (self.root/'secrets/executor-token').write_text('private-doctor-token')
        cfg={'runtimes':{'sim':{'image':'sha256:'+'a'*64,'driver_capabilities':['compute','utility','graphics'],
            'mounts':[{'source':'/host/assets','target':'/assets','readonly':True}]}},'instances':{'alpha':{
            'token_sha256':hashlib.sha256(b'private-doctor-token').hexdigest(),'uid':os.getuid(),'gid':os.getgid(),
            'gpu':True,'runtimes':['sim'],'data':str(self.data)}}}
        (self.executor/'config.json').write_text(json.dumps(cfg));self.calls=[]

    def transport(self,executor,token,action,data):
        self.calls.append((action,data));self.assertEqual(token,'private-doctor-token')
        if action=='start':
            self.work=Path(data['host_output']);return {'ticket':'alpha.'+data['ticket']}
        if action=='status':
            (self.work/'probe-result.json').write_text(json.dumps({'passed':True,'uid':os.getuid()}))
            return {'returned':True,'exit_code':0,'image_id':'sha256:'+'a'*64}
        return {'returned':True}

    def test_inspect_is_inert_and_probe_uses_broker_policy_actual_uid_and_private_receipt(self):
        result,_,_=runtime_doctor.inspect(self.root,self.executor,'sim',True,['/isaac-sim/python.sh'])
        self.assertEqual(result['status'],'unverified');self.assertEqual(list(self.data.iterdir()),[])
        result=runtime_doctor.run(self.root,self.executor,'sim',gpu=True,paths=['/isaac-sim/python.sh'],request=self.transport)
        self.assertEqual(result['status'],'passed');self.assertFalse(self.work.exists())
        start=next(d for a,d in self.calls if a=='start')
        self.assertEqual(start['runtime_id'],'sim');self.assertEqual(start['lane'],'gpu');self.assertEqual(start['stage'],'doctor')
        self.assertNotIn('network',start);self.assertNotIn('driver_capabilities',start)
        record=Path(result['record']);self.assertEqual(record.stat().st_mode&0o777,0o600)
        self.assertNotIn('private-doctor-token',record.read_text());self.assertIn('no scene',result['scope'])

    def test_busy_probe_never_replays_or_cancels_another_gpu_ticket(self):
        def busy(*args):self.calls.append(args[2]);return {'busy':True}
        result=runtime_doctor.run(self.root,self.executor,'sim',gpu=True,request=busy)
        self.assertEqual(result['status'],'pending');self.assertNotIn('cancel',self.calls)
        self.assertEqual(self.calls.count('start'),1);self.assertFalse(Path(result['workspace']).exists())

    def test_unknown_start_retains_workspace_and_does_not_replay(self):
        def unknown(executor,token,action,data):self.calls.append(action);raise TimeoutError()
        result=runtime_doctor.run(self.root,self.executor,'sim',gpu=True,request=unknown)
        self.assertEqual(result['status'],'unverified');self.assertTrue(Path(result['workspace']).is_dir())
        self.assertEqual(self.calls,['start','cancel']);self.assertIn('retained',result['cleanup'])

    def test_timeout_cancels_only_owned_ticket_and_removes_workspace_after_return(self):
        def slow(executor,token,action,data):
            self.calls.append((action,data))
            return {'returned':action=='cancel'}
        result=runtime_doctor.run(self.root,self.executor,'sim',timeout=1,request=slow,clock=iter([0,2]).__next__)
        self.assertEqual(result['status'],'failed');self.assertEqual(result['cleanup'],'returned_and_workspace_removed')
        cancel=next(d for a,d in self.calls if a=='cancel');self.assertEqual(cancel['ticket'],result['ticket'])

    def test_auth_loss_after_start_does_not_claim_return_or_delete_live_workspace(self):
        def revoked(executor,token,action,data):
            self.calls.append(action)
            if action=='start':return {'ticket':'alpha.'+data['ticket']}
            raise runtime_doctor.ExecutorError(403)
        result=runtime_doctor.run(self.root,self.executor,'sim',request=revoked)
        self.assertEqual(result['status'],'unverified');self.assertTrue(Path(result['workspace']).exists())
        self.assertEqual(self.calls,['start','status','cancel']);self.assertIn('unconfirmed',result['cleanup'])

    def test_missing_artifact_with_exit_zero_fails_diagnostic(self):
        def missing(executor,token,action,data):return {'returned':True,'exit_code':0}
        result=runtime_doctor.run(self.root,self.executor,'sim',request=missing)
        self.assertEqual(result['status'],'failed');self.assertEqual(result['error_type'],'ValueError')
