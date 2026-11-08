"""Real local sessions and HTTP ingress; no external identity provider."""
import json,tempfile,time,unittest,sys,http.client
from pathlib import Path
from unittest.mock import patch
from threading import Thread
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'dashboard'))
import sso

class SSOTest(unittest.TestCase):
 def setUp(self):
  t=tempfile.TemporaryDirectory();self.addCleanup(t.cleanup);self.root=Path(t.name)
  key=self.root/'key';key.write_text('k'*48)
  self.config={'issuer':'https://identity.test','origin':'https://loom.test','relay_key_file':str(key),'session_db':str(self.root/'sessions.sqlite3'),'session_seconds':604800,'network_users':{'member@example.test':{'subject':'userid:123'}}}
  self.auth=sso.SSO(self.config);self.claim_changes={}
 def finish(self):
  login=self.claim_changes.get('email','member@example.test');subject=self.claim_changes.get('sub','userid:123')
  self.auth.users[login]={'subject':subject}
  identity=self.auth.authorize({'Tailscale-User-Login':login,'Tailscale-User-Name':'Member'})
  cookie=self.auth.issue(identity,{})
  return {'Cookie':cookie.split(';')[0]},identity
 def test_native_allowlist_and_no_provider_requests(self):
  self.assertIsNone(self.auth.authorize({}))
  self.assertIsNone(self.auth.authorize({'Tailscale-User-Login':'stranger@test'}))
  with patch('urllib.request.urlopen',side_effect=AssertionError('No external login request')):
   headers,user=self.finish();self.assertEqual(self.auth.session(headers)['user'],user)
  self.assertEqual(user['subject'],'userid:123')
 def test_restart_preserves_cookie_and_fixed_expiry(self):
  headers,_=self.finish();old=self.auth.session(headers);restart=sso.SSO(self.config)
  self.assertEqual(restart.session(headers),old)
  with patch.object(sso.time,'time',return_value=old['expires']-1):self.assertEqual(restart.session(headers),old)
  with patch.object(sso.time,'time',return_value=old['expires']):self.assertIsNone(restart.session(headers))
 def test_logout_rotation_and_untrusted_identity(self):
  h,u=self.finish();new=self.auth.issue(u,h);self.assertIsNone(self.auth.session(h))
  headers={'Cookie':new.split(';')[0]};self.auth.logout(headers);self.assertIsNone(self.auth.session(headers))
  with self.assertRaises(ValueError):self.auth.issue({**u,'authentication':'unverified'}, {})
 def test_config_and_namespace_preservation(self):
  self.assertEqual(sso.SSO({**self.config,'issuer':'https://identity.test/'}).issuer,'https://identity.test/')
  for key,val in [('session_seconds',0),('session_seconds',True),('session_seconds',604801),('origin','http://loom.test'),('network_users',{'x':{}})]:
   with self.subTest(key=key),self.assertRaises(ValueError):sso.SSO({**self.config,key:val})
 def test_http_native_login_network_switch_and_retired_routes(self):
  from test_proxy import FakeOps
  from server import make_server
  from http.server import ThreadingHTTPServer
  api=ThreadingHTTPServer(('127.0.0.1',0),FakeOps);api.calls=[];api.history_calls=[]
  web=make_server('127.0.0.1',0,'http://127.0.0.1:'+str(api.server_port));web.sso=self.auth;web.development=True;web.tailscale_proxies=['127.0.0.1']
  for server in (api,web):
   th=Thread(target=server.serve_forever,kwargs={'poll_interval':.01},daemon=True);th.start()
   self.addCleanup(th.join,2);self.addCleanup(server.server_close);self.addCleanup(server.shutdown)
  def call(path,headers=None,method='GET'):
   c=http.client.HTTPConnection('127.0.0.1',web.server_port);c.request(method,path,headers=headers or {});r=c.getresponse();v=(r.status,r.read(),r.headers);c.close();return v
  net={'Tailscale-User-Login':'member@example.test'}
  self.assertEqual(call('/auth/login')[0],401)
  self.assertEqual(call('/auth/login',{'Tailscale-User-Login':'stranger@test'})[0],403)
  self.assertEqual(call('/api/status',net)[0],401)
  result=call('/auth/login',net);self.assertEqual(result[0],303);self.assertEqual(result[2]['Location'],'/')
  h={**net,'Cookie':result[2]['Set-Cookie'].split(';')[0]}
  self.assertEqual(call('/api/status',h)[0],200)
  self.assertEqual(call('/api/status',{**h,'Tailscale-User-Login':'other@test'})[0],401)
  self.assertEqual(call('/api/status',{'Cookie':h['Cookie']})[0],401)
  web.tailscale_proxies=[];self.assertEqual(call('/api/status',h)[0],401)
  web.tailscale_proxies=['127.0.0.1'];self.auth.users.clear();self.assertEqual(call('/api/status',h)[0],401)
  for path in ['/auth-demo/','/auth/callback?code=retired']:self.assertEqual(call(path)[0],404)
 def test_sharing_approval_revocation_is_current(self):
  import test_sharing
  f=test_sharing.SharingTests();f.setUp();self.addCleanup(f.doCleanups);f.ready()
  identity=self.auth.authorize({'Tailscale-User-Login':'actual@example.test'},f.a)
  self.assertEqual(identity['authentication'],'tailscale-sharing')
  self.assertEqual(identity['issuer'],self.auth.issuer)
  self.assertIsNone(self.auth.authorize({'Tailscale-User-Login':'unknown@test'},f.a))
 def test_agent_token_relay_requires_live_admin_session_and_csrf(self):
  import base64,hmac,hashlib
  from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
  from types import SimpleNamespace
  from server import make_server
  seen=[]
  class Capture(BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def do_GET(self):
    seen.append(dict(self.headers));self.send_response(200);self.send_header('Content-Type','application/json');self.end_headers();self.wfile.write(b'{"tokens":[],"owners":[]}')
   def do_POST(self):
    self.rfile.read(int(self.headers.get('Content-Length',0)));self.do_GET()
  api=ThreadingHTTPServer(('127.0.0.1',0),Capture)
  web=make_server('127.0.0.1',0,'http://127.0.0.1:'+str(api.server_port));web.sso=self.auth;web.development=True;web.tailscale_proxies=['127.0.0.1']
  web.admission=SimpleNamespace(is_admin=lambda user:False)
  for server in (api,web):
   th=Thread(target=server.serve_forever,kwargs={'poll_interval':.01},daemon=True);th.start()
   self.addCleanup(th.join,2);self.addCleanup(server.server_close);self.addCleanup(server.shutdown)
  cookie,user=self.finish();session=self.auth.session(cookie)
  h={**cookie,'Host':'loom.test','Tailscale-User-Login':'member@example.test'}
  def call(method='GET',headers=h,path='/api/admin/tokens'):
   c=http.client.HTTPConnection('127.0.0.1',web.server_port);c.request(method,path,body='{}' if method=='POST' else None,headers={'Content-Type':'application/json',**headers});r=c.getresponse();v=(r.status,r.read(),r.headers);c.close();return v
  self.assertEqual(call()[0],403);self.assertEqual(seen,[])
  web.admission.is_admin=lambda user:True
  self.assertEqual(call(headers={'Authorization':'Bearer fixture-owner'})[0],401)
  self.assertEqual(call('POST')[0],403)
  write={**h,'Origin':'https://loom.test','X-Loom-CSRF':session['csrf']}
  self.assertEqual(call('POST',{**write,'Origin':'https://evil.test'})[0],403)
  result=call('POST',write);self.assertEqual(result[0],200);self.assertIn('no-store',result[2]['Cache-Control'])
  encoded,signature=seen[-1]['X-Loom-Dashboard-Identity'].split('.')
  self.assertEqual(signature,hmac.new(self.auth.key,encoded.encode(),hashlib.sha256).hexdigest())
  identity=json.loads(base64.urlsafe_b64decode(encoded+'='*(-len(encoded)%4)))['identity']
  self.assertTrue(identity['access_admin']);self.assertNotIn('Authorization',seen[-1])
  self.assertEqual(call(path='/api/status')[0],200)
  encoded=seen[-1]['X-Loom-Dashboard-Identity'].split('.')[0]
  self.assertFalse(json.loads(base64.urlsafe_b64decode(encoded+'='*(-len(encoded)%4)))['identity']['access_admin'])
