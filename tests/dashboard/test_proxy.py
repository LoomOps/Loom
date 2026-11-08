"""Regression tests for the dashboard's HTTP boundary."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import sys
from pathlib import Path
from threading import Thread
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

sys.path.insert(0, str(Path(__file__).resolve().parents[2]/'dashboard'))
from server import make_server


class FakeOps(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.server.history_calls.append((self.path, self.headers.get("Authorization"), payload))
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b"[]")

    def do_GET(self):
        self.server.identity_headers={key:self.headers.get(key) for key in ('Tailscale-User-Login','Tailscale-User-Name')}
        self.server.calls.append((self.path, self.headers.get("Authorization"), self.headers.get("Origin")))
        if self.path=='/api/guide/bundle':
            self.send_response(200);self.send_header('Content-Type','application/zip');self.send_header('X-Loom-Revision','a'*64);self.end_headers();self.wfile.write(b'PK-fixture');return
        if self.path.startswith("/api/dashboard/media/"):
            self.send_response(200)
            text=self.path.endswith(('/markdown','/json','/empty','/large-text'))
            self.send_header("Content-Type", "text/markdown" if text else "image/svg+xml" if self.path.endswith("/svg") else "image/png")
            self.send_header("Content-Length", str(0 if self.path.endswith('/empty') else 256*1024+1 if self.path.endswith('/large-text') else 65 * 1024 * 1024 if self.path.endswith("/large") else len(b"png-bytes")))
            self.end_headers()
            self.wfile.write(b"png-bytes")
            return
        status = 302 if self.path.endswith("/redirect") else 403 if self.path.endswith("/denied") else 200
        self.send_response(status)
        self.send_header("Content-Type", "text/html" if self.path.endswith("/html") else "application/json")
        self.send_header("Set-Cookie", "ops-private=test")
        if status == 302:
            self.send_header("Location", "https://unexpected.example/")
        self.end_headers()
        self.wfile.write(json.dumps({"test": "response"}).encode())


class ProxyTest(unittest.TestCase):
    def test_tailnet_headers_only_forwarded_from_trusted_ingress(self):
        headers={'Tailscale-User-Login':'member@example.test'}
        self.assertEqual(self.request('/api/principal',headers=headers)[0],401)
        self.assertTrue(json.loads(self.request('/session',headers=headers)[2])['authentication_required'])
        self.web.tailscale_proxies=['127.0.0.1']
        self.assertEqual(self.request('/api/principal',headers=headers)[0],200)
        self.assertEqual(self.api.identity_headers['Tailscale-User-Login'],'member@example.test')
        self.assertFalse(json.loads(self.request('/session',headers=headers)[2])['authentication_required'])
        self.assertEqual(self.request('/api/principal',headers={**headers,'Origin':'https://attacker.test'})[0],403)
        self.assertEqual(self.request('/api/principal',headers={**headers,'Authorization':'Bearer explicit'})[0],200)
        self.assertIsNone(self.api.identity_headers['Tailscale-User-Login'])

    def test_job_contract_and_filtered_listing_are_read_only_relays(self):
        for path in ['/api/jobs/contract','/api/jobs/profiles','/api/jobs/work%3Aold','/api/jobs?category=sdg&execution_mode=pilot&offset=0&limit=10']:
            self.assertEqual(self.request(path)[0],401)
            self.assertEqual(self.request(path,headers={'Authorization':'Bearer test-credential'})[0],200)
            self.assertEqual(self.api.calls[-1][0],path)
            self.assertEqual(self.request(path,headers={'Authorization':'Bearer test-credential'},method='POST')[0],405)

    def test_release_status_and_refresh_have_fixed_authenticated_routes(self):
        auth={'Authorization':'Bearer test-credential'}
        self.assertEqual(self.request('/api/updates')[0],401)
        self.assertEqual(self.request('/api/updates',headers=auth)[0],200)
        self.assertEqual(self.request('/api/updates/check',method='POST',headers=auth,body={})[0],200)
        self.assertEqual(self.api.history_calls[-1],('/api/updates/check',auth['Authorization'],{}))
        self.assertEqual(self.request('/api/updates/check',method='POST',headers=auth,body={'repository':'other/repo'})[0],400)
        self.assertEqual(self.request('/api/updates/check',method='POST',headers={**auth,'Origin':'https://elsewhere.test'},body={})[0],403)

    def test_retired_entry_markup_is_not_embedded(self):
        body=self.request('/')[2].decode()
        self.assertNotIn('Loom 대시보드에 연결합니다.',body)
        self.assertNotIn('id="loom-access"',body)
        self.assertIn('id="loom-entry" hidden',body)
        self.assertIn('loom-entry-brand',body)

    def test_imported_loom_job_history_is_relayed(self):
        path='/api/history/get?source=loom-ops&kind=job_view&id=abc&revision=jobs-abc'
        self.assertEqual(self.request(path,headers={'Authorization':'Bearer test-credential'})[0],200)
        self.assertEqual(self.api.history_calls[-1][2],
            {'source':'loom-ops','kind':'job_view','id':'abc','revision':'jobs-abc'})

    def test_compressed_workspace_and_opt_out(self):
        import gzip
        _, _, original = self.request('/')
        status, headers, body = self.request('/', headers={'Accept-Encoding':'gzip'})
        self.assertEqual(status, 200)
        self.assertEqual(headers['Content-Encoding'], 'gzip')
        self.assertEqual(gzip.decompress(body), original)
        self.assertLess(len(body), len(original) / 2)
        _, headers, body = self.request('/', headers={'Accept-Encoding':'gzip;q=0, identity'})
        self.assertIsNone(headers.get('Content-Encoding'))
        self.assertEqual(body, original)

    def test_mcp_observation_cannot_call_tools(self):
        auth = {'Authorization':'Bearer test-credential'}
        for path, method in (('/api/mcp/initialize','initialize'),('/api/mcp/tools','tools/list')):
            self.assertEqual(self.request(path)[0],401)
            self.assertEqual(self.request(path,headers=auth)[0],200)
            self.assertEqual(self.api.history_calls[-1],('/mcp',auth['Authorization'],
                {'jsonrpc':'2.0','id':1,'method':method,'params':{}}))
            self.assertEqual(self.request(path,headers=auth,method='POST')[0],405)
        self.assertEqual(self.request('/api/mcp/call',headers=auth)[0],404)

    def test_mcp_reference_is_a_fixed_read_only_relay(self):
        self.assertEqual(self.request('/api/mcp/docs')[0],401)
        self.assertEqual(self.request('/api/mcp/docs',headers={'Authorization':'Bearer fixture'})[0],200)
        body=self.api.history_calls[-1][2]
        self.assertEqual(body['params'],{'name':'system','arguments':{'action':'describe','arguments':{}}})
        self.assertEqual(self.request('/api/mcp/docs?action=submit',headers={'Authorization':'Bearer fixture'})[0],404)
        self.assertEqual(self.request('/api/mcp/docs',headers={'Authorization':'Bearer fixture'},method='POST')[0],405)

    def start(self, server):
        worker = Thread(target=server.serve_forever, daemon=True)
        worker.start()
        self.addCleanup(worker.join, 3)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return f"http://127.0.0.1:{server.server_port}"

    def setUp(self):
        self.api = ThreadingHTTPServer(("127.0.0.1", 0), FakeOps)
        self.api.calls = []
        self.api.history_calls = []
        self.web = make_server("127.0.0.1", 0, self.start(self.api))
        self.base = self.start(self.web)

    def test_development_mode_requires_explicit_flag_and_preserves_origin_boundary(self):
        self.assertTrue(json.loads(self.request('/session')[2])['authentication_required'])
        self.assertEqual(self.request('/api/status')[0], 401)
        self.web.development = True
        self.assertFalse(json.loads(self.request('/session')[2])['authentication_required'])
        self.assertEqual(self.request('/api/status')[0], 200)
        self.assertEqual(self.api.calls[-1], ('/api/status', None, None))
        self.assertEqual(self.request('/api/status', headers={'Origin':'https://other.example'})[0], 403)
        self.assertEqual(self.request('/api/jobs', method='POST')[0], 405)

    def request(self, path, *, method="GET", headers=None, body=None):
        try:
            with urlopen(Request(self.base + path, method=method, headers=({'Content-Type':'application/json',**(headers or {})} if body is not None else headers or {}), data=json.dumps(body).encode() if body is not None else None), timeout=3) as response:
                return response.status, response.headers, response.read()
        except HTTPError as response:
            return response.code, response.headers, response.read()

    def test_relay_reads_and_preserves_denial_without_cookies(self):
        auth = {"Authorization": "Bearer test-credential"}
        for path in ("/api/status", "/api/docker", "/api/principal", "/api/jobs", "/api/jobs/exact",
                     "/api/registry", "/api/registry/record.id", "/api/events",
                     "/api/dashboard/catalog", "/api/dashboard/resources"):
            status, headers, body = self.request(path, headers=auth)
            self.assertEqual(status, 200)
            self.assertEqual(self.api.calls[-1], (path, auth["Authorization"], None))
            self.assertIsNone(headers.get("Set-Cookie"))
            self.assertEqual(headers["Cache-Control"], "no-store")
            self.assertEqual(json.loads(body), {"test": "response"})
        self.assertEqual(self.request("/api/jobs/denied", headers=auth)[0], 403)

    def test_auth_method_and_path_boundaries(self):
        auth = {"Authorization": "Bearer test-credential"}
        self.assertEqual(self.request("/api/status")[0], 401)
        self.assertEqual(self.request("/api/status", headers={**auth, "Origin": "https://other.example"})[0], 403)
        self.assertEqual(self.request("/api/status", headers={**auth, "Sec-Fetch-Site": "cross-site"})[0], 403)
        self.assertEqual(self.request("/api/jobs", method="POST", headers=auth)[0], 405)
        for path in ("/api/admin", "/api/docker/containers", "/mcp", "/api/jobs/../status", "/api/jobs/%2e%2e", "/api/status?token=test"):
            self.assertEqual(self.request(path, headers=auth)[0], 404, path)
        self.assertEqual(self.api.calls, [])

    def test_history_get_only_relays_explicit_reads(self):
        auth = {"Authorization": "Bearer test-credential"}
        page = "/api/history/list?kind=&offset=50"
        record = "/api/history/get?source=queue&kind=work&id=example%3Awork&revision=snapshot"
        self.assertEqual(self.request(page)[0], 401)
        self.assertEqual(self.request(page, headers={**auth, "Origin": "https://other.example"})[0], 403)
        for path in (page + "&offset=0", page + "&limit=1000", record.replace("queue", "unknown"),
                     record.replace("snapshot", "%2F"), "/api/history/list?kind=&offset=-1"):
            self.assertEqual(self.request(path, headers=auth)[0], 400)
        self.assertEqual(self.request(record, method="POST", headers=auth)[0], 405)
        self.assertEqual(self.request(record, method="DELETE", headers=auth)[0], 405)
        self.assertEqual(self.request("/api/jobs/reindex", method="POST", headers=auth)[0], 405)
        self.assertEqual(self.api.history_calls, [])
        self.assertEqual(self.request(page, headers=auth)[0], 200)
        self.assertEqual(self.request(record, headers=auth)[0], 200)
        self.assertEqual(self.api.history_calls, [
            ("/api/history/list", auth["Authorization"], {"kind": "", "offset": 50, "limit": 50}),
            ("/api/history/get", auth["Authorization"],
             {"source": "queue", "kind": "work", "id": "example:work", "revision": "snapshot"})
        ])

    def test_media_relay_is_scoped_and_bounded(self):
        auth = {"Authorization": "Bearer test-credential"}
        path = "/api/dashboard/media/test-ref"
        self.assertEqual(self.request(path)[0], 401)
        self.assertEqual(self.request(path + "?credential=bad", headers=auth)[0], 404)
        self.assertEqual(self.request(path + "/more", headers=auth)[0], 404)
        self.assertEqual(self.request(path, headers={**auth, "Sec-Fetch-Site": "cross-site"})[0], 403)
        self.assertEqual(self.request(path, method="POST", headers=auth)[0], 405)
        self.assertEqual(self.api.calls, [])
        status, headers, body = self.request(path, headers=auth)
        self.assertEqual((status, headers["Content-Type"], body), (200, "image/png", b"png-bytes"))
        self.assertEqual(headers["X-Content-Type-Options"], "nosniff")
        self.assertEqual(headers["Cache-Control"], "no-store")
        self.assertEqual(self.request("/api/dashboard/media/svg", headers=auth)[0], 502)
        self.assertEqual(self.request("/api/dashboard/media/large", headers=auth)[0], 413)

    def test_static_health_redirect_and_non_json(self):
        self.assertEqual(self.request("/healthz")[0], 200)
        status, headers, body = self.request("/")
        self.assertEqual(status, 200)
        self.assertIn(b'sandbox="allow-scripts"', body)
        self.assertIn(b"loom-access-token", body)
        self.assertNotIn(b"example.ts.net", body)
        self.assertNotIn(b"allow-same-origin", body)
        import http.client
        conn=http.client.HTTPConnection('127.0.0.1',self.web.server_port)
        try:
            conn.request('GET','/node');response=conn.getresponse()
            self.assertEqual(response.status,404);self.assertIsNone(response.getheader('Location'))
            response.read()
        finally:conn.close()
        self.assertIn("connect-src 'self'", headers["Content-Security-Policy"])
        self.assertEqual(self.request("/app.js")[0], 404)
        auth = {"Authorization": "Bearer test-credential"}
        self.assertEqual(self.request("/api/jobs/redirect", headers=auth)[0], 502)
        self.assertEqual(self.request("/api/jobs/html", headers=auth)[0], 502)

    def test_text_requires_auth_and_obeys_its_own_limit(self):
        auth={'Authorization':'Bearer test-credential'}
        self.assertEqual(self.request('/api/dashboard/media/markdown')[0],401)
        for name in ['markdown','json','empty']:
            self.assertEqual(self.request('/api/dashboard/media/'+name,headers=auth)[0],200)
        self.assertEqual(self.request('/api/dashboard/media/large-text',headers=auth)[0],413)

    def test_guides_and_bundle_are_authenticated_read_only(self):
        auth={'Authorization':'Bearer test-credential'}
        for path in ['/api/guide','/api/guide/jobs','/api/guide/bundle']:
            self.assertEqual(self.request(path)[0],401)
            self.assertEqual(self.request(path,headers=auth)[0],200)
            self.assertEqual(self.request(path,headers=auth,method='POST')[0],405)
        status,headers,body=self.request('/api/guide/bundle',headers=auth)
        self.assertEqual(headers['Content-Type'],'application/zip');self.assertEqual(body,b'PK-fixture')
        self.assertEqual(headers['X-Loom-Revision'],'a'*64)
        self.assertEqual(self.request('/api/guide/../secrets',headers=auth)[0],404)

    def test_queue_mutations_are_explicit_authenticated_and_origin_checked(self):
        path='/api/jobs/queue-actions';body={'action':'reorder','arguments':{'id':'job-a','position':'next'}}
        auth={'Authorization':'Bearer test-credential','Content-Type':'application/json'}
        self.assertEqual(self.request(path,method='POST',headers={'Content-Type':'application/json'},body=body)[0],401)
        for extra in [{'Origin':'https://other.invalid'},{'Sec-Fetch-Site':'cross-site'}]:
            self.assertEqual(self.request(path,method='POST',headers={**auth,**extra},body=body)[0],403)
        for bad in [{'action':'submit','arguments':{}},{'action':'cancel','arguments':{'id':'job-a'}},{**body,'group':'system'},{'action':'hold','arguments':{'id':'job-a','extra':True}}]:
            self.assertEqual(self.request(path,method='POST',headers=auth,body=bad)[0],400)
        self.assertEqual(self.api.history_calls,[])
        self.assertEqual(self.request(path,method='DELETE',headers=auth,body=body)[0],405)
        self.assertEqual(self.request(path,method='POST',headers=auth,body=body)[0],200)
        self.assertEqual(self.api.history_calls[-1],('/api/actions',auth['Authorization'],{'group':'jobs',**body}))
