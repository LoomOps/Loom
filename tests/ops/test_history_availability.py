import hashlib,json,tempfile,unittest
from pathlib import Path
import history_store

class HistoryAvailabilityTests(unittest.TestCase):
    def test_recorded_missing_is_not_original_deletion(self):
        original={'source_ref':'data/1/models/selected','status':'missing'}
        value=history_store.availability([original],{'machine_id':'one'},Path('/not-mounted'))
        self.assertEqual(value['locations'][0]['status'],'not_connected')
        self.assertEqual(original['status'],'missing')
    def test_present_removed_corrupt_and_escape(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'.loom-storage.json').write_text(json.dumps({'storage_id':'fixture'}))
            f=root/'model.json';f.write_text('data')
            item={'machine_id':'one','storage_id':'fixture','target_ref':'model.json','status':'available','size':4,'sha256':hashlib.sha256(b'data').hexdigest()}
            config={'machine_id':'one','storage_id':'fixture'}
            def state():return history_store.availability([item],config,root)['locations'][0]['status']
            self.assertEqual(state(),'verified');f.write_text('oops');self.assertEqual(state(),'mismatch')
            f.unlink();self.assertEqual(state(),'missing')
            item['target_ref']='../other';self.assertEqual(state(),'unavailable')
            self.assertEqual(item['status'],'available')
    def test_wrong_storage_identity_never_reads_copy(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'.loom-storage.json').write_text(json.dumps({'storage_id':'wrong'}))
            item={'machine_id':'one','storage_id':'fixture','target_ref':'x','status':'available'}
            value=history_store.availability([item],{'machine_id':'one','storage_id':'fixture'},root)
            self.assertEqual(value['locations'][0]['status'],'unavailable')
