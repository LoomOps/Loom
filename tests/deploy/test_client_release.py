import hashlib,io,json,tempfile,unittest,zipfile
from pathlib import Path
from unittest.mock import patch
import client_release

class ReleaseTests(unittest.TestCase):
 def archive(self,extra=None):
  data=io.BytesIO()
  with zipfile.ZipFile(data,'w') as z:
   z.writestr('VERSION','0.1.0-alpha.1\n');z.writestr('INSTALL.md','Read me')
   z.writestr('loom_use.py','pass');z.writestr('skills/loom-use/SKILL.md','fixture');z.writestr('clients/headers.py','pass')
   if extra:z.writestr(extra,'no')
  return data.getvalue()
 def lock(self,data):return {'version':'0.1.0-alpha.1','url':'https://github.com/LoomOps/Loom-Use/releases/download/v0.1.0-alpha.1/loom-use-0.1.0-alpha.1.zip','sha256':hashlib.sha256(data).hexdigest()}
 def test_pinned_source_and_cache_integrity(self):
  with tempfile.TemporaryDirectory() as t:
   p=Path(t);archive=p/'test.zip';data=self.archive();archive.write_bytes(data)
   with patch.object(client_release,'descriptor',return_value=self.lock(data)):
    target=client_release.prepare(p/'cache',archive)
    self.assertEqual(client_release.prepare(p/'cache'),target)
    (target/'INSTALL.md').write_text('changed')
    with self.assertRaises(ValueError):client_release.prepare(p/'cache')
 def test_bad_archive_never_publishes_directory(self):
  with tempfile.TemporaryDirectory() as t:
   p=Path(t);archive=p/'test.zip';data=self.archive('../escape');archive.write_bytes(data)
   with patch.object(client_release,'descriptor',return_value=self.lock(data)):
    with self.assertRaises(ValueError):client_release.prepare(p/'cache',archive)
   self.assertFalse((p/'cache/0.1.0-alpha.1').exists())
   archive.write_bytes(b'corrupt')
   with patch.object(client_release,'descriptor',return_value=self.lock(data)):
    with self.assertRaises(ValueError):client_release.prepare(p/'cache',archive)

 def test_aliases_injected_sources_and_linked_parent_directories_are_rejected(self):
  with tempfile.TemporaryDirectory() as t:
   p=Path(t);archive=p/'test.zip'
   for extra in ('./VERSION','clients//headers.py'):
    data=self.archive(extra);archive.write_bytes(data)
    with patch.object(client_release,'descriptor',return_value=self.lock(data)),self.assertRaises(ValueError):
     client_release.prepare(p/'bad',archive)
   data=self.archive();archive.write_bytes(data)
   with patch.object(client_release,'descriptor',return_value=self.lock(data)):
    target=client_release.prepare(p/'cache',archive)
    (target/'clients/__pycache__').mkdir();(target/'clients/__pycache__/headers.pyc').write_bytes(b'bytecode')
    self.assertEqual(client_release.prepare(p/'cache'),target)
    injected=target/'clients/extra.py';injected.write_text('untracked')
    with self.assertRaisesRegex(ValueError,'file set'):client_release.prepare(p/'cache')
    injected.unlink()
    (target/'clients').rename(p/'outside');(target/'clients').symlink_to(p/'outside',target_is_directory=True)
    with self.assertRaisesRegex(ValueError,'Linked'):client_release.prepare(p/'cache')
