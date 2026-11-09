import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import managed_resources as managed

class ManagedPermissionsTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.config={'database':str(self.root/'requests.sqlite3'),'tailnet':'fixture','machine_id':'fixture','device_id':'one'}
    def test_atomic_replacement_and_lock_keep_private_service_owner(self):
        managed.mutation(self.config,{},{},lambda:None)
        with managed.path(self.config).open() as previous:
            before=previous.read();previous.seek(0)
            managed.mutation(self.config,{}, {'fixture':{'after':'safe'}},lambda:None)
            self.assertEqual(previous.read(),before)
            self.assertNotEqual(managed.path(self.config).read_text(),before)
        self.assertTrue(managed.permissions_ready(self.config))
        self.assertEqual(managed.read(self.config)['operations'][-1]['state'],'applied')
        self.assertFalse(list(self.root.glob('.managed-resources-*')))
    def test_root_installer_chowns_both_new_lock_and_new_json(self):
        calls=[]
        with patch.object(managed.os,'geteuid',return_value=0),patch.object(managed.os,'fchown',side_effect=lambda fd,u,g:calls.append((u,g))):
            managed.mutation(self.config,{},{},lambda:None)
        self.assertEqual(calls,[(self.root.stat().st_uid,self.root.stat().st_gid)]*2)
    def test_linked_lock_does_not_touch_foreign_file(self):
        p=self.root/'foreign';p.write_text('unchanged')
        managed.path(self.config).with_suffix('.lock').symlink_to(p)
        with self.assertRaises(OSError):managed.mutation(self.config,{},{},lambda:None)
        self.assertEqual(p.read_text(),'unchanged')
    def test_doctor_rejects_wrong_permissions(self):
        managed.write(self.config,managed.read(self.config));managed.path(self.config).chmod(0o644)
        self.assertFalse(managed.permissions_ready(self.config))
