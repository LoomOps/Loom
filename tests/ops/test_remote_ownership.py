import sqlite3
import tempfile
from pathlib import Path
import unittest
import remote_fence


class RemoteOwnershipTests(unittest.TestCase):
    def test_only_current_session_can_execute_and_handoff_requires_return(self):
        with tempfile.TemporaryDirectory() as t:
            path=Path(t)/'authority.sqlite3';remote_fence.initialize(path)
            with sqlite3.connect(path) as c:
                claim={'session':'a'*64,'epoch':0}
                remote_fence.apply(c,'source','claim',claim)
                with self.assertRaises(PermissionError):
                    remote_fence.apply(c,'source','claim',{'session':'b'*64,'epoch':0})
                with self.assertRaises(PermissionError):
                    remote_fence.apply(c,'source','seal',{'archive':'c'*64})
                remote_fence.apply(c,'source','release',claim)
                remote_fence.apply(c,'source','seal',{'archive':'c'*64})
                remote_fence.apply(c,'target','activate',{'archive':'c'*64})
                with self.assertRaises(PermissionError):remote_fence.apply(c,'source','claim',claim)
                current=remote_fence.apply(c,'target','status',{})
                self.assertTrue(remote_fence.apply(c,'target','claim',{'session':'d'*64,'epoch':current['epoch']})['authorized'])


class ArchiveCredentialTests(unittest.TestCase):
    def test_export_and_old_archive_restore_strip_all_credentials(self):
        import portability
        with sqlite3.connect(':memory:') as c:
            for name in ('credentials','resident_tokens','user_profiles'):
                c.execute('CREATE TABLE '+name+'(id TEXT)');c.execute('INSERT INTO '+name+' VALUES(?)',('preserved-user' if name=='user_profiles' else 'secret-digest',))
            portability.clear_credentials(c)
            self.assertEqual(c.execute('SELECT count(*) FROM credentials').fetchone()[0],0)
            self.assertEqual(c.execute('SELECT count(*) FROM resident_tokens').fetchone()[0],0)
            self.assertEqual(c.execute('SELECT id FROM user_profiles').fetchone()[0],'preserved-user')
        with sqlite3.connect(':memory:') as old:portability.clear_credentials(old)

    def test_single_store_archive_refuses_silent_secondary_payload_loss(self):
        import portability,json
        with sqlite3.connect(':memory:') as c:
            c.execute('CREATE TABLE entity_locations(machine_id,storage_id,status)')
            c.execute('CREATE TABLE jobs(state,spec)')
            cfg={'machine_id':'m','storage_id':'s0'}
            portability.require_single_store(c,cfg)
            c.execute("INSERT INTO entity_locations VALUES('m','s1','available')")
            with self.assertRaisesRegex(ValueError,'multi-store'):portability.require_single_store(c,cfg)
            c.execute('DELETE FROM entity_locations')
            c.execute('INSERT INTO jobs VALUES(?,?)',('queued',json.dumps({'output_storage':'data/1'})))
            with self.assertRaisesRegex(ValueError,'secondary store'):portability.require_single_store(c,cfg)

from test_support import IsolatedOps

class ArchiveRoundTripTests(IsolatedOps):
    def test_real_export_and_restore_do_not_carry_resident_tokens(self):
        import json,sys
        from unittest.mock import patch
        import ops,portability
        with ops.db() as c:
            c.execute('INSERT INTO resident_tokens VALUES(?,?,?,?,?,?,?,NULL,NULL)',
                      ('token-id','digest-only','fixture','owner','["read"]',1,'admin'))
        archive=self.root/'archive'
        with patch.object(portability,'binding',return_value=(self.data,self.config)),patch.object(sys,'argv',['portability','export',str(self.root),str(archive)]):
            portability.main()
        with sqlite3.connect(archive/'ops.sqlite3') as c:
            self.assertEqual(c.execute('SELECT count(*) FROM resident_tokens').fetchone()[0],0)
            # Simulate an archive produced before the export correction.
            c.execute('INSERT INTO resident_tokens VALUES(?,?,?,?,?,?,?,NULL,NULL)',
                      ('old-token','old-digest','fixture','owner','["read"]',1,'admin'))
        manifest=json.loads((archive/'manifest.json').read_text())
        manifest['db_sha256']=portability.sha(archive/'ops.sqlite3')
        (archive/'manifest.json').write_text(json.dumps(manifest))
        target=self.root/'target';(target/'runtime').mkdir(parents=True)
        data=target/'data';data.mkdir()
        with patch.object(portability,'binding',return_value=(data,{**self.config,'machine_id':'clone','storage_id':'clone-data'})),patch.object(sys,'argv',['portability','restore',str(target),str(archive),'--mode','clone']):
            portability.main()
        with sqlite3.connect(target/'runtime/ops.sqlite3') as c:
            self.assertEqual(c.execute('SELECT count(*) FROM resident_tokens').fetchone()[0],0)
            self.assertEqual(c.execute('SELECT mode FROM node_control').fetchone()[0],'readonly')
