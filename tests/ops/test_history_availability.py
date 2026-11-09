import hashlib,json,tempfile,unittest,sqlite3
from pathlib import Path
import history_store

class HistoryAvailabilityTests(unittest.TestCase):
    def test_discovery_uses_active_snapshot_and_keeps_exact_history_keys(self):
        with sqlite3.connect(':memory:') as db:
            db.row_factory=sqlite3.Row
            history_store.init(db)
            def add(kind,identity,revision,snapshot):
                db.execute('INSERT INTO history_records VALUES(?,?,?,?,?,?)',
                    ('catalog',kind,identity,revision,snapshot,json.dumps({'original':{'id':identity,'label':'Preserved '+identity},'locations':[]})))
            add('model','old-only','1','old')
            add('model','model-a','source-v1','current')
            add('model','model-a','source-v2','current')
            add('dataset','dataset-a','123','current')
            add('agent_document','retired-instructions','1','current')
            add('model','native-collision','1','current')
            before=list(db.execute('SELECT * FROM history_records'))
            values=history_store.catalog_results(db, ['native-collision'])
            self.assertEqual([x['id'] for x in values],['dataset-a','model-a'])
            self.assertEqual(values[1]['history_ref']['revision'],'source-v2')
            self.assertEqual(values[1]['detail_action'],'results.historical')
            ref=values[1]['history_ref']
            self.assertEqual(history_store.get_record(db,ref['source'],ref['kind'],ref['id'],ref['revision'])['body']['original']['id'],'model-a')
            self.assertEqual(list(db.execute('SELECT * FROM history_records')),before)

    def test_mcp_list_registry_includes_preserved_models(self):
        from test_support import IsolatedOps
        import ops
        fixture=IsolatedOps();fixture.setUp()
        try:
            with ops.db() as db:
                db.execute('INSERT INTO history_records VALUES(?,?,?,?,?,?)',
                    ('catalog','model','preserved-model','1','active',json.dumps({'original':{'id':'preserved-model'},'locations':[]})))
            values=ops.operate('list_registry',{},fixture.reader,fixture.config)
            self.assertEqual(len(values),1)
            self.assertEqual(values[0]['record_type'],'historical')
            record=ops.operate('get_history',values[0]['history_ref'],fixture.reader,fixture.config)
            self.assertEqual(record['id'],'preserved-model')
        finally:
            fixture.doCleanups()

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
