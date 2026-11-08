"""The browser cannot select scopes or forge its Dashboard relay identity."""
import base64,hashlib,hmac,json,time
from email.message import Message
import access,ownership,ops
from test_support import IsolatedOps


class DashboardIdentityTests(IsolatedOps):
    def setUp(self):
        super().setUp();self.key=self.root/'key';self.key.write_text('k'*48)
        self.config['dashboard_auth']={'enabled':True,'issuer':'https://identity.test','relay_key_file':str(self.key),'link_tailscale_email':True}

    def headers(self,**changes):
        data={'identity':{'issuer':'https://identity.test','subject':'subject-1','login':'member@example.test','name':'Member'},
              'aud':'loom-dashboard-relay','exp':time.time()+30,**changes}
        raw=base64.urlsafe_b64encode(json.dumps(data).encode()).decode().rstrip('=')
        h=Message();h['X-Loom-Dashboard-Identity']=raw+'.'+hmac.new(self.key.read_bytes(),raw.encode(),hashlib.sha256).hexdigest();return h

    def test_profile_links_verified_identity_without_rewriting_items(self):
        actor=access.dashboard_actor(self.config,self.headers())
        self.assertEqual(actor['scopes'],['read','submit','cancel','events'])
        with ops.db() as c:
            existing=ownership.register(c,'member@example.test','Old name')
            current=ownership.register_identity(c,actor['identity'])
            self.assertEqual(existing['id'],current['id'])
            self.assertEqual(current['display_name'],'Member')
        self.config['dashboard_auth']['link_tailscale_email']=False
        different=access.dashboard_actor(self.config,self.headers())
        self.assertTrue(different['id'].startswith('identity-'))

    def test_rejects_tampering_expiry_wrong_audience_issuer_and_duplicate(self):
        for changes in [{'exp':0},{'exp':time.time()+3600},{'aud':'other'},
                        {'identity':{'issuer':'https://evil.test','subject':'s'}}]:
            with self.subTest(changes=changes),self.assertRaises(PermissionError):access.dashboard_actor(self.config,self.headers(**changes))
        h=self.headers();h['X-Loom-Dashboard-Identity']='bad'
        with self.assertRaises(PermissionError):access.dashboard_actor(self.config,h)
        h=Message();h['X-Loom-Dashboard-Identity']='x.y'
        with self.assertRaises(PermissionError):access.dashboard_actor(self.config,h)
        self.config['dashboard_auth']['enabled']=False
        with self.assertRaises(PermissionError):access.dashboard_actor(self.config,self.headers())

    def test_binds_explicit_actor_without_adding_privileges(self):
        self.config['tailscale_auth']={'users':{'member@example.test':'reader'}}
        actor=access.dashboard_actor(self.config,self.headers());self.assertEqual(actor['scopes'],['read'])
        self.assertEqual(actor['id'],'reader')

    def test_shared_dashboard_session_and_mcp_have_same_actor(self):
        person={'issuer':'https://identity.test','subject':'shared-user:321','login':'shared@example.test',
                'name':'Shared user','authentication':'tailscale-sharing','share_request':'request1'}
        dashboard=access.dashboard_actor(self.config,self.headers(identity=person))
        self.config['tailscale_auth']={'enabled':True,'trusted_proxies':['127.0.0.1'],'allow_new_users':True}
        h=Message();h['Tailscale-User-Login']='shared@example.test'
        mcp=access.tailscale_actor(self.config,h,'127.0.0.1')
        self.assertEqual(dashboard['id'],mcp['id'])
        self.assertEqual(dashboard['scopes'],mcp['scopes'])
        self.assertNotIn('admin',dashboard['scopes'])

    def test_persisted_identity_and_owner_keys_survive_provider_cleanup(self):
        self.config['dashboard_auth']['link_tailscale_email']=False
        actor=access.dashboard_actor(self.config,self.headers())
        old='previous-profile-id'
        with ops.db() as c:
            c.execute('INSERT INTO user_profiles VALUES(?,?,?,?,?)',(old,'previous-provider','member@example.test','Old',1))
            c.execute('INSERT INTO auth_identities VALUES(?,?,?)',('https://identity.test','subject-1',old))
            c.execute('INSERT INTO item_owners VALUES(?,?,?,?,?)',('job','historical',old,1,'owner'))
            current=ownership.register_identity(c,actor['identity'])
            self.assertEqual(current['id'],old)
            self.assertEqual(c.execute('SELECT owner FROM item_owners').fetchone()[0],old)
            self.assertEqual(c.execute('SELECT count(*) FROM user_profiles').fetchone()[0],1)
