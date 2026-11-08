"""Resumable Tailscale enrollment and instance configuration; no implicit policy edits."""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import time
from urllib.parse import urlsplit, quote
from urllib.request import Request, urlopen


def run(*args):
    return subprocess.check_output([str(a) for a in args],text=True,stderr=subprocess.PIPE).strip()


def write(path, value, uid=None, gid=None):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    temp=path.with_name(path.name+'.pending')
    temp.write_text(value if isinstance(value,str) else json.dumps(value,indent=2)+'\n')
    temp.chmod(0o600)
    if uid is not None:os.chown(temp,uid,gid)
    os.replace(temp,path)


def validate(plan):
    required={'tailnet','hostname','tag','https_port','bridge_cidr','users','sso','ssh'}
    if not isinstance(plan,dict) or not required.issubset(plan) or set(plan)-required-{'access_plan'}:raise ValueError('Use tailnet-plan.example.json fields')
    if plan.get('access_plan'):
        if not isinstance(plan['access_plan'],str) or not Path(plan['access_plan']).is_absolute():raise ValueError('Use an absolute access_plan path')
    if not isinstance(plan['tailnet'],str) or not plan['tailnet']:raise ValueError('Expected tailnet identity is required')
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,62}',plan['hostname']):raise ValueError('Invalid hostname')
    if not re.fullmatch(r'tag:[a-z0-9][a-z0-9-]{0,62}',plan['tag']):raise ValueError('Invalid server tag')
    if type(plan['https_port']) is not int or not 1<=plan['https_port']<=65535:raise ValueError('Invalid HTTPS port')
    net=ipaddress.ip_network(plan['bridge_cidr'])
    if net.version!=4 or net.prefixlen!=24 or not any(net.subnet_of(ipaddress.ip_network(block)) for block in ['10.0.0.0/8','172.16.0.0/12','192.168.0.0/16']):raise ValueError('Choose a private /24 Docker subnet')
    if type(plan['ssh']) is not bool or not isinstance(plan['users'],dict):raise ValueError('Invalid users/ssh')
    if plan['sso'] is not None:
        s=plan['sso']
        if set(s)-{'issuer','network_users','link_tailscale_email','session_seconds','provider','mode'}:raise ValueError('Use native Tailscale identity bindings')
        if s.get('issuer'):
            u=urlsplit(s['issuer'])
            if u.scheme!='https' or not u.hostname or u.username or u.password or u.query or u.fragment:raise ValueError('Use HTTPS identity namespace')
        bindings=s.get('network_users',{})
        if not isinstance(bindings,dict) or any('@' not in k or not isinstance(v,dict) or not isinstance(v.get('subject'),str) or not v['subject'] for k,v in bindings.items()):raise ValueError('Invalid native user bindings')
        if type(s.get('link_tailscale_email',True)) is not bool:raise ValueError('Explicit email-link policy required')
        lifetime=s.get('session_seconds',604800)
        if type(lifetime) is not int or not 60<=lifetime<=604800:raise ValueError('Invalid session lifetime')
    return plan


def status():
    raw=json.loads(run('tailscale','status','--json'))
    own=raw.get('Self') or {}
    return {'state':raw.get('BackendState'), 'tailnet':(raw.get('CurrentTailnet') or {}).get('Name'),
            'device_id':own.get('ID'),'dns':own.get('DNSName','').rstrip('.'),
            'tags':own.get('Tags',[]), 'login_url':raw.get('AuthURL') or None}


def enrollment(plan):
    s=status()
    if s['tailnet'] and s['tailnet']!=plan['tailnet']:raise ValueError('Machine belongs to a different tailnet; no automatic logout')
    if s['state']=='Running':
        if plan['tag'] not in s['tags']:
            # Preserve all existing tags. The tailnet must authorize this tag.
            run('tailscale','set','--advertise-tags='+','.join(sorted(set(s['tags']+[plan['tag']]))))
        if plan['ssh']:run('tailscale','set','--ssh=true')
        s=status()
        return {**s,'ready':plan['tag'] in s['tags'],'next':None if plan['tag'] in s['tags'] else 'Authorize the advertised tag in the tailnet policy, then rerun enroll'}
    if s['state']=='NeedsMachineAuth':
        return {**s,'ready':False,'next':'Tailnet administrator must approve this device, then rerun enroll'}
    if s['state'] in ('NeedsLogin','NoState'):
        # A bounded attempt emits login URL; never blocks the agent waiting for a person.
        try:run('tailscale','up','--timeout=5s','--hostname='+plan['hostname'],'--advertise-tags='+plan['tag'])
        except subprocess.CalledProcessError:pass
        return {**status(),'ready':False,'next':'Open login_url as the intended tailnet administrator, then rerun enroll'}
    return {**s,'ready':False,'next':'Start tailscaled or resolve its reported state, then rerun enroll'}


def policy(plan, device_id=None):
    # Common tag discovers Hub resources; never grants a user all Loom machines.
    machine='loom-'+plan['hostname']
    if plan['ssh'] and not device_id:raise ValueError('Inspect the enrolled device ID before generating SSH policy')
    import hashlib
    ssh_tag='tag:loom-machine-'+hashlib.sha256(str(device_id).encode()).hexdigest()[:12]
    return {'tagOwners':{plan['tag']:['group:loom-admins'],**({ssh_tag:['group:loom-admins']} if plan['ssh'] else {})},
            'hosts':{machine:'REPLACE_WITH_THIS_MACHINE_TAILSCALE_IP'},
            'groups':{'group:'+machine+'-users':[]},
            'grants':[{'src':['group:'+machine+'-users'],'dst':[machine],'ip':['tcp:'+str(plan['https_port'])]+(['tcp:22'] if plan['ssh'] else [])}],
            'ssh':[{'action':'check','src':['group:'+machine+'-users'],'dst':[ssh_tag],'users':['REPLACE_WITH_APPROVED_LOCAL_ACCOUNT']} ] if plan['ssh'] else [],
            'note':'tag:loom is Hub discovery only. Resolve this machine IP and define approved users; preserve existing policy. SSH requires assigning its unique device tag to this device only. Automatic enrollment uses the immutable machine/device identity and exact Tailscale addresses.'}


def env_values(path):
    return dict(line.split('=',1) for line in path.read_text().splitlines() if line and not line.startswith('#') and '=' in line)


def configure(root,plan,dns):
    root=Path(root)
    if not re.fullmatch(r'[A-Za-z0-9.-]+',dns) or not dns.endswith('.ts.net'):raise ValueError('Expected MagicDNS hostname')
    if any(p.is_symlink() for p in [root,*root.parents]):raise ValueError('Instance root must not be a symlink')
    uid,gid=root.stat().st_uid,root.stat().st_gid
    api_path=root/'config/api.json'
    api=json.loads(api_path.read_text())
    net=ipaddress.ip_network(plan['bridge_cidr']);gateway=str(net[1]);dashboard=str(net[3])
    network={'services':{'loom-ops':{'networks':{'default':{'ipv4_address':str(net[2])}}},
                         'loom-dashboard':{'networks':{'default':{'ipv4_address':dashboard}}}},
             'networks':{'default':{'ipam':{'config':[{'subnet':str(net),'ip_range':str(list(net.subnets(prefixlen_diff=1))[1])}]}}}}
    suffix='' if plan['https_port']==443 else ':'+str(plan['https_port'])
    origin='https://'+dns+suffix
    api['tailscale_auth']={'enabled':True,'trusted_proxies':[gateway,dashboard],
                          'users':{**api.get('tailscale_auth',{}).get('users',{}),**plan['users']},'allow_new_users':True}
    for actor in api['tailscale_auth']['users'].values():
        if actor not in {a['id'] for a in api['actors']}:raise ValueError('Unknown actor binding')
    if plan['sso']:
        s=plan['sso'];secret=root/'secrets/sso';secret.mkdir(parents=True,exist_ok=True);secret.chmod(0o700);os.chown(secret,uid,gid)
        if not (secret/'relay-key').exists():write(secret/'relay-key',secrets.token_urlsafe(48),uid,gid)
        runtime=root/'dashboard-auth';runtime.mkdir(exist_ok=True);runtime.chmod(0o700);os.chown(runtime,uid,gid)
        config={k:v for k,v in s.items() if k!='link_tailscale_email'}
        previous=json.loads((secret/'config.json').read_text()) if (secret/'config.json').exists() else {}
        issuer=s.get('issuer') or previous.get('issuer') or origin+'/identity'
        if previous.get('issuer') and issuer!=previous['issuer']:raise ValueError('Preserve the existing identity namespace')
        config.update(issuer=issuer,provider='tailscale',mode='network',origin=origin,relay_key_file='/etc/loom/sso/relay-key',session_db='/var/lib/loom-dashboard/sessions.sqlite3')
        write(secret/'config.json',config,uid,gid)
        api['dashboard_auth']={'enabled':True,'issuer':issuer,
            'relay_key_file':'/etc/loom/sso/relay-key','link_tailscale_email':s.get('link_tailscale_email',True)}
        volumes=[{'type':'bind','source':str(secret),'target':'/etc/loom/sso','read_only':True,'bind':{'create_host_path':False}}]
        network['services']['loom-ops']['volumes']=[{'type':'bind','source':str(secret/'relay-key'),'target':'/etc/loom/sso/relay-key','read_only':True,'bind':{'create_host_path':False}}]
        network['services']['loom-dashboard'].update(user=f'{uid}:{gid}',environment={'LOOM_SSO_CONFIG':'/etc/loom/sso/config.json'},
            volumes=volumes+[{'type':'bind','source':str(runtime),'target':'/var/lib/loom-dashboard','bind':{'create_host_path':False}}])
    elif api.get('dashboard_auth',{}).get('enabled'):
        raise ValueError('Existing SSO cannot be disabled implicitly')
    # A single backup set per application, no credentials printed or put in plans.
    backup=root/'network-backups'/str(time.time_ns());backup.mkdir(parents=True,mode=0o700)
    for name in ['config/api.json','network.env','compose-tailnet.yaml','config/tailnet-plan.json']:
        path=root/name
        if path.exists():write(backup/name.replace('/','_'),path.read_text(),uid,gid)
    env=env_values(root/'network.env') if (root/'network.env').exists() else {}
    env['LOOM_TAILSCALE_TRUSTED_PROXIES']=gateway
    # Own overlay is applied after the existing network config; never discard its settings.
    write(root/'compose-tailnet.yaml',network,uid,gid)
    write(root/'network.env',''.join(f'{k}={v}\n' for k,v in env.items()),uid,gid)
    write(api_path,api,uid,gid)
    write(root/'config/tailnet-plan.json',plan,uid,gid)
    return {'origin':origin,'mcp':origin+'/mcp','sso':bool(plan['sso']),'requires_compose_apply':True,'backup':str(backup)}


def expected_routes(root):
    env={**env_values(Path(root)/'compose.env'),**env_values(Path(root)/'network.env')};web='http://127.0.0.1:'+env['OPS_DASHBOARD_PORT'];api='http://127.0.0.1:'+env['OPS_PORT']
    routes={'/':web,'/api':api+'/api','/mcp':api+'/mcp','/auth':web+'/auth','/session':web+'/session'}
    for name in ['dashboard','docker','history','jobs','mcp','ops','principal','registry','status','guide']:
        routes['/api/'+name]=web+'/api/'+name
    return routes


def serve(root,plan,dns):
    routes=expected_routes(root)
    previous=json.loads(run('tailscale','serve','status','--json'))
    host=dns+':'+str(plan['https_port']);handlers=previous.get('Web',{}).get(host,{}).get('Handlers',{})
    for path,target in routes.items():
        if path in handlers and handlers[path].get('Proxy')!=target:
            raise ValueError('Serve route conflict at '+path+'; reconcile explicitly')
    write(Path(root)/'config/serve-before.json',previous)
    for path,target in routes.items():run('tailscale','serve','--bg','--https='+str(plan['https_port']),'--set-path='+path,target)
    return {'state':'configured','origin':'https://'+host,'routes':list(routes)}


def approve(device,credential_file):
    if not re.fullmatch(r'[A-Za-z0-9_-]+',device):raise ValueError('Invalid device ID')
    token=Path(credential_file).read_text().strip()
    req=Request('https://api.tailscale.com/api/v2/device/'+quote(device)+'/authorized',
        data=b'{"authorized":true}',headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'},method='POST')
    with urlopen(req,timeout=15) as response:response.read(1024*1024)
    return {'device_id':device,'approved':True}


def install_client():
    """Ubuntu/Debian official apt repository, with its own scoped signing key."""
    release=env_values(Path('/etc/os-release'));distro=release['ID'].strip('"');codename=release.get('VERSION_CODENAME','').strip('"')
    if distro not in ('ubuntu','debian') or not re.fullmatch('[a-z]+',codename):raise ValueError('Install Tailscale using the official instructions for this OS')
    run('apt-get','update');run('apt-get','install','-y','ca-certificates','curl')
    base=f'https://pkgs.tailscale.com/stable/{distro}/{codename}'
    for suffix,dest in [('.noarmor.gpg','/usr/share/keyrings/tailscale-archive-keyring.gpg'),('.tailscale-keyring.list','/etc/apt/sources.list.d/tailscale.list')]:
        with urlopen(base+suffix,timeout=20) as response:data=response.read(1024*1024)
        Path(dest).write_bytes(data);Path(dest).chmod(0o644)
    run('apt-get','update');run('apt-get','install','-y','tailscale');run('systemctl','enable','--now','tailscaled')
    return {'installed':True}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['inspect','install-client','enroll','policy','configure','serve','approve'])
    p.add_argument('--plan',type=Path);p.add_argument('--root',type=Path,default=Path('/loom/ops'))
    p.add_argument('--device');p.add_argument('--credential-file',type=Path);a=p.parse_args()
    try:
        if a.action=='inspect':result=status()
        elif a.action=='install-client':result=install_client()
        elif a.action=='approve':result=approve(a.device,a.credential_file)
        else:
            plan=validate(json.loads(a.plan.read_text()))
            if a.action=='enroll':
                result=enrollment(plan)
                write(a.plan.with_suffix('.progress.json'),result)
            elif a.action=='policy':result=policy(plan,status()['device_id'] if plan['ssh'] else None)
            else:
                current=status()
                if current['state']!='Running' or current['tailnet']!=plan['tailnet']:raise ValueError('Join the expected tailnet first')
                result=configure(a.root,plan,current['dns']) if a.action=='configure' else serve(a.root,plan,current['dns'])
        print(json.dumps(result,indent=2))
    except Exception as e:
        # subprocess stderr and API response bodies can contain credentials.
        p.exit(1,type(e).__name__+(': '+str(e) if isinstance(e,ValueError) else ': operation failed; inspect local service state')+'\n')
