"""Isolated dependency staging and SQLite; Docker/network boundaries are mocked."""
import importlib.util
from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('runtime_install',Path(__file__).resolve().parents[2]/'deploy/runtime.py')
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)


class RuntimeInstallTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        self.deps=self.root/'custom dependency store';self.deps.mkdir()
        self.installation=self.root/'instance';(self.installation/'config').mkdir(parents=True)
        (self.installation/'config/installation-plan.json').write_text(json.dumps({'dependencies':str(self.deps)}))
        self.executor=self.root/'executor';self.executor.mkdir()
        self.config={'instances':{'alpha':{'uid':1001,'gid':1001,'gpu':True}},'gpu_devices':['0']}
        (self.executor/'config.json').write_text(json.dumps(self.config))
        self.source=self.root/'recipe';self.source.mkdir()
        (self.source/'Dockerfile').write_text('FROM scratch\n')
        self.recipe={'id':'simulation','dependencies':[{'name':'sim','version':'6.0','kind':'python'}],
                     'validation':{'command':['python3','-c','print(1)'],'gpu':True}}
        self.save()
        self.calls=[]

    def save(self): (self.source/'runtime.json').write_text(json.dumps(self.recipe))

    def docker(self,argv,**kw):
        self.calls.append(argv)
        if argv[:2]==['docker','build']:
            Path(argv[argv.index('--iidfile')+1]).write_text('sha256:'+'a'*64)
        if argv[:2]==['docker','run']:
            kw['stdout'].write(json.dumps({'dependencies':{'sim':'6.0'},'checks':{'launch':True}}).encode())
        return subprocess.CompletedProcess(argv,0,stdout=b'')

    def test_custom_configured_root_build_validation_and_reused_profile_environment(self):
        result=runtime.install(self.installation,self.executor,'alpha',self.source,self.docker)
        self.assertTrue(Path(result['path']).is_relative_to(self.deps))
        self.assertTrue((Path(result['path'])/'context/runtime.json').is_file())
        self.assertTrue((Path(result['path'])/'receipt.json').is_file())
        rows=runtime.runtime_store.listing(self.executor,self.config,'alpha')['runtimes']
        self.assertEqual(rows[0]['manifest']['dependencies'][0]['version'],'6.0')
        command=next(c for c in self.calls if c[:2]==['docker','run'])
        for flag in ('--read-only','--cap-drop=ALL','--security-opt=no-new-privileges'):self.assertIn(flag,command)
        self.assertFalse(any('docker.sock' in a for a in command))
        self.assertEqual(command[command.index('--network')+1],'none')
        self.assertFalse(runtime.runtime_store.validation_fenced(self.executor))

    def test_missing_mismatching_or_redirected_root_never_falls_back(self):
        (self.installation/'config/installation-plan.json').unlink()
        with self.assertRaisesRegex(ValueError,'Configure'):runtime.settings(self.installation,self.executor,'alpha')
        self.config['instances']['alpha']['dependencies']=str(self.deps)
        (self.executor/'config.json').write_text(json.dumps(self.config))
        (self.installation/'config/installation-plan.json').write_text(json.dumps({'dependencies':str(self.root/'other')}))
        with self.assertRaisesRegex(ValueError,'differ'):runtime.settings(self.installation,self.executor,'alpha')
        (self.installation/'config/installation-plan.json').unlink()
        link=self.root/'link';link.symlink_to(self.deps)
        self.config['instances']['alpha']['dependencies']=str(link)
        (self.executor/'config.json').write_text(json.dumps(self.config))
        with self.assertRaisesRegex(ValueError,'redirected'):runtime.settings(self.installation,self.executor,'alpha')

    def test_failed_smoke_does_not_register_and_returns_container(self):
        def failure(argv,**kw):
            if argv[:2]==['docker','run']:raise subprocess.TimeoutExpired(argv,1)
            return self.docker(argv,**kw)
        with self.assertRaises(subprocess.TimeoutExpired):runtime.install(self.installation,self.executor,'alpha',self.source,failure)
        self.assertEqual(runtime.runtime_store.listing(self.executor,self.config,'alpha')['runtimes'],[])
        self.assertEqual(runtime.runtime_store.listing(self.executor,self.config,'alpha')['installations'][0]['state'],'failed')
        other={'instances':{'other':{'runtimes':[]}}}
        self.assertEqual(runtime.runtime_store.listing(self.executor,other,'other')['installations'],[])
        self.assertTrue(any(a[:3]==['docker','rm','-f'] for a in self.calls))
        self.assertFalse(runtime.runtime_store.validation_fenced(self.executor))

    def test_unconfirmed_validation_return_fences_dispatch(self):
        def failed_cleanup(argv,**kw):
            if argv[:3]==['docker','rm','-f']:return subprocess.CompletedProcess(argv,1)
            if argv[:3]==['docker','container','ls']:return subprocess.CompletedProcess(argv,1,stdout=b'')
            return self.docker(argv,**kw)
        with self.assertRaisesRegex(RuntimeError,'unconfirmed'):runtime.install(self.installation,self.executor,'alpha',self.source,failed_cleanup)
        self.assertTrue(runtime.runtime_store.validation_fenced(self.executor))
        self.assertEqual(runtime.runtime_store.listing(self.executor,self.config,'alpha')['runtimes'],[])

    def test_declared_version_is_not_measured_version(self):
        self.recipe['dependencies'][0]['version']='6.2';self.save()
        with self.assertRaisesRegex(ValueError,'exact installed'):runtime.install(self.installation,self.executor,'alpha',self.source,self.docker)
        self.assertEqual(runtime.runtime_store.listing(self.executor,self.config,'alpha')['runtimes'],[])

    def test_archive_traversal_and_symlinks_rejected(self):
        for name,kind in [('../escape','file'),('link','link')]:
            archive=self.root/(kind+'.tar')
            with tarfile.open(archive,'w') as bundle:
                item=tarfile.TarInfo(name)
                if kind=='link':item.type=tarfile.SYMTYPE;item.linkname='/etc/passwd';bundle.addfile(item)
                else:item.size=1;bundle.addfile(item,io.BytesIO(b'x'))
            with self.assertRaisesRegex(ValueError,'Unsafe'):runtime.unpack(archive,self.root/(kind+'-output'),'tar')
        (self.source/'secret').symlink_to('/etc/passwd')
        with self.assertRaisesRegex(ValueError,'links'):runtime.recipe(self.source)

    def test_download_hash_mismatch_never_unpacks(self):
        dep={'name':'source','url':'https://example.invalid/source','sha256':'0'*64}
        with patch.object(runtime,'urlopen',return_value=io.BytesIO(b'wrong')),self.assertRaisesRegex(ValueError,'hash mismatch'):
            runtime.download(dep,self.root/'downloaded')
        self.assertFalse((self.root/'downloaded').exists())

    def test_source_urls_cannot_publish_embedded_credentials_or_queries(self):
        dep=self.recipe['dependencies'][0]
        dep['sha256']='a'*64
        for url in ('https://user:secret@example.invalid/package',
                    'https://example.invalid/package?token=private',
                    'https://example.invalid/package#credential',
                    'https:///missing-host'):
            with self.subTest(url=url):
                dep['url']=url;self.save()
                with self.assertRaisesRegex(ValueError,'HTTPS'):runtime.recipe(self.source)
        self.assertEqual(self.calls,[])

    def test_dotted_dependency_names_have_distinct_download_evidence(self):
        payload=b'verified dependency'
        dep={'url':'https://example.invalid/source','sha256':hashlib.sha256(payload).hexdigest()}
        for name in ('isaac.sim','isaac.lab','isaac.sim.download'):
            with patch.object(runtime,'urlopen',return_value=io.BytesIO(payload)):
                runtime.download({**dep,'name':name},self.root/name)
        for name in ('isaac.sim','isaac.lab','isaac.sim.download'):
            self.assertEqual((self.root/name/'download').read_bytes(),payload)
            self.assertEqual((self.root/'.downloads'/name).read_bytes(),payload)

    def reconcile(self,attempt,instance='alpha'):
        output=io.StringIO()
        argv=['runtime.py','reconcile','--installation',str(self.installation),
              '--executor',str(self.executor),'--instance',instance,'--attempt',attempt]
        with patch.object(sys,'argv',argv),patch.object(runtime.subprocess,'run',side_effect=self.docker),redirect_stdout(output):
            runtime.main()
        return json.loads(output.getvalue())

    def test_reconcile_does_not_demote_a_ready_runtime_or_repeat_cleanup(self):
        result=runtime.install(self.installation,self.executor,'alpha',self.source,self.docker)
        before=runtime.runtime_store.listing(self.executor,self.config,'alpha')['installations']
        self.calls.clear()
        answer=self.reconcile(Path(result['path']).name)
        self.assertEqual(answer['status'],'ready')
        self.assertTrue(answer['registered'])
        self.assertEqual(answer['runtime_ref'],{k:result[k] for k in ('id','revision','sha256')})
        self.assertEqual(self.calls,[])
        self.assertEqual(runtime.runtime_store.listing(self.executor,self.config,'alpha')['installations'],before)

    def test_reconcile_enforces_instance_and_only_returns_existing_validation(self):
        attempt='c'*32
        evidence={'instance':'other','validation_container':'loom-runtime-check-'+attempt,
                  'validation_gpu':True,'container_returned':False}
        runtime.runtime_store.installation(self.executor,attempt,'simulation','failed',evidence)
        with self.assertRaisesRegex(PermissionError,'instance'):self.reconcile(attempt)
        self.assertEqual(self.calls,[])
        self.assertTrue(runtime.runtime_store.validation_fenced(self.executor))
        evidence['instance']='alpha'
        runtime.runtime_store.installation(self.executor,attempt,'simulation','failed',evidence)
        answer=self.reconcile(attempt)
        self.assertEqual(answer['status'],'failed');self.assertFalse(answer['registered'])
        self.assertFalse(runtime.runtime_store.validation_fenced(self.executor))
        self.assertEqual(len(self.calls),1);self.assertEqual(self.calls[0][:3],['docker','rm','-f'])
        self.calls.clear()
        self.assertEqual(self.reconcile(attempt),answer)
        self.assertEqual(self.calls,[])

    def test_reconcile_recovers_committed_registration_with_lost_ready_receipt(self):
        result=runtime.install(self.installation,self.executor,'alpha',self.source,self.docker)
        attempt=Path(result['path']).name
        with runtime.runtime_store.connect(self.executor,True) as c:
            evidence=json.loads(c.execute('SELECT evidence FROM runtime_installations WHERE id=?',(attempt,)).fetchone()[0])
        evidence.pop('runtime_ref')
        runtime.runtime_store.installation(self.executor,attempt,'simulation','failed',evidence)
        self.calls.clear()
        answer=self.reconcile(attempt)
        self.assertTrue(answer['registered']);self.assertEqual(answer['status'],'ready')
        inventory=runtime.runtime_store.listing(self.executor,self.config,'alpha')
        self.assertEqual(inventory['installations'][0]['state'],'ready')
        self.assertEqual(inventory['installations'][0]['runtime_ref'],answer['runtime_ref'])
        self.assertEqual(len(inventory['runtimes']),1)
        self.assertEqual(inventory['runtimes'][0]['sha256'],result['sha256'])
        self.assertEqual(self.calls,[])

    def test_separate_catalog_is_backed_up_without_rollback_erasing_later_revisions(self):
        sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
        self.addCleanup(sys.path.pop,0)
        import state_snapshot
        result=runtime.install(self.installation,self.executor,'alpha',self.source,self.docker)
        (self.installation/'compose.env').write_text('')
        (self.installation/'executor.env').write_text('LOOM_EXECUTOR_HOST='+str(self.executor)+'\n')
        archive=self.root/'archive';archive.mkdir()
        snapshot=state_snapshot.capture(self.installation,archive,self.root/'platform')
        self.assertTrue((snapshot/'runtimes.sqlite3').exists())
        manifest=runtime.runtime_store.detail(self.executor,'alpha',{k:result[k] for k in ('id','revision','sha256')})['manifest']
        manifest['runtime']['image']='sha256:'+'b'*64
        runtime.runtime_store.register(self.executor,'simulation',manifest,['alpha'])
        state_snapshot.restore(self.installation,snapshot)
        self.assertTrue(state_snapshot.verify(self.installation,snapshot)['preserved'])
        self.assertEqual(len(runtime.runtime_store.listing(self.executor,self.config,'alpha')['runtimes']),2)


class RuntimeInventoryTests(unittest.TestCase):
    def test_observation_uses_bounded_nonroot_probe_and_mount_metadata_without_rewriting_policy(self):
        import runtime_inventory
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);executor=root/'executor';executor.mkdir();packages=root/'preserved-packages';packages.mkdir()
            metadata=packages/'native_lib-1.2.dist-info';metadata.mkdir();(metadata/'METADATA').write_text('Name: native-lib\nVersion: 1.2\n')
            lab=packages/'third_party/IsaacLab';(lab/'source/isaaclab/config').mkdir(parents=True)
            (lab/'VERSION').write_text('2.3.0\n');(lab/'source/isaaclab/config/extension.toml').write_text('[package]\nversion="0.47.2"\n')
            policy={'image':'current:tag','network':'none','mounts':[{'source':str(packages),'target':'/opt/native','readonly':True}]}
            config={'instances':{'alpha':{'uid':995,'gid':982,'runtimes':['research']}},'runtimes':{'research':policy}}
            path=executor/'config.json';path.write_text(json.dumps(config));original=path.read_bytes();calls=[]
            def boundary(argv,**kwargs):
                calls.append(argv)
                value=([{'Id':'sha256:'+'c'*64}] if argv[1:3]==['image','inspect'] else {'python':'3.12.3','packages':[],'extensions':[],'versions':{}})
                return subprocess.CompletedProcess(argv,0,stdout=json.dumps(value),stderr='')
            receipt=runtime_inventory.capture(executor,'alpha','research',python='/usr/bin/python3',run=boundary)
            self.assertTrue(receipt['container_returned']);self.assertEqual(receipt['dependencies'],4)
            command=next(c for c in calls if c[1]=='run')
            for flag in ['--read-only','--network','--cap-drop','--user','--cpus','--memory','--pids-limit']:self.assertIn(flag,command)
            self.assertNotIn('--gpus',command);self.assertNotIn('--mount',command)
            self.assertEqual(path.read_bytes(),original)
            item=runtime_inventory.runtime_store.detail(executor,'alpha',{k:receipt[k] for k in ['id','revision','sha256']})
            versions={d['name']:d['version'] for d in item['manifest']['dependencies']}
            self.assertEqual(versions['IsaacLab'],'2.3.0');self.assertEqual(versions['isaaclab-extension'],'0.47.2')
            self.assertIn('no GPU',item['manifest']['validation']['scope'])
            self.assertTrue(any(d['name']=='native-lib' and d['version']=='1.2' for d in item['manifest']['dependencies']))
            self.assertEqual(runtime_inventory.capture(executor,'alpha','research',python='/usr/bin/python3',run=boundary)['revision'],receipt['revision'])

    def test_managed_dependency_origin_uses_the_configured_canonical_directory(self):
        import runtime_inventory
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);configured=root/'custom root';configured.mkdir();inside=configured/'lib';inside.mkdir();outside=root/'dependencies';outside.mkdir()
            for path in [inside,outside]:
                p=path/'module-1.dist-info';p.mkdir();(p/'METADATA').write_text('Name: module\nVersion: 1\n')
            alias=configured/'external-link';alias.symlink_to(outside,target_is_directory=True)
            policy={'mounts':[{'source':str(inside),'target':'/opt/a'},{'source':str(outside),'target':'/opt/b'},{'source':str(alias),'target':'/opt/c'}]}
            deps,_,_=runtime_inventory.mount_metadata(policy,configured)
            origins={d['scope']:d['origin']['type'] for d in deps}
            self.assertEqual(origins,{'/opt/a':'managed-directory','/opt/b':'external-mount','/opt/c':'external-mount'})

    def test_active_and_deprecated_extension_records_do_not_overwrite_each_other(self):
        import runtime_inventory
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/'config.json').write_text(json.dumps({'instances':{'alpha':{'uid':1,'gid':1,'runtimes':['sim']}},'runtimes':{'sim':{'image':'cached'}}}))
            def boundary(argv,**kwargs):
                value=[{'Id':'sha256:'+'c'*64}] if argv[1:3]==['image','inspect'] else {'python':'3.12','packages':[],'extensions':[{'name':'same-extension','version':'1','scope':'isaac-extension','deprecated':False},{'name':'same-extension','version':'1','scope':'isaac-extension','deprecated':True}],'versions':{}}
                return subprocess.CompletedProcess(argv,0,stdout=json.dumps(value),stderr='')
            receipt=runtime_inventory.capture(root,'alpha','sim',python='/usr/bin/python3',run=boundary)
            value=runtime_inventory.runtime_store.detail(root,'alpha',{k:receipt[k] for k in ('id','revision','sha256')})
            entries=[d for d in value['manifest']['dependencies'] if d['name']=='same-extension']
            self.assertEqual({d['deprecated'] for d in entries},{True,False})
