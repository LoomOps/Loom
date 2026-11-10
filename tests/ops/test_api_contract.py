import json
import os
from http.server import ThreadingHTTPServer
from threading import Thread
from urllib.error import HTTPError
from urllib.request import Request,urlopen
from unittest.mock import patch
import ops
from test_support import IsolatedOps

class APIContractTests(IsolatedOps):
    def test_release_actions_share_api_mcp_contract_and_require_auth(self):
        value={'status':'update_available','latest':{'tag':'0.2.0'},'automatic_install':False}
        with patch.object(ops.release_updates,'status',return_value=value),patch.object(ops.release_updates,'request',return_value={'accepted':True,'updates':value}):
            self.assertEqual(self.request('/api/updates',token=None)[0],403)
            self.assertEqual(self.request('/api/updates/check',{},token=None)[0],403)
            self.assertEqual(self.request('/api/updates')[1],value)
            self.assertEqual(self.request('/api/status')[1]['updates'],value)
            self.assertTrue(self.request('/api/updates/check',{})[1]['accepted'])
            self.assertEqual(self.request('/api/updates/check',{'install':True})[0],400)
            result=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call',
                'params':{'name':'system','arguments':{'action':'updates'}}})[1]['result']
            self.assertEqual(json.loads(result['content'][0]['text']),value)
            schema=self.request('/api/actions',{'group':'system','action':'describe','arguments':{'group':'system','action':'check_updates'}})[1]
            self.assertEqual(schema['schema']['properties'],{})
            self.assertEqual(self.request('/')[1],{'service':'Loom Ops API','mcp':'/mcp','status':'/api/status'})

    def tailnet(self, **changes):
        self.config['tailscale_auth']={'enabled':True,'trusted_proxies':['127.0.0.1'],
                                       'users':{'owner@example.test':'owner'},**changes}
        return {'Tailscale-User-Login':'owner@example.test','Tailscale-User-Name':'Test Owner'}

    def test_tailnet_identity_explicit_binding_and_bearer_precedence(self):
        headers=self.tailnet()
        status,principal=self.request('/api/principal',token=None,headers=headers)
        self.assertEqual(status,200);self.assertEqual(principal['id'],'owner')
        self.assertEqual(principal['identity']['login'],'owner@example.test')
        self.assertIsNone(principal['identity']['tailnet_role'])
        self.assertEqual(self.request('/api/principal',token='wrong',headers=headers)[0],403)
        self.assertEqual(self.request('/api/principal',token='fixture-reader',headers=headers)[1]['id'],'reader')

    def test_tailnet_unknown_untrusted_disabled_and_tagged_denied(self):
        headers=self.tailnet()
        self.assertEqual(self.request('/api/principal',token=None)[0],403)
        self.assertEqual(self.request('/api/principal',token=None,headers={'Tailscale-User-Login':'other@example.test'})[0],403)
        self.config['tailscale_auth']['trusted_proxies']=['192.0.2.1']
        self.assertEqual(self.request('/api/principal',token=None,headers=headers)[0],403)
        self.tailnet(enabled=False)
        self.assertEqual(self.request('/api/principal',token=None,headers=headers)[0],403)

    def test_tailnet_users_share_jobs_and_keep_distinct_responsibility(self):
        self.tailnet(allow_new_users=True)
        job=self.submit()
        headers={'Tailscale-User-Login':'member@example.test'}
        principal=self.request('/api/principal',token=None,headers=headers)[1]
        self.assertTrue(principal['id'].startswith('ts-'))
        self.assertEqual(principal['owners'],[principal['id']])
        self.assertEqual(self.request('/api/jobs',token=None,headers=headers)[1][0]['id'],job)
        self.assertEqual(self.request('/api/jobs/cancel',{'id':job},token=None,headers=headers)[0],200)
        self.assertEqual(self.request('/api/history/list',{'kind':'','offset':0,'limit':10},token=None,headers=headers)[0],200)
        spec=self.spec('tailnet-job');spec.update(owner=principal['id'],notify=principal['id'])
        status,created=self.request('/api/jobs',spec,token=None,headers=headers)
        self.assertEqual(status,200)
        self.assertEqual(self.request('/api/jobs',token=None,headers=headers)[1][0]['id'],created['id'])
        self.assertEqual(len(self.request('/api/jobs',token=None,headers={'Tailscale-User-Login':'second@example.test'})[1]),2)

    def test_streamable_http_version_negotiation_and_errors(self):
        for version in ops.MCP_VERSIONS:
            status,reply=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':version}})
            self.assertEqual(status,200);self.assertEqual(reply['result']['protocolVersion'],version)
        ping={'jsonrpc':'2.0','id':1,'method':'ping'}
        self.assertEqual(self.request('/mcp',ping,headers={'MCP-Protocol-Version':'2099-01-01'})[0],400)
        self.assertEqual(self.request('/mcp',ping,headers={'Accept':'text/html'})[0],406)
        self.assertEqual(self.request('/mcp',ping,headers={'Content-Type':'text/plain'})[0],415)
        self.assertEqual(self.request('/mcp',{'id':1,'method':'ping'})[0],400)
        self.assertEqual(self.request('/mcp',{**ping,'method':'initialize','params':[]})[0],200)
        self.assertEqual(self.request('/mcp',token=None)[0],405)

    def test_streamable_http_notifications_sse_and_stateless_get(self):
        for method in ('notifications/initialized','notifications/cancelled','notifications/unknown'):
            req=Request(self.base+'/mcp',data=json.dumps({'jsonrpc':'2.0','method':method}).encode(),
                        headers={'Authorization':'Bearer fixture-owner','Content-Type':'application/json'})
            with urlopen(req) as response:
                self.assertEqual(response.status,202);self.assertEqual(response.read(),b'')
        req=Request(self.base+'/mcp',data=json.dumps({'jsonrpc':'2.0','id':7,'method':'ping'}).encode(),
                    headers={'Authorization':'Bearer fixture-owner','Content-Type':'application/json','Accept':'text/event-stream'})
        with urlopen(req) as response:
            self.assertEqual(response.headers.get_content_type(),'text/event-stream')
            content=response.read().decode()
            self.assertTrue(content.startswith('event: message\ndata: '))
            self.assertEqual(json.loads(content.split('data: ',1)[1])['id'],7)
            self.assertIsNone(response.headers.get('Mcp-Session-Id'))

    def test_stdio_client_negotiates_version_and_accepts_empty_notification(self):
        import importlib.util
        from client_source import CLIENTS
        spec=importlib.util.spec_from_file_location('loom_stdio',CLIENTS/'stdio.py')
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        credential=self.root/'credential.json';credential.write_text(json.dumps({'token':'fixture-owner'}));credential.chmod(0o600)
        config=self.root/'client.json';config.write_text(json.dumps({'endpoint':self.base+'/mcp','credential':str(credential)}))
        client=module.OpsTransport(config)
        reply=client.rpc({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}})
        self.assertEqual(reply['result']['protocolVersion'],'2025-11-25')
        self.assertEqual(client.protocol_version,'2025-11-25')
        self.assertIsNone(client.rpc({'jsonrpc':'2.0','method':'notifications/initialized'}))
        self.assertIn('result',client.rpc({'jsonrpc':'2.0','id':2,'method':'ping'}))
        config.write_text(json.dumps({'endpoint':self.base+'/mcp','authentication':'tailscale'}))
        self.assertIsNone(module.OpsTransport(config).token)

    def test_duplicate_tailnet_identity_rejected(self):
        from email.message import Message
        self.tailnet()
        headers=Message();headers.add_header('Tailscale-User-Login','owner@example.test')
        headers.add_header('Tailscale-User-Login','other@example.test')
        with self.assertRaises(PermissionError):ops.access.tailscale_actor(self.config,headers,'127.0.0.1')

    def test_removed_repair_routes_and_flat_mcp_names_are_not_dispatchable(self):
        for path in ['/api/jobs/retry','/api/jobs/correct-metric-step']:
            self.assertEqual(self.request(path,{'id':'old','attempt':'old'})[0],404)
        for name in ['node_status','submit_job','retry_job','correct_metric_step']:
            status,reply=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':name,'arguments':{}}})
            self.assertEqual(status,200);self.assertTrue(reply['result']['isError'])
        with ops.db() as c:self.assertEqual(c.execute('select count(*) from jobs').fetchone()[0],0)

    def test_profile_discovery_exposes_inputs_only_with_submit_grant(self):
        self.config['profiles']={'fixture.train':{'schema':ops.execution.REVISION,'owners':['owner'],'revision':'v1',
            'inputs':{'dataset':'inputs'},'outputs':{'model':'model'},
            'params_schema':{'type':'object','properties':{'dataset':{'type':'object'}},'required':['dataset']},
            'runtime':{'lane':'gpu','command':['private/entry.py']},'limits':{'timeout_seconds':60,'max_attempts':1}}}
        status, profiles=self.request('/api/jobs/profiles')
        self.assertEqual(status,200)
        p=next(x for x in profiles if x['kind']=='fixture.train')
        self.assertEqual(p['params_schema']['required'],['dataset'])
        self.assertNotIn('private/entry.py',json.dumps(profiles))
        self.assertEqual(self.request('/api/jobs/profiles',token='fixture-reader')[1],[])

    def test_malformed_mcp_requests_do_not_drop_connection(self):
        self.assertEqual(self.request('/mcp',[])[0],400)
        for params in [None,[],{'name':[]},{'name':'get_job','arguments':None}]:
            status,reply=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':params})
            self.assertEqual(status,200);self.assertTrue(reply['result']['isError'])
        self.assertEqual(self.request('/api/status')[0],200)

    def test_job_contract_http_mcp_submission_and_filters_match(self):
        from job_contract import REVISION, CATEGORIES
        status, contract = self.request('/api/jobs/contract')
        self.assertEqual(status,200);self.assertEqual(contract['revision'],REVISION)
        discovery=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/list'})[1]['result']['tools']
        schema=ops.tool_schema('submit_job')
        self.assertEqual({t['name'] for t in discovery},{'jobs','recovery','events','components','results','system'})
        self.assertEqual(schema['properties']['category']['enum'],list(CATEGORIES))
        spec={**self.spec(),'category':'fine_tuning','execution_mode':'pilot'}
        rpc=self.request('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/call',
            'params':{'name':'jobs','arguments':{'action':'submit','arguments':spec}}})[1]['result']
        job=json.loads(rpc['content'][0]['text'])['id']
        self.assertEqual(self.request('/api/jobs',spec)[1]['id'],job)
        self.assertEqual(self.request('/api/jobs?include_consolidated=true')[0],200)
        self.assertEqual(self.request('/api/jobs?include_consolidated=invalid')[0],400)
        filtered=self.request('/api/jobs?category=fine_tuning&execution_mode=pilot&limit=1&offset=0')
        self.assertEqual(filtered[0],200);self.assertEqual(filtered[1][0]['id'],job)
        self.assertEqual(self.request('/api/jobs/'+job)[1]['execution_mode'],'pilot')
        for query in ['category=unknown','category=sdg&category=other','offset=no','limit=0','bogus=1']:
            self.assertEqual(self.request('/api/jobs?'+query)[0],400)
        self.assertEqual(self.request('/api/jobs/contract',token=None)[0],403)

    def setUp(self):
        super().setUp()
        for p in [patch.object(ops,'guard'),patch.dict(os.environ,{'LOOM_DEV_NO_AUTH':'0'})]:
            p.start();self.addCleanup(p.stop)
        self.server=ThreadingHTTPServer(('127.0.0.1',0),ops.API);self.server.config=self.config
        self.thread=Thread(target=self.server.serve_forever,kwargs={'poll_interval':.01},daemon=True);self.thread.start()
        self.addCleanup(self.close_server)
        self.base='http://127.0.0.1:'+str(self.server.server_port)
    def close_server(self):
        self.server.shutdown();self.server.server_close();self.thread.join(2)
    def request(self,path,body=None,token='fixture-owner',headers=None):
        h={'Content-Type':'application/json',**(headers or {})}
        if token:h['Authorization']='Bearer '+token
        request=Request(self.base+path,data=json.dumps(body).encode() if body is not None else None,headers=h)
        try:
            with urlopen(request,timeout=3) as r:return r.status,json.load(r)
        except HTTPError as r:return r.code,json.load(r)
    def test_auth_and_cross_origin_denial(self):
        self.assertEqual(self.request('/api/status',token=None)[0],403)
        self.assertEqual(self.request('/api/status',token='wrong')[0],403)
        self.assertEqual(self.request('/api/status',headers={'Origin':'https://other.invalid'})[0],403)
        self.assertEqual(self.request('/api/status')[0],200)
    def test_dev_bypass_is_explicit_and_does_not_bypass_origin(self):
        with patch.dict(os.environ,{'LOOM_DEV_NO_AUTH':'1'}):
            self.assertEqual(self.request('/api/status',token=None)[0],200)
            self.assertEqual(self.request('/api/status',token=None,headers={'Origin':'https://other.invalid'})[0],403)
    def test_submit_list_and_detail_obey_scope(self):
        status,result=self.request('/api/jobs',self.spec());self.assertEqual(status,200)
        identity=result['id']
        self.assertEqual(self.request('/api/jobs',self.spec())[1]['id'],identity)
        self.assertEqual(self.request('/api/jobs',self.spec(seconds=2))[0],400)
        self.assertEqual(self.request('/api/jobs/'+identity,token='fixture-reader')[0],200)
        self.assertEqual(self.request('/api/jobs',token='fixture-reader')[1][0]['id'],identity)
        self.assertEqual(self.request('/api/jobs',self.spec('other'),token='fixture-reader')[0],403)
    def test_mcp_discovery_matches_callable_groups(self):
        def rpc(method,params=None,token='fixture-owner'):
            return self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':method,'params':params or {}},token)[1]
        tools=rpc('tools/list',token='fixture-reader')['result']['tools']
        names={t['name'] for t in tools}
        self.assertIn('system',names);self.assertNotIn('submit_job',names);self.assertNotIn('node_status',names)
        for name in ['system_status','node_status']:
            self.assertTrue(rpc('tools/call',{'name':name,'arguments':{}})['result']['isError'])
        self.assertTrue(rpc('tools/call',{'name':'submit_job','arguments':self.spec()},'fixture-reader')['result']['isError'])
        self.assertEqual(rpc('unknown')['error']['code'],-32601)
    def test_invalid_json_argument_has_bounded_error(self):
        reply=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'get_job','arguments':{'id':[]}}})
        self.assertEqual(reply[0],200);self.assertTrue(reply[1]['result']['isError'])

    def test_queue_controls_http_mcp_scopes_schema_and_catalog_agree(self):
        a=self.request('/api/jobs',self.spec('queue-a'))[1]['id']
        b=self.request('/api/jobs',{**self.spec('queue-b'),'queue_position':'next'})[1]['id']
        status,queue=self.request('/api/jobs/queue');self.assertEqual(status,200)
        self.assertEqual([r['id'] for r in queue['entries']],[b,a])
        status,receipt=self.request('/api/actions',{'group':'jobs','action':'hold','arguments':{'id':b}})
        self.assertEqual(status,200);self.assertTrue(receipt['held'])
        rpc=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'jobs','arguments':{'action':'resume','arguments':{'id':b}}}})[1]['result']
        self.assertFalse(json.loads(rpc['content'][0]['text'])['held'])
        self.assertFalse(rpc.get('isError'))
        for action,args in [('hold',{}),('resume',{}),('reorder',{'position':'tail'})]:
            self.assertEqual(self.request('/api/actions',{'group':'jobs','action':action,'arguments':{'id':b,**args}},token='fixture-reader')[0],403)
        public=self.request('/api/jobs/queue',token='fixture-reader')[1]
        self.assertEqual(len(public['entries']),2);self.assertNotIn('spec',json.dumps(public))
        catalog=self.request('/api/dashboard/catalog')[1]
        self.assertEqual(catalog['queue'],self.request('/api/jobs/queue')[1]);revision=catalog['revision']
        self.request('/api/actions',{'group':'jobs','action':'reorder','arguments':{'id':b,'position':'tail'}})
        self.assertNotEqual(revision,self.request('/api/dashboard/revision')[1]['revision'])
        self.assertEqual(self.request('/api/jobs/contract')[1]['queue_policy']['scope'],'submit')

    def test_tokenless_tailnet_mcp_discovery_and_reconnect_permission(self):
        headers=self.tailnet(allow_new_users=True)
        def rpc(method,params=None,h=headers,token=None):
            return self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':method,'params':params or {}},token=token,headers=h)
        with patch.dict(os.environ,{'LOOM_DEV_NO_AUTH':'0'}):
            self.assertEqual(rpc('initialize')[0],200)
            tools=rpc('tools/list')[1]['result']['tools']
            self.assertIn('reconnect',next(t for t in tools if t['name']=='results')['inputSchema']['properties']['action']['enum'])
            call={'name':'system','arguments':{'action':'describe','arguments':{'group':'results','action':'reconnect'}}}
            result=rpc('tools/call',call)[1]['result']
            self.assertFalse(result.get('isError'));self.assertIn('storage',json.loads(result['content'][0]['text'])['schema']['properties'])
            self.assertEqual(rpc('initialize',h={})[0],403)
            self.assertEqual(rpc('initialize',token='wrong')[0],403)
            self.config['tailscale_auth']['trusted_proxies']=['192.0.2.1']
            self.assertEqual(rpc('initialize')[0],403)

    def test_reconnect_http_and_mcp_share_storage_integrity_and_scope(self):
        from pathlib import Path
        extra=self.root/'extra';extra.mkdir()
        (extra/'.loom-storage.json').write_text(json.dumps({'storage_id':'extra-id'}))
        self.config['data_stores']={'data/1':{'path':str(extra),'storage_id':'extra-id','mode':'rw'}}
        job=self.submit();row=self.claim(job)
        with patch.object(ops,'guard'):
            ops.execute(row,self.config)
            ops.process_result({**self.row(job),'retries':0},self.config)
        entity=json.loads(self.row(job)['result'])['entities'][0]
        source=self.data/entity['locator']['ref']
        (extra/'copy.json').write_bytes(source.read_bytes())
        (self.data/'copy.json').write_bytes(source.read_bytes())
        args={'id':job+'.observation','revision':1,'component':'observation.json','ref':'copy.json'}
        def mcp(a,token='fixture-owner'):
            return self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'results','arguments':{'action':'reconnect','arguments':a}}},token=token)[1]['result']
        for store in ('data/0','data/1'):
            a={**args,'storage':store}
            http=self.request('/api/registry/reconnect',a)
            result=mcp(a)
            self.assertEqual(http[0],200);self.assertFalse(result.get('isError'))
            self.assertEqual(http[1],json.loads(result['content'][0]['text']))
        self.assertEqual(self.request('/api/registry/reconnect',args,token='fixture-reader')[0],403)
        self.assertTrue(mcp(args,'fixture-reader')['isError'])
        for a in ({**args,'storage':'data/99'},{**args,'ref':'../outside'},{**args,'component':'missing'}):
            self.assertNotEqual(self.request('/api/registry/reconnect',a)[0],200)
            self.assertTrue(mcp(a)['isError'])
        (extra/'copy.json').write_text('corrupted')
        self.assertEqual(self.request('/api/registry/reconnect',{**args,'storage':'data/1'})[0],400)
        self.assertTrue(mcp({**args,'storage':'data/1'})['isError'])
