"""Network setup uses temp instances and fake CLI/API; never changes the host."""
import copy,importlib.util,json,os,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import sys
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'deploy'))
import tailnet


class TailnetTests(unittest.TestCase):
    def setUp(self):
        self.plan=json.loads((ROOT/'deploy/installer/tailnet-plan.example.json').read_text())
        self.plan['tailnet']='example.test'
        self.plan['https_port']=443

    def test_validation_and_policy_are_separate_from_application(self):
        tailnet.validate(self.plan)
        with patch.object(tailnet,'run') as run:
            p=tailnet.policy(self.plan);run.assert_not_called()
        self.assertEqual(p['grants'][0]['dst'],['loom-'+self.plan['hostname']]);self.assertEqual(p['ssh'],[])
        self.assertNotIn('tag:loom',p['grants'][0]['dst'])
        for field,value in [('hostname','bad\nname'),('bridge_cidr','0.0.0.0/24'),('https_port',False),('tailnet','')]:
            with self.subTest(field=field),self.assertRaises(ValueError):tailnet.validate({**self.plan,field:value})

    def test_enrollment_does_not_change_another_tailnet(self):
        with patch.object(tailnet,'status',return_value={'tailnet':'other','state':'Running'}),patch.object(tailnet,'run') as run:
            with self.assertRaises(ValueError):tailnet.enrollment(self.plan)
            run.assert_not_called()

    def test_ssh_policy_needs_real_device_identity_and_unique_tag(self):
        from hashlib import sha256
        self.plan['ssh']=True
        with self.assertRaises(ValueError):tailnet.policy(self.plan)
        p=tailnet.policy(self.plan,'device123')
        self.assertEqual(p['ssh'][0]['dst'],['tag:loom-machine-'+sha256(b'device123').hexdigest()[:12]])

    def test_session_lifetime_matches_dashboard_limits(self):
        for lifetime in (60,28800,604800):
            self.plan['sso']['session_seconds']=lifetime
            tailnet.validate(self.plan)
        for lifetime in (59,604801,True,604800.0,'604800'):
            self.plan['sso']['session_seconds']=lifetime
            with self.subTest(lifetime=lifetime),self.assertRaises(ValueError):tailnet.validate(self.plan)

    def test_pending_approval_returns_and_running_enrollment_preserves_tags(self):
        current={'tailnet':'example.test','state':'NeedsMachineAuth','tags':[]}
        with patch.object(tailnet,'status',return_value=current),patch.object(tailnet,'run') as run:
            self.assertFalse(tailnet.enrollment(self.plan)['ready']);run.assert_not_called()
        current['state']='Running';current['tags']=['tag:other']
        after={**current,'tags':['tag:other','tag:loom']}
        with patch.object(tailnet,'status',side_effect=[current,after]),patch.object(tailnet,'run') as run:
            self.assertTrue(tailnet.enrollment(self.plan)['ready'])
            self.assertIn('tag:loom,tag:other',run.call_args[0][-1])

    def instance(self,root):
        (root/'config').mkdir();(root/'config/api.json').write_text(json.dumps({'installation_id':'keep','actors':[{'id':'owner'}]}))
        (root/'network.env').write_text('OTHER=keep\n')
        (root/'compose.env').write_text('OPS_PORT=18892\nOPS_DASHBOARD_PORT=18893\n')
        (root/'compose-network.yaml').write_text('existing yaml preserved verbatim\n')

    def test_configure_is_repeatable_keeps_identity_and_separates_secrets(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);self.instance(root)
            result=tailnet.configure(root,self.plan,'loom.tail123.ts.net')
            import ipaddress
            network=json.loads((root/'compose-tailnet.yaml').read_text())
            ipam=network['networks']['default']['ipam']['config'][0]
            pool=ipaddress.ip_network(ipam['ip_range'])
            gateway=ipam['gateway']
            self.assertNotIn(ipaddress.ip_address(gateway),pool)
            self.assertEqual(tailnet.env_values(root/'network.env')['LOOM_TAILSCALE_TRUSTED_PROXIES'],gateway)
            self.assertIn(gateway,json.loads((root/'config/api.json').read_text())['tailscale_auth']['trusted_proxies'])
            for service in ['loom-ops','loom-dashboard']:
                fixed=ipaddress.ip_address(network['services'][service]['networks']['default']['ipv4_address'])
                self.assertNotIn(fixed,pool)
            secret=(root/'secrets/sso/relay-key').read_bytes()
            tailnet.configure(root,self.plan,'loom.tail123.ts.net')
            self.assertEqual(secret,(root/'secrets/sso/relay-key').read_bytes())
            self.assertEqual((root/'compose-network.yaml').read_text(),'existing yaml preserved verbatim\n')
            api=json.loads((root/'config/api.json').read_text());self.assertEqual(api['installation_id'],'keep')
            self.assertTrue(api['tailscale_auth']['allow_new_users']);self.assertTrue(api['dashboard_auth']['enabled'])
            self.assertEqual((root/'secrets/sso/relay-key').stat().st_mode&0o777,0o600)
            self.assertNotIn(secret.decode(),json.dumps(result));self.assertIn('OTHER=keep',(root/'network.env').read_text())
            self.assertEqual(len(list((root/'network-backups').iterdir())),2)
            with self.assertRaises(ValueError):tailnet.configure(root,{**self.plan,'sso':None},'loom.tail123.ts.net')

    def test_serve_conflict_checked_before_mutation_and_sso_routes_present(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);self.instance(root)
            before={'Web':{'loom.tail123.ts.net:443':{'Handlers':{'/':{'Proxy':'http://other'}}}}}
            with patch.object(tailnet,'run',return_value=json.dumps(before)) as run:
                with self.assertRaises(ValueError):tailnet.serve(root,self.plan,'loom.tail123.ts.net')
                self.assertEqual(run.call_count,1)
            with patch.object(tailnet,'run',return_value='{}') as run:
                result=tailnet.serve(root,self.plan,'loom.tail123.ts.net')
                self.assertIn('/auth',result['routes']);self.assertIn('/session',result['routes'])
                self.assertFalse(any('funnel' in c.args for c in run.call_args_list))

    def test_approval_uses_explicit_device_and_private_credential(self):
        with tempfile.TemporaryDirectory() as t:
            key=Path(t)/'key';key.write_text('fixture-secret')
            with patch.object(tailnet,'urlopen') as open:
                result=tailnet.approve('device123',key)
                req=open.call_args.args[0]
                self.assertTrue(req.full_url.endswith('/device/device123/authorized'))
                self.assertEqual(req.data,b'{"authorized":true}')
                self.assertNotIn('fixture-secret',json.dumps(result))

class IdentityRegistrationTests(unittest.TestCase):
    def test_existing_instance_dashboard_port_can_be_in_network_env(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            (root/'config').mkdir()
            (root/'compose.env').write_text('OPS_PORT=18892\n')
            (root/'network.env').write_text('OPS_DASHBOARD_PORT=18893\n')
            with patch.object(tailnet,'run',return_value='{}') as run:
                tailnet.serve(root,{'https_port':8443},'loom.tail123.ts.net')
                self.assertTrue(any(call.args[-1]=='http://127.0.0.1:18893' for call in run.call_args_list))
