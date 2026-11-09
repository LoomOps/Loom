import importlib.util
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'deploy'))
import host_identity as host

class HostIdentityTests(unittest.TestCase):
 def test_missing_account_is_created_once_and_existing_identity_reused(self):
  user=SimpleNamespace(pw_uid=987,pw_gid=986);group=SimpleNamespace(gr_gid=986)
  with patch.object(host.os,'geteuid',return_value=0),patch.object(host.grp,'getgrnam',side_effect=[KeyError(),group,group]),patch.object(host.pwd,'getpwnam',side_effect=[KeyError(),user,user]),patch.object(host.subprocess,'run') as commands:
   self.assertEqual(host.service_identity(),(987,986));self.assertEqual(host.service_identity(),(987,986))
   self.assertEqual(commands.call_count,2)
   self.assertIn('--system',commands.call_args_list[1].args[0]);self.assertIn('/usr/sbin/nologin',commands.call_args_list[1].args[0])
 def test_conflicting_existing_account_is_not_modified(self):
  with patch.object(host.os,'geteuid',return_value=0),patch.object(host.grp,'getgrnam',return_value=SimpleNamespace(gr_gid=986)),patch.object(host.pwd,'getpwnam',return_value=SimpleNamespace(pw_uid=1000,pw_gid=1000)),patch.object(host.subprocess,'run') as commands:
   with self.assertRaises(ValueError):host.service_identity()
   commands.assert_not_called()
 def test_explicit_identity_requires_both_nonroot_ids(self):
  for args in [(123,None),(None,123),(0,123),(123,0)]:
   with self.assertRaises(ValueError):host.identity(*args)
 def test_owner_walk_does_not_follow_symlinks(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t)/'owned';root.mkdir();outside=Path(t)/'external';outside.mkdir();(outside/'secret').write_text('external')
   (root/'link').symlink_to(outside,target_is_directory=True);(root/'file').write_text('local')
   with patch.object(host.os,'chown') as chown:
    host.own_tree(root,987,986)
    paths={Path(c.args[0]) for c in chown.call_args_list}
    self.assertEqual(paths,{root,root/'file'})
 def test_privileged_path_rejects_service_writable_parent(self):
  with tempfile.TemporaryDirectory() as t:
   with self.assertRaises(ValueError):host.protected_chain(Path(t))

 def test_layout_does_not_create_or_touch_retired_agent_directory(self):
  with tempfile.TemporaryDirectory() as t, patch.object(host.os,'chown'):
   root=Path(t)/'loom';host.platform_layout(root,987,986)
   self.assertEqual({p.name for p in root.iterdir()},{'shared','components','profiles','builds'})
   legacy=root/'agents';legacy.mkdir(mode=0o700);(legacy/'note').write_text('preserve')
   host.platform_layout(root,987,986)
   self.assertEqual((legacy/'note').read_text(),'preserve')
   self.assertEqual(legacy.stat().st_mode&0o777,0o700)
