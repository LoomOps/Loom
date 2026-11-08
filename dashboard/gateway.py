"""Public entry UI and a private, origin-bound browser reachability probe."""
import base64
import hashlib
import json
import os
from pathlib import Path
import re
from urllib.parse import parse_qs

HERE=Path(__file__).parent
ASSETS=HERE/'theme'


def public_route(h):
    from admission_http import reply
    path,_,query=h.path.partition('?')
    if h.command!='GET' or query:return False
    if path=='/healthz':
        reply(h,200,{'status':'running','service':'loom-gateway'});return True
    files={'/':'index.html','/style.css':'style.css','/theme.js':'theme.js','/app.js':None,'/brand.js':'brand.js','/brand-scene.js':'scene.js'}
    if path not in files:return False
    dashboard=h.server.admission.config['dashboard_origin']
    csp="default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-src "+dashboard+"; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"
    if path=='/':
        body=(HERE/'gateway.html').read_text().replace('<!-- LOOM_BRAND_LOGO -->',(HERE/'branding/logo.svg').read_text().strip()).encode();mime='text/html; charset=utf-8'
    elif path in ('/brand.js','/brand-scene.js'):body=(HERE/'branding'/files[path]).read_bytes();mime='text/javascript'
    elif path=='/app.js':body=(HERE/'gateway.js').read_bytes();mime='text/javascript'
    elif path=='/theme.js':body=(ASSETS/'theme.js').read_bytes();mime='text/javascript'
    else:
        body=(ASSETS/'style.css').read_bytes()+(HERE/'gateway.css').read_bytes()+(HERE/'branding/brand.css').read_bytes()
        mime='text/css'
    reply(h,200,body,mime,csp=csp);return True


def probe(h,query):
    """Called ONLY on the private Dashboard, never through the public listener."""
    service=getattr(h.server,'admission',None)
    if not service:return h.error(404,'gateway_unavailable')
    try:params=parse_qs(query,keep_blank_values=True,max_num_fields=4)
    except ValueError:return h.error(400,'invalid_probe')
    if set(params)!={'nonce'} or len(params['nonce'])!=1 or not re.fullmatch(r'[a-zA-Z0-9_-]{16,100}',params['nonce'][0]):
        return h.error(400,'invalid_probe')
    auth=h.server.sso
    session=h.dashboard_session() if auth else None
    network=h.tailnet_headers()
    identity=None
    if session:identity={key:session['user'].get(key,'') for key in ('name','login')}
    elif network and (not hasattr(auth,'authorize') or auth.authorize(network,service)):
        from email.header import decode_header,make_header
        login=network['Tailscale-User-Login']
        identity={'login':login,'name':str(make_header(decode_header(network.get('Tailscale-User-Name',login))))}
    data={'type':'loom-gateway-probe','nonce':params['nonce'][0],'authorized':bool(identity),'identity':identity,
          'destination':'/' if session or not auth else '/auth/login'}
    origin=service.config['origin']
    script='parent.postMessage('+json.dumps(data).replace('<','\\u003c')+','+json.dumps(origin)+');'
    digest=base64.b64encode(hashlib.sha256(script.encode()).digest()).decode()
    body=('<!doctype html><meta charset="utf-8"><script>'+script+'</script>').encode()
    h.send_response(200)
    for k,v in {'Content-Type':'text/html; charset=utf-8','Content-Length':str(len(body)),
                'Cache-Control':'no-store','Referrer-Policy':'no-referrer','X-Content-Type-Options':'nosniff',
                'Content-Security-Policy':"default-src 'none'; script-src 'sha256-"+digest+"'; frame-ancestors "+origin+"; base-uri 'none'"}.items():h.send_header(k,v)
    h.end_headers();h.wfile.write(body)


def main():
    from http.server import ThreadingHTTPServer
    from admission import make_admission
    from admission_http import PublicHandler,ReceiptAuth
    config=json.loads(Path(os.environ['LOOM_GATEWAY_CONFIG']).read_text())
    service=make_admission(config)
    # No management credential, SSO session key, API relay, or reconcile worker here.
    with ThreadingHTTPServer((config.get('bind','0.0.0.0'),config.get('port',8081)),PublicHandler) as server:
        server.admission=service;server.sso=ReceiptAuth(service);server.gateway=True
        server.serve_forever()


if __name__=='__main__':main()
