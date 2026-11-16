"""Read-only GPU driver observations; no NVIDIA tools or installed services."""
import json,runpy,subprocess,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[2]
class GPUObservationTests(unittest.TestCase):
    def test_driver_version_stays_text_and_is_collected_with_the_gpu_sample(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'sample.json'
            row='0, Fixture GPU, 1, 2, 10, 40, 8, 300, 580.178.04\n'
            with patch.object(sys,'argv',['collector',str(path)]),patch('subprocess.run',return_value=subprocess.CompletedProcess([],0,row)) as run:
                runpy.run_path(str(ROOT/'deploy/collect-dashboard-gpus.py'),run_name='__main__')
            sample=json.loads(path.read_text());self.assertEqual(sample['status'],'ok')
            self.assertEqual(sample['data'][0]['driver_version'],'580.178.04')
            self.assertIn('driver_version',run.call_args.args[0][1])
    def test_missing_driver_tool_is_unavailable_without_inventing_a_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'sample.json'
            with patch.object(sys,'argv',['collector',str(path)]),patch('subprocess.run',side_effect=FileNotFoundError):
                runpy.run_path(str(ROOT/'deploy/collect-dashboard-gpus.py'),run_name='__main__')
            sample=json.loads(path.read_text());self.assertEqual(sample['status'],'unavailable');self.assertIsNone(sample['data'])
