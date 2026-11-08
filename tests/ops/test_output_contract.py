import copy
import math
from unittest.mock import patch
import ops
import profiles
from test_support import IsolatedOps

class OutputContractTests(IsolatedOps):
    def test_artifact_cannot_escape_data_through_parent_or_symlink(self):
        secret=self.root/'outside';secret.write_text('outside fixture')
        (self.data/'alias').symlink_to(secret)
        for ref in ['../outside','alias',str(secret)]:
            with self.subTest(ref=ref),self.assertRaises(ValueError):ops.relative(self.data,ref)
    def test_artifact_must_exist_and_metrics_must_be_finite(self):
        (self.data/'out.json').write_text('{}')
        for path,value in [('missing',1),('out.json',float('nan')),('out.json',float('inf'))]:
            manifest={'entities':[{'id':'output','kind':'artifact','path':path}],
                'metrics':[{'name':'loss','entity_id':'output','value':value}]}
            with self.subTest(path=path,value=value),self.assertRaises(ValueError):ops.artifact(self.config,self.data,manifest)
    def test_typed_series_rejects_undefined_units_and_nonmonotonic_steps(self):
        profile={'outputs':{'model':'model'},'revision':'fixture-v1','sha256':'a'*64}
        manifest={'entities':[{'id':'model','kind':'model','path':'model.bin','provenance':{'profile':'fixture-v1','producer_sha256':'a'*64},'relations':[]}],
            'metrics':[{'name':'loss','entity_id':'model','definition':'fixture loss','unit':'MSE','denominator':'samples','series':[{'step':1,'value':.5},{'step':2,'value':.2}]}]}
        profiles.typed_contract(copy.deepcopy(manifest),profile,ops.ident)
        for field,value in [('unit',''),('series',[{'step':1,'value':.5},{'step':1,'value':.2}]),('series',[{'step':1,'value':math.nan}])]:
            candidate=copy.deepcopy(manifest);candidate['metrics'][0][field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):profiles.typed_contract(candidate,profile,ops.ident)
    def test_storage_guard_requires_actual_device_and_identity(self):
        config={**self.config,'storage_device':'8:1'}
        mount='12 1 8:1 / '+str(self.data)+' rw - ext4 /dev/test rw\n'
        with patch.object(ops.Path,'read_text',return_value=mount),patch.object(ops,'read',return_value={'storage_id':'fixture-data'}):ops.guard(config)
        with patch.object(ops.Path,'read_text',return_value=mount.replace('8:1','8:2')),self.assertRaises(RuntimeError):ops.guard(config)
        with patch.object(ops.Path,'read_text',return_value=mount),patch.object(ops,'read',return_value={'storage_id':'wrong'}),self.assertRaises(RuntimeError):ops.guard(config)
