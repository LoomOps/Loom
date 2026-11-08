"""Real Firefox + real dashboard relay, backed only by deterministic mock Ops."""
import base64
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
from threading import Thread
import time
import unittest
from unittest.mock import patch
from urllib.error import URLError

TESTS=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(TESTS),str(TESTS.parent.parent/'dashboard')]
from mock_ops import make_mock,fixture,native
import server
from verify_browser import request,FIREFOX
from verify_workspace import Workspace

class WorkspaceBrowserTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        binary=Path(os.environ.get('LOOM_TEST_FIREFOX',str(FIREFOX/'firefox')))
        gecko=Path(os.environ.get('LOOM_TEST_GECKODRIVER',str(FIREFOX/'geckodriver')))
        if not binary.is_file() or not gecko.is_file():raise RuntimeError('Firefox/geckodriver missing; set LOOM_TEST_FIREFOX and LOOM_TEST_GECKODRIVER')
        cls.api=make_mock();cls.web=server.make_server('127.0.0.1',0,'http://127.0.0.1:'+str(cls.api.server_port));cls.web.development=True
        for service in [cls.api,cls.web]:
            thread=Thread(target=service.serve_forever,kwargs={'poll_interval':.02},daemon=True);thread.start()
            cls.addClassCleanup(thread.join,2);cls.addClassCleanup(service.server_close);cls.addClassCleanup(service.shutdown)
        build=patch.object(server.build_version,'status',return_value={'build_id':'fixture','image_build_id':'fixture','live_source':True})
        build.start();cls.addClassCleanup(build.stop)
        profile_root=Path.home()/'snap/firefox/common'
        profile=tempfile.TemporaryDirectory(prefix='loom-mock-test-',dir=profile_root if profile_root.is_dir() else None);cls.addClassCleanup(profile.cleanup)
        with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
        driver=subprocess.Popen([str(gecko),'--port',str(port),'--profile-root',profile.name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        cls.addClassCleanup(driver.wait,10);cls.addClassCleanup(driver.terminate)
        endpoint='http://127.0.0.1:'+str(port)
        for _ in range(70):
            try:request(endpoint+'/status');break
            except URLError:time.sleep(.1)
        session=request(endpoint+'/session',{'capabilities':{'alwaysMatch':{'browserName':'firefox','moz:firefoxOptions':{'args':['-headless'],'binary':str(binary)},'timeouts':{'pageLoad':20000,'script':10000}}}})['value']['sessionId']
        cls.session=endpoint+'/session/'+session
        cls.addClassCleanup(lambda:request(cls.session,method='DELETE'))
        cls.base='http://127.0.0.1:'+str(cls.web.server_port)
    def wd(self,path,body=None):return request(self.session+path,body)['value']
    def setUp(self):
        self.api.errors.clear();self.api.fixture=fixture();self.api.actions.clear();self.api.calls.clear();self.ui=Workspace(self.wd)
        self.wd('/window/rect',{'width':1440,'height':1000});self.wd('/url',{'url':self.base})
        self.ui.wait("return document.querySelector('iframe')?.hidden===false");self.ui.frame()
        self.ui.wait("return document.querySelector('[data-resource-panel]')?.textContent.includes('/loom/data/0')")
    def test_readable_preview_safely_preserves_content(self):
        source=(TESTS.parent.parent/'dashboard/workspace/text-preview.js').read_text()
        script=source+r"""
const done=arguments[arguments.length-1];
(async()=>{
 const json='{"id":9007199254740993,"nested":[true,{"x":"a\\\"b"}]}';
 const view=await loomTextElement(new Blob([json],{type:'application/json'}),'fixture.json');
 document.body.append(view);
 if(!view.querySelector('pre').textContent.includes('9007199254740993')||!view.querySelector('pre').textContent.includes('\n'))throw Error('JSON precision or formatting');
 view.querySelector('button').click();if(view.querySelector('pre').textContent!==json)throw Error('Raw toggle');view.remove();
 const md=loomMarkdown('# Heading\n\n**Bold** and `code`\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n<script>window.previewInjected=true</script>\n<img src=x onerror="window.previewInjected=true">\n[x](javascript:alert(1))');
 document.body.append(md);
 if(!md.querySelector('h2')||!md.querySelector('strong')||md.querySelectorAll('td').length!==2)throw Error('Markdown format');
 if(md.querySelector('script,img')||md.querySelector('[href^="javascript:"]')||window.previewInjected)throw Error('Unsafe Markdown');md.remove();
 const invalid=await loomTextElement(new Blob(['{bad'],{type:'application/json'}),'bad.json');
 if(!invalid.textContent.includes('JSON 문법 오류'))throw Error('Invalid JSON');
 const empty=await loomTextElement(new Blob([''],{type:'text/plain'}),'empty.txt');if(!empty.textContent.includes('빈 파일'))throw Error('Empty text');
 try{await loomTextElement(new Blob([new Uint8Array([255])],{type:'text/plain'}),'bad.txt');throw Error('Accepted invalid UTF8');}catch(error){if(error.message==='Accepted invalid UTF8')throw error;}
 done({markdown:true,json_precision:true,raw_toggle:true,unsafe_html_inert:true,invalid_json:true,empty_text:true,invalid_utf8:true});
})().catch(error=>done({error:String(error)}));
"""
        result=self.wd('/execute/async',{'script':script,'args':[]})
        self.assertNotIn('error',result)
        self.assertTrue(all(result.values()))

    def test_release_check_is_visible_and_manual_request_does_not_mutate_jobs(self):
        ui=self.ui
        ui.wait("return document.querySelector('[data-loom-updates]')?.textContent==='Up to date'")
        ui.click('[data-loom-updates]')
        ui.wait("return document.querySelector('.loom-update-panel')?.textContent.includes('0.1.0-alpha.14')")
        ui.click('[data-update-now]')
        ui.wait("return document.querySelector('[data-loom-updates]')?.textContent==='Checking…'")
        self.assertIn(('POST','/api/updates/check'),self.api.calls)
        self.assertEqual(self.api.actions,[])
        ui.top();self.assertTrue(ui.js("return document.querySelector('#loom-entry').hidden"))
        self.assertFalse(ui.js("return !!document.querySelector('#loom-access')"))

    def test_brand_motion_has_long_empty_interval_and_shared_theme(self):
        ui=self.ui
        ui.wait("return document.querySelector('.pa-brand .loom-logo-token')?.getAnimations().length===2")
        def at(ms):
            return ui.js("""const logo=document.querySelector('.pa-brand .loom-logo');
              for(const a of logo.getAnimations({subtree:true})){a.pause();a.currentTime=arguments[0]}
              return {opacity:Number(getComputedStyle(logo.querySelector('.loom-logo-token')).opacity),
                gate:getComputedStyle(logo.querySelector('.loom-logo-gate')).transform,
                durations:logo.getAnimations({subtree:true}).map(a=>a.effect.getTiming().duration)};""",ms)
        for ms in [0,3500,10000,11900]:
            state=at(ms);self.assertEqual(state['opacity'],0)
            self.assertEqual(state['gate'],'matrix(1, 0, 0, 1, 0, 0)')
            self.assertEqual(set(state['durations']),{12000})
        self.assertEqual(at(6500)['opacity'],1)
        self.assertFalse(ui.js("return !!document.querySelector('#loom-brand-scene')"))
        for theme,color in [('light','rgb(66, 69, 72)'),('dark','rgb(225, 227, 230)')]:
            ui.js("document.querySelector('#pai-studio').style.colorScheme=arguments[0]",theme)
            self.assertEqual(ui.js("return getComputedStyle(document.querySelector('.loom-logo-base')).fill"),color)

    def test_sso_account_ui_and_reload(self):
        # Browser lifecycle uses local identity; HTTP trust and real cookies are covered in test_sso.
        class Auth:
            origin=self.base
            active=False
            def authorize(auth,network,admission=None):return {'name':'SSO Member','login':'member@example.test'}
            def issue(auth,identity,headers):auth.active=True;return 'fixture-session=1; Path=/; HttpOnly; SameSite=Lax'
            def session(auth,headers):
                return {'user':{'name':'SSO Member','login':'member@example.test'},'csrf':'fixture-csrf','expires':9999999999} if auth.active and 'fixture-session=1' in headers.get('Cookie','') else None
            def assertion(auth,identity):return 'fixture-relay'
            def logout(auth,headers):auth.active=False;return 'fixture-session=; Path=/; Max-Age=0'
        trust=patch.object(server.WebHandler,'tailnet_headers',return_value={'Tailscale-User-Login':'member@example.test'});trust.start();self.addCleanup(trust.stop)
        self.web.sso=Auth();self.addCleanup(setattr,self.web,'sso',None)
        self.wd('/frame',{'id':None});self.wd('/url',{'url':self.base})
        self.ui.wait("return document.querySelector('#loom-sso-login')?.hidden===false")
        self.assertTrue(self.ui.js("return document.querySelector('#loom-access-form').hidden"))
        self.ui.js("document.querySelector('#loom-sso-login').click()")
        self.ui.wait("return !document.querySelector('iframe').hidden");self.ui.frame()
        self.ui.wait("return document.querySelector('[data-loom-session-name]')?.textContent==='SSO Member'")
        self.wd('/frame',{'id':None})
        self.wd('/refresh',{})
        self.ui.wait("return !document.querySelector('iframe').hidden");self.ui.frame()
        self.ui.wait("return document.querySelector('[data-loom-session-name]')?.textContent==='SSO Member'")
        self.assertTrue(self.ui.js("return !!document.querySelector('.pa-sidebar [data-loom-session]') && !document.querySelector('.pa-top [data-loom-session]')"))
        self.ui.click('.pa-profile-trigger')
        self.ui.wait("return document.querySelector('.pa-loom-tailnet')?.textContent.includes('member@example.test')")
        self.assertFalse(self.ui.js("return !!document.querySelector('.pa-loom-access')"))
        self.assertTrue(self.ui.js("return document.querySelector('[data-go=access]').textContent==='Tailscale'"))
        self.ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
        self.assertFalse(self.ui.js("return !!document.querySelector('[data-loom-logout]')"))


    def test_tailnet_member_order_and_responsive_layout(self):
        import test_admission as fixture_access
        f=fixture_access.AdmissionTests();f.setUp();self.addCleanup(f.doCleanups)
        members=[{'id':'old','loginName':'a.long.member.email.for.wrapping@example.test','displayName':'Earlier Member','role':'Member','status':'ready','created':'2026-10-01T00:00:00Z'},
                 {'id':'new','loginName':'new@example.test','displayName':'New Member','role':'Member','status':'ready','created':'2026-10-03T00:00:00Z'}]
        member_patch=patch.object(f.a,'members',return_value=members);member_patch.start();self.addCleanup(member_patch.stop)
        f.a.request(f.user)
        class Auth:
            origin=self.base
            def session(auth,headers):return {'user':{**f.admin,'name':'Machine Owner'},'csrf':'fixture-admin-csrf'}
            def assertion(auth,identity):return 'fixture-relay'
        self.web.sso=Auth();self.web.admission=f.a
        self.addCleanup(setattr,self.web,'sso',None);self.addCleanup(delattr,self.web,'admission')
        ui=self.ui;ui.top();self.wd('/url',{'url':self.base})
        ui.wait("return !document.querySelector('iframe').hidden");ui.frame()
        ui.wait("return !!document.querySelector('.pa-profile-trigger')");ui.click('.pa-profile-trigger')
        ui.wait("return document.querySelectorAll('[data-tailnet-member]').length===3")
        self.assertEqual(ui.js("return [...document.querySelectorAll('[data-tailnet-member]')].slice(1).map(e=>e.dataset.tailnetMember)"),['old','new'])
        self.assertTrue(ui.js("return document.querySelector('[data-tailnet-member]').dataset.tailnetMember.startsWith('admin:')"))
        self.assertEqual(ui.js("return [...document.querySelectorAll('.pa-loom-tailnet h2')].map(e=>e.childNodes[0].textContent.trim())"),['Account','Member','Access Request'])
        self.assertEqual(ui.js("return document.querySelectorAll('[data-go=access] .pa-tailscale-symbol path').length"),9)
        self.assertFalse(ui.js("return !!document.querySelector('[data-loom-logout]')"))
        for width in [1440,390]:
            ui.top();self.wd('/window/rect',{'width':max(width,500),'height':1000});ui.js("document.querySelector('iframe').style.width=arguments[0]+'px'",width);ui.frame();ui.wait("return innerWidth===arguments[0]",width)
            for theme in ['light','dark']:
                ui.click('button[data-theme='+theme+']')
                self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"))
                self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-tailnet-table-wrap')].every(p=>p.scrollWidth<=p.clientWidth+1)"))
                if folder:=os.environ.get('LOOM_TEST_SCREENSHOTS'):
                    output=Path(folder);output.mkdir(parents=True,exist_ok=True);ui.top();(output/f'tailnet-{theme}-{width}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
        ui.top();ui.js("document.querySelector('iframe').style.width=''")

    def test_shared_machine_request_accept_approve_and_revoke(self):
        import test_sharing as fixture_sharing
        from admission_http import PublicHandler,ReceiptAuth
        from http.server import ThreadingHTTPServer
        f=fixture_sharing.SharingTests();f.setUp();self.addCleanup(f.doCleanups)
        public=ThreadingHTTPServer(('127.0.0.1',0),PublicHandler)
        origin='http://127.0.0.1:'+str(public.server_port)
        f.a.config['origin']=origin;f.a.config['dashboard_origin']=self.base
        public.admission=f.a;public.sso=ReceiptAuth(f.a);public.gateway=True
        public.sso.cookie='fixture-sharing-receipt'
        public.sso.set_cookie=lambda name,value,ttl:f'{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl}'
        thread=Thread(target=public.serve_forever,kwargs={'poll_interval':.02},daemon=True);thread.start()
        self.addCleanup(thread.join,2);self.addCleanup(public.server_close);self.addCleanup(public.shutdown)
        class Auth:
            origin=self.base
            active=False
            def session(auth,headers):return {'user':f.admin,'csrf':'fixture-admin-csrf'} if auth.active else None
            def assertion(auth,identity):return 'fixture-relay'
        auth=Auth();self.web.sso=auth;self.web.admission=f.a
        self.addCleanup(setattr,self.web,'sso',None);self.addCleanup(delattr,self.web,'admission')
        ui=self.ui;ui.top();self.wd('/url',{'url':origin})
        ui.wait("return !document.querySelector('#request').hidden")
        ui.wait("return document.documentElement.classList.contains('loom-scene-ready')")
        self.assertEqual(ui.js("return getComputedStyle(document.querySelector('.card')).backdropFilter"),'blur(44px) saturate(1.2)')
        self.assertEqual(ui.js("return document.querySelector('.loom-logo-gate').getAnimations()[0].effect.getTiming().duration"),8000)
        if folder:=os.environ.get('LOOM_TEST_SCREENSHOTS'):
            output=Path(folder);output.mkdir(parents=True,exist_ok=True)
            for theme in ['light','dark']:
                ui.js("window.LoomTheme.set(arguments[0])",theme)
                time.sleep(.6)
                (output/f'landing-applied-{theme}.png').write_bytes(base64.b64decode(self.wd('/screenshot')))
        self.assertTrue(ui.js("const b=document.querySelector('.wordmark').getBoundingClientRect(),h=document.querySelector('.hostname').getBoundingClientRect();return h.top>=b.bottom && Math.abs((h.left+h.width/2)-(b.left+b.width/2))<1"))
        self.assertTrue(ui.js("return !document.querySelector('.card #retry-connection') && !!document.querySelector('.entry-stack > #retry-connection') && !document.querySelector('h1')"))
        ui.js("document.querySelector('#email').value='typo@example.test'");ui.click('#request-form button')
        ui.wait("return !document.querySelector('#sent').hidden")
        self.assertFalse(ui.js("return !!document.querySelector('#device-form')"))
        self.assertIn('Request access via Tailscale',ui.js("return document.querySelector('#request-form button').textContent"))
        self.assertTrue(ui.js("return !!document.querySelector('#request-form button svg')"))
        ui.wait("return document.querySelector('#request-note').textContent.includes('기다려')")
        with f.a.db() as c:pending=dict(c.execute("SELECT * FROM share_requests WHERE email='typo@example.test'").fetchone())
        ui.wait("return document.querySelector('.card-shadow').getAnimations({subtree:true}).every(a=>a.constructor.name!=='Animation')")
        ui.js("""window.motionBefore={width:document.querySelector('#request-visual').getBoundingClientRect().width,font:getComputedStyle(document.querySelector('#sent .pending')).font};
          window.motionCopy=null;const observer=new MutationObserver(()=>{const ghost=document.querySelector('.motion-ghost');if(ghost){window.motionCopy={width:ghost.querySelector('svg').parentElement.getBoundingClientRect().width,font:getComputedStyle(ghost.querySelector('.pending')).font};observer.disconnect();}});
          observer.observe(document.querySelector('.card'),{childList:true});""")
        f.a.update(pending['id'],created=1)
        ui.js("window.dispatchEvent(new Event('focus'))")
        ui.wait("return document.querySelector('.pending').textContent.includes('지연')")
        ui.wait("return window.motionCopy!==null")
        self.assertTrue(ui.js("return Math.abs(window.motionBefore.width-window.motionCopy.width)<1 && window.motionBefore.font===window.motionCopy.font"))

        f.a.update(pending['id'],error='invite_delivery_uncertain')
        ui.js("window.dispatchEvent(new Event('focus'))")
        ui.wait("return document.querySelector('.pending').textContent.includes('발송을 확인')")
        f.a.update(pending['id'],error='tailscale_api_400')
        ui.js("window.dispatchEvent(new Event('focus'))")
        ui.wait("return !document.querySelector('#edit-email').hidden && document.querySelector('.pending').textContent.includes('준비하지 못')")
        self.assertNotIn('tailscale_api',ui.js("return document.body.textContent"))
        self.assertNotIn('존재하지 않는',ui.js("return document.body.textContent"))
        f.a.update(pending['id'],error=None)
        f.a.tick();self.assertEqual(len(f.api.shares),1)
        self.wd('/refresh',{});ui.wait("return !document.querySelector('#sent').hidden")
        ui.wait("return document.querySelector('.pending').textContent.includes('공유 초대 메일')")
        ui.wait("return document.querySelector('.card-shadow').getAnimations({subtree:true}).every(a=>a.constructor.name!=='Animation')")
        ui.js("window.motionStable=null;refreshRequest().then(()=>window.motionStable=document.querySelector('.card-shadow').getAnimations({subtree:true}).every(a=>a.constructor.name!=='Animation'))")
        ui.wait("return window.motionStable===true")
        self.assertTrue(ui.js("return !document.querySelector('.motion-ghost') && !document.querySelector('.card').style.height"))

        self.assertTrue(ui.js("return document.querySelector('#request-note').hidden"))
        f.a.update(pending['id'],sender_name='Sharing Admin',sender_email='sender@example.test')
        ui.js("window.dispatchEvent(new Event('focus'))")
        ui.wait("return document.querySelector('#request-note').textContent==='Sharing Admin · sender@example.test'")
        self.wd('/window/rect',{'width':390,'height':844})
        self.assertTrue(ui.js("return document.documentElement.scrollWidth<=innerWidth"))
        self.wd('/window/rect',{'width':1440,'height':1000})
        ui.js("window.dispatchEvent(new Event('focus'))")
        ui.wait("return !document.querySelector('#edit-email').hidden")
        self.assertEqual(ui.js("return document.querySelector('#request-visual [data-stage]:not([hidden])').dataset.stage"),'mail')
        self.assertEqual(ui.js("return document.querySelector('#edit-email').textContent"),'Use another email')
        ui.js("window.savedMatchMedia=window.matchMedia;window.matchMedia=q=>q==='(prefers-reduced-motion: reduce)'?{matches:true}:window.savedMatchMedia(q);clearMotion()")
        ui.click('#edit-email');ui.wait("return !document.querySelector('#request').hidden && document.querySelector('#email').value===''")
        self.assertTrue(ui.js("return document.querySelector('.card-shadow').getAnimations({subtree:true}).every(a=>a.constructor.name!=='Animation') && !document.querySelector('.motion-ghost')"))
        ui.js("window.matchMedia=window.savedMatchMedia")

        cancelled=next(r for r in f.a.rows(f.admin,admin=True) if r['id']==pending['id'])
        self.assertEqual(cancelled['state'],'revoking');self.assertTrue(cancelled['user_cancelled'])
        ui.js("document.querySelector('#email').value='member@example.test'");ui.click('#request-form button')
        ui.wait("return document.querySelector('#receipt-email').textContent==='member@example.test'")
        f.a.tick();self.assertEqual([i['email'] for i in f.api.shares],['member@example.test'])
        with f.a.db() as c:r=dict(c.execute("SELECT * FROM share_requests WHERE email='member@example.test'").fetchone())
        f.api.shares[0].update(accepted=True,acceptedBy={'id':321,'loginName':'actual@example.test'})
        f.a.update(r['id'],next_attempt=0);f.a.tick()
        self.assertNotIn('actual@example.test',str(f.api.policy))
        ui.js("window.dispatchEvent(new Event('focus'))")
        ui.wait("return document.querySelector('.pending').textContent==='관리자 승인을 기다리고 있습니다.'")
        self.assertEqual(ui.js("return document.querySelector('#request-visual [data-stage]:not([hidden])').dataset.stage"),'approval')
        self.assertTrue(ui.js("return document.querySelector('#request-spinner').hidden && document.querySelector('#request-note').textContent.includes('닫아도 됩니다')"))
        ui.js("window.savedFetch=window.fetch;window.fetch=()=>Promise.reject(new Error('offline'));window.dispatchEvent(new Event('focus'))")
        ui.wait("return document.querySelector('.pending').textContent==='최신 상태를 확인할 수 없습니다.'")
        ui.js("window.fetch=window.savedFetch;window.dispatchEvent(new Event('focus'))")
        ui.wait("return document.querySelector('.pending').textContent==='관리자 승인을 기다리고 있습니다.'")
        auth.active=True;self.wd('/url',{'url':self.base})
        ui.wait("return !document.querySelector('iframe').hidden");ui.frame()
        ui.wait("return !!document.querySelector('.pa-profile-trigger')");ui.click('.pa-profile-trigger')
        ui.wait("return document.querySelector('.pa-loom-access')?.textContent.includes('actual@example.test')")
        self.assertIn('Cancelled by user',ui.js("return document.querySelector('table[aria-label=\"Access Request\"]').textContent"))
        self.assertTrue(ui.js("return document.querySelector('[data-tailnet-member]').dataset.tailnetMember.startsWith('admin:')"))
        self.assertEqual(ui.js("return document.querySelectorAll('[data-tailnet-member]').length"),1)
        self.assertEqual(ui.js("return document.querySelector('[data-tailnet-member] [data-label=Role]').textContent"),'Admin')
        self.assertTrue(ui.js("return document.querySelector('table[aria-label=\"Access Request\"]').textContent.includes('Grant status')"))
        self.assertTrue(ui.js("return document.querySelector('[data-go=access]').textContent==='Tailscale'"))
        ui.click('[data-access-action=approve]');ui.wait("return document.querySelector('.pa-loom-access').textContent.includes('Granting access')")
        f.a.tick();ui.click('[data-access-refresh]')
        ui.wait("return document.querySelector('.pa-loom-access').textContent.includes('Granted')")
        self.assertEqual(ui.js("return [...document.querySelectorAll('[data-tailnet-member] [data-label=Role]')].map(e=>e.textContent)"),['Admin','Member'])
        ui.click('[data-share-revoke]');ui.wait("return document.querySelector('.pa-loom-access').textContent.includes('Revoking')")
        f.a.tick();ui.click('[data-access-refresh]')
        ui.wait("return document.querySelectorAll('[data-tailnet-member]').length===1 && document.querySelector('[data-tailnet-member]').dataset.tailnetMember.startsWith('admin:')")
        self.assertFalse(ui.js("return !!document.querySelector('[data-tailnet-member] [data-share-revoke]')"))
        self.assertEqual(f.api.shares,[])
        ui.wait("return !!document.querySelector('[data-access-action=remove]')")
        ui.click('[data-access-action=remove]')
        ui.wait("return !document.querySelector('table[aria-label=\"Access Request\"]')?.textContent.includes('member@example.test')")
        self.assertFalse(any(x['email']=='member@example.test' for x in f.a.rows(f.admin,admin=True)))
        ui.click('[data-access-action=remove]')
        ui.wait("return !document.querySelector('[data-access-action=remove]')")

        denied=f.a.request({**f.user,'subject':'denied-fixture','login':'denied@example.test'})
        f.a.decide(denied['id'],'deny',f.admin);f.a.tick();ui.click('[data-access-refresh]')
        ui.wait("return !!document.querySelector('[data-access-action=remove]')")
        ui.click('[data-access-action=remove]')
        ui.wait("return !document.querySelector('.pa-loom-access').textContent.includes('denied@example.test')")
        self.assertFalse(any(r['id']==denied['id'] for r in f.a.rows(f.admin,admin=True)))
        # Losing the receipt does not lose the last email, and never auto-submits it.
        auth.active=False
        ui.top();self.wd('/url',{'url':origin})
        request(self.session+'/cookie/fixture-sharing-receipt',method='DELETE')
        self.wd('/refresh',{})
        ui.wait("return !document.querySelector('#request').hidden")
        self.assertEqual(ui.js("return document.querySelector('#email').value"),'member@example.test')
        self.assertEqual(f.a.rows(f.admin,admin=True),[])
        ui.js("Object.defineProperty(window,'localStorage',{get(){throw new Error('blocked')}})")
        self.assertEqual(ui.js("return savedEmail()"),'')
        ui.js("rememberEmail('other@example.test')")


    def test_reload_preserves_open_tabs_and_active_document(self):
        ui=self.ui
        for page in ('models','datasets','docker','datasets'):ui.click('#pa-nav [data-go="'+page+'"]')
        read="return [...document.querySelectorAll('.pa-document-pane')].map(p=>({id:p.dataset.pane,active:p.classList.contains('is-active'),tabs:[...p.querySelectorAll('[role=tab]')].map(t=>({id:t.dataset.doc,selected:t.getAttribute('aria-selected')}))}))"
        expected=ui.js(read)
        ui.top()
        ui.wait("return history.state?.paiTabsV25?.workspace?.panes?.some(p=>p.active==='datasets')")
        self.wd('/refresh',{})
        ui.wait("return !document.querySelector('iframe').hidden");ui.frame()
        ui.wait("return document.querySelector('.is-active [data-doc= datasets]')?.getAttribute('aria-selected')==='true'")
        self.assertEqual(ui.js(read),expected)
        ui.click('[data-close-doc="models"]')
        expected=ui.js(read)
        ui.top();ui.wait("return history.state?.paiTabsV25?.workspace?.panes?.every(p=>p.tabs.every(t=>t.id!=='models'))")
        self.wd('/refresh',{})
        ui.wait("return !document.querySelector('iframe').hidden");ui.frame()
        ui.wait("return document.querySelector('.is-active [data-doc= datasets]')?.getAttribute('aria-selected')==='true'")
        self.assertEqual(ui.js(read),expected)

    def test_model_owner_property(self):
        self.ui.route('fixture-model')
        self.ui.wait("return [...document.querySelectorAll('.pa-job-properties')].some(p=>p.textContent.includes('Fixture Owner'))")
        self.assertTrue(self.ui.js("return [...document.querySelectorAll('.pa-job-properties dt')].some(p=>p.textContent==='Owner')"))

    def test_account_switch_rejects_old_admin_and_enters_as_shared_member(self):
        import test_sso,test_sharing
        auth_fixture=test_sso.SSOTest();auth_fixture.setUp();self.addCleanup(auth_fixture.doCleanups)
        sharing=test_sharing.SharingTests();sharing.setUp();self.addCleanup(sharing.doCleanups);sharing.ready()
        auth=auth_fixture.auth;auth.cookie='fixture-account-switch'
        auth_fixture.claim_changes={'sub':sharing.admin['subject'],'email':sharing.admin['login']}
        old,_=auth_fixture.finish()
        auth.config['provider']='tailscale';auth.origin=self.base
        auth.set_cookie=lambda name,value,ttl:f'{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl}'
        self.web.sso=auth;self.web.admission=sharing.a
        self.addCleanup(setattr,self.web,'sso',None);self.addCleanup(delattr,self.web,'admission')
        # Only the trusted Serve boundary is mocked. Cookies and sessions are real.
        original=self.web.RequestHandlerClass
        class NetworkHandler(original):
            def tailnet_headers(handler):return {'Tailscale-User-Login':handler.server.fixture_login}
        self.web.fixture_login='actual@example.test';self.web.RequestHandlerClass=NetworkHandler
        self.addCleanup(setattr,self.web,'RequestHandlerClass',original);self.addCleanup(delattr,self.web,'fixture_login')
        self.ui.top();self.wd('/cookie',{'cookie':{'name':auth.cookie,'value':old['Cookie'].split('=',1)[1],'path':'/'}})
        self.wd('/url',{'url':self.base})
        self.ui.wait("return !document.querySelector('#loom-entry').hidden && document.querySelector('iframe').hidden")
        self.ui.click('#loom-sso-login')
        self.ui.wait("return !document.querySelector('iframe').hidden");self.ui.frame()
        self.ui.wait("return document.querySelector('[data-loom-session-name]')?.textContent==='actual@example.test'")
        self.ui.click('.pa-profile-trigger')
        self.ui.wait("return document.querySelector('.pa-tailnet-badge')?.textContent==='Member'")
        self.assertFalse(self.ui.js("return !!document.querySelector('.pa-loom-access')"))
        self.ui.top();self.web.fixture_login='owner@example.test';self.wd('/refresh',{})
        self.ui.wait("return !document.querySelector('#loom-entry').hidden && document.querySelector('iframe').hidden")

    def test_job_classification_detail_and_filters(self):
        ui=self.ui
        ui.wait("return document.querySelector('[data-job-filter=jobCategory]')?.options.length===7")
        ui.js("const e=document.querySelector('[data-job-filter=jobCategory]');e.value='fine_tuning';e.dispatchEvent(new Event('change',{bubbles:true}))")
        ui.wait("return [...document.querySelectorAll('.pa-panel')].find(p=>p.querySelector('h2')?.textContent.startsWith('종료 이력'))?.querySelectorAll('tbody tr').length===2")
        self.assertTrue(ui.js("return document.querySelector('.pa-job-classification')?.textContent==='Fine-tuning (Pilot-test)'"))
        ui.js("const e=document.querySelector('[data-job-filter=jobExecutionMode]');e.value='full';e.dispatchEvent(new Event('change',{bubbles:true}))")
        ui.wait("return ![...document.querySelectorAll('.pa-panel')].find(p=>p.querySelector('h2')?.textContent.startsWith('종료 이력'))?.querySelector('tbody tr')")
        ui.route('fixture-live')
        ui.wait("return document.querySelector('.pa-job-properties')?.textContent.includes('Fine-tuning')")
        self.assertTrue(ui.js("return document.querySelector('.pa-job-properties').textContent.includes('Fixture Owner')"))
        self.assertTrue(ui.js("return document.querySelector('.pa-job-properties').textContent.includes('Job Type') && document.querySelector('.pa-job-properties').textContent.includes('Execution Mode') && document.querySelector('.pa-job-properties').closest('.pa-panel').textContent.includes('Job Properties')"))
        self.assertTrue(ui.js("return document.querySelector('.pa-job-properties').textContent.includes('Pilot-test')"))

    def test_queue_merges_live_and_imported_history_and_expands(self):
        ui=self.ui
        def rows():return ui.js("return [...document.querySelectorAll('.pa-panel')].find(p=>p.querySelector('h2')?.textContent.startsWith('종료 이력'))?.querySelectorAll('tbody tr').length")
        self.assertEqual(rows(),10);ui.click('[data-history-more]');self.assertEqual(rows(),12)
        self.assertTrue(ui.js("return !document.querySelector('#pa-content').textContent.includes('Loom Jobs')"))
        ui.click('[data-history-collapse]');self.assertEqual(rows(),10)
        self.assertTrue(ui.js("return document.querySelector('[data-resource-panel]').textContent.includes('data/0') && !document.querySelector('[data-resource-panel]').textContent.includes('data/1')"))
    def test_agent_navigation_removed_and_old_bookmark_redirects(self):
        ui=self.ui;ui.nav('Dashboard')
        self.assertFalse(ui.js("return !!document.querySelector('#pa-nav [data-go=agent]')"))
        ui.top();ui.js("location.hash='#/views/agent'");ui.frame()
        ui.wait("return document.querySelector('#pa-content [data-queue-history]') || document.querySelector('#pa-breadcrumb')?.textContent.includes('Dashboard')")
        self.assertFalse(ui.js("return document.querySelector('#pa-content').textContent.includes('에이전트가 사용하는 스킬')"))

    def test_detail_menu_alignment_properties_and_share(self):
        ui=self.ui
        for identity in ['fixture-model','work:fixture-0','fixture-live','fixture-imported']:
            with self.subTest(identity=identity):
                ui.route(identity)
                ui.wait("return document.querySelector('.pa-entity-title-row')")
                self.assertTrue(ui.js("const a=document.querySelector('.pa-entity-title-row h1').getBoundingClientRect(),b=document.querySelector('.pa-entity-menu>summary'),r=b.getBoundingClientRect();return Math.abs(a.top+a.height/2-r.top-r.height/2)<2 && getComputedStyle(b).borderWidth==='0px'"))
                if identity!='fixture-model':self.assertTrue(ui.js("return document.querySelector('.pa-job-properties')?.textContent.includes('Task level')"))
                if identity=='fixture-imported':ui.wait("return document.querySelector('#pa-content').textContent.includes('Exit code')")
                ui.click('.pa-entity-menu>summary');ui.click('[data-share-route]');ui.top()
                link=ui.wait("return document.querySelector('#pai-share-url')?.value");self.assertIn('/#/entities/',link)
                ui.click('[data-share-close]');ui.frame()
    def test_new_window_opens_only_selected_item(self):
        ui=self.ui;ui.route('fixture-model');ui.click('.pa-entity-menu>summary')
        old=self.wd('/window');handles=set(self.wd('/window/handles'))
        element=self.wd('/element',{'using':'css selector','value':'[data-open-route]'})['element-6066-11e4-a52e-4f735466cecf'];self.wd('/element/'+element+'/click',{});ui.top()
        for _ in range(60):
            new=set(self.wd('/window/handles'))-handles
            if new:break
            time.sleep(.1)
        self.assertEqual(len(new),1);self.wd('/window',{'handle':new.pop()})
        try:
            ui.wait("return document.querySelector('iframe')?.hidden===false");ui.frame()
            ui.wait("return document.querySelector('.pa-entity-title-row')?.textContent.includes('Fixture Model')")
            self.assertEqual(ui.js("return document.querySelectorAll('.pa-document-tab').length"),1)
        finally:
            ui.top();request(self.session+'/window',method='DELETE');self.wd('/window',{'handle':old});ui.frame()
    def test_docker_mcp_and_narrow_light_dark_layout(self):
        ui=self.ui;ui.nav('Docker');ui.wait("return document.querySelectorAll('[data-container-row]').length===2")
        ui.click('[data-container-row]');ui.wait("return document.querySelector('[data-container-extra]')?.hidden===false")
        ui.nav('MCP');ui.wait("return document.querySelector('.pa-doc-detail')?.textContent.includes('jobs.get')")
        ui.route('fixture-live')
        for width in [1440,390]:
            ui.top();self.wd('/window/rect',{'width':max(width,500),'height':1000});ui.js("document.querySelector('iframe').style.width=arguments[0]+'px'",width);ui.frame();ui.wait("return innerWidth===arguments[0]",width)
            for theme in ['light','dark']:
                ui.click('button[data-theme='+theme+']')
                self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"))
                if folder:=os.environ.get('LOOM_TEST_SCREENSHOTS'):
                    output=Path(folder);output.mkdir(parents=True,exist_ok=True);ui.top();(output/f'job-{theme}-{width}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
    def test_mcp_reference_navigation_examples_copy_and_mobile(self):
        ui=self.ui;ui.nav('MCP')
        ui.wait("return document.querySelector('[data-doc-action=submit]')")
        ui.click('[data-doc-action=submit]')
        self.assertTrue(ui.js("return document.querySelector('.pa-doc-detail').textContent.includes('jobs.submit')"))
        self.assertFalse(ui.js("return !!document.querySelector('.pa-doc-detail script')"))
        example=ui.js("return [...document.querySelectorAll('.pa-doc-code code')][1].textContent")
        import json
        self.assertEqual(json.loads(example)['params']['arguments']['action'],'submit')
        ui.top();ui.js("Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async text=>{window.copiedDoc=text}}})");ui.frame()
        ui.js("document.querySelectorAll('[data-doc-copy]')[1].click()")
        ui.wait("return document.querySelector('.pa-doc-copy-status').textContent.includes('복사했습니다')")
        ui.top();self.assertEqual(ui.js("return window.copiedDoc"),example);ui.frame()
        for theme in ['light','dark']:
            ui.click('button[data-theme='+theme+']');ui.top()
            ui.js("document.querySelector('iframe').style.width='390px'");ui.frame()
            ui.wait("return innerWidth===390")
            self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"))

    def test_resource_failure_is_visible_and_recovers(self):
        ui=self.ui;self.api.errors['/api/dashboard/resources']=503
        ui.wait("return document.querySelector('[data-resource-panel]').textContent.includes('연결 끊김')")
        self.api.errors.clear()
        ui.wait("return !document.querySelector('[data-resource-panel]').textContent.includes('연결 끊김')")
        self.assertFalse(any(method=='POST' and path not in ['/api/history/get','/mcp'] for method,path in self.api.calls))

    def test_queue_order_hold_resume_and_error_keep_controls_scoped(self):
        ui=self.ui
        entity,record=native('fixture-waiting')
        entity['state']=entity['properties']['state']='queued';record['state']='queued'
        entity['attempts']=[];entity['execution_intervals']=[]
        self.api.fixture['catalog']['entities'].append(entity);self.api.fixture['jobs'][entity['id']]=record
        self.api.fixture['catalog']['queue']={'entries':[
            {'id':'other-waiting','owner':'other','state':'queued','status':'queued','position':1,'held':False,'reason':'ready','can_reorder':False,'can_hold':False,'can_resume':False},
            {'id':'fixture-waiting','owner':'owner','state':'queued','status':'queued','position':2,'held':False,'reason':'ready','can_reorder':True,'can_hold':True,'can_resume':False}]}
        ui.top();self.wd('/url',{'url':self.base});ui.wait("return document.querySelector('iframe')?.hidden===false");ui.frame()
        ui.wait("return document.querySelector('[data-queue-id=fixture-waiting]')")
        self.assertFalse(ui.js("return !!document.querySelector('[data-queue-id=other-waiting]')"))
        ui.click('.pa-queue-control summary');ui.click('[data-queue-position=next]')
        ui.wait("return document.querySelector('.pa-queue-feedback')?.textContent==='큐 변경 완료'")
        self.assertEqual(self.api.actions[-1],{'group':'jobs','action':'reorder','arguments':{'id':'fixture-waiting','position':'next'}})
        ui.wait("return document.querySelector('.pa-current-work-queue tbody tr')?.textContent.includes('Fixture fixture-waiting')")
        ui.click('.pa-queue-control summary');ui.click('[data-queue-action=hold]')
        ui.wait("return document.querySelector('.pa-current-work-queue tbody tr')?.textContent.includes('Held')")
        ui.click('.pa-queue-control summary');ui.click('[data-queue-action=resume]')
        ui.wait("return !!document.querySelector('[data-queue-action=hold]') && !document.querySelector('[data-queue-action=resume]')")
        self.api.errors['/api/actions']=403
        ui.click('.pa-queue-control summary');ui.click('[data-queue-action=hold]')
        ui.wait("return document.querySelector('.pa-queue-feedback')?.textContent.includes('권한이 없습니다')")
        self.assertFalse(ui.js("return document.querySelector('[data-queue-action=hold]').disabled"))

    def test_theme_cookie_defaults_to_system_and_survives_reload(self):
        self.ui.top();self.ui.js("document.cookie='loom_theme=; Path=/; Max-Age=0'")
        self.wd('/url',{'url':self.base});self.ui.frame()
        self.ui.wait("return document.querySelector('button[data-theme=system]')?.getAttribute('aria-pressed')==='true'")
        for mode in ['dark','light','system']:
            self.ui.click('button[data-theme='+mode+']');self.ui.top()
            self.ui.wait("return document.cookie.split('; ').includes(arguments[0])",'loom_theme='+mode)
            self.wd('/refresh',{});self.ui.frame()
            self.ui.wait("return document.querySelector('button[data-theme='+arguments[0]+']')?.getAttribute('aria-pressed')==='true'",mode)

    def test_narrow_sidebar_no_scroll_logo_only_and_tooltips(self):
        for width,height in [(720,900),(390,844),(740,390)]:
            self.ui.top();self.wd('/window/rect',{'width':max(500,width),'height':height})
            self.ui.js("document.querySelector('iframe').style.width=arguments[0]+'px'",width)
            self.ui.frame();self.ui.wait('return innerWidth===arguments[0]',width)
            self.assertTrue(self.ui.js("const s=document.querySelector('.pa-sidebar');return s.scrollWidth<=s.clientWidth+1 && s.scrollHeight<=s.clientHeight+1 && getComputedStyle(s).overflowY==='hidden'"),(width,height))
            self.assertTrue(self.ui.js("return getComputedStyle(document.querySelector('.pa-brand>span:last-child')).display==='none' && document.querySelector('.pa-mark svg').getBoundingClientRect().width===25"))
            self.ui.js("document.querySelector('#pa-nav .pa-nav-item').dispatchEvent(new PointerEvent('pointerover',{bubbles:true,pointerType:'mouse'}))")
            self.ui.wait("return document.querySelector('#loom-nav-tooltip').matches(':popover-open')")
            self.assertTrue(self.ui.js("const t=document.querySelector('#loom-nav-tooltip'),b=document.querySelector('#pa-nav .pa-nav-item'),r=t.getBoundingClientRect();return t.textContent===b.textContent.trim() && r.left>=0 && r.right<=innerWidth && r.bottom<=innerHeight"))
            self.ui.js("document.querySelector('#pa-nav .pa-nav-item').dispatchEvent(new PointerEvent('pointerout',{bubbles:true,pointerType:'mouse'}))")
            self.ui.wait("return !document.querySelector('#loom-nav-tooltip').matches(':popover-open')")
            self.ui.js("document.querySelectorAll('#pa-nav .pa-nav-item')[1].focus()")
            self.ui.wait("return document.querySelector('#loom-nav-tooltip').matches(':popover-open')")
            self.ui.js("document.activeElement.blur()")
        self.ui.top();self.wd('/window/rect',{'width':1440,'height':1000});self.ui.js("document.querySelector('iframe').style.width='100%'");self.ui.frame()
        self.ui.wait("return getComputedStyle(document.querySelector('.pa-brand>span:last-child')).display!=='none'")

    def test_content_surfaces_resize_and_preserve_navigation(self):
        ui=self.ui
        ui.wait("return document.querySelectorAll('.pa-pane-body .loom-surface').length>0")
        self.assertTrue(ui.js("return !document.querySelector('.pa-sidebar .loom-surface,.pa-document-tabs .loom-surface')"))
        for width,height in [(1440,1000),(390,844)]:
            ui.top();self.wd('/window/rect',{'width':max(width,500),'height':height})
            ui.js("document.querySelector('iframe').style.width=arguments[0]+'px'",width);ui.frame()
            ui.wait('return innerWidth===arguments[0]',width)
            for mode in ['light','dark']:
                ui.click('button[data-theme='+mode+']')
                ui.wait("return document.querySelector('button[data-theme='+arguments[0]+']').getAttribute('aria-pressed')==='true'",mode)
                ui.wait("return [...document.querySelectorAll('.loom-surface')].filter(c=>c.getBoundingClientRect().width).every(c=>{const svg=c.querySelector(':scope>.loom-surface-skin');return svg&&c.querySelectorAll(':scope>.loom-surface-skin').length===1&&Math.abs(svg.viewBox.baseVal.width-c.getBoundingClientRect().width)<1})")
                self.assertTrue(ui.js("return [...document.querySelectorAll('.loom-surface')].every(c=>getComputedStyle(c).clipPath==='none')"))
                self.assertTrue(ui.js("""const gradients=[...document.querySelectorAll('.pa-resource-card linearGradient')];
                return gradients.length>0 && new Set(gradients.map(g=>g.id)).size===gradients.length && gradients.every(g=>{
                    const dx=Number(g.getAttribute('x2'))-Number(g.getAttribute('x1')),dy=Number(g.getAttribute('y2'))-Number(g.getAttribute('y1'));
                    return dx>0 && Math.abs(Math.atan2(dy,dx)+Math.PI/6)<.001 && g.closest('svg').firstChild.getAttribute('fill')==='url(#'+g.id+')';
                });"""))
                self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(c=>c.scrollWidth<=c.clientWidth+1)"))
                self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-activity-graph')].every(g=>{const a=g.getBoundingClientRect(),b=g.querySelector('.pa-activity-track').getBoundingClientRect();return b.top>=a.top-1 && b.bottom<=a.bottom+1})"))
                if folder:=os.environ.get('LOOM_TEST_SCREENSHOTS'):
                    out=Path(folder);out.mkdir(parents=True,exist_ok=True)
                    ui.top();(out/f'surfaces-{mode}-{width}.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
        ui.nav('Docker');ui.wait("return document.querySelectorAll('[data-container-row]').length===2")
        ui.click('[data-container-row]');ui.wait("return document.querySelector('[data-container-extra]')?.hidden===false")
        ui.nav('MCP');ui.wait("return document.querySelector('.pa-doc-detail.loom-surface svg.loom-surface-skin')!==null")

    def test_activity_visible_initialization_and_stable_scroll(self):
        ui=self.ui
        today_visible="""const el=document.querySelector('.pa-activity-graph');
        const date=new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Seoul',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date());
        const today=el?.querySelector('[data-activity-day="'+date+'"]');
        if(!today)return false;const a=today.getBoundingClientRect(),b=el.getBoundingClientRect();
        return b.width>0&&a.left>=b.left&&a.right<=b.right;"""
        ui.wait(today_visible)
        # Recreate the frame while hidden, then reveal it after layout has run.
        ui.top()
        ui.js("const frame=document.querySelector('iframe');frame.style.display='none';frame.srcdoc=frame.srcdoc")
        ui.frame();ui.wait("return !!document.querySelector('.pa-activity-graph')")
        self.assertTrue(ui.js("return document.querySelector('.pa-activity-graph').clientWidth===0"))
        ui.top();ui.js("document.querySelector('iframe').style.display=''");ui.frame()
        ui.wait(today_visible)
        ui.js("""window.calendar=document.querySelector('.pa-activity-graph');
        window.otherPanel=document.querySelector('[data-resource-panel]');
        window.oldStart=calendar.dataset.activityStart;
        for(let i=0;i<5;i++)window.dispatchEvent(new Event('resize'));
        calendar.scrollLeft=50;
        window.anchor=calendar.querySelectorAll('.pa-activity-week')[2].firstElementChild.dataset.activityDay;""")
        ui.wait("return calendar.dataset.activityStart!==oldStart")
        self.assertTrue(ui.js("return calendar===document.querySelector('.pa-activity-graph')&&otherPanel===document.querySelector('[data-resource-panel]')"))
        self.assertEqual(ui.js("return (Date.parse(calendar.dataset.activityStart)-Date.parse(oldStart))/86400000"),-364)
        self.assertTrue(ui.js("const event=new WheelEvent('wheel',{deltaX:20,cancelable:true,bubbles:true});calendar.dispatchEvent(event);return !event.defaultPrevented"))
        self.assertTrue(ui.js("""const a=calendar.querySelector('[data-activity-day="'+anchor+'"]').getBoundingClientRect(),b=calendar.getBoundingClientRect();
        return Math.abs(a.left-b.left+12)<3;"""))
        ui.click('[data-activity-period=today]');ui.wait(today_visible)

    def test_activity_years_follow_january_and_change_at_edges(self):
        ui=self.ui
        ui.wait("return document.querySelectorAll('.pa-activity-year').length>=2")
        ui.js("window.graph=document.querySelector('.pa-activity-graph');window.firstStart=graph.dataset.activityStart;graph.scrollLeft=graph.scrollWidth")
        ui.wait("return graph.dataset.activityStart!==firstStart")
        ui.js("""const weeks=graph.querySelectorAll('.pa-activity-week');
        window.pitch=weeks[1].offsetLeft-weeks[0].offsetLeft;
        window.yearLabel=[...graph.querySelectorAll('.pa-activity-year')].find(y=>Number(y.dataset.start)*pitch>graph.clientWidth/2+100 && Number(y.dataset.start)*pitch<graph.scrollWidth-graph.clientWidth-100);
        window.boundary=Number(yearLabel.dataset.start)*pitch;
        graph.scrollLeft=boundary-graph.clientWidth/2;""")
        ui.wait("return Math.abs(yearLabel.getBoundingClientRect().left-graph.getBoundingClientRect().left-graph.clientWidth/2)<2")
        self.assertTrue(ui.js("return graph.querySelector('.pa-activity-months').children[Number(yearLabel.dataset.start)].textContent==='1월'"))
        # Native scrolling moves the year with January before any scroll handler.
        self.assertTrue(ui.js("""const before=yearLabel.getBoundingClientRect().left;graph.scrollLeft+=17;
        return Math.abs(yearLabel.getBoundingClientRect().left-before+17)<1;"""))
        ui.js("graph.scrollLeft=boundary-yearLabel.offsetWidth-4;window.pinnedYear=graph.querySelector('.pa-activity-year-pin>span')")
        ui.wait("return yearLabel.hidden&&pinnedYear.textContent===yearLabel.textContent")
        for delta in [5,11,18]:
            ui.js("graph.scrollLeft+=arguments[0]",delta)
            ui.wait("return Math.abs(pinnedYear.getBoundingClientRect().left-graph.getBoundingClientRect().left)<2")
        ui.js("graph.scrollLeft=boundary+30")
        ui.wait("return yearLabel.hidden&&pinnedYear.textContent===yearLabel.textContent")
        # Reverse scrolling restores the previous year while January follows natively.
        ui.js("graph.scrollLeft=boundary-graph.clientWidth+10")
        ui.wait("return !yearLabel.hidden&&Math.abs(yearLabel.getBoundingClientRect().right-graph.getBoundingClientRect().right)<2")
        self.assertTrue(ui.js("return Number(pinnedYear.textContent)<Number(yearLabel.dataset.year)"))

    def test_card_surfaces_prepared_before_vertical_scroll(self):
        ui=self.ui
        ui.wait("return document.querySelectorAll('.pa-panel.loom-surface').length>2")
        ui.js("""window.pane=document.querySelector('.pa-pane-body');
        window.nextCard=[...pane.querySelectorAll('.pa-panel.loom-surface')].find(c=>{const y=c.getBoundingClientRect().top,b=pane.getBoundingClientRect().bottom;return y>b&&y<b+750});""")
        ui.wait("return nextCard?.querySelector(':scope>.loom-surface-skin')?.style.willChange==='transform'")
        self.assertTrue(ui.js("return !!nextCard.querySelector(':scope>.loom-surface-skin path').getAttribute('d')"))
        ui.js("window.originalSkin=nextCard.querySelector(':scope>.loom-surface-skin');pane.scrollTop+=400")
        ui.wait("return nextCard.getBoundingClientRect().top<pane.getBoundingClientRect().bottom")
        self.assertTrue(ui.js("return originalSkin===nextCard.querySelector(':scope>.loom-surface-skin')"))

    def test_resource_updates_preserve_surfaces_without_flicker(self):
        ui=self.ui
        ui.wait("return document.querySelector('[data-resource-panel]>.loom-surface-skin') && document.querySelectorAll('.pa-resource-card>.loom-surface-skin').length>=4")
        ui.js("""window.savedInspector=document.querySelector('[data-resource-panel]');
        window.savedSkins=[...savedInspector.querySelectorAll('.loom-surface-skin')];window.removedSkins=0;
        window.skinWatch=new MutationObserver(records=>{for(const r of records)for(const n of r.removedNodes)if(n.nodeType===1&&(n.matches('.loom-surface-skin')||n.querySelector('.loom-surface-skin')))removedSkins++;});
        skinWatch.observe(savedInspector,{childList:true,subtree:true});""")
        for value in [37,62,19]:
            self.api.fixture['resources']['sections']['cpu']['data']['utilization_percent']=value
            ui.wait("return document.querySelector('.pa-resource-card').textContent.includes(arguments[0])",str(value))
            self.assertTrue(ui.js("return savedInspector===document.querySelector('[data-resource-panel]') && savedSkins.every(n=>n.isConnected) && removedSkins===0"))
        ui.js('skinWatch.disconnect()')

    def test_refresh_keeps_catalog_icons_and_sidebar_nodes(self):
        ui=self.ui
        ui.wait("return document.querySelector('.pa-pane-body svg[data-lucide]')")
        ui.js("""window.savedIcons=[...document.querySelectorAll('.pa-pane-body svg[data-lucide]')];
        window.savedSidebar=document.querySelector('.pa-node-summary');
        window.savedSync=document.querySelector('[data-sync-control] summary');
        document.querySelector('[data-sync-control]').open=true;""")
        for revision in ['refresh-2','refresh-3']:
            self.api.fixture['catalog']['revision']=revision
            ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
            ui.wait("return document.querySelector('[data-sync-control]').textContent.includes(arguments[0])",revision)
            self.assertTrue(ui.js("return savedIcons.length>0 && savedIcons.every(n=>n.isConnected) && savedSidebar===document.querySelector('.pa-node-summary') && savedSync===document.querySelector('[data-sync-control] summary') && document.querySelector('[data-sync-control]').open"))

    def test_docker_and_mcp_refresh_keep_expansion_and_surfaces(self):
        ui=self.ui;ui.nav('Docker')
        ui.wait("return document.querySelectorAll('[data-container-row]').length===2 && document.querySelector('.pa-loom-docker .loom-surface-skin')")
        ui.click('[data-container-row]')
        ui.js("""window.savedRow=document.querySelector('[data-container-row]');savedRow.focus();
        window.savedExtra=document.querySelector('[data-container-extra]');
        window.savedDockerSkins=[...document.querySelectorAll('.pa-loom-docker .loom-surface-skin')];""")
        for value in [31,47]:
            self.api.fixture['docker']['containers'][0]['stats']['cpu_percent']=value
            ui.js("parent.postMessage({type:'loom-docker-request'},'*')")
            ui.wait("return document.querySelector('.pa-loom-docker').textContent.includes(arguments[0])",str(value)+'%')
            self.assertEqual(ui.js("return [savedRow===document.querySelector('[data-container-row]'),savedExtra.isConnected,!savedExtra.hidden,savedRow.getAttribute('aria-expanded')==='true',document.activeElement===savedRow,savedDockerSkins.every(n=>n.isConnected)]"),[True]*6)
        ui.nav('MCP');ui.wait("return document.querySelector('[data-doc-action=submit]')")
        ui.click('[data-doc-action=submit]')
        ui.wait("return document.querySelector('.pa-doc-detail>.loom-surface-skin')")
        ui.js("""window.savedDoc=document.querySelector('.pa-doc-detail');
        window.savedDocSkin=savedDoc.querySelector('.loom-surface-skin');
        window.docRefresh=false;window.addEventListener('message',e=>{if(e.data?.type==='loom-mcp')window.docRefresh=true});
        parent.postMessage({type:'loom-mcp-request'},'*');""")

        ui.wait('return window.docRefresh')
        self.assertTrue(ui.js("return savedDoc===document.querySelector('.pa-doc-detail') && savedDocSkin.isConnected && savedDoc.textContent.includes('jobs.submit')"))

    def test_explorer_storage_scope_fixed_columns_and_inline_preview(self):
        ui=self.ui
        entity=next(e for e in self.api.fixture['catalog']['entities'] if e['id']=='fixture-model')
        paths=['data/0/datasets/session/episode/metadata.json','references/loom/private.json','loom/ops/config.json','data/9/unmounted.json']
        entity['loom_files']=[{'id':str(i),'path':path,'source_ref':path,'target_ref':path,'status':'available','size':24} for i,path in enumerate(paths)]
        entity['loom_files'][0]['preview_id']='metadata-preview'
        entity['loom_media']=[{'id':'metadata-preview','label':'metadata.json','mime':'application/json','status':'available'}]
        self.api.fixture['catalog']['revision']='explorer-fixture'
        ui.js("parent.postMessage({type:'pai-catalog-refresh'},'*')")
        ui.wait("return document.querySelector('[data-sync-control]').textContent.includes('explorer-fixture')")
        ui.top();self.wd('/window/rect',{'width':1440,'height':1000});ui.frame()
        ui.nav('Explorer')
        self.assertEqual(ui.js("return [...document.querySelectorAll('.pa-finder-favorites button')].map(b=>b.textContent.trim())"),['data/0'])
        self.assertEqual(ui.js("return document.querySelectorAll('.pa-finder-column').length"),1)
        self.assertEqual(ui.js("return document.querySelector('.pa-finder-column').getBoundingClientRect().width"),300)
        self.assertGreater(ui.js("return document.querySelector('.pa-finder-preview').getBoundingClientRect().width"),500)
        self.assertFalse(ui.js("return /private.json|config.json|unmounted.json|references/.test(document.querySelector('.pa-finder').textContent)"))
        for path in ['data/0','data/0/datasets','data/0/datasets/session','data/0/datasets/session/episode','data/0/datasets/session/episode/metadata.json']:
            ui.click('[data-column-node="storage/'+path+'"]')
            self.assertLessEqual(ui.js("return document.querySelectorAll('.pa-finder-column').length"),2)
            self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-finder-column')].every(c=>c.getBoundingClientRect().width===300)"))
        self.assertEqual(ui.js("return [...document.querySelectorAll('.pa-column-title')].map(n=>n.textContent)"),['session','episode'])
        self.assertTrue(ui.js("return document.querySelector('.pa-finder-path').textContent.includes('datasets')"))
        self.assertTrue(ui.js("return !!document.querySelector('.pa-finder-preview [data-loom-media=metadata-preview]') && !document.querySelector('.pa-finder + .pa-panel')"))
        if folder:=os.environ.get('LOOM_TEST_SCREENSHOTS'):
            out=Path(folder);out.mkdir(parents=True,exist_ok=True)
            ui.top();(out/'explorer-fixed-columns.png').write_bytes(base64.b64decode(self.wd('/screenshot')));ui.frame()
        ui.click('.pa-finder-path [data-column-node="storage/data/0/datasets"]')
        self.assertEqual(ui.js("return [...document.querySelectorAll('.pa-column-title')].map(n=>n.textContent)"),['data/0','datasets'])
        ui.top();ui.js("document.querySelector('iframe').style.width='390px'");ui.frame()
        self.assertTrue(ui.js("return [...document.querySelectorAll('.pa-pane-body')].every(p=>p.scrollWidth<=p.clientWidth+1)"))

    def test_gateway_request_and_verified_private_redirect(self):
        import test_admission as fixture_access
        from admission_http import PublicHandler,ReceiptAuth
        from http.server import ThreadingHTTPServer
        f=fixture_access.AdmissionTests();f.setUp();self.addCleanup(f.doCleanups)
        public=ThreadingHTTPServer(('127.0.0.1',0),PublicHandler)
        origin='http://127.0.0.1:'+str(public.server_port)
        f.a.config.update(origin=origin,dashboard_origin=self.base)
        public.admission=f.a;public.sso=ReceiptAuth(f.a);public.gateway=True
        public.sso.cookie='fixture-gateway-receipt'
        public.sso.set_cookie=lambda name,value,ttl:f'{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl}'
        t=Thread(target=public.serve_forever,kwargs={'poll_interval':.02},daemon=True);t.start()
        self.addCleanup(t.join,2);self.addCleanup(public.server_close);self.addCleanup(public.shutdown)
        self.web.admission=f.a;self.addCleanup(delattr,self.web,'admission')
        self.ui.top();self.wd('/url',{'url':origin})
        self.ui.wait("return !document.querySelector('#request').hidden")
        self.assertEqual(self.ui.js('return location.origin'),origin)
        self.ui.js("document.querySelector('#email').value='requester@example.test'")
        self.ui.click('#request-form button')
        self.ui.wait("return !document.querySelector('#sent').hidden && document.querySelector('.pending').textContent==='Tailscale 공유를 준비하고 있습니다.'")
        with f.a.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM share_requests').fetchone()[0],1)
        self.assertEqual(f.api.calls,[])
        # A request marked ready is not authenticated and cannot itself redirect.
        with f.a.db() as c:c.execute("UPDATE share_requests SET state='ready'")
        self.wd('/refresh',{})
        self.ui.wait("return !document.querySelector('#retry-connection').disabled")
        self.assertEqual(self.ui.js('return location.origin'),origin)
        self.ui.js("window.dispatchEvent(new MessageEvent('message',{origin:arguments[0],source:window,data:{type:'loom-gateway-probe',authorized:true,identity:{login:'forged@test'},destination:'/'}}))",self.base)
        self.assertTrue(self.ui.js("return document.querySelector('#recognized').hidden"))
        class Auth:
            def session(auth,headers):return {'user':{'name':'Verified member','login':'member@example.test'},'csrf':'fixture'}
            def assertion(auth,identity):return 'fixture'
        self.web.sso=Auth();self.addCleanup(setattr,self.web,'sso',None)
        self.wd('/refresh',{})
        self.ui.wait("return document.querySelector('#recognized-name')?.textContent==='Verified member'")
        self.assertTrue(self.ui.js("return !document.querySelector('#continue') && document.querySelector('#request').hidden"))
        self.ui.wait("return location.origin===arguments[0] && !!document.querySelector('#codex-visualization')",self.base)
