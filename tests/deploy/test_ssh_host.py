import json,os,sys,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import ssh_host

class HostTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
        self.config={'state':str(self.root/'state'),'homes':str(self.root/'home'),'client_uid':os.getuid(),'client_gid':os.getgid()}
        (self.root/'home').mkdir();self.a=ssh_host.Accounts(self.config);self.users={};self.commands=[]
        self.person='ts-'+'a'*24;self.name='loomu_research'
        self.lookup=patch.object(self.a,'account',side_effect=lambda n:self.users.get(n));self.lookup.start();self.addCleanup(self.lookup.stop)
        def run(*args,**kw):
            self.commands.append(args)
            if args[0]=='useradd':
                home=args[args.index('--home-dir')+1];Path(home).mkdir()
                self.users[args[-1]]=SimpleNamespace(pw_uid=os.getuid(),pw_gid=os.getgid(),pw_dir=home,pw_gecos='Loom SSH '+self.person)
            return ''
        stop=patch('ssh_host.terminate_personal');stop.start();self.addCleanup(stop.stop)
        self.run=patch('ssh_host.command',side_effect=run);self.run.start();self.addCleanup(self.run.stop)
    def call(self,action='ensure',name=None):return self.a.handle({'action':action,'owner':self.person,'username':name or self.name})
    def test_lifecycle_idempotence_and_no_privileged_group(self):
        if os.getuid()<1000:self.skipTest('User-space fixture needs ordinary uid')
        self.assertEqual(self.call()['state'],'disabled');self.call();self.call('enable');self.call('disable')
        self.assertEqual(sum(c[0]=='useradd' for c in self.commands),1)
        self.assertNotIn('sudo',str(self.commands));self.assertNotIn('docker',str(self.commands))
        self.assertTrue((self.root/'home'/self.name/'work').is_dir())
        self.assertEqual(self.a.handle({'action':'inspect','owner':self.person})['state'],'disabled')
    def test_existing_account_and_injection_refused(self):
        self.users[self.name]=SimpleNamespace(pw_uid=1001)
        with self.assertRaises(ValueError):self.call()
        for name in ('root','loom','loomu_../a','loomu_x;id'):
            with self.assertRaises(ValueError):self.call(name=name)
        with self.assertRaises(ValueError):self.a.handle({'action':'exec','owner':self.person,'command':'id'})
        self.assertFalse(self.commands)
    def test_replaced_uid_cannot_be_killed(self):
        if os.getuid()<1000:self.skipTest('User-space fixture needs ordinary uid')
        self.call();self.commands.clear();self.users[self.name].pw_uid+=1
        with self.assertRaises(ValueError):self.call('disable')
        self.assertFalse(self.commands)
    def test_symlink_home_not_followed(self):
        home=self.root/'home'/self.name;home.symlink_to(self.root/'state',target_is_directory=True)
        with self.assertRaises(ValueError):self.call()
        self.assertFalse(self.commands)
    def test_cross_owner_name_binding_rejected(self):
        if os.getuid()<1000:self.skipTest('User-space fixture needs ordinary uid')
        self.call()
        with self.assertRaises(ValueError):self.a.handle({'action':'ensure','owner':'ts-'+'b'*24,'username':self.name})

class DoctorTests(unittest.TestCase):
    def test_absent_is_optional_and_socket_failure_is_not_ready(self):
        import ssh_setup
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);self.assertEqual(ssh_setup.doctor(root),{'enabled':False,'ready':True})
            (root/'config').mkdir();state,runtime,name=ssh_setup.paths(root)
            (root/'config/ssh.json').write_text(json.dumps({'unit':name,'socket':str(runtime/'control.sock')}))
            with patch.object(ssh_setup,'run',side_effect=lambda *a:'{"RunSSH":true}' if a[:2]==('tailscale','debug') else 'active'):
                result=ssh_setup.doctor(root)
            self.assertFalse(result['ready']);self.assertFalse(result['checks']['socket']);self.assertFalse(result['checks']['metadata'])

class SocketTests(unittest.TestCase):
    def test_kernel_peer_uid_not_payload_authorizes_socket(self):
        import socket,socketserver,threading
        with tempfile.TemporaryDirectory() as t:
            path=str(Path(t)/'control.sock');server=socketserver.UnixStreamServer(path,ssh_host.Handler)
            calls=[]
            server.accounts=SimpleNamespace(config={'client_uid':os.getuid()},handle=lambda value:calls.append(value) or {'state':'fixture'})
            th=threading.Thread(target=server.serve_forever,kwargs={'poll_interval':.01},daemon=True);th.start()
            try:
                def request(value):
                    with socket.socket(socket.AF_UNIX) as s:
                        s.connect(path);s.sendall(json.dumps(value).encode()+b'\n');return s.recv(4096)
                self.assertEqual(json.loads(request({'action':'list'}))['result']['state'],'fixture')
                if os.getuid()!=0:
                    server.accounts.config['client_uid']=os.getuid()+1
                    try:raw=request({'action':'list','uid':server.accounts.config['client_uid']})
                    except (ConnectionResetError,BrokenPipeError):raw=b''
                    self.assertEqual(raw,b'');self.assertEqual(len(calls),1)
            finally:server.shutdown();th.join();server.server_close()


class ProcessBoundaryTests(unittest.TestCase):
    def test_same_uid_container_and_executor_processes_are_excluded(self):
        from types import SimpleNamespace
        original=Path.read_text
        with patch.object(Path,'stat',return_value=SimpleNamespace(st_uid=1234,st_ino=42)),patch.object(Path,'read_text',return_value='0::/system.slice/docker-abc.scope'):
            self.assertFalse(ssh_host.personal_host_process(123,1234))
        with patch.object(Path,'stat',return_value=SimpleNamespace(st_uid=1234,st_ino=42)),patch.object(Path,'read_text',return_value='0::/system.slice/loom-executor.service'):
            self.assertFalse(ssh_host.personal_host_process(123,1234))
        with patch.object(Path,'stat',return_value=SimpleNamespace(st_uid=1234,st_ino=42)),patch.object(Path,'read_text',return_value='0::/user.slice/user-1234.slice'):
            self.assertTrue(ssh_host.personal_host_process(123,1234))
        with patch.object(Path,'stat',side_effect=lambda p,*a,**k:SimpleNamespace(st_uid=1234,st_ino=99 if str(p)=='/proc/123/ns/pid' else 42),autospec=True):
            self.assertFalse(ssh_host.personal_host_process(123,1234))

class UnitLifecycleTests(unittest.TestCase):
    def test_restart_preserves_directory_and_stack_waits_for_helper(self):
        import ssh_setup
        with tempfile.TemporaryDirectory() as t:
            folder=Path(t);realpath=Path
            def path(value):
                return folder/'dropin' if value=='/etc/systemd/system/loom-stack.service.d' else realpath(value)
            with patch.object(ssh_setup,'Path',side_effect=path),patch.object(ssh_setup,'unit') as unit:
                ssh_setup.configure_service(folder/'state','fixture.service')
            body=unit.call_args.args[1]
            self.assertIn('RuntimeDirectoryPreserve=yes',body)
            self.assertIn('Requires=fixture.service',(folder/'dropin/ssh.conf').read_text())
            self.assertIn('After=fixture.service',(folder/'dropin/ssh.conf').read_text())
