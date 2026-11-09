"""Admin-issued resident tokens use real HTTP, signed identity and SQLite."""
import base64
import hashlib
import hmac
import json
import time
from unittest.mock import patch
import ops
import ownership
import test_api_contract
from http.server import ThreadingHTTPServer
from threading import Thread
from test_support import IsolatedOps

class ServiceTokenTests(IsolatedOps):
    request=test_api_contract.APIContractTests.request
    close_server=test_api_contract.APIContractTests.close_server
    def setUp(self):
        IsolatedOps.setUp(self)
        for p in [patch.object(ops,'guard'),patch.dict('os.environ',{'LOOM_DEV_NO_AUTH':'0'})]:
            p.start();self.addCleanup(p.stop)
        self.server=ThreadingHTTPServer(('127.0.0.1',0),ops.API);self.server.config=self.config
        self.thread=Thread(target=self.server.serve_forever,kwargs={'poll_interval':.01},daemon=True);self.thread.start()
        self.addCleanup(self.close_server)
        self.base='http://127.0.0.1:'+str(self.server.server_port)
        self.key=self.root/'key';self.key.write_text('k'*48)
        self.config['dashboard_auth']={'enabled':True,'issuer':'https://identity.test','relay_key_file':str(self.key),'link_tailscale_email':True}
        with ops.db() as c:self.person=ownership.register(c,'member@example.test','Member')
    def signed(self,admin=True):
        value={'identity':{'issuer':'https://identity.test','subject':'admin','login':'admin@example.test','access_admin':admin,'token_owner_logins':getattr(self,'eligible_logins',['member@example.test'])},'aud':'loom-dashboard-relay','exp':time.time()+30}
        raw=base64.urlsafe_b64encode(json.dumps(value).encode()).decode().rstrip('=')
        return {'X-Loom-Dashboard-Identity':raw+'.'+hmac.new(self.key.read_bytes(),raw.encode(),hashlib.sha256).hexdigest()}
    def issue(self,scopes=None):
        code,result=self.request('/api/admin/tokens',{'label':'Resident test','owner':self.person['id'],'scopes':scopes or ['read','submit','cancel','events']},token=None,headers=self.signed())
        self.assertEqual(code,200,result);return result
    def test_no_expiry_restart_list_redaction_and_immediate_revoke(self):
        issued=self.issue();self.assertIsNone(issued['expires'])
        with ops.db() as c:
            stored=dict(c.execute('SELECT * FROM resident_tokens').fetchone())
            self.assertNotIn(issued['token'],json.dumps(stored))
        ops.init()
        token=issued['token']
        with patch('access.time.time',return_value=time.time()+365*86400):
            code,actor=self.request('/api/principal',token=token)
        self.assertEqual(code,200);self.assertEqual(actor['id'],self.person['id'])
        self.assertEqual(actor['credential_id'],issued['credential_id']);self.assertNotIn('identity',actor)
        code,listed=self.request('/api/admin/tokens',token=None,headers=self.signed())
        self.assertEqual(code,200);self.assertNotIn('digest',json.dumps(listed));self.assertNotIn(token,json.dumps(listed))
        for _ in range(2):
            self.assertEqual(self.request('/api/admin/tokens/revoke',{'id':issued['credential_id']},token=None,headers=self.signed())[0],200)
        self.assertEqual(self.request('/api/principal',token=token)[0],403)
        self.assertEqual(self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/list'},token=token)[0],403)
    def test_bootstrap_member_resident_and_dev_cannot_manage(self):
        token=self.issue()['token']
        for credential in ('fixture-owner','fixture-reader',token,None):
            with self.subTest(credential=credential is None):
                self.assertEqual(self.request('/api/admin/tokens',token=credential)[0],403)
        self.assertEqual(self.request('/api/admin/tokens',token=None,headers=self.signed(False))[0],403)
        with patch.dict('os.environ',{'LOOM_DEV_NO_AUTH':'1'}):
            self.assertEqual(self.request('/api/admin/tokens',token=None)[0],403)
    def test_permissions_owner_and_existing_jobs_survive_revocation(self):
        writable=self.issue()['token'];readonly=self.issue(['read'])['token']
        spec=self.spec();spec.update(owner=self.person['id'],notify=self.person['id'])
        self.assertEqual(self.request('/api/jobs',spec,token=readonly)[0],403)
        code,job=self.request('/api/jobs',spec,token=writable);self.assertEqual(code,200,job)
        self.assertEqual(self.row(job['id'])['owner'],self.person['id'])
        self.assertEqual(self.request('/api/jobs',token=readonly)[0],200)
        with ops.db() as c:c.execute('UPDATE resident_tokens SET revoked=1')
        self.assertEqual(self.row(job['id'])['state'],'queued')
    def test_invalid_owner_scopes_and_escalation_rejected(self):
        valid={'label':'agent','owner':self.person['id'],'scopes':['read']}
        for change in ({'owner':'unknown'},{'scopes':['admin']},{'scopes':[]},{'label':''},{'expires':None}):
            self.assertEqual(self.request('/api/admin/tokens',{**valid,**change},token=None,headers=self.signed())[0],400)

    def test_removed_owner_hidden_and_stale_submission_rejected_without_erasing_history(self):
        issued=self.issue()
        self.eligible_logins=[]
        code,listed=self.request('/api/admin/tokens',token=None,headers=self.signed())
        self.assertEqual(code,200)
        self.assertNotIn(self.person['id'],[p['id'] for p in listed['owners']])
        self.assertIn(self.person['id'],[p['id'] for p in listed['owner_profiles']])
        self.assertIn(issued['credential_id'],[t['id'] for t in listed['tokens']])
        code,_=self.request('/api/admin/tokens',{'label':'stale','owner':self.person['id'],'scopes':['read']},token=None,headers=self.signed())
        self.assertEqual(code,400)
        with ops.db() as c:
            self.assertEqual(c.execute('SELECT count(*) FROM resident_tokens').fetchone()[0],1)
            self.assertIsNotNone(c.execute('SELECT id FROM user_profiles WHERE id=?',(self.person['id'],)).fetchone())
        self.assertEqual(self.request('/api/admin/tokens/revoke',{'id':issued['credential_id']},token=None,headers=self.signed())[0],200)
