"""Installation-scoped ownership journal. Never infer ownership from a Loom name."""
import copy
import fcntl
import hashlib
import json
import os
from pathlib import Path
import time
import tempfile
import stat
from contextlib import contextmanager


def identity(c):
    return {k:c[k] for k in ('tailnet','machine_id','device_id')}


def path(c):
    return Path(c['database']).parent/'managed-resources.json'


def flatten(policy):
    out={}
    for section in ('groups','tagOwners','hosts'):
        for name,value in policy.get(section,{}).items():out[section+'/'+name]={'section':section,'name':name,'value':value}
    for section in ('grants','acls','ssh','nodeAttrs'):
        for value in policy.get(section,[]):
            key=hashlib.sha256(json.dumps(value,sort_keys=True).encode()).hexdigest()
            out[section+'/'+key]={'section':section,'value':value}
    return out


def read(c):
    p=path(c)
    if p.is_symlink():raise ValueError('Managed resource journal must not be a symlink')
    if not p.exists():return {'schema':'loom-managed-resources/v1','identity':identity(c),'resources':{},'operations':[]}
    data=json.loads(p.read_text())
    if data.get('schema')!='loom-managed-resources/v1' or data.get('identity')!=identity(c):raise ValueError('Managed resource identity mismatch')
    return data


def _owner(p):
    # Parent is the installation's service-owned admission directory. Root-run
    # installers must not introduce root-only state or retain an old bad owner.
    return p.parent.stat()


@contextmanager
def journal_lock(c, nonblocking=False):
    p=path(c).with_suffix('.lock');owner=_owner(p)
    fd=os.open(p,os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'a') as lock:
        info=os.fstat(lock.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink!=1:raise ValueError('Invalid journal lock')
        if os.geteuid()==0:os.fchown(lock.fileno(),owner.st_uid,owner.st_gid)
        os.fchmod(lock.fileno(),0o600)
        fcntl.flock(lock,fcntl.LOCK_EX|(fcntl.LOCK_NB if nonblocking else 0))
        yield lock


def write(c,data):
    p=path(c)
    if p.is_symlink():raise ValueError('Managed resource journal must not be a symlink')
    owner=_owner(p)
    fd,name=tempfile.mkstemp(prefix='.managed-resources-',dir=p.parent)
    try:
        with os.fdopen(fd,'w') as stream:
            os.fchmod(stream.fileno(),0o600)
            if os.geteuid()==0:os.fchown(stream.fileno(),owner.st_uid,owner.st_gid)
            json.dump(data,stream,indent=2);stream.write('\n');stream.flush();os.fsync(stream.fileno())
        os.replace(name,p)
    finally:
        Path(name).unlink(missing_ok=True)


def permissions_ready(c):
    if not path(c).parent.exists():return True
    owner=_owner(path(c))
    for p in (path(c),path(c).with_suffix('.lock')):
        if not p.exists():continue
        info=p.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_uid!=owner.st_uid or info.st_gid!=owner.st_gid or stat.S_IMODE(info.st_mode)!=0o600:
            return False
    return True


def mutation(c,before,after,send):
    """Persist intent before the provider write; keep ambiguous outcomes visible."""
    p=path(c);p.parent.mkdir(parents=True,exist_ok=True)
    with journal_lock(c):
        data=read(c)
        if any(op['state']=='pending' for op in data['operations']):raise ValueError('Unconfirmed managed resource write; reconcile journal before retry')
        changes={k:{'before':before.get(k),'after':after.get(k)} for k in before.keys()|after.keys() if before.get(k)!=after.get(k)}
        if not changes:
            if not p.exists():write(c,data)
            return None
        for k,change in changes.items():
            previous=data['resources'].get(k)
            if previous and previous['after']!=change['before']:
                raise ValueError('Managed resource changed outside this installation')
        op={'at':time.time(),'state':'pending','changes':changes};data['operations'].append(op);write(c,data)
        try:result=send()
        except Exception as exc:
            # Explicit HTTP rejection is not an uncertain network outcome.
            if getattr(exc,'status',0) in (400,401,403,404,409,412,422):op['state']='rejected';write(c,data)
            raise
        for k,change in changes.items():
            previous=data['resources'].get(k)
            data['resources'][k]={'before':previous['before'] if previous else change['before'],'after':change['after']}
        op['state']='applied';write(c,data)
        return result


def policy_write(c,api,url,before,after,headers):
    return mutation(c,flatten(before),flatten(after),lambda:api.call('POST',url,after,headers))


def tags_write(c,api,before,after):
    value=lambda tags:{'device-tag/'+t:{'section':'device-tag','name':t,'value':True} for t in tags}
    return mutation(c,value(before),value(after),lambda:api.call('POST','/device/'+c['device_id']+'/tags',{'tags':after}))


def revert(c,policy,preserve_tags=()):
    """Exact inverse of owned changes; preserve preexisting and externally edited state."""
    data=read(c)
    if any(op['state']=='pending' for op in data['operations']):raise ValueError('Unconfirmed managed resource write; reconcile journal before uninstall')
    current=flatten(policy);new=copy.deepcopy(policy);conflicts=[]
    for key,change in data['resources'].items():
        before,after=change['before'],change['after'];item=after or before
        if item['section'] in ('device-tag','invitation'):continue
        now=current.get(key)
        if now==before:continue
        if now!=after:conflicts.append(key);continue
        section=item['section']
        if section=='tagOwners' and item['name'] in preserve_tags:continue
        if 'name' in item:
            if before is None:new.get(section,{}).pop(item['name'],None)
            else:new.setdefault(section,{})[item['name']]=copy.deepcopy(before['value'])
        else:
            values=new.setdefault(section,[])
            if after and after['value'] in values:values.remove(after['value'])
            if before and before['value'] not in values:values.append(copy.deepcopy(before['value']))
    # A generated group/tag may have acquired references outside our owned diff.
    removed=set(policy.get('groups',{}))-set(new.get('groups',{}))
    removed|=set(policy.get('tagOwners',{}))-set(new.get('tagOwners',{}))
    def strings(v):
        if isinstance(v,str):yield v
        elif isinstance(v,list):
            for x in v:yield from strings(x)
        elif isinstance(v,dict):
            for x in v.values():yield from strings(x)
    referenced=removed & set(strings(new))
    # Shared tags can legitimately serve another installation or SSH rule.
    for tag in list(referenced):
        if tag.startswith('tag:'):
            new.setdefault('tagOwners',{})[tag]=copy.deepcopy(policy['tagOwners'][tag]);referenced.remove(tag)
    if conflicts or referenced:raise ValueError('Managed resources changed or still referenced: '+', '.join(sorted(set(conflicts)|referenced)))
    return new


def owned_tags(c):
    return [k.split('/',1)[1] for k,v in read(c)['resources'].items() if k.startswith('device-tag/') and v['before'] is None and v['after'] is not None]


def summary(c):
    data=read(c)
    return {'schema':data['schema'],'identity':data['identity'],'resources':len(data['resources']),
            'pending':sum(op['state']=='pending' for op in data['operations']),
            'present':path(c).exists()}


def invitation(c,invite_id,request_id,created,state):
    """Keep provider IDs after request rows are removed; never store email or tokens."""
    key='invitation/'+str(invite_id);old=read(c)['resources'].get(key,{}).get('after')
    new={'section':'invitation','name':str(invite_id),'value':{'request':request_id,'created':bool(created),'state':state}}
    mutation(c,{key:old} if old else {},{key:new},lambda:None)


def reconcile(c,policy,tags):
    """Explicit local receipt repair from observed provider state; never replay writes."""
    p=path(c)
    with journal_lock(c):
        data=read(c);observed=flatten(policy)
        observed.update({'device-tag/'+t:{'section':'device-tag','name':t,'value':True} for t in tags})
        for op in data['operations']:
            if op['state']!='pending':continue
            after=all(observed.get(k)==v['after'] for k,v in op['changes'].items())
            before=all(observed.get(k)==v['before'] for k,v in op['changes'].items())
            if not after and not before:raise ValueError('Mixed or externally changed outcome; inspect the ownership journal')
            if after:
                for k,v in op['changes'].items():
                    prior=data['resources'].get(k)
                    if prior and prior['after']!=v['before']:raise ValueError('Resource history differs; manual reconciliation required')
                    data['resources'][k]={'before':prior['before'] if prior else v['before'],'after':v['after']}
            op['state']='reconciled-applied' if after else 'reconciled-not-applied'
        write(c,data)
    return summary(c)
