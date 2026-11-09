"""Install the public access-request listener and private approval worker."""
import argparse
import json
import os
from pathlib import Path
import re
import sys
import time
from urllib.parse import urlsplit
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'dashboard'))
from admission import validate, TailnetAPI, machine_tag, RemoteError
from tailnet import run,write
import managed_resources


def load(plan):
    c=json.loads(Path(plan).read_text())
    c.setdefault('enrollment_mode','sharing')
    validate(c)
    if c.get('bind') not in ('0.0.0.0','127.0.0.1'):raise ValueError('invalid_container_bind')
    if type(c.get('host_port')) is not int or not 1024<=c['host_port']<=65535:raise ValueError('invalid_host_port')
    if c.get('port')!=8081:raise ValueError('public_container_port_must_be_8081')
    for p in [c['credential_file']]:
        path=Path(p)
        if not path.is_absolute() or not path.is_file() or not path.read_bytes().strip():raise ValueError('private_credential_file_required')
    credential=json.loads(Path(c['credential_file']).read_text())
    fields={'api':{'type','token'},'oauth':{'type','client_id','client_secret'}}.get(credential.get('type'))
    if not fields or set(credential)!=fields or not all(credential.values()):raise ValueError('invalid_management_credential')
    if c['enrollment_mode']=='sharing' and credential['type']!='api':raise ValueError('sharing_requires_user_api_access_token')
    return c


def inspect(c):
    api=TailnetAPI(c['credential_file'])
    device,_=api.call('GET','/device/'+c['device_id'])
    if not set(c['addresses']).issubset(device.get('addresses',[])):raise ValueError('machine_identity_changed')
    from urllib.parse import quote
    base='/tailnet/'+quote(c['tailnet'],safe='')
    if json.loads(Path(c['credential_file']).read_text()).get('type')!='api':raise ValueError('sharing_requires_user_api_access_token')
    api.call('GET','/device/'+c['device_id']+'/device-invites')
    members,_=api.call('GET',base+'/users?type=member')
    if not isinstance(members,dict) or not isinstance(members.get('users'),list):raise ValueError('membership_check_unavailable')
    policy,_=api.call('GET',base+'/acl')
    from sharing import pending_policy
    pending_policy(policy,{**c,'machine_tags':device.get('tags',[])})
    return {'read_access_verified':True,'write_access_verified':False,'machine':c['machine_id'],
            'hub_tag_present':'tag:loom' in device.get('tags',[])}


def configure(root, c):
    root=Path(root);uid,gid=root.stat().st_uid,root.stat().st_gid
    if not (root/'secrets/sso/config.json').is_file():raise ValueError('private_dashboard_sso_required')
    for p in [root,*root.parents]:
        if p.is_symlink():raise ValueError('instance_symlink_not_supported')
    secret=root/'secrets/admission';state=root/'admission'
    for p in [secret,state]:p.mkdir(parents=True,exist_ok=True);p.chmod(0o700);os.chown(p,uid,gid)
    if c.get('managed_resources_version'):
        managed_resources.mutation({**c,'database':str(state/'requests.sqlite3')},{},{},lambda:None)
    for name in ('managed-resources.json','managed-resources.lock'):
        if (state/name).exists():os.chown(state/name,uid,gid)
    backup=root/'network-backups'/('admission-'+str(time.time_ns()));backup.mkdir(parents=True,mode=0o700)
    for path in [secret/'config.json',root/'compose-admission.yaml']:
        if path.exists():write(backup/path.name,path.read_text(),uid,gid)
    runtime=json.loads(json.dumps(c))
    runtime['native_logins']=sorted(json.loads((root/'secrets/sso/config.json').read_text()).get('network_users',{}))
    runtime['separate_gateway']=True
    runtime['credential_file']='/etc/loom/admission/management.json'
    runtime['database']='/var/lib/loom-admission/requests.sqlite3'
    write(secret/'management.json',Path(c['credential_file']).read_text(),uid,gid)
    write(secret/'config.json',runtime,uid,gid)
    gateway=json.loads(json.dumps(runtime))
    gateway['credential_file']='/not-mounted/management.json'
    write(secret/'gateway.json',gateway,uid,gid)
    diagnostics=root/'diagnostics';diagnostics.mkdir(mode=0o700,exist_ok=True);os.chown(diagnostics,uid,gid);diagnostics.chmod(0o700)
    mounts=[{'type':'bind','source':str(diagnostics),'target':'/var/lib/loom-diagnostics','bind':{'create_host_path':False}}, {'type':'bind','source':str(state),'target':'/var/lib/loom-admission','bind':{'create_host_path':False}}]
    gateway_service={
        'image':'${LOOM_DASHBOARD_IMAGE:-loom-dashboard:dev-build}',
        'entrypoint':['python3','-B','/opt/loom-dashboard/gateway.py'],
        'depends_on':['loom-dashboard'],'user':f'{uid}:{gid}','init':True,'read_only':True,'restart':'unless-stopped',
        'cap_drop':['ALL'],'security_opt':['no-new-privileges:true'],
        'environment':{'LOOM_DIAGNOSTICS_DIR':'/var/lib/loom-diagnostics','LOOM_GATEWAY_CONFIG':'/etc/loom/gateway.json'},
        'ports':[f"127.0.0.1:{c['host_port']}:8081"],
        'volumes':mounts+[{'type':'bind','source':str(secret/'gateway.json'),'target':'/etc/loom/gateway.json','read_only':True,'bind':{'create_host_path':False}}],
        'healthcheck':{'test':['CMD','python3','-c',"from urllib.request import urlopen; urlopen('http://127.0.0.1:8081/healthz',timeout=2).close()"],'interval':'15s','timeout':'3s','retries':3}}
    if (root/'development').exists():
        gateway_service['entrypoint']=['python3','-B','/opt/loom-dashboard/dev.py']
        gateway_service['environment']['LOOM_UI_ROLE']='gateway'
        gateway_service['volumes'].append({'type':'bind','source':str(Path(__file__).resolve().parents[1]/'dashboard'),'target':'/opt/loom-dashboard','read_only':True})
    overlay={'services':{'loom-dashboard':{
        'environment':{'LOOM_DIAGNOSTICS_DIR':'/var/lib/loom-diagnostics','LOOM_ADMISSION_CONFIG':'/etc/loom/admission/config.json'},
        'volumes':[{'type':'bind','source':str(secret),'target':'/etc/loom/admission','read_only':True,'bind':{'create_host_path':False}},*mounts]},
        'loom-gateway':gateway_service}}
    write(root/'compose-admission.yaml',overlay,uid,gid)
    write(root/'config/access-plan.json',c,uid,gid)
    return {'configured':True,'public_origin':c['origin'],'private_admin':c['dashboard_origin']+'/#/views/tailscale',
            'requires_compose_apply':True,'published':False}


def register_machine(c):
    api=TailnetAPI(c['credential_file'])
    device,_=api.call('GET','/device/'+c['device_id'])
    if not set(c['addresses']).issubset(device.get('addresses',[])):raise ValueError('machine_identity_changed')
    from urllib.parse import quote
    policy_path='/tailnet/'+quote(c['tailnet'],safe='')+'/acl'
    policy,headers=api.call('GET',policy_path)
    before=json.loads(json.dumps(policy))
    required_tags=['tag:loom',machine_tag(c)]
    if any(tag not in policy.get('tagOwners',{}) for tag in required_tags):
        etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
        if not etag:raise ValueError('policy_etag_required')
        for tag in required_tags:policy.setdefault('tagOwners',{}).setdefault(tag,['autogroup:admin'])
        result,_=api.call('POST',policy_path+'/validate',policy)
        if result.get('errors') or result.get('message'):raise ValueError('tag_policy_validation_failed')
        managed_resources.policy_write(c,api,policy_path,before,policy,{'If-Match':etag})
    tags=sorted(set(device.get('tags',[])+required_tags))
    managed_resources.tags_write(c,api,device.get('tags',[]),tags)
    return {'hub_tag':'tag:loom','user_access_granted':False}


def private_access(c, logins):
    """Grant only explicitly configured native users this machine's private port."""
    from urllib.parse import quote
    if not logins or any(not re.fullmatch(r'[^\s@]+@[^\s@]+',v) for v in logins):raise ValueError('explicit_native_users_required')
    api=TailnetAPI(c['credential_file']);path='/tailnet/'+quote(c['tailnet'],safe='')+'/acl'
    grant={'src':sorted(logins),'dst':c['addresses'],'ip':['tcp:'+str(p) for p in sorted(c['ports'])]}
    for _ in range(3):
        policy,headers=api.call('GET',path)
        if grant in policy.get('grants',[]):return {'private_grant_changed':False}
        from sharing import pending_policy
        device,_=api.call('GET','/device/'+c['device_id'])
        if not set(c['addresses']).issubset(device.get('addresses',[])):raise ValueError('machine_identity_changed')
        pending_policy(policy,{**c,'machine_tags':device.get('tags',[])})
        etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
        if not etag:raise ValueError('policy_etag_required')
        before=json.loads(json.dumps(policy))
        policy.setdefault('grants',[]).append(grant)
        result,_=api.call('POST',path+'/validate',policy)
        if result.get('errors') or result.get('message'):raise ValueError('private_policy_validation_failed')
        try:
            managed_resources.policy_write(c,api,path,before,policy,{'If-Match':etag})
            return {'private_grant_changed':True}
        except RemoteError as e:
            if e.status!=412:raise
    raise ValueError('policy_concurrent_change')


def gateway_access(c):
    """Keep the public gateway reachable when MagicDNS selects the VPN path."""
    from sharing import public_gateway_port
    from urllib.parse import quote
    port=public_gateway_port(c)
    if port is None:return {'gateway_grant_changed':False}
    api=TailnetAPI(c['credential_file'])
    device,_=api.call('GET','/device/'+c['device_id'])
    if not set(c['addresses']).issubset(device.get('addresses',[])):raise ValueError('machine_identity_changed')
    path='/tailnet/'+quote(c['tailnet'],safe='')+'/acl'
    grant={'src':['autogroup:member','autogroup:shared'],'dst':c['addresses'],'ip':['tcp:'+str(port)]}
    for _ in range(3):
        policy,headers=api.call('GET',path)
        if grant in policy.get('grants',[]):return {'gateway_grant_changed':False}
        etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
        if not etag:raise ValueError('policy_etag_required')
        before=json.loads(json.dumps(policy))
        policy.setdefault('grants',[]).append(grant)
        result,_=api.call('POST',path+'/validate',policy)
        if result.get('errors') or result.get('message'):raise ValueError('gateway_policy_validation_failed')
        try:
            managed_resources.policy_write(c,api,path,before,policy,{'If-Match':etag})
            return {'gateway_grant_changed':True}
        except RemoteError as e:
            if e.status!=412:raise
    raise ValueError('policy_concurrent_change')


def funnel_capability(c):
    """Own only an exact machine-tag capability, never a broad member exception."""
    from urllib.parse import quote
    api=TailnetAPI(c['credential_file']);path='/tailnet/'+quote(c['tailnet'],safe='')+'/acl'
    grant={'target':[machine_tag(c)],'attr':['funnel']}
    for _ in range(3):
        policy,headers=api.call('GET',path)
        if grant in policy.get('nodeAttrs',[]):return {'funnel_capability_changed':False}
        before=json.loads(json.dumps(policy));policy.setdefault('nodeAttrs',[]).append(grant)
        etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
        if not etag:raise ValueError('policy_etag_required')
        result,_=api.call('POST',path+'/validate',policy)
        if result.get('errors') or result.get('message'):raise ValueError('funnel_policy_validation_failed')
        try:
            managed_resources.policy_write(c,api,path,before,policy,{'If-Match':etag})
            return {'funnel_capability_changed':True}
        except RemoteError as e:
            if e.status!=412:raise
    raise ValueError('policy_concurrent_change')


def publish(c):
    u=urlsplit(c['origin']);port=u.port or 443
    if port not in (443,8443,10000):raise ValueError('funnel_port_not_supported')
    own=json.loads(run('tailscale','status','--json'))
    if own.get('Self',{}).get('DNSName','').rstrip('.')!=u.hostname:raise ValueError('funnel_machine_hostname_mismatch')
    status=json.loads(run('tailscale','serve','status','--json'))
    target='http://127.0.0.1:'+str(c['host_port'])
    handlers=status.get('Web',{}).get(u.hostname+':'+str(port),{}).get('Handlers',{})
    if handlers and handlers!={'/':{'Proxy':target}}:raise ValueError('dedicated_public_port_required')
    # Never publish the operational Dashboard port or an unready target.
    from urllib.request import urlopen
    try:
        with urlopen(target+'/api/session',timeout=5) as r:session=json.load(r)
    except Exception:raise ValueError('request_listener_not_ready') from None
    if session.get('machine')!=c['machine_id']:raise ValueError('wrong_public_listener')
    for path in ('/api/status','/mcp','/access-admin'):
        from urllib.error import HTTPError
        try:
            with urlopen(target+path,timeout=5) as r:pass
        except HTTPError as e:
            if e.code==404:continue
        raise ValueError('public_listener_exposes_private_routes')
    funnel_capability(c)
    run('tailscale','funnel','--bg','--https='+str(port),target)
    return {'published':c['origin'],'scope':'request_listener_only',**gateway_access(c)}


def save_credential(path):
    import getpass
    path=Path(path)
    if not path.is_absolute() or any(p.is_symlink() for p in [path,*path.parents]):raise ValueError('absolute_private_file_required')
    if path.exists():raise ValueError('credential_exists_choose_new_path')
    token=getpass.getpass('Tailscale API access token (hidden): ').strip()
    if not token.startswith('tskey-api-') or any(c.isspace() for c in token):raise ValueError('api_access_token_required_not_auth_key')
    path.parent.mkdir(parents=True,exist_ok=True);path.parent.chmod(0o700)
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'w') as f:json.dump({'type':'api','token':token},f)
    return {'credential_file':str(path),'stored':True}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['credential','check','inspect','configure','register-machine','publish','reconcile-resources'])
    p.add_argument('--plan',type=Path);p.add_argument('--root',type=Path,default=Path('/loom/ops'))
    p.add_argument('--output',type=Path,default=Path('/loom/private-inputs/tailscale-management.json'));a=p.parse_args()
    try:
        if a.action=='credential':
            print(json.dumps(save_credential(a.output)));raise SystemExit(0)
        if not a.plan:raise ValueError('plan_required')
        c=load(a.plan)
        if a.action=='reconcile-resources':
            from urllib.parse import quote
            api=TailnetAPI(c['credential_file']);policy,_=api.call('GET','/tailnet/'+quote(c['tailnet'],safe='')+'/acl');device,_=api.call('GET','/device/'+quote(c['device_id'],safe=''))
            print(json.dumps(managed_resources.reconcile(c,policy,device.get('tags',[]))));raise SystemExit(0)
        result={'valid':True} if a.action=='check' else inspect(c) if a.action=='inspect' else configure(a.root,c) if a.action=='configure' else register_machine(c) if a.action=='register-machine' else publish(c)
        print(json.dumps(result,indent=2))
    except Exception as e:
        p.exit(1,(str(e) if isinstance(e,ValueError) else type(e).__name__+': check private configuration and permissions')+'\n')
