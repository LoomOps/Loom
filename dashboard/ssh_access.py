"""Admin SSH lifecycle; personal Tailscale identities map to one managed Unix account."""
import hashlib,json,os,re,socket,time
from pathlib import Path
from urllib.parse import quote,urlsplit
from admission import RemoteError,machine_tag
import managed_resources


def owner(login):return 'ts-'+hashlib.sha256(login.strip().casefold().encode()).hexdigest()[:24]


def host(action,person=None,name=None):
    value={'action':action}
    if person:value['owner']=person
    if name:value['username']=name
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(40);s.connect(os.environ.get('LOOM_SSH_SOCKET','/run/loom-ssh/control.sock'))
        s.sendall(json.dumps(value).encode()+b'\n')
        with s.makefile('rb') as f:raw=f.readline(65537)
    if len(raw)>65536:raise ValueError('invalid_host_response')
    result=json.loads(raw)
    if result.get('error'):raise ValueError(result['error'])
    return result['result']


class SSHAccess:
    def __init__(self,service):
        self.service=service;self.config=service.config
        with service.db() as c:
            c.execute('CREATE TABLE IF NOT EXISTS ssh_accounts(owner TEXT PRIMARY KEY,login TEXT UNIQUE,username TEXT UNIQUE,state TEXT,error TEXT,updated REAL)')

    def rows(self):
        with self.service.db() as c:return [dict(r) for r in c.execute('SELECT * FROM ssh_accounts ORDER BY login')]

    def save(self,person,login,name,state,error=None):
        with self.service.db() as c:
            c.execute('INSERT INTO ssh_accounts VALUES(?,?,?,?,?,?) ON CONFLICT(owner) DO UPDATE SET state=excluded.state,error=excluded.error,updated=excluded.updated',
                (person,login,name,state,error,time.time()))
        self.export()

    def export(self):
        folder=Path(os.environ.get('LOOM_SSH_EXPORT','/var/lib/loom-ssh-info'))
        if not folder.is_dir():return
        rows={r['owner']:self.info(r) for r in self.rows()}
        p=folder/'connections.json';tmp=folder/'connections.pending'
        tmp.write_text(json.dumps(rows));tmp.chmod(0o644);os.replace(tmp,p)

    def info(self,r):
        hostname=urlsplit(self.config['dashboard_origin']).hostname
        homes=self.config.get('ssh_homes','/home')
        return {'state':r['state'],'host':hostname,'port':22,'username':r['username'],
                'authentication':'tailscale-ssh','home':homes+'/'+r['username'],
                'addresses':self.config['addresses'],'command':'ssh '+r['username']+'@'+hostname,
                'workspaces':{'components':homes+'/'+r['username']+'/components','profiles':homes+'/'+r['username']+'/profiles'},
                'error':r['error']}

    def policy(self,login,name,remove=False):
        self.service.check_device()
        path='/tailnet/'+quote(self.config['tailnet'],safe='')+'/acl'
        grant={'src':[login],'dst':self.config['addresses'],'ip':['tcp:22']}
        rule={'action':'accept','src':[login],'dst':[machine_tag(self.config)],'users':[name]}
        for _ in range(3):
            old,headers=self.service.api.call('GET',path);new=json.loads(json.dumps(old))
            for section,item in [('grants',grant),('ssh',rule)]:
                values=new.setdefault(section,[])
                if remove:
                    # A preexisting rule is never claimed or removed.
                    key=next(iter(managed_resources.flatten({section:[item]})))
                    entry=managed_resources.read(self.config)['resources'].get(key)
                    if item in values and entry and entry['before'] is None:values.remove(item)
                elif item not in values:values.append(item)
            if old==new:return
            etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
            if not etag:raise ValueError('policy_etag_required')
            result,_=self.service.api.call('POST',path+'/validate',new)
            if result.get('errors') or result.get('message'):raise ValueError('ssh_policy_validation_failed')
            try:managed_resources.policy_write(self.config,self.service.api,path,old,new,{'If-Match':etag});return
            except RemoteError as e:
                if e.status!=412:raise
        raise ValueError('policy_concurrent_change')

    def change(self,login,name,action,actor,eligible):
        if not self.service.is_admin(actor):raise PermissionError('admin_required')
        if not isinstance(login,str) or login!=login.strip().casefold() or not re.fullmatch(r'[^\s@]+@[^\s@]+',login):raise ValueError('invalid_login')
        if action not in ('enable','disable'):raise ValueError('invalid_ssh_action')
        person=owner(login)
        with self.service.lock:
            existing=next((r for r in self.rows() if r['owner']==person),None)
            if existing and name!=existing['username']:raise ValueError('account_binding_is_immutable')
            if action=='enable' and login not in eligible:raise ValueError('approved_member_required')
            if not existing and action=='disable':raise ValueError('ssh_account_not_found')
            if not re.fullmatch(r'loomu_[a-z0-9][a-z0-9_]{0,23}',name or ''):raise ValueError('invalid_ssh_username')
            self.save(person,login,name,'enabling' if action=='enable' else 'disabling')
            try:
                if action=='enable':
                    host('ensure',person,name)
                    self.policy(login,name)
                    host('enable',person,name)
                else:
                    host('disable',person,name) # Block locally before attempting remote cleanup.
                    self.policy(login,name,True)
                self.save(person,login,name,'enabled' if action=='enable' else 'disabled')
                with self.service.db() as c:c.execute('INSERT INTO audit(request,action,actor,at) VALUES(?,?,?,?)',('ssh:'+person,'ssh_'+action,actor['subject'],time.time()))
            except Exception:
                if action=='enable':
                    try:host('disable',person,name)
                    except Exception:pass
                self.save(person,login,name,'error','ssh_'+action+'_incomplete')
                raise
        return self.info(next(r for r in self.rows() if r['owner']==person))

    def revoke(self,login):
        r=next((r for r in self.rows() if r['login']==login and r['state']!='disabled'),None)
        if not r:return
        # Used by Sharing's authenticated revocation worker, not the public gateway.
        self.save(r['owner'],login,r['username'],'disabling')
        host('disable',r['owner'],r['username']);self.policy(login,r['username'],True)
        self.save(r['owner'],login,r['username'],'disabled')
