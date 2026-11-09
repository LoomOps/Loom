import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch
import diagnostics

class DiagnosticsTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)/'diagnostics';diagnostics._seen.clear()
    def test_sensitive_exception_text_never_written(self):
        try:raise ValueError('Bearer secret-token user@example.test /home/private https://secret/?token=x')
        except ValueError as error:result=diagnostics.record('test','read',error,directory=self.root)
        self.assertIsNotNone(result)
        raw=(self.root/'events.jsonl').read_text();item=json.loads(raw)
        for value in ('secret-token','user@example','/home/private','https://','raise ValueError'):self.assertNotIn(value,raw)
        self.assertEqual(item['exception'],'ValueError');self.assertTrue(item['frames'])
        self.assertEqual((self.root/'events.jsonl').stat().st_mode&0o777,0o600)
        self.assertEqual(self.root.stat().st_mode&0o777,0o700)
    def test_failure_never_overrides_caller(self):
        with patch.object(diagnostics.os,'open',side_effect=OSError('full disk secret')):
            self.assertIsNone(diagnostics.record('test','read',directory=self.root))
    def test_rate_limit_and_bounded_rotation(self):
        with patch.object(diagnostics,'MAX_BYTES',512),patch.object(diagnostics,'BACKUPS',2):
            for i in range(15):self.assertIsNotNone(diagnostics.record('test',f'op{i}',directory=self.root))
            self.assertIsNone(diagnostics.record('test','op14',directory=self.root))
        files=list(self.root.glob('*.jsonl'));self.assertLessEqual(len(files),3)
        for file in files:
            for line in file.read_text().splitlines():json.loads(line)
    def test_symlink_never_overwrites_target(self):
        target=Path(self.temp.name)/'target';target.write_text('keep')
        self.root.mkdir(mode=0o700);(self.root/'events.jsonl').symlink_to(target)
        self.assertIsNone(diagnostics.record('test','read',directory=self.root))
        self.assertEqual(target.read_text(),'keep')
    def test_busy_writer_does_not_wait(self):
        import fcntl,time
        self.root.mkdir(mode=0o700)
        with (self.root/'.lock').open('w') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX);start=time.monotonic()
            self.assertIsNone(diagnostics.record('test','read',directory=self.root))
            self.assertLess(time.monotonic()-start,.5)
    def test_concurrent_events_are_complete_lines(self):
        threads=[threading.Thread(target=diagnostics.record,args=('test',f'op{i}'),kwargs={'directory':self.root}) for i in range(12)]
        for t in threads:t.start()
        for t in threads:t.join()
        rows=[json.loads(s) for s in (self.root/'events.jsonl').read_text().splitlines()]
        self.assertEqual(len(rows),12)

    def test_retention_defaults_overrides_and_invalid_values(self):
        with patch.dict(os.environ,{},clear=True):
            self.assertEqual(diagnostics.retention(),(64*1024*1024,15))
        with patch.dict(os.environ,{'LOOM_DIAGNOSTICS_SEGMENT_MIB':'128','LOOM_DIAGNOSTICS_BACKUPS':'31'}):
            self.assertEqual(diagnostics.retention(),(128*1024*1024,31))
        with patch.dict(os.environ,{'LOOM_DIAGNOSTICS_SEGMENT_MIB':'invalid','LOOM_DIAGNOSTICS_BACKUPS':'-1'}):
            self.assertEqual(diagnostics.retention(),(64*1024*1024,15))
    def test_zero_backups_rotates_current_file(self):
        with patch.object(diagnostics,'MAX_BYTES',512),patch.dict(os.environ,{'LOOM_DIAGNOSTICS_BACKUPS':'0'}):
            for i in range(10):diagnostics.record('test',f'zero{i}',directory=self.root)
        self.assertEqual(len(list(self.root.glob('*.jsonl'))),1)
        self.assertEqual(json.loads((self.root/'events.jsonl').read_text().splitlines()[-1])['operation'],'zero9')
