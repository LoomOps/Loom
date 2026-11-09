import io,json,zipfile,shutil,importlib.util,tempfile
from pathlib import Path
from unittest.mock import patch
from urllib.request import Request,urlopen
from urllib.error import HTTPError
import onboarding,ops
from client_source import PACKAGE
from test_api_contract import APIContractTests
from test_support import IsolatedOps

class GuideTests(IsolatedOps):
 def test_bundle_is_scoped_and_contains_no_credential(self):
  contract=ops.operate('get_job_contract',{},self.reader,self.config)
  data=onboarding.bundle(self.reader,ops.TOOLS,ops.tool_schema,contract)
  with zipfile.ZipFile(io.BytesIO(data)) as bundle:
   self.assertIn('entry.md',bundle.namelist());self.assertIn('skills/loom-use/SKILL.md',bundle.namelist())
   self.assertNotIn('clients/stdio.py',bundle.namelist());self.assertIn('client-release.json',bundle.namelist());self.assertIn('README.md',bundle.namelist())
   self.assertEqual(json.loads(bundle.read('manifest.json'))['package'],'loom_use')
   self.assertEqual(json.loads(bundle.read('manifest.json'))['title'],'Loom Use')
   for name in bundle.namelist():
    if name.endswith(('.md','.json')):
     self.assertNotRegex(bundle.read(name).decode(),r'[\uac00-\ud7a3]',name)
   self.assertFalse(any(n.startswith('instances/') or n.endswith('verify_entry.py') for n in bundle.namelist()))
   tools=json.loads(bundle.read('mcp-tools.json'))['tools'];self.assertEqual(len(tools),4)
   jobs=next(t for t in tools if t['name']=='jobs');self.assertNotIn('submit',jobs['inputSchema']['properties']['action']['enum'])
   raw=b''.join(bundle.read(name) for name in bundle.namelist());self.assertNotIn(self.reader['token_sha256'].encode(),raw)
 def test_same_revision_skips_docs_and_guide_change_is_detected(self):
  with tempfile.TemporaryDirectory() as temp:
   guides=Path(temp)/'guides';shutil.copytree(onboarding.GUIDES,guides)
   with patch.object(onboarding,'GUIDES',guides):
    before=onboarding.identity()
    self.assertTrue(onboarding.read({'known_revision':before['revision']})['not_modified'])
    p=guides/'jobs.md';p.write_text(p.read_text()+'\nChanged example.\n')
    after=onboarding.identity()
    self.assertNotEqual(before['guide_revision'],after['guide_revision']);self.assertEqual(before['contract_revision'],after['contract_revision'])
    self.assertNotIn('not_modified',onboarding.read({'known_revision':before['revision']}))
    with patch('build_version.status',return_value={'build_id':'next-deployment'}):
     self.assertNotEqual(after['revision'],onboarding.identity()['revision'])
 def test_guide_cannot_read_other_files(self):
  for name in ['../ops.py','/etc/passwd','secrets','entry.md',1]:
   with self.assertRaises(ValueError):onboarding.read({'id':name})

 def test_adapter_changes_refresh_the_usage_bundle_identity(self):
  with tempfile.TemporaryDirectory() as temp:
   guides=Path(temp)/'loom_use';shutil.copytree(onboarding.GUIDES,guides)
   with patch.object(onboarding,'GUIDES',guides):
    before=onboarding.identity();p=guides/'client-release.json';lock=json.loads(p.read_text());lock['version']='0.1.0-alpha.99';p.write_text(json.dumps(lock))
    after=onboarding.identity()
    self.assertNotEqual(before['guide_revision'],after['guide_revision'])
    self.assertEqual(before['contract_revision'],after['contract_revision'])

class GuideHTTPTests(APIContractTests):
 def test_error_trace_is_available_to_standard_mcp_clients(self):
  trace='02c11225fd5740a9833b384f68aad3af'
  with patch('ops.diagnostics.record',return_value=trace):
   error=self.request('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'system','arguments':{'action':'invalid'}}})[1]['result']
  self.assertTrue(error['isError'])
  self.assertEqual(error['structuredContent'],{'error':'tool_error','incident_id':trace})
  self.assertEqual(error['incident_id'],trace)
  self.assertIn('loom',error['_meta'])
 def test_authenticated_download_and_mcp_discovery_identity(self):
  self.assertEqual(self.request('/api/guide',token=None)[0],403)
  status,index=self.request('/api/guide');self.assertEqual(status,200)
  init=self.request('/mcp',{'jsonrpc':'2.0','id':1,'method':'initialize','params':{}})[1]['result']
  self.assertEqual(init['serverInfo']['title'],'Loom Use')
  self.assertIn('system.guide',init['instructions']);self.assertEqual(init['_meta']['loom']['revision'],index['manifest']['revision'])
  error=self.request('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'system','arguments':{'action':'guide','arguments':{'id':'../secret'}}}})[1]['result']
  self.assertTrue(error['isError']);self.assertIn('loom',error['_meta'])
  req=Request(self.base+'/api/guide/bundle',headers={'Authorization':'Bearer fixture-reader'})
  with urlopen(req) as response:
   self.assertEqual(response.headers['Content-Type'],'application/zip');self.assertEqual(response.headers['X-Loom-Revision'],index['manifest']['revision'])
   self.assertIn('manifest.json',zipfile.ZipFile(io.BytesIO(response.read())).namelist())
 def test_refresh_new_user_repeat_and_new_deployment(self):
  spec=importlib.util.spec_from_file_location('guide_refresh',PACKAGE/'refresh.py');refresh=importlib.util.module_from_spec(spec);spec.loader.exec_module(refresh)
  with tempfile.TemporaryDirectory() as temp:
   first=refresh.refresh(self.base,temp,'fixture-reader');self.assertTrue(first['updated'])
   self.assertFalse(refresh.refresh(self.base,temp,'fixture-reader')['updated'])
   with patch('build_version.status',return_value={'build_id':'new-deployment'}):
    second=refresh.refresh(self.base,temp,'fixture-reader');self.assertTrue(second['updated']);self.assertNotEqual(first['path'],second['path'])
   self.assertTrue(Path(first['path']).is_dir())
   other=refresh.refresh(self.base,temp,'fixture-owner');self.assertTrue(other['updated'])
   self.assertNotEqual(first['path'],other['path'])
 def test_refresh_rejects_unsafe_connection(self):
  spec=importlib.util.spec_from_file_location('guide_refresh',PACKAGE/'refresh.py');refresh=importlib.util.module_from_spec(spec);spec.loader.exec_module(refresh)
  for url in ['http://remote.example','https://user:secret@server','file:///etc/passwd']:
   with self.assertRaises(ValueError):refresh.refresh(url,self.root,'test')

 def test_visible_tool_version_preserves_first_payload_and_http_shape(self):
  body={'jsonrpc':'2.0','id':1,'method':'tools/call','params':{'name':'jobs','arguments':{'action':'list'}}}
  result=self.request('/mcp',body)[1]['result']
  self.assertEqual(json.loads(result['content'][0]['text']),self.request('/api/jobs')[1])
  self.assertEqual(len(result['content']),2)
  self.assertEqual(result['content'][-1]['text'],onboarding.visible_version(result['_meta']['loom']))
  with patch('build_version.status',return_value={'build_id':'updated-build'}):
   newer=self.request('/mcp',body)[1]['result']
  self.assertNotEqual(result['content'][-1]['text'],newer['content'][-1]['text'])
  self.assertEqual(result['content'][0],newer['content'][0])
  body['params']={'name':'system','arguments':{'action':'guide','arguments':{'id':'missing'}}}
  failure=self.request('/mcp',body)[1]['result']
  self.assertTrue(failure['isError']);self.assertEqual(failure['content'][0]['text'],'Unknown guide')
  self.assertEqual(failure['content'][-1]['text'],result['content'][-1]['text'])
  ping=self.request('/mcp',{'jsonrpc':'2.0','id':2,'method':'ping'})[1]['result']
  self.assertNotIn('content',ping)


 def test_tokenless_guide_refresh_binds_cache_to_verified_network_user(self):
  import access
  spec=importlib.util.spec_from_file_location('guide_refresh',PACKAGE/'refresh.py');refresh=importlib.util.module_from_spec(spec);spec.loader.exec_module(refresh)
  with tempfile.TemporaryDirectory() as temp:
   with self.assertRaises(HTTPError):refresh.refresh(self.base,temp,'',authentication='tailscale')
   def actor(login):return {**self.reader,'id':login.split('@')[0],'identity':{'provider':'tailscale','login':login,'name':login}}
   with patch.object(access,'tailscale_actor',return_value=actor('first@example.com')):
    first=refresh.refresh(self.base,temp,'',authentication='tailscale')
    self.assertFalse(refresh.refresh(self.base,temp,'',authentication='tailscale')['updated'])
   with patch.object(access,'tailscale_actor',return_value=actor('second@example.com')):
    second=refresh.refresh(self.base,temp,'',authentication='tailscale')
   self.assertNotEqual(first['path'],second['path'])
