import importlib.util
from pathlib import Path
import tempfile,unittest,json,os
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('loom_installer',ROOT/'deploy/install.py');installer=importlib.util.module_from_spec(spec);spec.loader.exec_module(installer)
class InstallationTests(unittest.TestCase):
 def test_new_install_is_authenticated_without_research_dependencies(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp)/'instance';data=Path(temp)/'data'
   result=installer.install(root,data,19021,19022,19023,'fixture','loom-ops:test','loom-dashboard:test',uid=os.getuid() or 1001,gid=os.getgid() or 1001)
   config=json.loads((root/'config/api.json').read_text())
   self.assertTrue((root.parent/'profiles/fixture').is_dir());self.assertTrue((root.parent/'components/fixture').is_dir())
   self.assertEqual(config['profiles'],{});self.assertTrue((root/'secrets/tls/server-key.pem').is_file())
   self.assertEqual((root/'secrets/tls/server-key.pem').stat().st_mode&0o777,0o600)
   self.assertNotIn('cyclo_profile',config);self.assertIn('LOOM_DEV_NO_AUTH=0',(root/'build.env').read_text())
   with self.assertRaises(ValueError):installer.install(root,data,19021,19022,19023,'fixture','x','y')

 def test_invalid_request_does_not_create_host_account(self):
  with tempfile.TemporaryDirectory() as temp,patch.object(installer,'identity') as identity:
   with self.assertRaises(ValueError):installer.install(Path(temp)/'i',Path(temp)/'d',19021,19021,19023,'fixture','x','y')
   identity.assert_not_called()
 def test_default_install_uses_service_identity_and_prepares_host_services(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp)/'platform/ops';data=Path(temp)/'data';uid=os.getuid() or 1001;gid=os.getgid() or 1001
   with patch.object(installer,'identity',return_value=(uid,gid)) as identity,patch.object(installer,'platform_layout',side_effect=lambda path,*args:path.mkdir(parents=True)),patch('host_services.install_observations') as services:
    result=installer.install(root,data,19021,19022,19023,'fixture','x','y')
   identity.assert_called_once_with(None,None);services.assert_called_once_with(root.parent,root,'fixture',uid,gid)
   self.assertEqual(result['service_account'],'loom')
   self.assertIn(f'OPS_UID={uid}',(root/'compose.env').read_text())
 def test_executor_grant_uses_instance_uid_not_privileged_broker_uid(self):
  import runpy,sys,io,contextlib
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp)/'instance';data=Path(temp)/'data';executor=Path(temp)/'executor'
   uid=os.getuid() or 1001;gid=os.getgid() or 1001
   installer.install(root,data,19021,19022,19023,'fixture','x','y',uid=uid,gid=gid)
   dependencies=Path(temp)/'custom-dependencies';dependencies.mkdir()
   (root/'config/installation-plan.json').write_text(json.dumps({'dependencies':str(dependencies)}))
   argv=['executor-attach.py',str(root),'--data',str(data),'--instance','fixture','--executor',str(executor),'--network','bridge','--driver-capability','compute','--driver-capability','utility','--driver-capability','graphics']
   with patch.object(sys,'argv',argv),patch('os.geteuid',return_value=0),patch('os.chown'),patch('host_identity.protected_chain'),contextlib.redirect_stdout(io.StringIO()):
    runpy.run_path(str(ROOT/'deploy/executor-attach.py'),run_name='__main__')
   config=json.loads((executor/'config.json').read_text())
   self.assertEqual(config['uid'],0)
   self.assertEqual(config['instances']['fixture']['dependencies'],str(dependencies))
   self.assertEqual(json.loads((root/'config/api.json').read_text())['dependency_directory'],str(dependencies))
   self.assertEqual((config['instances']['fixture']['uid'],config['instances']['fixture']['gid']),(uid,gid))
   self.assertEqual(config['instances']['fixture']['declared_runtime']['network'],'bridge')
   self.assertEqual(config['instances']['fixture']['declared_runtime']['driver_capabilities'],['compute','utility','graphics'])
   self.assertEqual((root/'secrets/executor-token').stat().st_mode&0o777,0o600)
