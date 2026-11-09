"""Restricted root Unix service for Loom-managed personal SSH accounts.
No shell, arbitrary path, key issuance, sudo, Docker access, or existing-account adoption.
"""
import argparse, fcntl, grp, json, os, pwd, re, socket, socketserver, sqlite3, struct, subprocess, time, signal
from pathlib import Path


def command(*args, ok=(0,)):
    r=subprocess.run(list(args),capture_output=True,text=True,timeout=30)
    if r.returncode not in ok:raise RuntimeError('host_command_failed:'+args[0])
    return r.stdout


def username(value):
    if not isinstance(value,str) or not re.fullmatch(r'loomu_[a-z0-9][a-z0-9_]{0,23}',value):
        raise ValueError('Use loomu_ followed by up to 24 lowercase letters, digits or underscores')
    return value


def personal_host_process(pid,uid):
    """Container UIDs can equal a Unix user's numeric UID; never kill by UID alone."""
    try:
        p=Path('/proc')/str(pid)
        if p.stat().st_uid!=uid:return False
        if (p/'ns/pid').stat().st_ino!=Path('/proc/self/ns/pid').stat().st_ino:return False
        cgroup=(p/'cgroup').read_text()
        if any(v in cgroup for v in ('docker','containerd','kubepods','libpod','loom-executor')):return False
        return True
    except (OSError,ValueError):return False


def terminate_personal(name,uid):
    command('loginctl','terminate-user',name,ok=(0,1))
    for _ in range(3):
        killed=0
        for p in Path('/proc').iterdir():
            if not p.name.isdigit():continue
            fd=None
            try:
                fd=os.pidfd_open(int(p.name))
                if personal_host_process(int(p.name),uid):
                    signal.pidfd_send_signal(fd,signal.SIGKILL);killed+=1
            except ProcessLookupError:pass
            finally:
                if fd is not None:os.close(fd)
        if not killed:return


class Accounts:
    def __init__(self,config):
        self.config=config;self.root=Path(config['state']);self.root.mkdir(parents=True,exist_ok=True,mode=0o700)
        with self.db() as c:
            c.execute('CREATE TABLE IF NOT EXISTS accounts(owner TEXT PRIMARY KEY,name TEXT UNIQUE,uid INTEGER,state TEXT,created REAL)')
            c.execute('CREATE TABLE IF NOT EXISTS audit(at REAL,owner TEXT,action TEXT,state TEXT)')

    def db(self):
        c=sqlite3.connect(self.root/'accounts.sqlite3',timeout=40);c.row_factory=sqlite3.Row;return c

    def account(self,name):
        try:return pwd.getpwnam(name)
        except KeyError:return None

    def handle(self,v):
        if set(v)-{'action','owner','username'} or v.get('action') not in ('inspect','ensure','enable','disable','list'):
            raise ValueError('invalid_host_action')
        if v['action']=='list':
            with self.db() as c:return {'accounts':[dict(r) for r in c.execute('SELECT * FROM accounts')]}
        owner=v.get('owner','')
        if not re.fullmatch(r'ts-[a-f0-9]{24}',owner):raise ValueError('invalid_owner')
        with (self.root/'lock').open('a') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX)
            with self.db() as c:
                row=c.execute('SELECT * FROM accounts WHERE owner=?',(owner,)).fetchone()
                if v['action']=='inspect':return dict(row) if row else {'state':'absent'}
                name=username(v.get('username') or (row['name'] if row else ''))
                if row and row['name']!=name:raise ValueError('account_binding_is_immutable')
                account=self.account(name)
                if not row:
                    if v['action']!='ensure':raise ValueError('account_not_managed')
                    if account:raise ValueError('existing_account_not_adopted')
                    if c.execute('SELECT 1 FROM accounts WHERE name=?',(name,)).fetchone():raise ValueError('account_already_bound')
                    c.execute('INSERT INTO accounts VALUES(?,?,NULL,?,?)',(owner,name,'creating',time.time()));c.commit()
                    row=c.execute('SELECT * FROM accounts WHERE owner=?',(owner,)).fetchone()
                home=Path(self.config['homes'])/name
                marker='Loom SSH '+owner
                if not account:
                    if v['action']=='disable' and row['uid'] is None:
                        c.execute('UPDATE accounts SET state=? WHERE owner=?',('disabled',owner))
                        return {'name':name,'state':'disabled','account_absent':True}
                    if row['uid'] is not None:raise ValueError('managed_account_missing')
                    if home.exists() or home.is_symlink():raise ValueError('home_already_exists')
                    command('useradd','--create-home','--home-dir',str(home),'--shell','/usr/sbin/nologin','--comment',marker,name)
                    account=self.account(name)
                if account.pw_uid<1000 or account.pw_dir!=str(home) or account.pw_gecos!=marker or (row['uid'] is not None and row['uid']!=account.pw_uid):
                    raise ValueError('managed_account_identity_changed')
                # Never follow an attacker-controlled replacement home.
                if home.is_symlink() or not home.is_dir() or home.stat().st_uid!=account.pw_uid:raise ValueError('home_identity_changed')
                if v['action']=='ensure':
                    if row['state']=='creating':
                        os.chmod(home,0o700)
                        for leaf in ('components','profiles','work'):
                            p=home/leaf;p.mkdir(mode=0o700,exist_ok=True)
                            fd=os.open(p,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
                            try:os.fchown(fd,account.pw_uid,account.pw_gid)
                            finally:os.close(fd)
                        command('usermod','--lock',name)
                    if self.config.get('shared_group'):command('usermod','--append','--groups',self.config['shared_group'],name)
                    state=row['state'] if row['state'] not in ('creating',) else 'disabled'
                elif v['action']=='enable':
                    command('usermod','--shell','/bin/bash','--expiredate','',name)
                    # Tailscale authenticates; the Unix password remains locked.
                    state='enabled'
                else:
                    command('usermod','--shell','/usr/sbin/nologin','--expiredate','1',name)
                    # Bound to the recorded personal UID; never the Loom service UID.
                    terminate_personal(name,account.pw_uid)
                    state='disabled'
                c.execute('UPDATE accounts SET uid=?,state=? WHERE owner=?',(account.pw_uid,state,owner))
                c.execute('INSERT INTO audit VALUES(?,?,?,?)',(time.time(),owner,v['action'],state))
                return {'name':name,'uid':account.pw_uid,'state':state,'home':str(home)}


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.request.settimeout(40)
        _,uid,_=struct.unpack('3i',self.request.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
        if uid not in (0,self.server.accounts.config['client_uid']):return
        try:
            raw=self.rfile.readline(8193)
            if len(raw)>8192:raise ValueError('request_too_large')
            result={'result':self.server.accounts.handle(json.loads(raw))}
        except (ValueError,RuntimeError) as e:result={'error':str(e)}
        except Exception:result={'error':'host_operation_failed'}
        self.wfile.write(json.dumps(result).encode()+b'\n')


def serve(config):
    path=Path(config['socket']);path.parent.mkdir(parents=True,exist_ok=True)
    if path.is_symlink():raise ValueError('linked_socket')
    if path.exists():path.unlink()
    server=socketserver.UnixStreamServer(str(path),Handler);server.accounts=Accounts(config)
    os.chown(path,0,config['client_gid']);os.chmod(path,0o660)
    server.serve_forever()


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('config',type=Path);a=p.parse_args()
    serve(json.loads(a.config.read_text()))
