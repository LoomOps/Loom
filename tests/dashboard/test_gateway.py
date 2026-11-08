import json
import threading
import unittest
from pathlib import Path
import sys
sys.path[:0]=[str(Path(__file__).parent),str(Path(__file__).parents[2]/'dashboard')]
import test_admission as fixture
from server import make_server

class GatewayTests(unittest.TestCase):
    def setUp(self):
        self.f=fixture.AdmissionTests();self.f.setUp();self.addCleanup(self.f.doCleanups)

    def private(self):
        s=make_server('127.0.0.1',0,'http://127.0.0.1:1');s.admission=self.f.a
        t=threading.Thread(target=s.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(s.server_close);self.addCleanup(s.shutdown)
        return s

    def test_public_gateway_ui_is_not_an_api_relay_or_authentication(self):
        s=self.f.start_http();s.gateway=True
        code,body,headers=self.f.http(s,'/')
        self.assertEqual(code,200);self.assertIn(b'id="recognized"',body)
        self.assertNotIn(b'id="device-form"',body)
        self.assertNotIn(b'Continue with Tailscale',body)
        self.assertIn(b'Use another email',body)
        self.assertIn(b'Request access via Tailscale',body)
        self.assertNotIn(b'id="request-link"',body)
        self.assertIn(b'id="retry-connection"',body)
        self.assertIn(b'class="hostname" data-destination',body)
        self.assertEqual(body.count(b'data-destination'),1)
        self.assertIn('frame-src https://private.test',headers['Content-Security-Policy'])
        self.assertNotIn(b'example-machine',body)
        self.assertNotIn(b'Loom machine',body)
        self.assertIn(b'class="card-shadow" hidden',body)
        for path in ['/mcp','/session','/gateway/probe?nonce=1234567890123456','/api/jobs','/admission/requests','/auth/login','/api/devices']:
            self.assertEqual(self.f.http(s,path)[0],404,path)
        code,body,_=self.f.http(s,'/api/session',headers={'Tailscale-User-Login':'forged@example.test'})
        self.assertIsNone(json.loads(body)['user']['login'])
        self.assertEqual(self.f.api.calls,[])

    def test_brand_assets_are_public_static_files_only(self):
        s=self.f.start_http();s.gateway=True
        for path,marker in [('/brand.js',b'requestAnimationFrame'),('/brand-scene.js',b'Path2D'),('/style.css',b'blur(30px)')]:
            code,body,headers=self.f.http(s,path)
            self.assertEqual(code,200,path);self.assertIn(marker,body)
            self.assertIn("script-src 'self'",headers['Content-Security-Policy'])
        self.assertEqual(self.f.http(s,'/branding/../gateway.py')[0],404)
        self.assertEqual(self.f.api.calls,[])

    def test_private_probe_ignores_spoofed_network_identity_and_receipts(self):
        s=self.private();s.tailscale_proxies=['192.0.2.1']
        path='/gateway/probe?nonce=abcdefghijklmnop'
        _,body,headers=self.f.http(s,path,headers={'Tailscale-User-Login':'forged@example.test','Cookie':'__Host-loom-access-receipt=invalid'})
        self.assertIn(b'"authorized": false',body);self.assertNotIn(b'forged@example',body)
        self.assertIn(b'"reason": "identity_unavailable"',body)
        self.assertIn('frame-ancestors https://request.test',headers['Content-Security-Policy'])
        self.assertIn("script-src 'sha256-",headers['Content-Security-Policy'])
        self.assertNotIn('X-Frame-Options',headers)
        for query in ['nonce=x','nonce=abcdefghijklmnop&nonce=abcdefghijklmnop','nonce=abcdefghijklmnop&return=https://evil.test']:
            self.assertEqual(self.f.http(s,'/gateway/probe?'+query)[0],400)

    def test_private_probe_uses_verified_session_or_trusted_serve(self):
        s=self.private();s.tailscale_proxies=['127.0.0.1']
        path='/gateway/probe?nonce=abcdefghijklmnop'
        _,body,_=self.f.http(s,path,headers={'Tailscale-User-Login':'member@example.test'})
        self.assertIn(b'"authorized": true',body);self.assertIn(b'member@example.test',body)
        class Auth:
            def session(self,headers):return {'user':{'name':'</script>','login':'verified@example.test'}} if headers.get('Cookie')=='verified' else None
        s.sso=Auth()
        _,body,_=self.f.http(s,path,headers={'Cookie':'verified'})
        self.assertIn(b'verified@example.test',body);self.assertIn(b'\\u003c/script>',body)
        self.assertIn(b'"destination": "/"',body)
        _,body,_=self.f.http(s,path,headers={'Tailscale-User-Login':'member@example.test'})
        self.assertIn(b'"destination": "/auth/login"',body)

    def test_native_admin_probe_does_not_need_cookie_and_unbound_user_is_distinct(self):
        s=self.private();s.tailscale_proxies=['127.0.0.1']
        class Auth:
            config={'provider':'tailscale'}
            def session(self,headers):return None
            def authorize(self,network,admission):
                return {'login':'admin@example.test'} if network.get('Tailscale-User-Login')=='admin@example.test' else None
        s.sso=Auth()
        path='/gateway/probe?nonce=abcdefghijklmnop'
        _,body,_=self.f.http(s,path,headers={'Tailscale-User-Login':'admin@example.test'})
        self.assertIn(b'"authorized": true',body);self.assertIn(b'"destination": "/auth/login"',body)
        _,body,_=self.f.http(s,path,headers={'Tailscale-User-Login':'unbound@example.test'})
        self.assertIn(b'"reason": "access_required"',body)
        s.tailscale_proxies=['192.0.2.1']
        _,body,_=self.f.http(s,path,headers={'Tailscale-User-Login':'admin@example.test'})
        self.assertIn(b'"reason": "identity_unavailable"',body)
        self.assertNotIn(b'admin@example.test',body)

    def test_private_entry_redirects_without_rendering_legacy_login(self):
        s=self.private();s.admission.config['separate_gateway']=True
        s.tailscale_proxies=['127.0.0.1']
        class Auth:
            config={'provider':'tailscale'}
            def session(self,headers):
                return {'user':{'login':'owner@example.test'},'csrf':'fixture'} if headers.get('Cookie')=='old-owner' else None
        s.sso=Auth()
        for headers in [{},{'Cookie':'old-owner','Tailscale-User-Login':'other@example.test'}]:
            code,body,h=self.f.http(s,'/',headers=headers)
            self.assertEqual(code,303);self.assertEqual(h['Location'],'https://request.test/')
            self.assertNotIn(b'Access token',body)
        headers={'Cookie':'old-owner','Tailscale-User-Login':'owner@example.test'}
        self.assertEqual(self.f.http(s,'/',headers=headers)[0],200)
        self.assertEqual(self.f.http(s,'/node',headers=headers)[0],404)
