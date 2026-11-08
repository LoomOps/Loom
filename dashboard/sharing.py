"""Machine Sharing enrollment; invitations never confer Loom approval by themselves."""
import fcntl
import hashlib
import ipaddress
import json
import re
import time
import uuid
from urllib.parse import quote, urlsplit
from admission import Admission, RemoteError, group_name

TERMINAL=('denied','revoked','expired')


def public_gateway_port(config):
    """Only the dedicated public listener may bypass pre-approval gating."""
    origin=urlsplit(config['origin']);private=urlsplit(config['dashboard_origin'])
    port=origin.port or 443
    if origin.hostname!=private.hostname or port in config['ports'] or port==(private.port or 443):
        return None
    return port


def pending_policy(policy, config, login=None):
    """Reject pre-approval access to this machine; preserve unrelated policy."""
    if not policy.get('grants') and not policy.get('acls'):
        raise ValueError('sharing_requires_explicit_deny_default_policy')
    # Tailscale also includes accepted external recipients in autogroup:member
    # for a device shared with them (policy-file reference).
    sources={'*','autogroup:shared','autogroup:member'}
    if login:sources.add(login)
    for _ in range(len(policy.get('groups',{}))+1):
        sources.update(g for g,users in policy.get('groups',{}).items() if sources.intersection(users))
    addresses=[ipaddress.ip_address(a) for a in config['addresses']]
    tags=set(config.get('machine_tags',[]))
    gateway=public_gateway_port(config)
    def broad_source(items):
        if sources.intersection(items):return True
        for value in items:
            value=policy.get('hosts',{}).get(value,value)
            try:
                net=ipaddress.ip_network(value,strict=False)
                pools=(ipaddress.ip_network('100.64.0.0/10'),ipaddress.ip_network('fd7a:115c:a1e0::/48'))
                if net.num_addresses>1 and any(net.version==p.version and net.overlaps(p) for p in pools):return True
            except ValueError:pass
        return False
    def matches(dst):
        dst=policy.get('hosts',{}).get(dst,dst)
        if dst=='*' or dst in tags or dst=='autogroup:tagged':return True
        try:return any(a in ipaddress.ip_network(dst,strict=False) for a in addresses)
        except ValueError:return False
    for r in policy.get('grants',[]):
        if broad_source(r.get('src',[])) and any(matches(d) for d in r.get('dst',[])):
            if r.get('ip') and not (gateway and all(p=='tcp:'+str(gateway) for p in r['ip'])):
                raise ValueError('existing_policy_allows_unapproved_shared_user')
    for r in policy.get('acls',[]):
        if r.get('action')=='accept' and broad_source(r.get('src',[])):
            for d in r.get('dst',[]):
                dst,ports=d.rsplit(':',1)
                if matches(dst) and not (gateway and r.get('proto')=='tcp' and ports==str(gateway)):
                    raise ValueError('existing_policy_allows_unapproved_shared_user')


class SharingAdmission(Admission):
    def __init__(self,config,api=None):
        super().__init__(config,api)
        with self.db() as c:
            c.executescript('''CREATE TABLE IF NOT EXISTS share_requests(
              id TEXT PRIMARY KEY,machine TEXT NOT NULL,email TEXT NOT NULL,state TEXT NOT NULL,
              created REAL NOT NULL,updated REAL NOT NULL,expires REAL NOT NULL,
              invite_id TEXT,accepted_email TEXT,user_id TEXT,accepted_at REAL,approved_by TEXT,
              error TEXT,attempts INTEGER NOT NULL DEFAULT 0,next_attempt REAL NOT NULL DEFAULT 0,
              send_started INTEGER NOT NULL DEFAULT 0,terminal TEXT);
              CREATE UNIQUE INDEX IF NOT EXISTS share_active_email ON share_requests(machine,email)
                WHERE state NOT IN ('denied','revoked','expired');
              CREATE TABLE IF NOT EXISTS share_receipts(machine TEXT,issuer TEXT,subject TEXT,request TEXT,
                PRIMARY KEY(machine,issuer,subject));''')
            columns={r['name'] for r in c.execute('PRAGMA table_info(share_requests)')}
            for field in ('sender_name','sender_email'):
                if field not in columns:c.execute('ALTER TABLE share_requests ADD COLUMN '+field+' TEXT')

    def existing_access(self,email):
        native={str(x).strip().casefold() for x in self.config.get('native_logins',[])}
        native.update(a['subject'].removeprefix('tailscale:').strip().casefold()
                      for a in self.config['admins'] if a['subject'].startswith('tailscale:'))
        if email in native:return True
        with self.db() as c:
            return bool(c.execute("SELECT 1 FROM share_requests WHERE machine=? AND accepted_email=? AND state='ready' AND approved_by IS NOT NULL",(self.config['machine_id'],email)).fetchone())

    def request(self,user):
        email=str(user.get('login') or '').strip().casefold()
        if not re.fullmatch(r'[^\s@]{1,128}@[^\s@]{1,128}',email):raise ValueError('email_required')
        if self.existing_access(email):raise ValueError('access_check_required')
        with self.lock,self.db() as c:
            c.execute('BEGIN IMMEDIATE')
            row=c.execute("SELECT * FROM share_requests WHERE machine=? AND email=? AND state NOT IN ('denied','revoked','expired')",(self.config['machine_id'],email)).fetchone()
            if not row:
                now=time.time()
                if c.execute('SELECT count(*) FROM share_requests WHERE created>?',(now-3600,)).fetchone()[0]>=self.config.get('sharing_hourly_limit',20):raise ValueError('request_rate_limited')
                if c.execute('SELECT count(*) FROM share_requests WHERE email=? AND created>?',(email,now-86400)).fetchone()[0]>=3:raise ValueError('request_rate_limited')
                if c.execute("SELECT count(*) FROM share_requests WHERE state NOT IN ('ready','denied','revoked','expired')").fetchone()[0]>=self.config.get('sharing_pending_limit',200):raise ValueError('request_capacity_reached')
                key=uuid.uuid4().hex
                c.execute('INSERT INTO share_requests(id,machine,email,state,created,updated,expires) VALUES(?,?,?,?,?,?,?)',(key,self.config['machine_id'],email,'requested',now,now,now+7*86400))
                c.execute('INSERT INTO audit(request,action,actor,at) VALUES(?,?,?,?)',(key,'share_requested',user['subject'],now))
                row=c.execute('SELECT * FROM share_requests WHERE id=?',(key,)).fetchone()
            c.execute('INSERT OR REPLACE INTO share_receipts VALUES(?,?,?,?)',(self.config['machine_id'],user['issuer'],user['subject'],row['id']))
            return dict(row)

    def rows(self,user,admin=False):
        if admin and not self.is_admin(user):raise PermissionError('admin_required')
        with self.db() as c:
            select="SELECT r.*, EXISTS(SELECT 1 FROM audit a WHERE a.request=r.id AND a.action='requester_cancel') AS user_cancelled, EXISTS(SELECT 1 FROM audit a WHERE a.request=r.id AND a.action='share_requested' AND a.actor=?) AS original_requester FROM share_requests r "
            if admin:rows=c.execute(select+'WHERE r.machine=? ORDER BY r.created DESC LIMIT 200',(user['subject'],self.config['machine_id'])).fetchall()
            else:rows=c.execute(select+'JOIN share_receipts s ON s.request=r.id WHERE s.machine=? AND s.issuer=? AND s.subject=?',(user['subject'],self.config['machine_id'],user['issuer'],user['subject'])).fetchall()
        return [{**dict(r),'can_cancel':bool(r['original_requester'] and r['state'] in ('requested','invited','awaiting_approval'))} for r in rows]

    def update(self,key,**fields):
        with self.db() as c:
            c.execute('BEGIN IMMEDIATE')
            previous=c.execute('SELECT state,terminal FROM share_requests WHERE id=?',(key,)).fetchone()
            # A public cancellation can arrive while the private worker is sending
            # an invitation. Keep its resulting invite ID for cleanup, never revive it.
            if previous and previous['terminal'] and fields.get('state') not in (*TERMINAL,'revoking',None):
                fields['state']=previous['state'];fields['next_attempt']=0
            fields['updated']=time.time()
            c.execute('UPDATE share_requests SET '+','.join(k+'=?' for k in fields)+' WHERE id=?',(*fields.values(),key))
            if previous and 'state' in fields and previous['state']!=fields['state']:
                c.execute('INSERT INTO audit(request,action,actor,at) VALUES(?,?,?,?)',(key,'sharing_state:'+fields['state'],'automation',time.time()))

    def cancel_own_pending(self,user,strict=False):
        """Only the original receipt can withdraw an unapproved request.

        Another receipt associated through email deduplication is not proof of
        ownership. Approved access is never revoked through a public receipt.
        """
        with self.db() as c:
            c.execute('BEGIN IMMEDIATE')
            row=c.execute('SELECT r.* FROM share_requests r JOIN share_receipts s ON s.request=r.id WHERE s.machine=? AND s.issuer=? AND s.subject=?',
                (self.config['machine_id'],user['issuer'],user['subject'])).fetchone()
            if not row:return False
            if row['state'] not in ('requested','invited','awaiting_approval'):
                if strict and row['state'] not in TERMINAL:raise PermissionError('request_no_longer_cancellable')
                return False
            creator=c.execute("SELECT 1 FROM audit WHERE request=? AND action='share_requested' AND actor=?",(row['id'],user['subject'])).fetchone()
            if not creator:
                if strict:raise PermissionError('original_requester_required')
                return False
            c.execute("UPDATE share_requests SET state='revoking',terminal='revoked',next_attempt=0,error=NULL,updated=? WHERE id=?",(time.time(),row['id']))
            c.execute('INSERT INTO audit(request,action,actor,at) VALUES(?,?,?,?)',(row['id'],'requester_cancel',user['subject'],time.time()))
            return True

    def decide(self,key,action,user):
        if not self.is_admin(user):raise PermissionError('admin_required')
        if action not in ('approve','deny','retry','revoke','remove'):raise ValueError('invalid_decision')
        with self.lock,self.db() as c:
            c.execute('BEGIN IMMEDIATE')
            r=c.execute('SELECT * FROM share_requests WHERE id=? AND machine=?',(key,self.config['machine_id'])).fetchone()
            if not r:raise ValueError('request_not_found')
            if action=='remove':
                if r['state'] not in ('denied','revoked'):raise ValueError('only_denied_or_revoked_request_removable')
                c.execute('DELETE FROM share_receipts WHERE machine=? AND request=?',(self.config['machine_id'],key))
                c.execute('DELETE FROM share_requests WHERE id=?',(key,))
            elif action=='approve':
                if r['state']!='awaiting_approval' or not r['accepted_email'] or not r['user_id']:raise ValueError('share_acceptance_required')
                c.execute("UPDATE share_requests SET state='approved',approved_by=?,next_attempt=0,error=NULL,updated=? WHERE id=?",(json.dumps(user),time.time(),key))
            elif action in ('deny','revoke'):
                if r['state'] in TERMINAL: return
                c.execute("UPDATE share_requests SET state='revoking',terminal=?,next_attempt=0,error=NULL,updated=? WHERE id=?",('revoked' if action=='revoke' else 'denied',time.time(),key))
            else:
                if not r['error'] or r['state'] in TERMINAL:raise ValueError('not_retryable')
                c.execute('UPDATE share_requests SET next_attempt=0,error=NULL,updated=? WHERE id=?',(time.time(),key))
                if r['error']=='invite_delivery_uncertain':
                    # A deliberate admin retry may send again; automatic retries never do.
                    c.execute('UPDATE share_requests SET send_started=0 WHERE id=?',(key,))
            c.execute('INSERT INTO audit(request,action,actor,at) VALUES(?,?,?,?)',(key,'share_'+action,user['subject'],time.time()))

    @property
    def invitepath(self):return '/device/'+quote(self.config['device_id'],safe='')+'/device-invites'

    def invites(self):
        self.check_device()
        rows,_=self.api.call('GET',self.invitepath)
        return rows.get('invites',[]) if isinstance(rows,dict) else rows

    def members(self):
        invites=self.invites()
        with self.db() as c:local={r['invite_id']:dict(r) for r in c.execute('SELECT * FROM share_requests WHERE machine=?',(self.config['machine_id'],)) if r['invite_id']}
        rows=[]
        for i in invites:
            if not i.get('accepted') or not i.get('acceptedBy'):continue
            u=i['acceptedBy'];r=local.get(str(i['id']),{})
            if (r.get('state')!='ready' or not r.get('approved_by') or
                str(u.get('id') or '')!=r.get('user_id') or
                str(u.get('loginName') or '').strip().casefold()!=r.get('accepted_email')):continue
            rows.append({'id':str(i['id']),'loginName':u.get('loginName'),'displayName':u.get('displayName') or u.get('loginName'),
                         'role':'Member','status':r['state'],'created':i.get('created'),
                         'request_id':r.get('id'),'accepted_at':r.get('accepted_at')})
        return sorted(rows,key=lambda r:(r.get('accepted_at') or float('inf'),r.get('created') or '',r['id']))

    def authorize_network(self,network):
        login=str(network.get('Tailscale-User-Login') or '').strip().casefold()
        if not login:return None
        with self.db() as c:
            row=c.execute("SELECT * FROM share_requests WHERE machine=? AND accepted_email=? AND state='ready' ORDER BY created LIMIT 1",(self.config['machine_id'],login)).fetchone()
        if not row:return None
        from email.header import decode_header,make_header
        return {'subject':'shared-user:'+row['user_id'],'login':login,
                'name':str(make_header(decode_header(network.get('Tailscale-User-Name') or login)))[:256],
                'authentication':'tailscale-sharing','share_request':row['id']}

    def revoke_share(self,invite_id,user):
        if not self.is_admin(user):raise PermissionError('admin_required')
        invite=next((i for i in self.invites() if str(i['id'])==invite_id),None)
        if not invite or not invite.get('acceptedBy'):raise ValueError('shared_user_not_found')
        with self.db() as c:
            row=c.execute('SELECT id FROM share_requests WHERE machine=? AND invite_id=?',(self.config['machine_id'],invite_id)).fetchone()
            if row:key=row['id']
            else:
                now=time.time();key=uuid.uuid4().hex;person=invite['acceptedBy']
                c.execute('INSERT INTO share_requests(id,machine,email,state,created,updated,expires,invite_id,accepted_email,user_id) VALUES(?,?,?,?,?,?,?,?,?,?)',
                    (key,self.config['machine_id'],str(invite.get('email') or person['loginName']).casefold(),'ready',now,now,now,invite_id,str(person['loginName']).casefold(),str(person['id'])))
        self.decide(key,'revoke',user)

    def session_allowed(self,user,network):
        if user.get('authentication')!='tailscale-sharing':return True
        current=self.authorize_network(network)
        return bool(current and current['subject']==user['subject'] and current['share_request']==user.get('share_request'))

    def policy(self,row,remove=False):
        self.check_device()
        login=row['accepted_email']
        if not login:return
        group=group_name(self.config['machine_id'],self.config['device_id'])
        for _ in range(3):
            old,headers=self.api.call('GET',self.tailpath+'/acl')
            etag=next((v for k,v in headers.items() if k.lower()=='etag'),None)
            if not etag:raise ValueError('policy_etag_required')
            if remove and group not in old.get('groups',{}):return
            new=json.loads(json.dumps(old));members=new.setdefault('groups',{}).setdefault(group,[])
            if remove:
                # Keep another approved share for the same actual identity intact.
                with self.db() as c:
                    other=c.execute("SELECT 1 FROM share_requests WHERE machine=? AND accepted_email=? AND id!=? AND state='ready'",(self.config['machine_id'],login,row['id'])).fetchone()
                if not other:members[:]=[m for m in members if m!=login]
            else:
                if login not in members:members.append(login)
                grant={'src':[group],'dst':self.config['addresses'],'ip':['tcp:'+str(p) for p in self.config['ports']]}
                if grant not in new.setdefault('grants',[]):new['grants'].append(grant)
            if old==new:return
            result,_=self.api.call('POST',self.tailpath+'/acl/validate',new)
            if result.get('errors') or result.get('message'):raise ValueError('policy_validation_failed')
            with self.db() as c:c.execute('INSERT INTO policy_backups(request,etag,body,at) VALUES(?,?,?,?)',(row['id'],etag,json.dumps(old),time.time()))
            try:self.api.call('POST',self.tailpath+'/acl',new,{'If-Match':etag});return
            except RemoteError as e:
                if e.status!=412:raise
        raise ValueError('policy_concurrent_change')

    def preflight(self,login=None):
        device=self.check_device();policy,_=self.api.call('GET',self.tailpath+'/acl')
        pending_policy(policy,{**self.config,'machine_tags':device.get('tags',[])},login)

    def reconcile(self,row):
        if row['state']=='revoking':
            if row['send_started'] and not row['invite_id']:
                matches=[i for i in self.invites() if str(i.get('email','')).casefold()==row['email']]
                if len(matches)>1:raise ValueError('ambiguous_device_invitation')
                if matches:row={**row,'invite_id':str(matches[0]['id'])};self.update(row['id'],invite_id=row['invite_id'])
            self.policy(row,remove=True)
            if row['invite_id']:
                try:self.api.call('DELETE','/device-invites/'+quote(row['invite_id'],safe=''))
                except RemoteError as e:
                    if e.status!=404:raise
            self.update(row['id'],state=row['terminal'] or 'revoked',error=None,next_attempt=0);return
        if row['state']!='ready' and row['expires']<time.time():
            self.update(row['id'],state='revoking',terminal='expired',next_attempt=0);return
        invites=self.invites()
        if not row['invite_id']:
            # A failed native login must never become an invitation to an admin
            # or existing member. Only this private worker holds API credentials.
            if self.existing_access(row['email']):raise ValueError('access_check_required')
            users,_=self.api.call('GET',self.tailpath+'/users?type=member')
            if not isinstance(users,dict) or not isinstance(users.get('users'),list):raise ValueError('membership_check_unavailable')
            if any(str(u.get('loginName','')).strip().casefold()==row['email'] for u in users['users']):
                raise ValueError('access_check_required')
            self.preflight()
            matches=[i for i in invites if str(i.get('email','')).casefold()==row['email'] and not i.get('multiUse') and not i.get('allowExitNode')]
            if len(matches)>1:raise ValueError('ambiguous_device_invitation')
            if not matches:
                if row['send_started']:raise ValueError('invite_delivery_uncertain')
                self.update(row['id'],send_started=1)
                matches,_=self.api.call('POST',self.invitepath,[{'email':row['email'],'multiUse':False,'allowExitNode':False}])
            if not isinstance(matches,list) or len(matches)!=1 or not matches[0].get('id'):raise ValueError('invalid_device_invitation')
            invite=matches[0]
            self.update(row['id'],invite_id=str(invite['id']),state='invited',error=None,next_attempt=time.time()+30)
            row={**row,'invite_id':str(invite['id']),'state':'invited'}
        else:
            invite=next((i for i in invites if str(i['id'])==row['invite_id']),None)
            if not invite:
                self.policy(row,remove=True);self.update(row['id'],state='revoked',error=None,next_attempt=0);return
        if invite.get('sharerId') and not row.get('sender_email'):
            # Presentation metadata only: failure must not delay invitations or grants.
            try:
                sender,_=self.api.call('GET','/users/'+quote(str(invite['sharerId']),safe=''))
                email=sender.get('loginName')
                if isinstance(email,str) and re.fullmatch(r'[^\s@]{1,128}@[^\s@]{1,128}',email):
                    self.update(row['id'],sender_name=str(sender.get('displayName') or '')[:200],sender_email=email)
            except (RemoteError,OSError,ValueError,TypeError,AttributeError):pass
        if not invite.get('accepted'):
            if row['accepted_email']:raise ValueError('share_acceptance_changed')
            self.update(row['id'],next_attempt=time.time()+30,error=None);return
        accepted=invite.get('acceptedBy') or {};login=str(accepted.get('loginName') or '').strip().casefold();uid=str(accepted.get('id') or '')
        if not login or not uid:raise ValueError('share_acceptance_identity_missing')
        if row['accepted_email'] and (login!=row['accepted_email'] or uid!=row['user_id']):
            self.update(row['id'],state='revoking',terminal='revoked',error='share_identity_changed',next_attempt=0);return
        if not row['accepted_email']:
            self.preflight(login)
            self.update(row['id'],accepted_email=login,user_id=uid,accepted_at=time.time(),state='awaiting_approval',next_attempt=time.time()+30,error=None);return
        if row['state']=='awaiting_approval':
            self.update(row['id'],next_attempt=time.time()+30,error=None);return
        if row['state'] in ('approved','ready'):
            if not row['approved_by']:raise ValueError('machine_approval_required')
            self.policy(row)
            self.update(row['id'],state='ready',next_attempt=time.time()+60,error=None)

    def tick(self):
        with self.lock,open(str(self.path)+'.lock','a') as lock:
            try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
            except BlockingIOError:return
            with self.db() as c:rows=c.execute("SELECT * FROM share_requests WHERE machine=? AND state NOT IN ('denied','revoked','expired') AND next_attempt<=? ORDER BY next_attempt LIMIT 20",(self.config['machine_id'],time.time())).fetchall()
            for r in rows:
                try:self.reconcile(dict(r))
                except Exception as e:
                    code=str(e) if isinstance(e,(ValueError,RemoteError)) else 'upstream_unavailable'
                    self.update(r['id'],error=code,attempts=r['attempts']+1,next_attempt=time.time()+min(3600,60*2**min(r['attempts'],6)))
