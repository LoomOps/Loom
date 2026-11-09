import copy
import json
import threading
import time
from unittest.mock import patch
import unittest
import test_admission as fixtures
from test_admission import FakeAPI
from admission import RemoteError, group_name, make_admission
from sharing import SharingAdmission, pending_policy


class SharingAPI(FakeAPI):
    def __init__(self):
        super().__init__();self.shares=[];self.create_timeout=False
        self.policy['grants']=[{'src':['owner@example.test'],'dst':['100.64.0.1'],'ip':['*']}]
    def call(self,method,path,value=None,headers=None):
        if 'device-invites' not in path:return super().call(method,path,value,headers)
        self.calls.append((method,path,copy.deepcopy(value),headers))
        if path=='/device/server1/device-invites':
            if method=='POST':
                self.shares.append({'id':str(len(self.shares)+1),'created':'2026-11-08T00:00:00Z',**value[0],'accepted':False})
                if self.create_timeout:raise OSError('timeout')
            return copy.deepcopy(self.shares),{}
        if method=='DELETE':
            self.shares=[s for s in self.shares if s['id']!=path.rsplit('/',1)[-1]];return {},{}
        raise AssertionError((method,path))


class SharingTests(unittest.TestCase):
    row=fixtures.AdmissionTests.row
    http=fixtures.AdmissionTests.http
    # Use fixture helpers, not the old membership-contract tests.
    def setUp(self):
        fixtures.AdmissionTests.setUp(self);self.api=SharingAPI();self.config['enrollment_mode']='sharing'
        self.a=make_admission(self.config,self.api)
    def received(self,login='actual@example.test'):
        self.api.shares[0].update(accepted=True,acceptedBy={'id':321,'loginName':login})
        self.a.update(self.row()['id'],next_attempt=0);self.a.tick()
    def ready(self):
        self.a.request(self.user);self.a.tick();self.received()
        self.a.decide(self.row()['id'],'approve',self.admin);self.a.tick()
    def test_sharing_accept_then_approve_actual_identity(self):
        r=self.a.request(self.user)
        with self.assertRaises(ValueError):self.a.decide(r['id'],'approve',self.admin)
        self.a.tick();self.assertEqual(self.row()['state'],'invited')
        self.assertEqual(self.api.shares[0]['email'],'member@example.test')
        self.assertFalse(self.api.shares[0]['multiUse']);self.assertFalse(self.api.shares[0]['allowExitNode'])
        self.received();self.assertEqual(self.row()['state'],'awaiting_approval')
        self.assertNotIn(group_name('installation1','server1'),self.api.policy['groups'])
        self.a.decide(r['id'],'approve',self.admin);self.a.tick()
        self.assertEqual(self.row()['state'],'ready')
        self.assertEqual(self.api.policy['groups'][group_name('installation1','server1')],['actual@example.test'])
        self.assertFalse(any('/user-invites' in p or '/authorized' in p for _,p,_,_ in self.api.calls))
        self.assertEqual(self.a.members()[0]['loginName'],'actual@example.test')
        self.assertNotIn('inviteUrl',json.dumps(self.a.members()))
    def test_existing_native_admin_and_approved_identity_cannot_request_share(self):
        self.config['admins']=[{'issuer':'https://private.test/identity','subject':'tailscale:owner@example.test'}]
        for email in [' Owner@Example.Test ', 'native@example.test']:
            self.config['native_logins']=['native@example.test']
            with self.assertRaisesRegex(ValueError,'access_check_required'):
                self.a.request({**self.user,'login':email})
        self.assertEqual(self.api.calls,[])
        self.config['admins']=[{'issuer':'https://identity.test','subject':'admin'}]
        self.ready()
        calls=len(self.api.calls)
        with self.assertRaisesRegex(ValueError,'access_check_required'):
            self.a.request({**self.user,'login':'actual@example.test'})
        self.assertEqual(len(self.api.calls),calls)

    def test_private_worker_blocks_existing_tailnet_member_without_sending(self):
        self.api.users=[{'loginName':self.user['login'],'type':'member'}]
        self.a.request(self.user);self.a.tick()
        self.assertEqual(self.row()['error'],'access_check_required')
        self.assertFalse(self.row()['send_started']);self.assertEqual(self.api.shares,[])
        self.assertFalse(any(m=='POST' for m,_,_,_ in self.api.calls))

    def test_old_pending_admin_request_is_rechecked_before_invitation(self):
        self.a.request(self.user)
        self.config['native_logins']=[self.user['login']]
        self.a.tick()
        self.assertEqual(self.row()['error'],'access_check_required')
        self.assertEqual(self.api.shares,[])

    def test_membership_lookup_failure_cannot_fall_through_to_send(self):
        self.a.request(self.user);self.api.fail='/users?type=member';self.a.tick()
        self.assertEqual(self.api.shares,[]);self.assertFalse(self.row()['send_started'])
        self.assertEqual(self.row()['error'],'tailscale_api_403')

    def test_invitation_sender_is_resolved_without_blocking_sharing(self):
        self.a.request(self.user);self.a.tick()
        self.api.shares[0]['sharerId']=42
        original=self.api.call
        def fail(method,path,*args,**kwargs):
            if path=='/users/42':raise RemoteError(503)
            return original(method,path,*args,**kwargs)
        self.a.update(self.row()['id'],next_attempt=0)
        with patch.object(self.api,'call',side_effect=fail):self.a.tick()
        self.assertEqual(self.row()['state'],'invited');self.assertIsNone(self.row()['error'])
        self.assertIsNone(self.row()['sender_email'])
        def success(method,path,*args,**kwargs):
            if path=='/users/42':return {'displayName':'Sharing Admin','loginName':'sender@example.test'},{}
            return original(method,path,*args,**kwargs)
        self.a.update(self.row()['id'],next_attempt=0)
        with patch.object(self.api,'call',side_effect=success):self.a.tick()
        from admission_http import safe_rows
        public=safe_rows([self.row()])[0]
        self.assertEqual(public['sender_email'],'sender@example.test')
        self.assertEqual(public['sender_name'],'Sharing Admin')
        self.assertNotIn('invite_id',public)

    def test_sharing_dedup_cross_receipts_and_concurrent(self):
        others=[{**self.user,'subject':str(i)} for i in range(5)]
        services=[SharingAdmission(self.config,self.api) for _ in others]
        threads=[threading.Thread(target=a.request,args=(u,)) for a,u in zip(services,others)]
        for t in threads:t.start()
        for t in threads:t.join()
        ids={self.a.rows(u)[0]['id'] for u in others};self.assertEqual(len(ids),1)
        self.a.tick();self.assertEqual(len(self.api.shares),1)
    def test_sharing_restart_after_uncertain_send_reuses_invite(self):
        self.a.request(self.user);self.api.create_timeout=True;self.a.tick()
        self.a=SharingAdmission(self.config,self.api);self.a.update(self.row()['id'],next_attempt=0);self.a.tick()
        self.assertEqual(len(self.api.shares),1);self.assertEqual(self.row()['state'],'invited')
    def test_reused_manual_share_survives_request_revocation(self):
        self.api.shares=[{'id':'manual','email':self.user['login'],'accepted':False,'multiUse':False,'allowExitNode':False}]
        self.a.request(self.user);self.a.tick()
        self.assertFalse(self.row()['send_started'])
        self.a.decide(self.row()['id'],'deny',self.admin);self.a.tick()
        self.assertEqual(self.row()['state'],'denied')
        self.assertEqual([s['id'] for s in self.api.shares],['manual'])
        self.assertFalse(any(m=='DELETE' for m,_,_,_ in self.api.calls))
    def test_existing_invitation_repairs_missing_ownership_receipt(self):
        import managed_resources
        self.a.request(self.user);self.a.tick()
        managed_resources.path(self.config).unlink()
        self.a.update(self.row()['id'],next_attempt=0);self.a.tick()
        saved=managed_resources.read(self.config)['resources']['invitation/'+self.row()['invite_id']]
        self.assertTrue(saved['after']['value']['created'])
        self.assertEqual(len(self.api.shares),1)
    def test_sharing_deny_revoke_and_session_recheck(self):
        self.ready();network={'Tailscale-User-Login':'actual@example.test'}
        identity=self.a.authorize_network(network);self.assertTrue(identity)
        self.assertFalse(self.a.is_admin(identity));self.assertTrue(self.a.session_allowed(identity,network))
        self.assertFalse(self.a.session_allowed(identity,{}))
        self.a.decide(self.row()['id'],'revoke',self.admin)
        self.assertFalse(self.a.session_allowed(identity,network));self.a.tick()
        self.assertEqual(self.row()['state'],'revoked');self.assertEqual(self.api.shares,[])
        self.assertEqual(self.api.policy['groups'][group_name('installation1','server1')],[])
        self.assertEqual(self.api.policy['grants'][0]['src'],['owner@example.test'])
    def test_sharing_expiry_and_external_removal(self):
        self.a.request(self.user);self.a.tick();self.a.update(self.row()['id'],expires=0,next_attempt=0)
        self.a.tick();self.a.tick();self.assertEqual(self.row()['state'],'expired');self.assertEqual(self.api.shares,[])
    def test_sharing_broad_policy_blocks_invite(self):
        self.api.policy['grants'].append({'src':['autogroup:shared'],'dst':['100.64.0.1'],'ip':['tcp:8443']})
        self.a.request(self.user);self.a.tick()
        self.assertEqual(self.api.shares,[]);self.assertEqual(self.row()['error'],'existing_policy_allows_unapproved_shared_user')
        with self.assertRaises(ValueError):pending_policy({},self.config)
        for source in ['autogroup:member','100.64.0.0/10','*']:
            with self.subTest(source=source),self.assertRaises(ValueError):
                pending_policy({'grants':[{'src':[source],'dst':['100.64.0.1'],'ip':['*']}]},self.config)

    def test_public_gateway_does_not_grant_private_access_before_approval(self):
        c={**self.config,'origin':'https://machine.test:10000','dashboard_origin':'https://machine.test:8443','ports':[8443]}
        rule={'src':['autogroup:shared'],'dst':c['addresses'],'ip':['tcp:10000']}
        pending_policy({'grants':[rule]},c)
        for ports in [['tcp:10000','tcp:8443'],['*'],['tcp:1-65535'],['udp:10000']]:
            with self.subTest(ports=ports),self.assertRaises(ValueError):
                pending_policy({'grants':[{**rule,'ip':ports}]},c)
        with self.assertRaises(ValueError):
            pending_policy({'grants':[rule]},{**c,'ports':[10000,8443]})
    def test_sharing_external_share_listing_and_revoke(self):
        self.api.shares=[{'id':'external','accepted':True,'acceptedBy':{'id':9,'loginName':'shared@example.test'},'created':'2026-11-08T00:00:00Z'}]
        self.assertEqual(self.a.members(),[])
        self.a.revoke_share('external',self.admin);self.a.tick();self.assertEqual(self.api.shares,[])

    def test_members_only_contains_granted_matching_identities(self):
        self.a.request(self.user);self.assertEqual(self.a.members(),[])
        self.a.tick();self.assertEqual(self.a.members(),[])
        self.received();self.assertEqual(self.a.members(),[])
        self.a.decide(self.row()['id'],'approve',self.admin)
        self.assertEqual(self.a.members(),[])  # Policy application has not completed.
        self.a.tick();self.assertEqual(self.a.members()[0]['role'],'Member')
        self.api.shares[0]['acceptedBy']['id']=999
        self.assertEqual(self.a.members(),[])
        self.api.shares[0]['acceptedBy']['id']=321
        self.a.decide(self.row()['id'],'revoke',self.admin)
        self.assertEqual(self.a.members(),[])  # Hide immediately, before remote cleanup.

    def test_admin_removes_denied_request_without_a_replacement_status(self):
        r=self.a.request(self.user)
        with self.assertRaises(ValueError):self.a.decide(r['id'],'remove',self.admin)
        self.a.decide(r['id'],'deny',self.admin)
        with self.assertRaises(ValueError):self.a.decide(r['id'],'remove',self.admin)
        self.a.tick()
        with self.assertRaises(PermissionError):self.a.decide(r['id'],'remove',self.user)
        self.a.decide(r['id'],'remove',self.admin)
        self.assertEqual(self.a.rows(self.admin,admin=True),[]);self.assertEqual(self.a.rows(self.user),[])
        with self.a.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM share_receipts').fetchone()[0],0)
            self.assertEqual(c.execute("SELECT count(*) FROM audit WHERE request=? AND action='share_remove'",(r['id'],)).fetchone()[0],1)
        self.assertNotEqual(self.a.request(self.user)['id'],r['id'])
    def test_revoked_request_removal_requires_completed_cleanup_and_admin(self):
        self.ready();key=self.row()['id']
        with self.assertRaises(ValueError):self.a.decide(key,'remove',self.admin)
        self.a.decide(key,'revoke',self.admin)
        with self.assertRaises(ValueError):self.a.decide(key,'remove',self.admin)
        self.a.tick();self.assertEqual(self.row()['state'],'revoked');self.assertEqual(self.api.shares,[])
        with self.assertRaises(PermissionError):self.a.decide(key,'remove',self.user)
        self.a.decide(key,'remove',self.admin)
        self.assertEqual(self.a.rows(self.admin,admin=True),[])
        self.assertEqual(self.a.rows(self.user),[])
        with self.a.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM share_receipts').fetchone()[0],0)
            self.assertEqual(c.execute("SELECT count(*) FROM audit WHERE request=? AND action='share_remove'",(key,)).fetchone()[0],1)

    def test_sharing_receipt_is_not_network_auth(self):
        self.ready();self.assertIsNone(self.a.authorize_network({'login':'actual@example.test'}))
        self.assertIsNone(self.a.authorize_network({'Tailscale-User-Login':'member@example.test'}))
    def test_sharing_conflict_and_retry_preserves_unrelated_policy(self):
        self.a.request(self.user);self.a.tick();self.received();self.api.conflict=True
        self.a.decide(self.row()['id'],'approve',self.admin);self.a.tick()
        self.assertEqual(self.row()['state'],'ready');self.assertIn('group:concurrent',self.api.policy['groups'])

    def test_sharing_private_session_requires_approved_serve_identity(self):
        from pathlib import Path
        from server import make_server
        from sso import SSO
        self.ready()
        key=Path(self.tmp.name)/'relay-key';key.write_text('k'*48)
        auth=SSO({'issuer':'https://identity.test','origin':'https://private.test',
                  'relay_key_file':str(key),'session_db':str(Path(self.tmp.name)/'sessions.db')})
        s=make_server('127.0.0.1',0,'http://127.0.0.1:1');s.admission=self.a;s.sso=auth;s.tailscale_proxies=['127.0.0.1']
        t=threading.Thread(target=s.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(s.server_close);self.addCleanup(s.shutdown)
        network={'Tailscale-User-Login':'actual@example.test'}
        with patch('urllib.request.urlopen',side_effect=AssertionError('No provider request')):
            self.assertEqual(self.http(s,'/auth/login')[0],401)
            code,_,h=self.http(s,'/auth/login',headers=network)
        self.assertEqual(code,303);cookie=h['Set-Cookie'].split(';')[0]
        headers={**network,'Cookie':cookie}
        data=json.loads(self.http(s,'/session',headers=headers)[1])
        self.assertFalse(data['authentication_required']);self.assertFalse(data['user']['access_admin'])
        self.assertEqual(data['user']['authentication'],'tailscale-sharing')
        self.assertTrue(json.loads(self.http(s,'/session',headers={'Cookie':cookie})[1])['authentication_required'])
        self.assertEqual(self.http(s,'/admission/members',headers=headers)[0],403)
        s.tailscale_proxies=['192.0.2.1']
        self.assertTrue(json.loads(self.http(s,'/session',headers=headers)[1])['authentication_required'])
        s.tailscale_proxies=['127.0.0.1'];self.a.decide(self.row()['id'],'revoke',self.admin)
        self.assertTrue(json.loads(self.http(s,'/session',headers=headers)[1])['authentication_required'])

    def test_sharing_public_request_never_leaks_acceptor_or_share_url(self):
        from http.server import ThreadingHTTPServer
        from admission_http import PublicHandler,ReceiptAuth
        s=ThreadingHTTPServer(('127.0.0.1',0),PublicHandler);s.admission=self.a;s.sso=ReceiptAuth(self.a)
        t=threading.Thread(target=s.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(s.server_close);self.addCleanup(s.shutdown)
        _,body,h=self.http(s,'/api/session');session=json.loads(body)
        headers={'Cookie':h['Set-Cookie'].split(';')[0],'Origin':self.config['origin'],'X-Loom-CSRF':session['csrf'],'Content-Type':'application/json'}
        code,body,_=self.http(s,'/api/request','POST',{'email':'member@example.test'},headers)
        self.assertEqual(code,200);r=json.loads(body)['request'];self.a.tick()
        self.api.shares[0].update(accepted=True,acceptedBy={'id':7,'loginName':'actual@example.test'},inviteUrl='https://secret-invite.test')
        self.a.update(r['id'],next_attempt=0);self.a.tick()
        body=self.http(s,'/api/request',headers=headers)[1]
        self.assertIn(b'awaiting_approval',body);self.assertNotIn(b'actual@example',body);self.assertNotIn(b'secret-invite',body)
        self.assertEqual(self.http(s,'/api/device-name','POST',{'name':'laptop'},headers)[0],404)

    def test_old_admin_cookie_cannot_override_current_network_user(self):
        import test_sso
        from server import make_server
        f=test_sso.SSOTest();f.setUp();self.addCleanup(f.doCleanups)
        f.claim_changes={'sub':self.admin['subject'],'email':self.admin['login']}
        old,_=f.finish()  # Verified network login + persisted opaque session.
        f.auth.config['provider']='tailscale'
        self.ready()
        s=make_server('127.0.0.1',0,'http://127.0.0.1:1');s.admission=self.a;s.sso=f.auth;s.tailscale_proxies=['127.0.0.1']
        t=threading.Thread(target=s.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(s.server_close);self.addCleanup(s.shutdown)
        owner={'Cookie':old['Cookie'],'Tailscale-User-Login':'owner@example.test'}
        self.assertTrue(json.loads(self.http(s,'/session',headers=owner)[1])['user']['access_admin'])
        self.assertEqual(self.http(s,'/admission/requests',headers=owner)[0],200)
        for login in [None,'actual@example.test','unapproved@example.test']:
            h={'Cookie':old['Cookie']}
            if login:h['Tailscale-User-Login']=login
            self.assertIsNone(json.loads(self.http(s,'/session',headers=h)[1])['user'])
            for path in ['/api/status','/admission/requests','/admission/members']:
                self.assertEqual(self.http(s,path,headers=h)[0],401)
            code,_,_=self.http(s,'/admission/decide','POST',{'id':self.row()['id'],'action':'revoke'},
                {**h,'Origin':f.auth.origin,'X-Loom-CSRF':f.auth.session(old)['csrf'],'Content-Type':'application/json'})
            self.assertEqual(code,401)
        # Gateway must recognize the current network member, not the old admin.
        member={'Cookie':old['Cookie'],'Tailscale-User-Login':'actual@example.test'}
        _,body,_=self.http(s,'/gateway/probe?nonce=abcdefghijklmnop',headers=member)
        self.assertIn(b'actual@example.test',body);self.assertNotIn(b'owner@example.test',body)
        self.assertIn(b'"destination": "/auth/login"',body)
        code,_,headers=self.http(s,'/auth/login',headers=member)
        self.assertEqual(code,303)
        member['Cookie']=headers['Set-Cookie'].split(';')[0]
        user=json.loads(self.http(s,'/session',headers=member)[1])['user']
        self.assertEqual(user['login'],'actual@example.test');self.assertFalse(user['access_admin'])
        self.assertEqual(self.http(s,'/admission/requests',headers=member)[0],403)
        s.tailscale_proxies=['192.0.2.1']
        self.assertIsNone(json.loads(self.http(s,'/session',headers=member)[1])['user'])
        self.assertEqual(self.http(s,'/auth/login',headers=member)[0],401)
    def test_requester_cancel_cleans_invite_and_never_revokes_approved_access(self):
        self.a.request(self.user);self.a.tick()
        self.assertTrue(self.a.rows(self.user)[0]['can_cancel'])
        self.assertTrue(self.a.cancel_own_pending(self.user));self.a.tick()
        self.assertTrue(self.a.rows(self.admin,admin=True)[0]['user_cancelled'])
        self.assertFalse(self.a.rows(self.user)[0]['can_cancel'])
        self.assertEqual(self.row()['state'],'revoked');self.assertEqual(self.api.shares,[])
        self.ready()
        with self.assertRaises(PermissionError):self.a.cancel_own_pending(self.user,strict=True)
        self.assertFalse(self.a.cancel_own_pending(self.user));self.assertEqual(self.row()['state'],'ready')

    def test_other_receipt_cannot_cancel_by_knowing_email(self):
        self.a.request(self.user);self.a.tick()
        other={**self.user,'subject':'different-browser'};self.a.request(other)
        self.assertFalse(self.a.rows(other)[0]['can_cancel'])
        with self.assertRaises(PermissionError):self.a.cancel_own_pending(other,strict=True)
        self.assertFalse(self.a.cancel_own_pending(other))
        self.assertEqual(self.row()['state'],'invited')

    def test_cancel_during_send_cannot_be_overwritten_by_worker(self):
        r=self.a.request(self.user)
        self.a.update(r['id'],send_started=1)
        self.assertTrue(self.a.cancel_own_pending(self.user))
        self.api.shares=[{'id':'late','accepted':False,'email':self.user['login']}]
        self.a.update(r['id'],state='invited',invite_id='late',next_attempt=time.time()+30)
        self.assertEqual(self.row()['state'],'revoking');self.assertEqual(self.row()['next_attempt'],0)
        self.a.tick();self.assertEqual(self.api.shares,[])

    def test_public_reset_rotates_receipt_and_allows_new_email(self):
        from http.server import ThreadingHTTPServer
        from admission_http import PublicHandler,ReceiptAuth
        s=ThreadingHTTPServer(('127.0.0.1',0),PublicHandler);s.admission=self.a;s.sso=ReceiptAuth(self.a)
        t=threading.Thread(target=s.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(s.server_close);self.addCleanup(s.shutdown)
        _,body,h=self.http(s,'/api/session');old=json.loads(body)
        headers={'Cookie':h['Set-Cookie'].split(';')[0],'Origin':self.config['origin'],'X-Loom-CSRF':old['csrf'],'Content-Type':'application/json'}
        code,_,_=self.http(s,'/api/request','POST',{'email':'typo@example.test'},headers);self.assertEqual(code,200)
        self.a.tick()
        self.assertEqual(self.http(s,'/api/request/reset','POST',{}, {**headers,'X-Loom-CSRF':'bad'})[0],403)
        code,body,_=self.http(s,'/api/request/reset','POST',{},headers);self.assertEqual(code,200)
        self.assertTrue(json.loads(body)['cancellation_queued'])
        new=json.loads(self.http(s,'/api/session',headers=headers)[1]);self.assertIsNone(new['user']['login'])
        self.assertNotEqual(new['csrf'],old['csrf']);self.assertNotEqual(new['user']['subject'],old['user']['subject'])
        self.assertEqual(json.loads(self.http(s,'/api/request',headers=headers)[1])['requests'],[])
        self.assertEqual(self.http(s,'/api/request','POST',{'email':'correct@example.test'},headers)[0],403)
        headers['X-Loom-CSRF']=new['csrf']
        self.assertEqual(self.http(s,'/api/request','POST',{'email':'correct@example.test'},headers)[0],200)
        self.a.tick()
        self.assertEqual([i['email'] for i in self.api.shares],['correct@example.test'])

class SharingLocalFailureTests(unittest.TestCase):
    setUp=SharingTests.setUp
    row=SharingTests.row
    def test_local_permission_error_is_not_an_upstream_outage(self):
        self.a.request(self.user)
        with patch.object(self.a,'reconcile',side_effect=PermissionError('secret filesystem path')),patch('diagnostics.record') as collect:
            self.a.tick()
        self.assertEqual(self.row()['error'],'local_permission_denied')
        self.assertEqual(collect.call_args.kwargs['code'],'local_permission_denied')
        self.assertEqual(self.row()['state'],'requested')
