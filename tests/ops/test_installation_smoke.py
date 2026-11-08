"""The shipped installer producer must pass the real result/indexing contract."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from unittest.mock import patch
import ops
from test_support import IsolatedOps

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('installation_smoke',ROOT/'deploy/installer/smoke.py')
smoke=importlib.util.module_from_spec(spec);spec.loader.exec_module(smoke)


class ShippedInstallationSmokeTests(IsolatedOps):
    def test_shipped_prepare_and_run_finalize_and_register_with_actual_contract(self):
        row=self.claim(self.submit())
        profile={'outputs':{'proof':'artifact'},'revision':'installation-test','sha256':'a'*64}
        current=json.loads(row['spec']);current['profile']=profile
        with ops.db() as c:c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(current),row['id']))
        row=self.row(row['id']);folder=self.data/'runs'/row['id']/row['attempt'];folder.mkdir(parents=True)
        source=self.root/'producer.py';source.write_text(smoke.SOURCE)
        request=folder/'request.json';request.write_text(json.dumps({'output_dir':str(folder),'profile':profile}))
        for phase in ('prepare','run'):subprocess.run([sys.executable,str(source),phase,str(request)],check=True)
        with patch.object(ops,'guard'):ops.process_result(self.processing(row),self.config)
        self.assertEqual(self.row(row['id'])['state'],'complete')
        result=ops.operate('get_entity',{'id':row['id']+'.proof','revision':1},self.owner,self.config)
        self.assertEqual(result['provenance'],{'profile':profile['revision'],'producer_sha256':profile['sha256']})
        self.assertEqual(result['job'],row['id']);self.assertTrue(result['components'])
