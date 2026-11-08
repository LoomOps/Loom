"""Read-only installation diagnostics. Receipts never substitute for live checks."""
import json
import time
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

STAGES = ('plan', 'host', 'images', 'configuration', 'services', 'network', 'acceptance')
STEPS = {name: f'steps/{i:02d}-{name}.md' for i, name in enumerate(STAGES, 1)}
STEPS['acceptance']='steps/07-handoff.md'
SERVICES = ('loom-ops', 'loom-dashboard', 'loom-gateway')


def request(url, body=None):
    req = Request(url, data=json.dumps(body).encode() if body else None,
                  headers={'Content-Type': 'application/json'})
    try:
        with urlopen(req, timeout=8) as response:
            raw = response.read(2 * 1024 * 1024)
            try: value = json.loads(raw)
            except (ValueError, UnicodeDecodeError): value = None
            return response.status, value
    except HTTPError as error:
        return error.code, None


def service_plan(plan):
    """There is exactly one supported new-installation service contract."""
    from tailnet import validate
    tail = plan.get('tailscale')
    if not isinstance(tail, dict):
        raise ValueError('Full Tailscale/SSO/Sharing plan required; boolean installation modes are retired')
    validate(tail)
    sso = tail.get('sso') or {}
    if not sso.get('network_users') or not tail.get('access_plan'):
        raise ValueError('Explicit Admin user bindings and Sharing Gateway are required')
    if sso.get('provider', 'tailscale') != 'tailscale' or sso.get('mode', 'network') != 'network':
        raise ValueError('Native Tailscale identity is required')
    from admission import validate as validate_access
    access = json.loads(Path(tail['access_plan']).read_text())
    validate_access(access)
    private = urlsplit(access['dashboard_origin']); public = urlsplit(access['origin'])
    if (private.hostname != public.hostname or private.hostname.split('.')[0] != tail['hostname']
        or (private.port or 443) != tail['https_port'] or (public.port or 443) not in (443,8443,10000)
        or (private.port or 443) == (public.port or 443) or access['tailnet'] != tail['tailnet']):
        raise ValueError('Gateway and private Dashboard must use separate HTTPS ports on this machine')
    if access.get('enrollment_mode', 'sharing') != 'sharing' or set(access['ports']) != {tail['https_port']}:
        raise ValueError('Share only this machine private service port')
    issuer = sso.get('issuer') or access['dashboard_origin'] + '/identity'
    bindings = {v['subject'] for v in sso['network_users'].values()}
    if any(a['issuer'] != issuer or a['subject'] not in bindings for a in access['admins']):
        raise ValueError('Every Admin must match an explicit native user identity')
    ports = [plan[k] for k in ('api_port','dashboard_port','tls_port')] + [access.get('host_port')]
    if any(type(p) is not int or not 1024 <= p <= 65535 for p in ports) or len(set(ports)) != 4:
        raise ValueError('Choose four distinct local service ports')
    if access.get('port') != 8081 or access.get('bind') != '0.0.0.0':
        raise ValueError('Gateway requires its dedicated container listener')
    credential = Path(access['credential_file'])
    if not credential.is_absolute(): raise ValueError('Use an absolute protected credential path')
    if any('REPLACE_' in str(v) for v in (access['machine_id'], access['device_id'])):
        raise ValueError('Resolve installation and Tailscale device identities before applying')
    return access


def machine_matches(plan, raw):
    access=service_plan(plan);own=raw.get('Self') or {}
    return (raw.get('BackendState')=='Running'
            and (raw.get('CurrentTailnet') or {}).get('Name')==plan['tailscale']['tailnet']
            and own.get('ID')==access['device_id']
            and own.get('DNSName','').rstrip('.')==urlsplit(access['dashboard_origin']).hostname
            and set(access['addresses']).issubset(own.get('TailscaleIPs',[])))


def recommendations(info):
    tools = info['tools']
    return [
        {'topic': 'services', 'recommendation': 'Install Ops, Dashboard, Gateway, native Tailscale identity and Sharing together. No token-only fallback.'},
        {'topic': 'gpu', 'recommendation': 'Enable GPU after container exposure verification.' if tools.get('nvidia-smi') else 'Use CPU unless GPU hardware and a compatible host driver are confirmed.'},
        {'topic': 'storage', 'recommendation': 'Prefer an existing mounted data volume; choose a new output subdirectory. Inventory every intended store; never format or select all disks automatically.'},
        {'topic': 'dependencies', 'recommendation': 'Use /dependencies for selected editable sources; research runtimes belong to Job profiles, not the host installer.'},
        {'topic': 'identity', 'recommendation': 'Confirm the human Admin email; do not infer it from the OS account or a tagged device owner. Store management credentials only in a protected local file.'},
        {'topic': 'handoff', 'recommendation': 'Lead with the verified public Gateway URL. Include private Dashboard/MCP, storage, service version and verification limits; never print an owner token.'},
    ]


def doctor(setup, plan, stage=None):
    """Accumulate independent failures; never repair, enroll, submit Jobs or grant access."""
    root = setup.ROOT; checks = []
    def check(group, name, action, remedy):
        if stage and group != stage: return
        try:
            value = action()
            if value is False: raise ValueError('Observed state differs from installation contract')
            checks.append({'stage': group, 'check': name, 'status': 'pass'})
        except Exception as error:
            # Never echo subprocess stderr, credentials or provider response bodies.
            checks.append({'stage': group, 'check': name, 'status': 'fail',
                           'error': type(error).__name__, 'next': remedy})
    def read(name): return json.loads((root/name).read_text())
    def access(): return service_plan(plan)
    def config():
        a = access(); s = read('secrets/sso/config.json'); api = read('config/api.json')
        c = read('secrets/admission/config.json'); gateway = read('secrets/admission/gateway.json')
        if not (api.get('dashboard_auth',{}).get('enabled') and api.get('tailscale_auth',{}).get('enabled')): return False
        if s.get('provider') != 'tailscale' or s.get('mode') != 'network' or s.get('origin') != a['dashboard_origin']: return False
        if s.get('network_users') != plan['tailscale']['sso']['network_users']: return False
        if s.get('issuer') != a['admins'][0]['issuer'] or api['dashboard_auth'].get('issuer') != s['issuer']: return False
        if c.get('admins') != a['admins'] or c.get('origin') != a['origin'] or not c.get('separate_gateway'): return False
        if c.get('enrollment_mode') != 'sharing' or c.get('machine_id') != api.get('machine_id'): return False
        if gateway.get('credential_file') != '/not-mounted/management.json': return False
        if api.get('machine_name') != a['name']: return False
        from tailnet import env_values
        return env_values(root/'build.env').get('LOOM_DEV_NO_AUTH') == '0'
    def stores():
        paths = [plan['data'], *plan.get('additional_data', [])]
        if not all(Path(p).is_dir() for p in paths): return False
        if plan['mount'] and setup.run('findmnt','-rn','-o','UUID','--mountpoint',plan['mount']['target']).lower() != plan['mount']['uuid'].lower(): return False
        return read('config/storage-plan.json') == {'primary': paths[0], 'read_only': paths[1:]}
    def container(service):
        ids = setup.run('docker','ps','-aq','--filter','label=com.docker.compose.project=loom',
                        '--filter','label=com.docker.compose.service='+service).split()
        if len(ids) != 1: return False
        c = json.loads(setup.run('docker','inspect',ids[0]))[0]
        if not c['State'].get('Running') or c['State'].get('Health',{}).get('Status','healthy') != 'healthy': return False
        expected = setup.manifest()['images']['ops' if service=='loom-ops' else 'dashboard']['id']
        if c['Image'] != expected or 'LOOM_DEV_NO_AUTH=1' in c['Config'].get('Env',[]): return False
        mounts = {m['Destination']: m for m in c['Mounts']}
        expected_mounts = {
            'loom-ops': {'/etc/loom/api.json':root/'config/api.json','/etc/loom/sso/relay-key':root/'secrets/sso/relay-key','/run/loom-observations':root.parent/'shared/observations/loom'},
            'loom-dashboard': {'/etc/loom/sso':root/'secrets/sso','/etc/loom/admission':root/'secrets/admission'},
            'loom-gateway': {'/etc/loom/gateway.json':root/'secrets/admission/gateway.json','/var/lib/loom-admission':root/'admission'},
        }[service]
        if any(mounts.get(k,{}).get('Source') != str(v) for k,v in expected_mounts.items()):return False
        if service=='loom-gateway' and set(mounts)!=set(expected_mounts):return False
        bindings=c.get('HostConfig',{}).get('PortBindings',{})
        expected_ports={'loom-ops':{'8080/tcp':plan['api_port'],'8443/tcp':plan['tls_port']},
                        'loom-dashboard':{'8080/tcp':plan['dashboard_port']},
                        'loom-gateway':{'8081/tcp':access()['host_port']}}[service]
        if bindings!={k:[{'HostIp':'127.0.0.1','HostPort':str(v)}] for k,v in expected_ports.items()}:return False
        if service == 'loom-ops':
            for i,p in enumerate([plan['data'],*plan.get('additional_data',[])]):
                m = mounts.get('/loom/data/'+str(i), {})
                if m.get('Source') != p or m.get('RW') is not False: return False
            paths=['/loom/data/'+str(i) for i in range(1+len(plan.get('additional_data',[])))]
            code='import os,sys; sys.exit(0 if all(os.access(p,os.R_OK|os.X_OK) for p in sys.argv[1:]) else 1)'
            setup.run('docker','exec',ids[0],'python3','-c',code,*paths)
        if service == 'loom-gateway':
            if any(m['Destination'].startswith('/etc/loom/admission') or 'relay-key' in m['Destination'] for m in c['Mounts']): return False
        return True
    def routes():
        a = access(); u = urlsplit(a['origin']); d = urlsplit(a['dashboard_origin'])
        state = json.loads(setup.run('tailscale','serve','status','--json'))
        public = f'{u.hostname}:{u.port or 443}'; private = f'{d.hostname}:{d.port or 443}'
        if state.get('Web',{}).get(public,{}).get('Handlers') != {'/':{'Proxy':f"http://127.0.0.1:{a['host_port']}"}}: return False
        handlers = state.get('Web',{}).get(private,{}).get('Handlers',{})
        from tailnet import expected_routes
        if any(handlers.get(k,{}).get('Proxy') != v for k,v in expected_routes(root).items()): return False
        return state.get('AllowFunnel',{}).get(public) is True and not state.get('AllowFunnel',{}).get(private,False)
    def policy():
        from access_setup import TailnetAPI
        from urllib.parse import quote
        from sharing import pending_policy
        a = access(); api = TailnetAPI(a['credential_file'])
        device,_ = api.call('GET','/device/'+a['device_id'])
        if not set(a['addresses']).issubset(device.get('addresses',[])): return False
        p,_ = api.call('GET','/tailnet/'+quote(a['tailnet'],safe='')+'/acl')
        pending_policy(p,{**a,'machine_tags':device.get('tags',[])})
        port = str(urlsplit(a['origin']).port or 443)
        grant = {'src':['autogroup:member','autogroup:shared'],'dst':a['addresses'],'ip':['tcp:'+port]}
        private_grant={'src':sorted(plan['tailscale']['sso']['network_users']),'dst':a['addresses'],'ip':['tcp:'+str(port) for port in sorted(a['ports'])]}
        return grant in p.get('grants',[]) and private_grant in p.get('grants',[])
    def gateway():
        a = access(); code,s = request(a['origin']+'/api/session')
        return code==200 and s.get('machine')==a['machine_id']
    check('plan','full_service_contract',lambda: (setup.validate(plan),access()),'Regenerate the complete plan; read steps/01-plan.md.')
    check('host','host_tools',lambda: all(setup.shutil.which(t) for t in setup.TOOLS+['tailscale']),'Install missing host packages; read steps/02-host.md.')
    check('host','tailnet_identity',lambda: machine_matches(plan,json.loads(setup.run('tailscale','status','--json'))),'Complete enrollment into the planned tailnet.')
    check('host','management_access',lambda: __import__('access_setup').inspect(__import__('access_setup').load(plan['tailscale']['access_plan'])),'Check the protected management credential and machine policy.')
    def images():
        m=setup.manifest()
        return all(setup.run('docker','image','inspect',x['tag'],'--format','{{.Id}}')==x['id'] for x in m['images'].values())
    check('images','immutable_images',images,'Build/load the complete matching installation kit; read steps/03-images.md.')
    check('configuration','identity_gateway_config',config,'Apply the SSO/Sharing configuration stage; no token-only fallback.')
    check('configuration','storage_paths',stores,'Reconcile the selected storage paths and generated mounts.')
    check('configuration','executor_config',lambda: read('config/api.json').get('executor',{}).get('host_data')==plan['data'],'Attach the executor to the selected primary data directory.')
    for svc in SERVICES:
        check('services',svc,lambda svc=svc: container(svc),'Reconcile the matching Compose service; read steps/05-services.md.')
    check('services','local_api_mcp_executor',lambda: setup.verify_core(plan)['build']==setup.manifest()['build_id'],'Inspect service logs; do not replace failed checks with a manual PASS.')
    def observations():
        folder=root.parent/'shared/observations/loom'
        for name in ('docker-status.json','dashboard-gpus.json','dashboard-host-metrics.json'):
            p=folder/name
            if not 0<=time.time()-p.stat().st_mtime<180:return False
            if not isinstance(json.loads(p.read_text()),dict):return False
        return True
    check('services','observations_fresh',observations,'Inspect the collector logs and observation mount; the timer alone does not prove data freshness.')
    check('services','observations_timer',lambda: setup.run('systemctl','is-active','loom-observations.timer')=='active' and setup.run('systemctl','is-enabled','loom-observations.timer')=='enabled','Restore the host observation collector and timer.')
    check('services','startup_enabled',lambda: all(setup.run('systemctl','is-enabled',s)=='enabled' for s in ['loom-stack','loom-executor']),'Enable both startup units.')
    check('services','dashboard_native_entry',lambda: request(f"http://127.0.0.1:{plan['dashboard_port']}/session")[1].get('sso') is True,'Configure Dashboard native identity and apply Compose.')
    check('network','serve_funnel_isolation',routes,'Apply private Serve routes and the separate public Gateway publication.')
    check('network','sharing_policy',policy,'Reconcile exact-machine Gateway grants without broadening private access.')
    check('network','gateway_https',gateway,'Check Gateway HTTPS and the effective public URL.')
    for path in ['/api/status','/mcp','/access-admin']:
        check('network','gateway_blocks_'+path,lambda path=path: request(access()['origin']+path)[0]==404,'Remove private routes from the public Gateway.')
    if plan.get('gpu'):
        check('services','gpu_container',lambda: setup.run('docker','run','--rm','--network','none','--gpus','all','--entrypoint','nvidia-smi',setup.manifest()['images']['python']['id'],'-L'),'Repair container GPU visibility before completion.')
    check('acceptance','cpu_job_result',lambda: __import__('smoke').verify(setup,plan),'Run setup.py smoke with this plan; preserve and inspect any failed Job.')
    stages = {s: ('pass' if all(c['status']=='pass' for c in checks if c['stage']==s) else 'fail') for s in STAGES if not stage or s==stage}
    ready = bool(checks) and all(c['status']=='pass' for c in checks)
    return {'schema':'loom-install-doctor/v1','ready':ready,'scope':stage or 'all', 'stages':stages,'checks':checks,
            'next_document':'steps/07-handoff.md' if ready and not stage else STEPS.get(next((s for s,v in stages.items() if v=='fail'),stage or 'plan')),
            're_read':'INSTALL.md', 'live_validation':['Other-client browser identity/approval and actual reboot are separate checks; this report does not claim them.']}


def handoff(setup, plan):
    result = doctor(setup, plan)
    if not result['ready']: return {'status':'incomplete','doctor':result}
    a = service_plan(plan)
    return {'status':'services_ready','gateway':a['origin'],'dashboard':a['dashboard_origin'],
            'mcp':a['dashboard_origin']+'/mcp','build':setup.manifest()['build_id'],
            'services':list(SERVICES),'executor':'host systemd service','gpu_enabled':plan['gpu'],
            'storage':{'primary':plan['data'],'read_only':plan.get('additional_data',[])},
            'dependencies':plan['dependencies'], 'admins':a['admins'], 'doctor':result,
            'updates':setup.read_api(plan,'/api/status').get('updates'),
            'remaining_validation':result['live_validation'],
            'credentials':'Protected local files only. Do not print tokens or use the owner token as the user login instructions.'}
