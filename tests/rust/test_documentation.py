"""Execute documented discovery and examples against a disposable native service."""
import io,json,re,unittest,zipfile
from urllib.request import Request,urlopen
import test_service as services

class DocumentationWorkflowTests(unittest.TestCase):
 setUp=services.RustServiceTests.setUp
 stop=services.RustServiceTests.stop
 get=services.RustServiceTests.get
 post=services.RustServiceTests.post
 call=services.RustServiceTests.call
 wait=services.RustServiceTests.wait
 def test_guide_index_bundle_and_targeted_reference_are_consistent(self):
  index=self.call('system','guide')
  with urlopen(Request(self.url+'/api/guide/bundle',headers={'Authorization':'Bearer '+self.token}),timeout=5) as r:
   bundle=zipfile.ZipFile(io.BytesIO(r.read()))
  for document in index['documents']:
   self.assertIn(document['type'],('routing','how-to','reference','tutorial'))
   self.assertTrue(document['summary'])
   guide=self.call('system','guide',{'id':document['id']})
   self.assertEqual(bundle.read(document['path']).decode(),guide['content'])
  group=self.call('system','describe',{'group':'jobs'})
  action=self.call('system','describe',{'group':'jobs','action':'submit'})
  self.assertEqual(next(a for a in group['actions'] if a['action']=='submit'),action)
  self.assertTrue(self.call('system','guide',{'known_revision':index['manifest']['revision']})['not_modified'])
 def test_first_job_tutorial_runs_and_replay_preserves_one_execution(self):
  text=self.call('system','guide',{'id':'quickstart'})['content']
  request=json.loads(re.search(r'```json\n(.*?)\n```',text,re.S)[1])
  arguments=request['params']['arguments']['arguments'];arguments['owner']='owner';arguments['notify']='owner'
  # The actual tutorial request, through its documented MCP envelope.
  result=self.post('/mcp',request)['result'];self.assertFalse(result.get('isError',False))
  job=json.loads(result['content'][0]['text']);done=self.wait(job['id'])
  replay=self.post('/mcp',request)['result'];self.assertEqual(json.loads(replay['content'][0]['text'])['id'],job['id'])
  self.assertEqual(done['status'],'completed');self.assertEqual(len(done['attempts']),1)
  self.assertEqual(len(self.call('jobs','list',{'view':'all'})),1)
  self.assertTrue(done['result']['artifact_ready'])
  arguments['params']['seconds']=1
  conflict=self.post('/mcp',request)['result'];self.assertTrue(conflict['isError'])
 def test_advertised_authoring_examples_validate_real_generated_source(self):
  action=self.call('system','describe',{'group':'components','action':'scaffold'})
  draft=self.call('components','scaffold',action['example'])
  validation=self.call('components','validate_package',{'manifest':draft['manifest'],'files':draft['files']})
  self.assertTrue(validation['valid']);self.assertFalse(validation['build_executed'])
  sample=self.call('system','describe',{'group':'components','action':'validate_package'})
  self.assertTrue(self.call('components','validate_package',sample['example'])['valid'])
  bad=self.post('/mcp',{'jsonrpc':'2.0','id':9,'method':'tools/call','params':{'name':'components','arguments':{'action':'validate_package','arguments':{'manifest':{},'files':{'../escape.rs':'fn main() {}'}}}}})['result']
  self.assertTrue(bad['isError'])
