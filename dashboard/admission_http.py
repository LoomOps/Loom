import diagnostics
"""A separate public listener: only access requests, never Dashboard/MCP/API relay."""
import json
import hmac
import threading
import secrets
import time
import hashlib
from datetime import datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs
from admission import RemoteError, make_admission
from sso import SSO, cookie_value

ROOT=Path(__file__).parent/'public'


class ReceiptAuth:
    """Receipt of an unverified email request; NOT a Tailscale login/credential."""
    cookie='__Host-loom-access-receipt'
    set_cookie=SSO.set_cookie

    def __init__(self, service):
        self.service=service;self.origin=service.config['origin']
        with service.db() as c:
            c.execute('CREATE TABLE IF NOT EXISTS receipts(id TEXT PRIMARY KEY,subject TEXT,csrf TEXT,email TEXT,expires REAL,created REAL)')

    def session(self, headers):
        token=cookie_value(headers,self.cookie)
        with self.service.db() as c:
            r=c.execute('SELECT * FROM receipts WHERE id=? AND expires>?',(hashlib.sha256(token.encode()).hexdigest(),time.time())).fetchone()
        if not r:return None
        return {'user':{'issuer':self.origin+'/requests','subject':r['subject'],'login':r['email'],'name':r['email'] or ''},'csrf':r['csrf']}

    def issue(self):
        token=secrets.token_urlsafe(32);now=time.time()
        with self.service.lock,self.service.db() as c:
            c.execute('DELETE FROM receipts WHERE expires<=?',(now,))
            if c.execute('SELECT count(*) FROM receipts WHERE created>?',(now-60,)).fetchone()[0]>=120 or c.execute('SELECT count(*) FROM receipts').fetchone()[0]>=10000:
                raise ValueError('request_capacity_reached')
            c.execute('INSERT INTO receipts VALUES(?,?,?,?,?,?)',(hashlib.sha256(token.encode()).hexdigest(),secrets.token_urlsafe(24),secrets.token_urlsafe(32),None,now+604800,now))
        return self.set_cookie(self.cookie,token,604800)

    def bind(self, session, email):
        import re
        if not isinstance(email,str) or not re.fullmatch(r'[^\s@]{1,128}@[^\s@]{1,128}',email):raise ValueError('email_required')
        email=email.casefold()
        if session['user']['login'] and session['user']['login']!=email:raise ValueError('receipt_email_changed')
        with self.service.db() as c:c.execute('UPDATE receipts SET email=? WHERE subject=?',(email,session['user']['subject']))
        session['user']['login']=email

    def logout(self, headers):
        with self.service.db() as c:c.execute('DELETE FROM receipts WHERE id=?',(hashlib.sha256(cookie_value(headers,self.cookie).encode()).hexdigest(),))
        return self.set_cookie(self.cookie,'',0)

    def reset(self,session):
        # Rotate the logical receipt and CSRF; stale tabs cannot submit the old email.
        with self.service.db() as c:
            c.execute('UPDATE receipts SET subject=?,csrf=?,email=NULL WHERE subject=?',
                (secrets.token_urlsafe(24),secrets.token_urlsafe(32),session['user']['subject']))


def reply(h, status, value, mime='application/json', cookies=(), csp=None):
    body=value if isinstance(value,bytes) else json.dumps(value).encode()
    h.send_response(status)
    for k,v in {'Content-Type':mime,'Content-Length':str(len(body)),'Cache-Control':'no-store',
                'X-Content-Type-Options':'nosniff','Referrer-Policy':'no-referrer',
                'Content-Security-Policy':csp or "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"}.items():h.send_header(k,v)
    for cookie in cookies:h.send_header('Set-Cookie',cookie)
    try:h.end_headers();h.wfile.write(body)
    except (BrokenPipeError,ConnectionResetError):pass


def payload(h, session, origin):
    if not session: raise PermissionError('login_required')
    if h.headers.get('Origin')!=origin or not hmac.compare_digest(h.headers.get('X-Loom-CSRF',''),session['csrf']):
        raise PermissionError('csrf_required')
    n=int(h.headers.get('Content-Length','0'))
    if not 0<n<=4096 or h.headers.get_content_type()!='application/json': raise ValueError('invalid_body')
    v=json.loads(h.rfile.read(n))
    if not isinstance(v,dict): raise ValueError('invalid_body')
    return v


def safe_rows(rows, admin=False):
    fields={'id','machine','email','state','created','updated','error','sender_name','sender_email','user_cancelled','can_cancel'}
    if admin: fields|={'attempts','next_attempt','user_id','accepted_email','accepted_at','invite_id'}
    return [{k:v for k,v in r.items() if k in fields} for r in rows]


def admin_route(h):
    service=getattr(h.server,'admission',None)
    session=h.dashboard_session()
    if not service: return reply(h,503,{'code':'access_enrollment_not_configured'})
    if not session: return reply(h,401,{'code':'login_required'})
    if not service.is_admin(session['user']): return reply(h,403,{'code':'admin_required'})
    try:
        if h.command=='GET' and h.path=='/admission/session':
            return reply(h,200,{'csrf':session['csrf'],'machine':service.config['machine_id'],'name':service.config.get('name',service.config['machine_id'])})
        if h.path=='/admission/ssh':
            import ssh_access
            ssh=ssh_access.SSHAccess(service)
            if h.command=='GET':
                configured=Path(__import__('os').environ.get('LOOM_SSH_SOCKET','/run/loom-ssh/control.sock')).exists()
                return reply(h,200,{'configured':configured,'accounts':ssh.rows()})
            if h.command=='POST':
                v=payload(h,session,h.server.sso.origin)
                if set(v)!={'login','username','action'}:raise ValueError('invalid_ssh_request')
                eligible={m['loginName'].casefold() for m in service.members()}
                eligible.add(session['user']['login'].casefold())
                return reply(h,200,ssh.change(v['login'],v['username'],v['action'],session['user'],eligible))
        if h.command=='GET' and h.path=='/admission/requests':
            return reply(h,200,{'requests':safe_rows(service.rows(session['user'],admin=True),True)})
        if h.command=='GET' and h.path=='/admission/members':
            user=session['user']
            administrator={'id':'admin:'+hashlib.sha256((user['issuer']+'\0'+user['subject']).encode()).hexdigest()[:24],
                'loginName':user.get('login'),'displayName':user.get('name') or user.get('login'),
                'role':'Admin','status':'machine_admin','created':None}
            return reply(h,200,{'members':[administrator,*service.members()]})
        if h.command=='POST' and h.path=='/admission/decide':
            v=payload(h,session,h.server.sso.origin)
            if set(v)!={'id','action'}: raise ValueError('invalid_decision')
            service.decide(v['id'],v['action'],session['user'])
            return reply(h,200,{'accepted':True})
        if h.command=='POST' and h.path=='/admission/share-revoke':
            v=payload(h,session,h.server.sso.origin)
            if set(v)!={'id'} or not isinstance(v['id'],str) or len(v['id'])>128:raise ValueError('invalid_share')
            service.revoke_share(v['id'],session['user'])
            return reply(h,200,{'accepted':True})
        return reply(h,404,{'code':'route_unavailable'})
    except PermissionError as e:return reply(h,403,{'code':str(e)})
    except (RemoteError,OSError):return reply(h,502,{'code':'tailnet_members_unavailable'})
    except (ValueError,TypeError):return reply(h,400,{'code':'invalid_request'})


class PublicHandler(BaseHTTPRequestHandler):
    def setup(self):
        self.request.settimeout(15);super().setup()

    def log_message(self,*args): pass

    def redirect(self,url,cookies=()):
        self.send_response(303);self.send_header('Location',url);self.send_header('Content-Length','0')
        self.send_header('Cache-Control','no-store');self.send_header('Referrer-Policy','no-referrer')
        for cookie in cookies:self.send_header('Set-Cookie',cookie)
        self.end_headers()

    def handle_route(self):
        from gateway import public_route
        if public_route(self):return
        a=self.server.admission;auth=self.server.sso
        session=auth.session(self.headers)
        path,_,query=self.path.partition('?')
        try:
            if self.command=='GET':
                if path=='/api/session':
                    cookies=[]
                    if isinstance(auth,ReceiptAuth) and not session:
                        cookie=auth.issue();cookies=[cookie]
                        session=auth.session({'Cookie':cookie.split(';')[0]})
                    return reply(self,200,{'user':session['user'] if session else None,'csrf':session['csrf'] if session else None,
                        'name':a.config.get('name',a.config['machine_id']),'machine':a.config['machine_id'],
                        'dashboard':a.config['dashboard_origin'],'mode':'invite'},cookies=cookies)
                if path=='/api/request':
                    if not session:return reply(self,401,{'code':'login_required'})
                    return reply(self,200,{'requests':safe_rows(a.rows(session['user']))})
            if self.command=='POST' and path in ('/api/request','/api/request/reset'):
                session=auth.session(self.headers);v=payload(self,session,auth.origin)
                if path=='/api/request/reset':
                    if v:raise ValueError('unexpected_fields')
                    cancelled=a.cancel_own_pending(session['user'],strict=True)
                    auth.reset(session)
                    return reply(self,200,{'reset':True,'cancellation_queued':cancelled})
                if set(v)!={'email'}:raise ValueError('email_required')
                auth.bind(session,v['email'])
                return reply(self,200,{'request':safe_rows([a.request(session['user'])])[0]})
            return reply(self,404,{'code':'route_unavailable'})
        except PermissionError:return reply(self,403,{'code':'forbidden'})
        except ValueError as error:
            # Do not disclose whether the email is an admin, native member or
            # approved recipient. It never establishes identity or grants access.
            code='access_check_required' if str(error)=='access_check_required' else 'request_failed'
            return reply(self,409 if code=='access_check_required' else 400,{'code':code})
        except Exception as exc:
            diagnostics.record('gateway','request',exc,code='request_failed')
            # No provider responses, tokens, exception text or account enumeration.
            return reply(self,400,{'code':'request_failed'})

    do_GET=handle_route
    do_POST=handle_route
    def do_HEAD(self):reply(self,405,{'code':'method_not_allowed'})


def start(config, private_server):
    # Also protect existing installations whose saved admission config predates
    # native_logins. No identity keys or session material enter the public worker.
    config={**config,'native_logins':list(getattr(private_server.sso,'users',{}))}
    service=make_admission(config)
    if config.get('separate_gateway'):
        private_server.admission=service
        stop=threading.Event()
        threading.Thread(target=service.run,args=(stop,),daemon=True).start()
        return None,stop
    public=ThreadingHTTPServer((config.get('bind','127.0.0.1'),config.get('port',8081)),PublicHandler)
    auth=ReceiptAuth(service)
    public.sso=auth;public.admission=service;private_server.admission=service
    stop=threading.Event()
    threading.Thread(target=public.serve_forever,daemon=True).start()
    threading.Thread(target=service.run,args=(stop,),daemon=True).start()
    return public,stop
