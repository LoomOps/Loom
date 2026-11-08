import copy
import http.client
import json
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from http.server import ThreadingHTTPServer
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'dashboard'))
from admission import make_admission, RemoteError, group_name
from admission_http import PublicHandler, ReceiptAuth
from server import make_server


class FakeAPI:
    def __init__(self):
        self.users=[];self.invites=[];self.calls=[];self.policy={'groups':{},'grants':[],'tagOwners':{'tag:loom':['owner@example.test']}}
        self.devices=[{'id':'server1','addresses':['100.64.0.1'],'tags':['tag:loom']},
                      {'id':'laptop','user':'member@example.test','hostname':'my-laptop','authorized':True}]
        self.conflict=False;self.fail=None

    def call(self, method,path,value=None,headers=None):
        self.calls.append((method,path,copy.deepcopy(value),headers))
        if self.fail and path.endswith(self.fail):raise RemoteError(403)
        if path.endswith('/users?type=member'):return {'users':copy.deepcopy(self.users)},{}
        if path.endswith('/devices'):return {'devices':copy.deepcopy(self.devices)},{}
        if path.startswith('/device/'):
            key=path.split('/')[2];device=next(d for d in self.devices if d['id']==key)
            if path.endswith('/authorized'):device['authorized']=value['authorized']
            return copy.deepcopy(device),{}
        if path.endswith('/acl/validate'):return {},{}
        if path.endswith('/acl'):
            if method=='POST':
                if self.conflict:
                    self.conflict=False;self.policy['groups']['group:concurrent']=['other@example.test'];raise RemoteError(412)
                if headers!={'If-Match':'"v1"'}:raise AssertionError('etag')
                self.policy=copy.deepcopy(value)
            return copy.deepcopy(self.policy),{'ETag':'"v1"'}
        raise AssertionError((method,path))


class AdmissionTests(unittest.TestCase):

    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.config={'origin':'https://request.test','dashboard_origin':'https://private.test','machine_id':'installation1',
            'enrollment_mode':'sharing','name':'Fixture machine','tailnet':'example.test','device_id':'server1','addresses':['100.64.0.1'],
            'ports':[443],'admins':[{'issuer':'https://identity.test','subject':'admin'}],
            'database':str(Path(self.tmp.name)/'requests.db'),'credential_file':'not-used'}
        self.api=FakeAPI();self.a=make_admission(self.config,self.api)
        self.user={'issuer':'https://request.test/requests','subject':'receipt1','login':'member@example.test'}
        self.admin={'issuer':'https://identity.test','subject':'admin','login':'owner@example.test'}

    def row(self):return self.a.rows(self.user)[0]












    def start_http(self):
        server=ThreadingHTTPServer(('127.0.0.1',0),PublicHandler);server.admission=self.a;server.sso=ReceiptAuth(self.a)
        t=threading.Thread(target=server.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(server.server_close);self.addCleanup(server.shutdown)
        return server

    def http(self,server,path,method='GET',value=None,headers=None):
        c=http.client.HTTPConnection('127.0.0.1',server.server_port)
        body=json.dumps(value).encode() if value is not None else None
        c.request(method,path,body,headers or {});r=c.getresponse();result=(r.status,r.read(),r.headers);c.close();return result

    def test_existing_native_account_request_reports_attention_without_invite(self):
        self.config['native_logins']=['owner@example.test']
        s=self.start_http();_,body,h=self.http(s,'/api/session');session=json.loads(body)
        headers={'Cookie':h['Set-Cookie'].split(';')[0],'Origin':'https://request.test','Content-Type':'application/json','X-Loom-CSRF':session['csrf']}
        code,body,_=self.http(s,'/api/request','POST',{'email':'OWNER@example.test'},headers)
        self.assertEqual(code,409);self.assertEqual(json.loads(body),{'code':'access_check_required'})
        with self.a.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM share_requests').fetchone()[0],0)
        self.assertEqual(self.api.calls,[])

    def test_public_receipt_only_own_requests_csrf_and_no_private_routes(self):
        s=self.start_http();code,body,h=self.http(s,'/api/session');self.assertEqual(code,200)
        session=json.loads(body);self.assertEqual(session['mode'],'invite')
        self.assertIn('HttpOnly',h['Set-Cookie']);self.assertIn('Secure',h['Set-Cookie'])
        headers={'Cookie':h['Set-Cookie'].split(';')[0],'Origin':'https://request.test','Content-Type':'application/json','X-Loom-CSRF':session['csrf']}
        for path in ['/mcp','/api/status','/api/jobs','/access-admin','/admission/requests','/api/devices','/../server.py']:
            self.assertEqual(self.http(s,path,headers=headers)[0],404)
        self.assertEqual(self.http(s,'/api/request','POST',{'email':'member@example.test'},{**headers,'Origin':'https://evil.test'})[0],403)
        self.assertEqual(self.http(s,'/api/request','POST',{'email':'member@example.test'},headers)[0],200)
        self.assertEqual(self.http(s,'/api/device','POST',{'id':'laptop'},headers)[0],404)
        self.assertEqual(len(json.loads(self.http(s,'/api/request',headers=headers)[1])['requests']),1)
        _,_,other=self.http(s,'/api/session')
        self.assertEqual(json.loads(self.http(s,'/api/request',headers={'Cookie':other['Set-Cookie'].split(';')[0]})[1])['requests'],[])
        self.assertEqual(self.api.calls,[])


    def test_only_sharing_configuration_is_accepted(self):
        from admission import validate
        for mode in ['membership', 'other']:
            with self.assertRaises(ValueError):validate({**self.config,'enrollment_mode':mode})
        with self.a.db() as c:
            tables={r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        self.assertIn('share_requests',tables)
        self.assertNotIn('requests',tables)

    def test_previous_records_are_preserved_but_not_dispatched(self):
        with self.a.db() as c:
            c.execute('CREATE TABLE requests(id TEXT,state TEXT)')
            c.execute("INSERT INTO requests VALUES('old','approved')")
        self.a=make_admission(self.config,self.api)
        self.a.tick()
        self.assertEqual(self.api.calls,[])
        with self.a.db() as c:self.assertEqual(tuple(c.execute('SELECT * FROM requests').fetchone()),('old','approved'))
