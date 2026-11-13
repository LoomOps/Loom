"""Delayed addresses and boot policy with fake clocks and isolated command fixtures."""
import contextlib
import errno
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock,patch

import boot_readiness as boot
import host_services

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('boot_authority_fixture',ROOT/'ops/remote_fence.py')
authority=importlib.util.module_from_spec(spec);spec.loader.exec_module(authority)


class Clock:
    def __init__(self):self.now=0
    def time(self):return self.now
    def sleep(self,seconds):self.now+=seconds


class BootReadinessTests(unittest.TestCase):
    def test_only_exact_published_addresses_are_waited_for(self):
        ports=[{'host_ip':ip} for ip in ('127.0.0.1','::1','0.0.0.0','::','100.64.0.42','100.64.0.42','fd7a:115c:a1e0::42')]+[{}]
        self.assertEqual(boot.published_addresses({'services':{'api':{'ports':ports},'worker':{}}}),{'100.64.0.42','fd7a:115c:a1e0::42'})
        with self.assertRaises(ValueError):boot.published_addresses({'services':{'api':{'ports':['100.64.0.42:8443:8443']}}})
        with self.assertRaises(ValueError):boot.published_addresses({'services':{'api':{'ports':[{'host_ip':'not-an-ip'}]}}})

    def test_delayed_address_outlasts_original_boot_race_without_false_readiness(self):
        clock=Clock();calls=[]
        def addresses(timeout):
            calls.append((clock.now,timeout))
            return {'100.64.0.42'} if clock.now>=12 else set()
        boot.wait_for_addresses({'100.64.0.42'},20,probe=addresses,clock=clock.time,sleep=clock.sleep)
        self.assertEqual(clock.now,12)
        self.assertTrue(all(0<timeout<=5 for _,timeout in calls))

    def test_missing_or_failing_probe_has_a_bounded_deadline(self):
        for failure in (False,True):
            clock=Clock()
            def addresses(timeout):
                if failure:raise PermissionError('fixture interface unavailable')
                return set()
            with self.subTest(failure=failure),self.assertRaisesRegex(TimeoutError,'100.64.0.42'):
                boot.wait_for_addresses({'100.64.0.42'},3,probe=addresses,clock=clock.time,sleep=clock.sleep)
            self.assertEqual(clock.now,3)

    def test_loopback_wildcard_only_needs_no_interface_probe(self):
        probe=Mock(side_effect=AssertionError('Must not query host network'))
        boot.wait_for_addresses(set(),probe=probe);probe.assert_not_called()

    def test_ipv6_scope_and_tentative_addresses(self):
        rows=[{'ifname':'test0','addr_info':[{'local':'100.64.0.42'},
            {'local':'fd7a:115c:a1e0::42','tentative':True},{'local':'fe80::42'}]}]
        with patch.object(boot.subprocess,'run',return_value=subprocess.CompletedProcess([],0,json.dumps(rows))):
            self.assertEqual(boot.local_addresses(2),{'100.64.0.42','fe80::42','fe80::42%test0'})

    def test_failed_compose_probe_does_not_echo_private_config(self):
        error=subprocess.CalledProcessError(1,['fixture'],output='private-config',stderr='private-credential')
        output=io.StringIO()
        with patch.object(sys,'argv',['boot','--','fixture']),patch.object(boot.subprocess,'run',side_effect=error),contextlib.redirect_stderr(output):
            self.assertEqual(boot.main(),1)
        self.assertNotIn('private-',output.getvalue())

    def test_compose_render_time_consumes_the_same_deadline(self):
        with patch.object(sys,'argv',['boot','--timeout','20','--','docker','compose','-f','override.yaml']),patch.object(boot.time,'monotonic',side_effect=[0,7]),patch.object(boot.subprocess,'run',return_value=subprocess.CompletedProcess([],0,'{"services":{}}')) as command,patch.object(boot,'wait_for_addresses') as wait:
            self.assertEqual(boot.main(),0)
        self.assertEqual(command.call_args.args[0],['docker','compose','-f','override.yaml','config','--format','json'])
        self.assertEqual(command.call_args.kwargs['timeout'],20)
        wait.assert_called_once_with(set(),13)

    def test_unit_policy_preserves_custom_environment_mounts_and_longer_timeouts(self):
        body='[Unit]\nAfter=docker.service tailscaled.service\nRequiresMountsFor=/data/custom\n[Service]\nType=oneshot\nRemainAfterExit=yes\nEnvironment=LOOM_PROJECT=custom\nRestart=no\nTimeoutStartSec=180\nExecStart=/bin/true\n[Install]\nWantedBy=multi-user.target\n'
        fixed=boot.harden_unit(body,stack=True)
        self.assertIn('Restart=on-failure\nRestartSec=15',fixed)
        self.assertIn('StartLimitIntervalSec=0',fixed);self.assertIn('TimeoutStartSec=300',fixed)
        for line in ('After=docker.service tailscaled.service','RequiresMountsFor=/data/custom','Environment=LOOM_PROJECT=custom','ExecStart=/bin/true'):
            self.assertIn(line,fixed)
        self.assertEqual(boot.harden_unit(fixed,stack=True),fixed)
        for value in ('600','10min','infinity','0'):
            self.assertIn('TimeoutStartSec='+value,boot.harden_unit(body.replace('TimeoutStartSec=180','TimeoutStartSec='+value),stack=True))

    def test_hardened_unit_is_valid_systemd_without_activation(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'fixture.service'
            path.write_text(boot.harden_unit('[Service]\nType=oneshot\nExecStart=/bin/true\nRemainAfterExit=yes\n',stack=True))
            p=subprocess.run(['systemd-analyze','verify',str(path)],capture_output=True,text=True,timeout=10)
            self.assertEqual(p.returncode,0,p.stderr)

    def test_generated_authority_unit_is_hardened_without_reinitializing_state(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);state=root/'shared/authority';state.mkdir(parents=True)
            for name in ('authority.sqlite3','policy.json','cert.pem','key.pem'):(state/name).write_text('preserved')
            with patch.object(host_services,'code',return_value=root/'shared/services'),patch.object(host_services,'unit') as write,patch.object(host_services.subprocess,'run') as activate:
                host_services.install_authority(root,'100.64.0.42',19924,activate=False)
            body=write.call_args.args[1]
            self.assertIn('--bind 100.64.0.42',body);self.assertIn('RestartSec=15',body)
            self.assertIn('StartLimitIntervalSec=0',body);activate.assert_not_called()
            self.assertTrue(all(p.read_text()=='preserved' for p in state.iterdir()))


class AuthorityListenerTests(unittest.TestCase):
    def test_address_binding_waits_for_assignment(self):
        clock=Clock();marker=object();calls=[]
        def bind(address,handler):
            calls.append(address)
            if clock.now<12:raise OSError(errno.EADDRNOTAVAIL,'Address unavailable')
            return marker
        server=authority.listener('100.64.0.42',19924,None,20,clock=clock.time,sleep=clock.sleep,factory=bind)
        self.assertIs(server,marker);self.assertEqual(clock.now,12)
        self.assertTrue(all(address==('100.64.0.42',19924) for address in calls))

    def test_address_timeout_and_other_bind_errors_are_not_hidden(self):
        clock=Clock()
        with self.assertRaises(TimeoutError):
            authority.listener('100.64.0.42',19924,None,3,clock=clock.time,sleep=clock.sleep,factory=Mock(side_effect=OSError(errno.EADDRNOTAVAIL,'unavailable')))
        self.assertEqual(clock.now,3)
        for code in (errno.EADDRINUSE,errno.EACCES):
            clock=Clock();bind=Mock(side_effect=OSError(code,'fixture'))
            with self.subTest(errno=code),self.assertRaises(OSError):
                authority.listener('127.0.0.1',19924,None,3,clock=clock.time,sleep=clock.sleep,factory=bind)
            self.assertEqual(bind.call_count,1);self.assertEqual(clock.now,0)


class ComposeWrapperTests(unittest.TestCase):
    def setUp(self):
        temporary=tempfile.TemporaryDirectory();self.addCleanup(temporary.cleanup);self.root=Path(temporary.name)
        for name in ('source/deploy','source/ops','instance/diagnostics','bin'):(self.root/name).mkdir(parents=True)
        for name in ('compose.sh','boot_readiness.py'):shutil.copy2(ROOT/'deploy'/name,self.root/'source/deploy'/name)
        for name in ('compose.env','network.env'):(self.root/'instance'/name).write_text('')
        (self.root/'instance/compose.override.yaml').write_text('fixture override')
        self.trace=self.root/'trace.jsonl'
        docker='''#!/usr/bin/env python3
import sys,os,json
with open(os.environ['TRACE'],'a') as f:f.write(json.dumps(['docker',*sys.argv[1:]])+'\\n')
if 'config' in sys.argv:print(json.dumps({'services':{'api':{'ports':[{'host_ip':'100.64.0.42'}]}}}))
'''
        ip='''#!/usr/bin/env python3
import os,json
with open(os.environ['TRACE'],'a') as f:f.write(json.dumps(['ip'])+'\\n')
print(json.dumps([{'ifname':'test0','addr_info':[{'local':'100.64.0.42'}]}]))
'''
        for name,source in (('docker',docker),('ip',ip)):
            path=self.root/'bin'/name;path.write_text(source);path.chmod(0o755)
        self.env={**os.environ,'PATH':str(self.root/'bin')+':'+os.environ['PATH'],'LOOM_INSTANCE':str(self.root/'instance'),'LOOM_PROJECT':'fixture','TRACE':str(self.trace)}

    def test_up_uses_final_override_and_waits_before_launch(self):
        p=subprocess.run(['bash',str(self.root/'source/deploy/compose.sh'),'up','-d','--no-build'],env=self.env,capture_output=True,text=True,timeout=10)
        self.assertEqual(p.returncode,0,p.stderr)
        calls=[json.loads(line) for line in self.trace.read_text().splitlines()]
        config=next(c for c in calls if 'config' in c);up=next(c for c in calls if 'up' in c)
        self.assertIn(str(self.root/'instance/compose.override.yaml'),config)
        self.assertIn(str(self.root/'instance/compose.override.yaml'),up)
        self.assertLess(calls.index(config),calls.index(['ip']));self.assertLess(calls.index(['ip']),calls.index(up))

    def test_stop_and_config_do_not_wait_or_require_network_readiness_helper(self):
        (self.root/'source/deploy/boot_readiness.py').unlink()
        for action in ('stop','config'):
            with self.subTest(action=action):
                p=subprocess.run(['bash',str(self.root/'source/deploy/compose.sh'),action],env=self.env,capture_output=True,text=True,timeout=10)
                self.assertEqual(p.returncode,0,p.stderr)
        calls=[json.loads(line) for line in self.trace.read_text().splitlines()]
        self.assertNotIn(['ip'],calls)
