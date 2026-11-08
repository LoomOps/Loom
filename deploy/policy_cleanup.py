"""Repair reviewed orphan policy after an installation was removed without its journal."""
import argparse,copy,json,sys
from pathlib import Path
from urllib.parse import quote
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'dashboard'))
from admission import TailnetAPI
from tailnet import write


def candidate(policy,addresses,group=None,retired_idp_addresses=()):
    new=copy.deepcopy(policy)
    if group:
        if not group.startswith('group:loom-access-') or policy.get('groups',{}).get(group)!=[]:
            raise ValueError('Orphan repair only accepts an explicitly selected empty Loom access group')
        for section,value in policy.items():
            if section not in ('groups','grants') and group in json.dumps(value):raise ValueError('Group has other policy references')
        for name,members in policy.get('groups',{}).items():
            if group in members:raise ValueError('Group has nested references')
        matched=[g for g in policy.get('grants',[]) if group in g.get('src',[])]
        if not matched or any(g.get('src')!=[group] or set(g.get('dst',[]))!=set(addresses) or set(g)!={'src','dst','ip'} or not set(g['ip']).issubset({'tcp:443','tcp:8443'}) for g in matched):
            raise ValueError('Selected group does not exclusively match this retired Loom service')
        new['grants']=[g for g in new.get('grants',[]) if g not in matched];new['groups'].pop(group)
    if retired_idp_addresses:
        # Only the obsolete IdP application capability; no device/SSH/IP permissions.
        new['grants']=[g for g in new.get('grants',[]) if not (set(g)=={'src','dst','app'} and set(g['dst'])<=set(retired_idp_addresses) and g['dst'] and set(g['app'])=={'tailscale.com/cap/tsidp'})]
    return new


def plan(api,tailnet,device,group=None,retired_idp=None):
    machine,_=api.call('GET','/device/'+quote(device,safe=''))
    idp,_=api.call('GET','/device/'+quote(retired_idp,safe='')) if retired_idp else ({}, {})
    url='/tailnet/'+quote(tailnet,safe='')+'/acl'
    old,headers=api.call('GET',url);new=candidate(old,machine['addresses'],group,idp.get('addresses',[]))
    validation,_=api.call('POST',url+'/validate',new)
    if validation.get('errors') or validation.get('message'):raise ValueError('Candidate failed provider validation')
    return {'schema':'loom-orphan-policy/v1','tailnet':tailnet,'device':device,'addresses':machine['addresses'],'before':old,'after':new,
            'group':group,'retired_idp':retired_idp,'retired_idp_addresses':idp.get('addresses',[]),
            'scope':'Policy only; selected retired installation. No device deletion, logout, SSH changes or sharing-invitation cleanup.'}


def apply(api,p,confirmation):
    if p.get('schema')!='loom-orphan-policy/v1' or confirmation!=p['device']:raise ValueError('Confirm the removed installation device ID')
    if candidate(p['before'],p['addresses'],p.get('group'),p.get('retired_idp_addresses',[]))!=p['after']:raise ValueError('Cleanup plan differs from scoped selection')
    if p.get('retired_idp'):
        idp,_=api.call('GET','/device/'+quote(p['retired_idp'],safe=''))
        if set(idp['addresses'])!=set(p['retired_idp_addresses']):raise ValueError('Retired IdP identity changed')
    device,_=api.call('GET','/device/'+quote(p['device'],safe=''))
    if set(device['addresses'])!=set(p['addresses']):raise ValueError('Device identity changed')
    url='/tailnet/'+quote(p['tailnet'],safe='')+'/acl';current,headers=api.call('GET',url)
    if current==p['after']:return {'verified':True,'changed':False}
    if current!=p['before']:raise ValueError('Policy changed since review; generate a fresh plan')
    etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
    if not etag:raise ValueError('ETag required')
    check,_=api.call('POST',url+'/validate',p['after'])
    if check.get('errors') or check.get('message'):raise ValueError('Candidate failed provider validation')
    api.call('POST',url,p['after'],{'If-Match':etag})
    actual,_=api.call('GET',url)
    if actual!=p['after']:raise ValueError('Policy outcome changed; inspect before retry')
    return {'verified':True,'changed':True}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['plan','apply']);p.add_argument('--credential-file',type=Path,required=True);p.add_argument('--plan',type=Path,required=True);p.add_argument('--tailnet');p.add_argument('--device');p.add_argument('--group');p.add_argument('--retired-idp');p.add_argument('--confirm-removed-device');a=p.parse_args()
    api=TailnetAPI(str(a.credential_file))
    if a.action=='plan':
        if not a.tailnet or not a.device or not (a.group or a.retired_idp):p.error('Select tailnet, device and explicit orphan group and/or retired IdP')
        if a.plan.exists():p.error('Plan exists; review or choose a new path')
        result=plan(api,a.tailnet,a.device,a.group,a.retired_idp);write(a.plan,result)
        print(json.dumps({'plan':str(a.plan),'changed':False,'scope':result['scope']}))
    else:print(json.dumps(apply(api,json.loads(a.plan.read_text()),a.confirm_removed_device)))
