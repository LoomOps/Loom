import copy
import json
from pathlib import Path
import sys
from unittest.mock import patch
import component_store,execution,ops,profile_store,profiles,registry,dashboard_read
from test_support import IsolatedOps

class IndependentProfileTests(IsolatedOps):
    def setUp(self):
        super().setUp()
        for module,name in ((component_store,'components'),(profile_store,'profiles')):
            p=patch.object(module,'ROOT',self.root/name);p.start();self.addCleanup(p.stop)
        self.config.update(runtime_ids=['python'])
        self.definition={'params_schema':{'type':'object','properties':{'epochs':{'type':'integer','minimum':1}},'required':['epochs'],'additionalProperties':False},'defaults':{'epochs':2},'inputs':{},'outputs':{'dataset':'dataset'},
            'metadata_schemas':{'dataset':{'type':'object','properties':{'episodes':{'type':'integer','minimum':0},'modalities':{'type':'array','items':{'enum':['rgb','depth']}}},'required':['episodes','modalities'],'additionalProperties':False}},
            'runtime':{'executor':'container','runtime_id':'python','command':['python3','/component/run.py']},'limits':{'timeout_seconds':60,'max_attempts':4}}
    def action(self,action,**args):return ops.operate('components',{'action':action,'arguments':args},self.owner,self.config)
    def register(self):
        c=self.action('register',id='source',manifest={},files={'run.py':'print(1)'})
        return self.action('register_profile',id='train',profile=self.definition,component={'id':'source','revision':c['revision']})
    def test_unsupported_producer_cannot_be_discovered_or_submitted(self):
        self.config['profiles']={'old.train':{'entry':'producers/train.py','revision':'old'}}
        self.assertNotIn('old.train',[p['kind'] for p in profiles.describe(self.config,self.owner)])
        with self.assertRaisesRegex(ValueError,'Unsupported profile'):
            self.submit({**self.spec(),'kind':'old.train','params':{}})
        with ops.db() as c:self.assertEqual(c.execute('select count(*) from jobs').fetchone()[0],0)
    def test_independent_source_and_profile_freeze_defaults(self):
        first=self.register()
        job=self.submit({**self.spec(),'kind':first['kind'],'params':{}})
        self.definition['defaults']['epochs']=3
        second=self.register()
        old=json.loads(self.row(job)['spec'])
        self.assertEqual(old['params']['epochs'],2);self.assertEqual(old['profile']['revision'],first['kind'])
        self.assertNotEqual(second['kind'],first['kind'])
        self.assertEqual(self.action('get_profile',id='train',revision=1)['profile']['defaults'],{'epochs':2})
        listed=ops.operate('components',{'action':'profiles'},self.owner,self.config)
        self.assertEqual(next(x for x in listed if x['kind']==first['kind'])['metadata_schemas'],self.definition['metadata_schemas'])
    def test_clean_dashboard_requires_no_historical_catalog(self):
        with ops.db() as c:catalog,media=dashboard_read.projection(c,self.owner,self.config,self.local)
        self.assertEqual(catalog['entities'],[]);self.assertEqual(media,{})
    def test_scope_and_reserved_fields(self):
        self.register()
        with self.assertRaises(ValueError):self.action('get_profile',id='train',owner='other')
        for extra in ({'component_path':'/etc'},{'source_pins':{}},{'submitters':['other']}):
            with self.assertRaises(ValueError):self.action('register_profile',id='bad',profile={**self.definition,**extra})
        with self.assertRaises(PermissionError):ops.operate('components',{'action':'register_profile','arguments':{'id':'x','profile':self.definition}},self.reader,self.config)
    def test_metadata_validated_and_indexed_with_schema(self):
        kind=self.register()['kind'];job=self.submit({**self.spec(),'kind':kind,'params':{}});row=self.row(job);p=json.loads(row['spec'])['profile']
        entity={'id':'dataset','kind':'dataset','path':'data.json','provenance':{'profile':p['revision'],'producer_sha256':p['sha256']},'relations':[], 'label':'RGB samples','metadata':{'episodes':140,'modalities':['rgb']}}
        result={'entities':[entity],'metrics':[]}
        profiles.typed_contract(result,p,ops.ident)
        for value in ({'episodes':-1,'modalities':['rgb']},{'episodes':140}, {'episodes':'140','modalities':['rgb']}):
            bad=copy.deepcopy(result);bad['entities'][0]['metadata']=value
            with self.assertRaises(ValueError):profiles.typed_contract(bad,p,ops.ident)
        path=self.data/'data.json';path.write_text('{}')
        entity.update(sha256=profiles.file_sha(path),locator={'ref':'data.json','machine_id':'fixture','storage_id':'fixture-data'})
        with ops.db() as c:
            registry.index(c,row,result,self.config,self.data,ops.relative)
            saved=registry.view(c,self.owner,job+'.dataset')
        self.assertEqual(saved['metadata'],entity['metadata']);self.assertEqual(saved['label'],'RGB samples')
        self.assertEqual(saved['metadata_schema'],p['metadata_schemas']['dataset'])
        with ops.db() as c:catalog,_=dashboard_read.projection(c,self.owner,self.config,self.local)
        visible=next(e for e in catalog['entities'] if e['id']==job+'.dataset')
        self.assertEqual(visible['properties']['episodes'],140)
    def test_dynamic_runtime_is_pinned_once_and_frozen(self):
        self.config['executor']={'socket':'fixture'}
        self.definition['runtime'].pop('runtime_id');self.definition['runtime']['image']='my-trainer:latest'
        digest='sha256:'+'a'*64
        with patch('runtime_client.call',return_value={'image':digest,'resources':{'cpus':1,'memory':'2g','pids':512}}) as resolve:
            registered=self.register()
        self.assertEqual(resolve.call_count,1)
        job=self.submit({**self.spec(),'kind':registered['kind'],'params':{}})
        self.assertEqual(json.loads(self.row(job)['spec'])['profile']['runtime']['image'],digest)
    def test_output_schema_shape_and_unknown_output_fail_at_registration(self):
        for schemas in ([],{'typo':{'type':'object'}}):
            self.definition['metadata_schemas']=schemas
            with self.assertRaises(ValueError):self.register()
    def test_remote_schema_rejected_before_registration(self):
        self.definition['metadata_schemas']['dataset']={'$ref':'https://example.test/schema'}
        with self.assertRaises(ValueError):self.register()
    def test_group_action_cannot_be_overridden(self):
        with self.assertRaises(ValueError):self.action('list',action_override='register')

class RuntimePolicyTests(IsolatedOps):
    def setUp(self):
        super().setUp();sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'executor'))
        from runtime_policy import declared
        self.declared=declared;self.policy={'enabled':True};self.digest='sha256:'+'b'*64
    def test_cached_image_resolution_and_resource_limits(self):
        self.assertEqual(self.declared({'image':'python:3.12-slim'},self.policy,lambda *a:self.digest)['image'],self.digest)
        for resources in ({'cpus':0},{'cpus':float('nan')},{'cpus':50},{'memory':'90g'},{'pids':1},{'mounts':['/']},{'network':'host'}):
            with self.assertRaises(ValueError):self.declared({'image':'python','resources':resources},self.policy,lambda *a:self.digest)
    def test_disabled_and_missing_images_fail_without_pull(self):
        with self.assertRaises(PermissionError):self.declared({'image':'python'},{},lambda *a:self.digest)
        def missing(*args):raise RuntimeError('No such image')
        with self.assertRaises(RuntimeError):self.declared({'image':'missing'},self.policy,missing)
