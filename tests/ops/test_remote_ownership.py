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
