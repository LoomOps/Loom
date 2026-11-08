import hashlib
from pathlib import Path
import tempfile
import unittest
import dashboard_read as view

class FilePreviewTests(unittest.TestCase):
 def indexed(self,root,name,content):
  p=root/name;p.write_bytes(content);location={'source_ref':name,'target_ref':name,'status':'available','size':len(content),'sha256':hashlib.sha256(content).hexdigest()}
  index={};items=view.add_media(index,['fixture'],[location]);return index,items[0],location
 def test_markdown_json_and_empty_text_are_readable_without_media_relaxation(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp)
   for name,data,mime in [('README.md',b'# Heading','text/markdown'),('item.json',b'{"id":9007199254740993}','application/json'),('empty.txt',b'','text/plain')]:
    index,item,location=self.indexed(root,name,data)
    descriptor,stream=view.open_media(index,item['id'],root)
    self.assertEqual(descriptor['mime'],mime)
    with stream:self.assertEqual(stream.read(),data)
    files=view.file_entries(index,['fixture'],[location],'data/1')
    self.assertEqual(files[0]['path'],'data/1/'+name);self.assertEqual(files[0]['preview_id'],item['id'])
 def test_text_size_limit_and_integrity_remain_enforced(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);index,item,_=self.indexed(root,'large.md',b'x'*(view.MAX_TEXT_BYTES+1))
   self.assertEqual(item['status'],'too_large');self.assertIsNone(view.open_media(index,item['id'],root)[1])
   index,item,_=self.indexed(root,'small.json',b'{"x":1}')
   (root/'small.json').write_bytes(b'{"x":2}')
   self.assertEqual(view.open_media(index,item['id'],root)[0]['status'],'corrupt')
 def test_unknown_id_and_escape_are_rejected(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);index,item,location=self.indexed(root,'safe.json',b'{}')
   with self.assertRaises(FileNotFoundError):view.open_media(index,'unknown',root)
   location['target_ref']='../secret.json'
   with self.assertRaises(ValueError):view.open_media(index,item['id'],root)
 def test_unknown_file_type_remains_visible_but_not_served(self):
  location={'source_ref':'model.safetensors','target_ref':'models/model.safetensors','status':'available','size':999}
  index={};self.assertEqual(view.add_media(index,['x'],[location]),[])
  item=view.file_entries(index,['x'],[location],'data/0')[0]
  self.assertIsNone(item['preview_id']);self.assertEqual(item['path'],'data/0/models/model.safetensors')
