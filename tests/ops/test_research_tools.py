"""Real registries, HTTP/MCP and frozen Job specs; no installed state or Jobs."""
import copy
import json
from pathlib import Path
from http.server import ThreadingHTTPServer
from threading import Thread
from unittest.mock import patch
import component_store,tool_store,ops,dashboard_read,profile_store,runtime_client
import test_api_contract
from test_support import IsolatedOps


class ResearchToolTests(IsolatedOps):
    def setUp(self):
        super().setUp()
        for module,name in ((component_store,'components'),(profile_store,'profiles')):
            p=patch.object(module,'ROOT',self.root/name);p.start();self.addCleanup(p.stop)
        self.config['runtime_ids']=['python']
        self.profile={'params_schema':{'type':'object'},'inputs':{},'outputs':None,
                      'runtime':{'executor':'container','runtime_id':'python','command':['python3','/tools/planner/plan.py']},
                      'limits':{'timeout_seconds':60,'max_attempts':1}}

    def action(self,name,actor=None,**args):
        return ops.operate('components',{'action':name,'arguments':args},actor or self.owner,self.config)

    def register(self,code='print(1)'):
        return self.action('register_tool',id='planner',manifest={'label':'Motion planner'},files={'plan.py':code})

    def test_revisions_are_idempotent_and_jobs_keep_exact_tool_pins(self):
        first=self.register();self.assertEqual(self.register()['revision'],1)
        p=self.action('register_profile',id='hybrid',profile={**self.profile,'tools':{'planner':{'id':'planner','revision':1}}})
        job=self.submit({**self.spec(),'kind':p['kind'],'params':{}})
        self.register('print(2)')
        frozen=json.loads(self.row(job)['spec'])['profile'];pin=frozen['tool_pins']['planner']
        self.assertEqual(pin['revision'],1);self.assertEqual(pin['sha256'],first['sha256'])
        tool_store.verify(frozen['tool_pins'])
        self.assertEqual(self.action('get_tool',id='planner',revision=1)['manifest']['label'],'Motion planner')
        self.assertEqual(self.action('list_tools',actor=self.reader)[0]['revision'],2)

    def test_explicit_legacy_copy_preserves_original_component_and_job(self):
        old=self.action('register',id='hybrid',manifest={'profiles':{'run':self.profile}},files={'run.py':'old runner'})
        job=self.submit({**self.spec(),'kind':old['profiles'][0],'params':{}});before=self.row(job)['spec']
        tool=self.action('register_tool',id='hybrid',manifest={'label':'Hybrid runner'},component={'id':'hybrid','revision':1})
        self.assertEqual(tool['manifest']['origin']['revision'],1)
        self.assertEqual(self.action('get',id='hybrid')['revision'],1)
        self.assertEqual(before,self.row(job)['spec'])
        self.assertEqual((tool_store.snapshot_path('owner','hybrid',1)/'run.py').read_text(),'old runner')

    def test_invalid_sources_hooks_unknown_revision_and_forged_pins_rejected(self):
        for files in ({'../escape':'bad'},{'a':'one','a/b':'two'},{'tool.json':'bad'}):
            with self.assertRaises(ValueError):self.action('register_tool',id='bad',manifest={},files=files)
        with self.assertRaises(ValueError):self.action('register_tool',id='bad',manifest={'profiles':{}},files={'x':'x'})
        with self.assertRaises(PermissionError):self.action('register_tool',actor=self.reader,id='bad',manifest={},files={'x':'x'})
        with self.assertRaises(ValueError):self.action('register_profile',id='bad',profile={**self.profile,'tools':{'planner':{'id':'missing','revision':1}}})
        with self.assertRaises(ValueError):self.action('register_profile',id='bad',profile={**self.profile,'tool_pins':{}})

    def test_launch_detects_tampered_revision_before_executor_call(self):
        self.register()
        with ops.db() as c:pins=tool_store.bind(c,{'planner':{'id':'planner','revision':1}},'owner')
        (tool_store.snapshot_path('owner','planner',1)/'plan.py').write_text('changed')
        with self.assertRaisesRegex(ValueError,'revision changed'):tool_store.verify(pins)

    def test_tool_store_symlink_cannot_expand_the_host_grant(self):
        outside=self.root/'outside';outside.mkdir();component_store.ROOT.mkdir()
        (component_store.ROOT/'.tools').symlink_to(outside,target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'symlink'):self.register()
        self.assertEqual(list(outside.iterdir()),[])

    def test_runtime_request_and_executor_payload_use_pinned_readonly_paths(self):
        self.register()
        with ops.db() as c:pins=tool_store.bind(c,{'planner':{'id':'planner','revision':1}},'owner')
        folder=self.data/'runs';folder.mkdir();request=folder/'request.json';request.write_text('{"params":{}}')
        calls=[]
        def broker(config,action,data):
            calls.append((action,data));return {'ticket':'fixture','returned':True,'exit_code':0}
        config={'executor':{'host_data':str(self.data),'host_components':'/host/components'}}
        spec={'profile':{**self.profile,'tool_pins':pins},'params':{}}
        with patch.object(runtime_client,'call',side_effect=broker):
            runtime_client.Container(config,{'id':'job','attempt':'one','owner':'owner'},spec,'run',request,folder,self.data)
        payload=next(value for action,value in calls if action=='start')
        self.assertEqual(payload['tools'],{'planner':'/host/components/.tools/owner/planner/revisions/1'})
        self.assertEqual(json.loads((folder/'run-request.json').read_text())['tools'],{'planner':'/tools/planner'})

    def test_dashboard_lists_tools_separately_from_components(self):
        self.register();self.action('register',id='lifecycle',manifest={},files={'run.py':'adapter'})
        with ops.db() as c:catalog,_=dashboard_read.projection(c,self.owner,self.config,self.local)
        self.assertEqual({e['kind'] for e in catalog['entities']},{'tool','component'})
        self.assertEqual(next(e for e in catalog['entities'] if e['kind']=='tool')['label'],'Motion planner')

    def test_dashboard_revisions_preserve_exact_profile_bindings_without_writes(self):
        self.register('old planner')
        profile={**self.profile,'tools':{'planner':{'id':'planner','revision':1}}}
        self.action('register',id='hybrid',manifest={'profiles':{'run':profile}},files={'run.py':'old adapter'})
        self.action('register_profile',id='independent',profile=profile)
        self.register('new planner')
        self.action('register',id='hybrid',manifest={'profiles':{'run':{**profile,'tools':{'planner':{'id':'planner','revision':2}}}}},files={'run.py':'new adapter'})
        with ops.db() as c:
            before=list(c.iterdump())
            catalog,_=dashboard_read.projection(c,self.owner,self.config,self.local)
            self.assertEqual(before,list(c.iterdump()))
        tool=next(e['loom_extension'] for e in catalog['entities'] if e['kind']=='tool')
        self.assertEqual([v['revision'] for v in tool['revisions']],[2,1])
        self.assertEqual({p['kind'] for p in tool['revisions'][1]['used_by']},{'owner.hybrid.run.r1','owner.independent.r1'})
        self.assertEqual([p['kind'] for p in tool['revisions'][0]['used_by']],['owner.hybrid.run.r2'])
        component=next(e['loom_extension'] for e in catalog['entities'] if e['kind']=='component')
        self.assertEqual(component['revisions'][1]['profiles'][0]['tools']['planner']['revision'],1)
        self.assertEqual(component['revisions'][0]['profiles'][0]['tools']['planner']['revision'],2)
        self.assertNotEqual(component['revisions'][0]['source_pins'],component['revisions'][1]['source_pins'])

    def test_real_http_and_mcp_discover_and_forward_optional_actions(self):
        server=ThreadingHTTPServer(('127.0.0.1',0),ops.API);server.config=self.config
        thread=Thread(target=server.serve_forever,kwargs={'poll_interval':.01},daemon=True);thread.start()
        self.addCleanup(thread.join,2);self.addCleanup(server.server_close);self.addCleanup(server.shutdown)
        self.base='http://127.0.0.1:'+str(server.server_port)
        request=lambda *args,**kwargs:test_api_contract.APIContractTests.request(self,*args,**kwargs)
        with patch.dict('os.environ',{'LOOM_DEV_NO_AUTH':'0'}):
            status,value=request('/api/actions',{'group':'components','action':'register_tool','arguments':{'id':'planner','manifest':{},'files':{'plan.py':'pass'}}})
            self.assertEqual(status,200)
            result=request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'components','arguments':{'action':'get_tool','arguments':{'id':'planner','revision':1}}}})[1]['result']
            self.assertEqual(json.loads(result['content'][0]['text'])['sha256'],value['sha256'])
            schema=request('/api/actions',{'group':'system','action':'describe','arguments':{'group':'components','action':'register_tool'}})[1]
            self.assertEqual(schema['scope'],'submit')
            self.assertEqual(request('/api/actions',{'group':'components','action':'register_tool','arguments':{'id':'x','manifest':{},'files':{'a':'x'}}},token='fixture-reader')[0],403)

    def test_dashboard_reads_legacy_profile_source_pins_without_rewriting_manifest(self):
        self.action('register',id='legacy',manifest={'profiles':{'run':self.profile}},files={'legacy.py':'preserved'})
        with ops.db() as c:
            manifest=json.loads(c.execute("SELECT manifest FROM components WHERE id='legacy'").fetchone()[0]);manifest.pop('source_pins')
            c.execute("UPDATE components SET manifest=? WHERE id='legacy'",(json.dumps(manifest),));c.commit()
            before=list(c.iterdump())
            rows=dashboard_read.extension_inventory(c,self.owner,self.config)
            self.assertEqual(before,list(c.iterdump()))
        revision=rows[0]['loom_extension']['revisions'][0]
        self.assertTrue(revision['source_pins_recorded'])
        self.assertEqual(list(revision['source_pins']),['legacy.py'])
