import copy,json,os,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'dashboard'))
import test_admission as fixtures
from test_sharing import SharingAPI
from admission import make_admission,RemoteError
from ssh_access import SSHAccess,owner

class SSHTests(unittest.TestCase):
    def setUp(self):
        fixtures.AdmissionTests.setUp(self)
        self.api=SharingAPI();self.a=make_admission(self.config,self.api);self.s=SSHAccess(self.a)
        self.env=patch.dict(os.environ,{'LOOM_SSH_EXPORT':str(Path(self.tmp.name)/'ssh-info')});self.env.start();self.addCleanup(self.env.stop)
        Path(os.environ['LOOM_SSH_EXPORT']).mkdir()
        self.calls=[]
        def host(action,person=None,name=None):
            self.calls.append(action);return {'state':'disabled' if action=='disable' else 'enabled'}
        self.mock=patch('ssh_access.host',side_effect=host);self.mock.start();self.addCleanup(self.mock.stop)
    def change(self,action='enable',login='member@example.test',name='loomu_member'):
        return self.s.change(login,name,action,self.admin,{login})
    def test_enable_disable_preserves_unrelated_policy_and_home(self):
        original=copy.deepcopy(self.api.policy)
        self.assertEqual(self.change()['state'],'enabled')
        self.assertEqual(self.calls,['ensure','enable'])
        rule=self.api.policy['ssh'][-1];self.assertEqual(rule['users'],['loomu_member']);self.assertEqual(rule['src'],['member@example.test'])
        self.assertEqual(self.change('disable')['state'],'disabled')
        for k,v in original.items():self.assertEqual(self.api.policy[k],v)
        self.assertEqual(self.calls[-1],'disable')
        result=json.loads((Path(os.environ['LOOM_SSH_EXPORT'])/'connections.json').read_text())
        self.assertEqual(result[owner('member@example.test')]['state'],'disabled')
    def test_unauthorized_no_host_or_policy_mutation(self):
        with self.assertRaises(PermissionError):self.s.change('member@example.test','loomu_member','enable',self.user,{'member@example.test'})
        with self.assertRaises(ValueError):self.s.change('member@example.test','loomu_member','enable',self.admin,set())
        self.assertEqual(self.calls,[])
    def test_policy_failure_disables_account_and_retains_retry_state(self):
        with patch.object(self.s,'policy',side_effect=RemoteError(403)):
            with self.assertRaises(RemoteError):self.change()
        self.assertEqual(self.calls,['ensure','disable']);self.assertEqual(self.s.rows()[0]['state'],'error')
        self.assertEqual(self.change()['state'],'enabled')
    def test_disable_blocks_locally_before_remote_failure(self):
        self.change();self.calls.clear()
        with patch.object(self.s,'policy',side_effect=OSError):
            with self.assertRaises(OSError):self.change('disable')
        self.assertEqual(self.calls,['disable']);self.assertEqual(self.s.rows()[0]['state'],'error')
    def test_existing_binding_cannot_move_to_another_account(self):
        self.change()
        with self.assertRaises(ValueError):self.change(name='loomu_other')
        self.assertEqual(len(self.s.rows()),1)
    def test_share_revocation_revokes_ssh_too(self):
        self.change();self.s.revoke('member@example.test')
        self.assertEqual(self.s.rows()[0]['state'],'disabled')
    def test_admin_or_member_cannot_create_system_account(self):
        for name in ['root','loom','tyui','loomu_a;id','loomu_../x']:
            with self.assertRaises(ValueError):self.change(name=name)
        self.assertFalse(self.calls)
    def test_existing_ssh_rule_not_claimed(self):
        from admission import machine_tag
        rule={'action':'accept','src':['member@example.test'],'dst':[machine_tag(self.config)],'users':['loomu_member']}
        self.api.policy.setdefault('ssh',[]).append(rule)
        self.change();self.change('disable');self.assertIn(rule,self.api.policy['ssh'])

    def test_real_http_requires_current_admin_session_and_csrf(self):
        from test_sso import SSOTest
        from server import make_server
        from threading import Thread
        import http.client
        f=SSOTest();f.setUp();self.addCleanup(f.doCleanups)
        cookie,person=f.finish();self.config['admins']=[{'issuer':person['issuer'],'subject':person['subject']}]
        web=make_server('127.0.0.1',0,'http://127.0.0.1:1');web.sso=f.auth;web.admission=self.a;web.tailscale_proxies=['127.0.0.1']
        thread=Thread(target=web.serve_forever,kwargs={'poll_interval':.01},daemon=True);thread.start()
        self.addCleanup(thread.join,2);self.addCleanup(web.server_close);self.addCleanup(web.shutdown)
        headers={**cookie,'Tailscale-User-Login':person['login'],'Content-Type':'application/json'}
        body={'login':person['login'],'username':'loomu_http','action':'enable'}
        def call(headers):
            c=http.client.HTTPConnection('127.0.0.1',web.server_port);c.request('POST','/admission/ssh',json.dumps(body),headers)
            r=c.getresponse();out=(r.status,r.read());c.close();return out
        self.assertEqual(call({})[0],401)
        self.assertEqual(call(headers)[0],403);self.assertFalse(self.calls)
        good={**headers,'Origin':f.auth.origin,'X-Loom-CSRF':f.auth.session(cookie)['csrf']}
        self.assertEqual(call({**good,'Origin':'https://evil.test'})[0],403)
        status,data=call(good);self.assertEqual(status,200,data);self.assertEqual(json.loads(data)['state'],'enabled')
        self.assertEqual(call({**good,'Tailscale-User-Login':'other@example.test'})[0],401)
