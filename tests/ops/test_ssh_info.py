import json,os,tempfile,unittest,sys
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'ops'))
from ssh_info import connection

class SSHInfoTests(unittest.TestCase):
    def test_per_principal_and_no_fake_permission_on_missing_data(self):
        with tempfile.TemporaryDirectory() as t,patch.dict(os.environ,{'LOOM_SSH_INFO':t+'/connections.json'}):
            self.assertEqual(connection({'id':'one'})['state'],'not_configured')
            p=Path(t);(p/'connections.json').write_text(json.dumps({'one':{'state':'enabled','username':'loomu_one'}}));(p/'machine.json').write_text('{"host":"server.test","addresses":["100.64.0.1"]}')
            self.assertEqual(connection({'id':'one'})['username'],'loomu_one')
            other=connection({'id':'two'});self.assertEqual(other['state'],'not_enabled');self.assertNotIn('username',other);self.assertEqual(other['addresses'],['100.64.0.1'])
            (p/'connections.json').write_text('broken');self.assertEqual(connection({'id':'one'})['state'],'unavailable')
