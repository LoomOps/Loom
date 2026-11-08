import copy
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'deploy'))
import access_setup


class AccessSetupTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
        self.c=json.loads((ROOT/'deploy/installer/access-plan.example.json').read_text())
        self.c.update(machine_id='install1',device_id='device1',credential_file=str(self.root/'private.json'))
        (self.root/'private.json').write_text(json.dumps({'type':'api','token':'secret-fixture'}))
        self.plan=self.root/'plan.json';self.plan.write_text(json.dumps(self.c))

    def test_sharing_setup_keeps_credentials_private(self):
        c=access_setup.load(self.plan);self.assertEqual(c['enrollment_mode'],'sharing')
        (self.root/'secrets/sso').mkdir(parents=True);(self.root/'secrets/sso/config.json').write_text('{}')
        result=access_setup.configure(self.root,c)
        self.assertNotIn('secret-fixture',json.dumps(result))
        runtime=json.loads((self.root/'secrets/admission/config.json').read_text())
        self.assertEqual(runtime['credential_file'],'/etc/loom/admission/management.json')
        overlay=json.loads((self.root/'compose-admission.yaml').read_text())
        self.assertEqual(list(overlay['services']),['loom-dashboard','loom-gateway'])
        self.assertNotIn('ports',overlay['services']['loom-dashboard'])
        self.assertTrue(runtime['separate_gateway'])
        self.assertNotIn('management.json',json.dumps(overlay['services']['loom-gateway']['volumes']))
        self.assertEqual(overlay['services']['loom-gateway']['ports'],['127.0.0.1:18904:8081'])
        self.assertEqual((self.root/'secrets/admission/management.json').stat().st_mode&0o777,0o600)
        access_setup.configure(self.root,c)
        self.assertTrue(list((self.root/'network-backups').glob('admission-*/compose-admission.yaml')))

    def test_credential_prompt_hides_secret_and_refuses_overwrite(self):
        p=self.root/'credentials/key.json'
        with patch('getpass.getpass',return_value='tskey-api-fixture-secret'):
            result=access_setup.save_credential(p)
        self.assertEqual(p.stat().st_mode&0o777,0o600)
        self.assertNotIn('tskey-api',json.dumps(result))
        with self.assertRaisesRegex(ValueError,'credential_exists'):access_setup.save_credential(p)
        with patch('getpass.getpass',return_value='tskey-auth-wrong'),self.assertRaises(ValueError):
            access_setup.save_credential(self.root/'wrong.json')

    def test_missing_credentials_or_mixed_origin_rejected(self):
        for change in [{'credential_file':str(self.root/'absent')},{'origin':self.c['dashboard_origin']},{'ports':[True]},{'admins':[]}]:
            self.plan.write_text(json.dumps({**self.c,**change}))
            with self.subTest(change=change),self.assertRaises((ValueError,KeyError)):access_setup.load(self.plan)

    def test_hub_tag_addition_keeps_other_tags_and_grants_no_access(self):
        with patch.object(access_setup,'TailnetAPI') as factory:
            factory.return_value.call.side_effect=[({'addresses':self.c['addresses'],'tags':['tag:existing']},{}),({'tagOwners':{tag:['autogroup:admin'] for tag in ['tag:loom',access_setup.machine_tag(self.c)]}},{}),({}, {})]
            result=access_setup.register_machine(self.c)
            call=factory.return_value.call.call_args
            self.assertEqual(call.args,('POST','/device/device1/tags',{'tags':sorted(['tag:existing','tag:loom',access_setup.machine_tag(self.c)])}))
            self.assertFalse(result['user_access_granted'])

    def test_publish_refuses_port_with_private_routes_before_network_change(self):
        from urllib.parse import urlsplit
        host=urlsplit(self.c['origin']).hostname
        old={'Web':{host+':10000':{'Handlers':{'/':{'Proxy':'http://127.0.0.1:18893'},'/mcp':{'Proxy':'http://127.0.0.1:18892/mcp'}}}}}
        with patch.object(access_setup,'run',side_effect=[json.dumps({'Self':{'DNSName':host+'.'}}),json.dumps(old)]) as run:
            with self.assertRaisesRegex(ValueError,'dedicated_public_port'):access_setup.publish(self.c)
            self.assertEqual(run.call_count,2)

    def test_publish_uses_only_checked_public_listener(self):
        from urllib.parse import urlsplit
        from urllib.error import HTTPError
        host=urlsplit(self.c['origin']).hostname
        class Response:
            def __enter__(self):return self
            def __exit__(self,*a):pass
            def read(self):return json.dumps({'machine':'install1'}).encode()
        def fetch(url,**kw):
            if url.endswith('/api/session'):return Response()
            raise HTTPError(url,404,'not found',{},None)
        with patch.object(access_setup,'run',side_effect=[json.dumps({'Self':{'DNSName':host+'.'}}),'{}','']) as run,patch('urllib.request.urlopen',side_effect=fetch),patch.object(access_setup,'gateway_access',return_value={'gateway_grant_changed':True}) as grant:
            r=access_setup.publish(self.c)
            grant.assert_called_once_with(self.c)
            self.assertEqual(run.call_args.args,('tailscale','funnel','--bg','--https=10000','http://127.0.0.1:18904'))
            self.assertEqual(r['scope'],'request_listener_only')

    def test_gateway_grant_is_exact_idempotent_and_preserves_other_rules(self):
        old={'grants':[{'src':['owner@example.test'],'dst':self.c['addresses'],'ip':['tcp:8443']}]}
        policy=copy.deepcopy(old)
        def call(method,path,value=None,headers=None):
            if path=='/device/device1':return {'addresses':self.c['addresses']},{}
            if method=='GET':return copy.deepcopy(policy),{'ETag':'revision1'}
            if path.endswith('/validate'):return {},{}
            self.assertEqual(headers,{'If-Match':'revision1'})
            policy.update(value);return {},{}
        with patch.object(access_setup,'TailnetAPI') as factory:
            factory.return_value.call.side_effect=call
            self.assertTrue(access_setup.gateway_access(self.c)['gateway_grant_changed'])
            self.assertFalse(access_setup.gateway_access(self.c)['gateway_grant_changed'])
        self.assertEqual(policy['grants'][0],old['grants'][0])
        self.assertEqual(policy['grants'][1],{'src':['autogroup:member','autogroup:shared'],'dst':self.c['addresses'],'ip':['tcp:10000']})
