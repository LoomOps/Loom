"""Loom Dashboard. Serve static assets and relay explicit read APIs and scoped queue controls."""
import http.client
import ipaddress
import gzip
import build_version
import json
import mimetypes
import os
from pathlib import Path
import re
import socket
import hmac
import sso
import admission_http
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit, unquote

PUBLIC = Path(__file__).with_name("public")
FILES = {"/": "workspace.html", "/icon.svg": "icon.svg"}
IDENTIFIER = r"[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,159}"
READ_ROUTES = (
    "/api/guide", r"/api/guide/[a-z][a-z-]{0,40}",
    "/api/status", "/api/updates", "/api/docker", "/api/principal", "/api/jobs", "/api/events", "/api/registry",
    "/api/dashboard/catalog", "/api/dashboard/resources", "/api/dashboard/revision",
    rf"/api/registry/{IDENTIFIER}",
    rf"/api/jobs/{IDENTIFIER}",
)
MEDIA_ROUTE = rf"/api/dashboard/media/{IDENTIFIER}"
MEDIA_TYPES = {"image/png", "image/jpeg", "image/webp", "video/mp4", "video/webm"}
TEXT_TYPES = {"text/markdown", "text/plain", "application/json", "application/x-ndjson"}
MEDIA_TYPES.update(TEXT_TYPES)
MAX_TEXT = 256 * 1024
MAX_MEDIA = 64 * 1024 * 1024
HISTORY_ROUTES = {"/api/history/list", "/api/history/get"}
MCP_READ_ROUTES = {"/api/mcp/initialize": "initialize", "/api/mcp/tools": "tools/list"}
HISTORY_KEY = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,191}")
HISTORY_SOURCES = {"queue", "catalog", "run-history", "publication", "loom-ops"}


def history_args(path, query):
    if len(query) > 1024:
        raise ValueError("History query too long")
    params = parse_qs(query, keep_blank_values=True, strict_parsing=True, max_num_fields=4)
    if any(len(values) != 1 for values in params.values()):
        raise ValueError("Repeated history argument")
    args = {key: values[0] for key, values in params.items()}
    if path == "/api/history/list":
        if (args.keys() != {"kind", "offset"} or
                (args["kind"] and not HISTORY_KEY.fullmatch(args["kind"])) or
                not re.fullmatch(r"[0-9]{1,9}", args["offset"])):
            raise ValueError("Invalid history filter")
        return {"kind": args["kind"], "offset": int(args["offset"]), "limit": 50}
    if (args.keys() != {"source", "kind", "id", "revision"} or args["source"] not in HISTORY_SOURCES or
            any(not HISTORY_KEY.fullmatch(value) for value in args.values())):
        raise ValueError("Invalid history identity")
    return args


MAX_JSON = 8 * 1024 * 1024
MAX_CATALOG = 16 * 1024 * 1024
CSP = ("default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; "
       "connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
# The original srcdoc inherits the parent policy. Its own CSP further denies connections.
WORKSPACE_CSP = (
    re.search(r'<meta http-equiv="Content-Security-Policy" content="([^"]+)">', (PUBLIC / "workspace.html").read_text())[1]
    + "; frame-ancestors 'none'"
)


class WebHandler(BaseHTTPRequestHandler):
    def setup(self):
        self.request.settimeout(12)
        super().setup()

    def log_message(self, *_):
        pass

    def send_headers(self, status, size, content_type, csp=CSP, encoding=None):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(size))
        self.send_header("Cache-Control", "no-store")
        if re.fullmatch(r"[0-9a-f]{64}",getattr(self,"loom_revision","") or ""):
            self.send_header("X-Loom-Revision",self.loom_revision)
        self.send_header("Vary", "Accept-Encoding")
        if encoding:
            self.send_header("Content-Encoding", encoding)
        self.send_header("Content-Security-Policy", csp)
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("X-Frame-Options", "DENY")
        self.end_headers()

    def reply(self, status, body, content_type="application/json; charset=utf-8", csp=CSP):
        encoding = None
        accepted = dict((parts[0].strip(), parts[1:]) for parts in
                        (item.split(";") for item in self.headers.get("Accept-Encoding", "").split(",")))
        if len(body) > 1024 and "gzip" in accepted and not any(
                re.fullmatch(r"\s*q=0(?:\.0*)?\s*", value) for value in accepted["gzip"]):
            body = gzip.compress(body, compresslevel=3, mtime=0)
            encoding = "gzip"
        self.send_headers(status, len(body), content_type, csp, encoding)
        if self.command != "HEAD":
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass

    def error(self, status, code):
        self.reply(status, json.dumps({"code": code}).encode())

    def do_HEAD(self):
        if self.path not in FILES and self.path != "/healthz":
            return self.error(404, "route_unavailable")
        self.do_GET()

    def do_GET(self):
        if self.path == '/api/admin/tokens':
            return self.relay('GET',self.path)
        if self.path=='/' and self.entry_origin() and not self.dashboard_session():
            # Do not briefly render the retired token/login screen on the way in.
            return self.auth_redirect(self.entry_origin()+'/')
        if self.path.partition('?')[0]=='/gateway/probe':
            from gateway import probe
            return probe(self,self.path.partition('?')[2])
        if self.path.startswith('/auth-demo/'):
            return self.error(404,'route_unavailable')
        if self.path.startswith('/admission/'):
            return admission_http.admin_route(self)
        auth_path, _, auth_query = self.path.partition('?')
        if auth_path.startswith('/auth/'):
            return self.auth_route(auth_path, auth_query)
        if self.path == '/session':
            live = os.environ.get('LOOM_DASHBOARD_DEV') == '1'
            revision = str(max((PUBLIC / name).stat().st_mtime_ns for name in set(FILES.values()))) if live else None
            session = self.dashboard_session()
            identity = self.tailnet_headers() if not self.server.sso else {}
            authenticated = False
            if identity:
                upstream = self.server.upstream
                connect = http.client.HTTPSConnection if upstream.scheme == 'https' else http.client.HTTPConnection
                connection = connect(upstream.hostname, upstream.port, timeout=3)
                try:
                    connection.request('GET','/api/principal',headers=identity)
                    response=connection.getresponse()
                    authenticated=response.status==200
                    response.read(MAX_JSON+1)
                except (OSError,http.client.HTTPException):
                    pass
                finally:
                    connection.close()
            return self.reply(200, json.dumps({'authentication_required': not (session if self.server.sso else self.server.development or authenticated),
                                              'sso':bool(self.server.sso), 'login_url':'/auth/login' if self.server.sso else None,
                                              'entry_url':self.entry_origin(),
                                              'user':{**session['user'],'access_admin':bool(getattr(self.server,'admission',None) and self.server.admission.is_admin(session['user']))} if session else None,
                                              'csrf':session['csrf'] if session else None,
                                              'tailnet':{'machine':self.server.admission.config.get('name',self.server.admission.config['machine_id'])} if session and getattr(self.server,'admission',None) else None,
                                              'live_reload': live, 'revision': revision,
                                              'build':build_version.status()}).encode())
        if self.path == "/healthz":
            return self.reply(200, b'{"status":"running"}')
        if self.path in FILES:
            asset = PUBLIC / FILES[self.path]
            mime = mimetypes.guess_type(asset.name)[0] or "application/octet-stream"
            return self.reply(200, asset.read_bytes(), mime, WORKSPACE_CSP if self.path == "/" else CSP)
        path, _, query = self.path.partition("?")
        # Expose the Job event feed separately from catalog transport.
        if self.path == '/api/ops/events':
            return self.relay('GET', '/api/events')
        if self.path == '/api/mcp/docs':
            body=json.dumps({'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'system','arguments':{'action':'describe','arguments':{}}}}).encode()
            return self.relay('POST','/mcp',body)
        if self.path in MCP_READ_ROUTES:
            body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": MCP_READ_ROUTES[self.path],
                               "params": {}}).encode()
            return self.relay("POST", "/mcp", body)
        if path in HISTORY_ROUTES:
            try:
                body = json.dumps(history_args(path, query)).encode()
            except ValueError:
                return self.error(400, "invalid_history_query")
            return self.relay("POST", path, body)
        if path == "/api/jobs" and query:
            return self.relay("GET", self.path)
        if re.fullmatch(MEDIA_ROUTE, self.path):
            return self.relay("GET", self.path, media=True)
        if not any(re.fullmatch(route, unquote(self.path)) for route in READ_ROUTES):
            return self.error(404, "route_unavailable")
        return self.relay("GET", self.path)

    def auth_redirect(self, target, cookies=()):
        self.send_response(303)
        self.send_header('Location',target)
        self.send_header('Cache-Control','no-store')
        self.send_header('Referrer-Policy','no-referrer')
        for value in cookies:self.send_header('Set-Cookie',value)
        self.send_header('Content-Length','0')
        self.end_headers()

    def auth_route(self, path, query):
        if path not in ('/auth/login','/auth/logout'):return self.error(404,'route_unavailable')
        auth=self.server.sso
        if not auth:return self.error(404,'sso_not_configured')
        try:
            if path=='/auth/login' and self.command=='GET':
                network=self.tailnet_headers()
                if not network:return self.error(401,'tailscale_identity_required')
                admission=getattr(self.server,'admission',None)
                identity=auth.authorize(network,admission)
                if not identity:return self.error(403,'machine_access_required')
                return self.auth_redirect('/',[auth.issue(identity,self.headers)])
            if path=='/auth/logout' and self.command=='POST':
                session=self.dashboard_session()
                if not self.valid_session_write(session):return self.error(403,'csrf_required')
                return self.auth_redirect('/',[auth.logout(self.headers)])
            return self.error(405,'method_not_allowed')
        except Exception:
            # Provider responses may contain secrets; expose no exception text.
            return self.error(503,'sso_provider_unavailable')

    def valid_session_write(self, session):
        return bool(session and self.headers.get('Origin')==self.server.sso.origin and
                    hmac.compare_digest(self.headers.get('X-Loom-CSRF',''),session['csrf']))

    def entry_origin(self):
        service=getattr(self.server,'admission',None)
        if self.server.sso and service and service.config.get('separate_gateway'):
            return service.config['origin']
        return None

    def dashboard_session(self,auth=None):
        auth=auth or self.server.sso
        session=auth.session(self.headers) if auth else None
        # A browser cookie must never override the current trusted VPN identity.
        # Local sessions never override a changed network account or revoked share.
        if session and getattr(auth,'config',{}).get('provider')=='tailscale':
            network=self.tailnet_headers().get('Tailscale-User-Login','').strip().casefold()
            login=str(session['user'].get('login') or '').strip().casefold()
            if not network or not login or network!=login:return None
        if session and hasattr(auth,'authorize'):
            current=auth.authorize(self.tailnet_headers(),getattr(self.server,'admission',None))
            if not current or any(current.get(k)!=session['user'].get(k) for k in ('issuer','subject','login')):return None
        if session and session['user'].get('authentication')=='tailscale-sharing':
            admission=getattr(self.server,'admission',None)
            if not admission or not hasattr(admission,'session_allowed') or not admission.session_allowed(session['user'],self.tailnet_headers()):return None
        return session

    def tailnet_headers(self):
        try:
            trusted=any(ipaddress.ip_address(self.client_address[0])==ipaddress.ip_address(address)
                        for address in self.server.tailscale_proxies)
        except ValueError:
            trusted=False
        if not trusted or len(self.headers.get_all('Tailscale-User-Login',[])) != 1:
            return {}
        return {key:self.headers[key] for key in ('Tailscale-User-Login','Tailscale-User-Name') if self.headers.get(key)}

    def relay(self, method, path, body=None, *, media=False):
        if self.headers.get("Sec-Fetch-Site") == "cross-site":
            return self.error(403, "cross_site_forbidden")
        origin = self.headers.get("Origin")
        if origin:
            try:
                site = urlsplit(origin)
            except ValueError:
                return self.error(403, "cross_site_forbidden")
            if site.scheme not in ("http", "https") or site.netloc.lower() != self.headers.get("Host", "").lower():
                return self.error(403, "cross_site_forbidden")
        bearer = self.headers.get("Authorization", "")
        session=self.dashboard_session()
        admin_route = path in ('/api/admin/tokens','/api/admin/tokens/revoke')
        if admin_route:
            admission=getattr(self.server,'admission',None)
            if not session:return self.error(401,'login_required')
            if not admission or not admission.is_admin(session['user']):return self.error(403,'admin_required')
        if self.server.sso:
            if not session:return self.error(401,'sso_required')
            if self.command!='GET' and not self.valid_session_write(session):return self.error(403,'csrf_required')
            # Browser routes cannot bypass logout using Serve headers or a token.
            bearer=''
            person={**session['user'],'access_admin':bool(admin_route)}
            identity={'X-Loom-Dashboard-Identity':self.server.sso.assertion(person)}
        else:
            identity=self.tailnet_headers() if not bearer else {}
        if not ((self.server.development or identity) and not bearer) and not re.fullmatch(r"Bearer [!-~]{1,4096}", bearer):
            return self.error(401, "credential_required")
        upstream = self.server.upstream
        connect = http.client.HTTPSConnection if upstream.scheme == "https" else http.client.HTTPConnection
        connection = connect(upstream.hostname, upstream.port, timeout=8)
        try:
            headers = {"Accept": ", ".join(sorted(MEDIA_TYPES)) if media else "application/json"}
            headers.update(identity)
            if bearer:
                headers['Authorization'] = bearer
            if body is not None:
                headers["Content-Type"] = "application/json"
            connection.request(method, path, body=body, headers=headers)
            response = connection.getresponse()
            self.loom_revision=response.headers.get("X-Loom-Revision")
            if 300 <= response.status < 400:
                return self.error(502, "unexpected_api_redirect")
            content_type = response.headers.get_content_type()
            if path == '/api/guide/bundle' and response.status == 200:
                data=response.read(MAX_JSON+1)
                if content_type!='application/zip' or len(data)>MAX_JSON:return self.error(502,'invalid_guide_bundle')
                return self.reply(200,data,'application/zip')
            if media and response.status == 200:
                if content_type not in MEDIA_TYPES:
                    return self.error(502, "invalid_media_response")
                size = response.length
                if size is None or size < 0 or (size == 0 and content_type not in TEXT_TYPES):
                    return self.error(502, "invalid_media_response")
                if size > (MAX_TEXT if content_type in TEXT_TYPES else MAX_MEDIA):
                    return self.error(413, "media_too_large")
                self.send_headers(200, size, content_type, "default-src 'none'; sandbox")
                try:
                    while chunk := response.read(65536):
                        self.wfile.write(chunk)
                except (socket.timeout, OSError, http.client.HTTPException):
                    self.close_connection = True
                return
            limit = MAX_CATALOG if path == "/api/dashboard/catalog" else MAX_JSON
            data = response.read(limit + 1)
            if len(data) > limit or content_type != "application/json":
                return self.error(502, "invalid_api_response")
            self.reply(response.status, data)
        except (socket.timeout, OSError, http.client.HTTPException):
            self.error(503, "node_unavailable")
        finally:
            connection.close()

    def do_POST(self):
        if self.path in ('/api/admin/tokens','/api/admin/tokens/revoke'):
            try:
                size=int(self.headers.get('Content-Length','0'))
                if not 0<size<=4096 or self.headers.get_content_type()!='application/json':raise ValueError()
                body=self.rfile.read(size)
                if not isinstance(json.loads(body),dict):raise ValueError()
            except (ValueError,TypeError):return self.error(400,'invalid_token_request')
            return self.relay('POST',self.path,body)
        if self.path.startswith('/auth-demo/'):
            return self.error(404,'route_unavailable')
        if self.path.startswith('/admission/'):
            return admission_http.admin_route(self)
        if self.path.startswith('/auth/'):
            return self.auth_route(self.path,'')
        if self.path == '/api/updates/check':
            try:
                length=int(self.headers.get('Content-Length','0'))
                if not 0 < length <= 256 or self.headers.get_content_type()!='application/json':raise ValueError()
                if json.loads(self.rfile.read(length)) != {}:raise ValueError()
            except (ValueError,TypeError):return self.error(400,'invalid_update_check')
            return self.relay('POST','/api/updates/check',b'{}')
        if self.path != '/api/jobs/queue-actions':return self.error(405, 'read_only')
        try:
            size=int(self.headers.get('Content-Length','0'))
            if not 0<size<=8192 or self.headers.get_content_type()!='application/json':raise ValueError()
            payload=json.loads(self.rfile.read(size))
            if not isinstance(payload,dict) or set(payload)!={'action','arguments'}:raise ValueError()
            action=payload['action'];args=payload['arguments']
            if action not in ('reorder','hold','resume') or not isinstance(args,dict):raise ValueError()
            allowed={'id','reason'} | ({'position','reference'} if action=='reorder' else set())
            if set(args)-allowed or not isinstance(args.get('id'),str) or not re.fullmatch(IDENTIFIER,args['id']):raise ValueError()
            if action=='reorder' and args.get('position') not in ('next','tail','before','after'):raise ValueError()
        except (ValueError,TypeError):return self.error(400,'invalid_queue_action')
        return self.relay('POST','/api/actions',json.dumps({'group':'jobs',**payload}).encode())

    def reject_write(self):self.error(405,'read_only')
    do_PUT = do_DELETE = do_PATCH = do_OPTIONS = reject_write


def make_server(bind, port, api_origin):
    origin = urlsplit(api_origin)
    if (origin.scheme not in ("http", "https") or not origin.hostname or origin.username is not None
            or origin.password is not None or origin.path or origin.query or origin.fragment):
        raise ValueError("LOOM_API_ORIGIN must be an HTTP(S) origin")
    server = ThreadingHTTPServer((bind, port), WebHandler)
    server.upstream = origin
    server.development = os.environ.get('LOOM_DEV_NO_AUTH') == '1'
    server.tailscale_proxies = os.getenv('LOOM_TAILSCALE_TRUSTED_PROXIES','').split()
    config_path=os.getenv('LOOM_SSO_CONFIG')
    server.sso=sso.SSO(json.loads(Path(config_path).read_text())) if config_path else None
    return server


if __name__ == "__main__":
    with make_server(os.getenv("LOOM_DASHBOARD_BIND", "127.0.0.1"), int(os.getenv("LOOM_DASHBOARD_PORT", "8080")),
                     os.environ["LOOM_API_ORIGIN"]) as server:
        access_config=os.getenv('LOOM_ADMISSION_CONFIG')
        public=stopped=None
        if access_config:
            public,stopped=admission_http.start(json.loads(Path(access_config).read_text()),server)
        try:server.serve_forever()
        finally:
            if stopped:stopped.set()
            if public:public.shutdown();public.server_close()
