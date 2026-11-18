"""Compile the actual API drafts and execute their producer protocol. No scientific output."""
from native_binaries import binary as native_binary
import hashlib,json,os,subprocess,unittest
from pathlib import Path
import test_service as services
ROOT=Path(__file__).resolve().parents[2]
class NativeAuthoringTests(unittest.TestCase):
 setUp=services.RustServiceTests.setUp
 stop=services.RustServiceTests.stop
 get=services.RustServiceTests.get
 post=services.RustServiceTests.post
 call=services.RustServiceTests.call
 def test_tool_component_and_hybrid_drafts_compile_and_emit_real_receipts(self):
  cargo=os.environ.get('CARGO','cargo');dependencies=self.root/'configured-dependencies';dependencies.mkdir();target=dependencies/'build-cache'
  for template in ('tool','component','hybrid'):
   with self.subTest(template=template):
    draft=self.call('components','scaffold',{'template':template,'name':'fixture-'+template});source=dependencies/template;source.mkdir()
    files=draft['files']
    if template=='hybrid':files['worker.py']="from pathlib import Path\nimport sys\n(Path(sys.argv[1])/'worker-receipt.txt').write_text('External worker fixture completed\\n')\n"
    for name,text in files.items():path=source/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
    env={**os.environ,'CARGO_TARGET_DIR':str(target)}
    for command in ([cargo,'generate-lockfile','--offline'],[cargo,'build','--offline','--locked']):
     result=subprocess.run(command,cwd=source,env=env,capture_output=True,text=True,timeout=90);self.assertEqual(result.returncode,0,result.stderr)
    files['Cargo.lock']=(source/'Cargo.lock').read_text();report=self.call('components','validate_package',{'manifest':draft['manifest'],'files':files});self.assertTrue(report['valid']);self.assertFalse(report['build_executed'])
    output=self.root/('output-'+template);output.mkdir();request=self.root/('request-'+template+'.json');request.write_text(json.dumps({'job':'fixture','attempt':'actual','stage':'run','output_dir':str(output),'component_path':str(source),'params':{},'profile':{'revision':'fixture.r1','sha256':'a'*64}}))
    result=subprocess.run([str(target/'debug'/('fixture-'+template)),str(request)],cwd=source,capture_output=True,text=True,timeout=10);self.assertEqual(result.returncode,0,result.stderr);manifest=json.loads((output/'result.json').read_text());self.assertEqual(len(manifest['entities']),1);self.assertTrue((output/manifest['entities'][0]['path']).is_file())
 def test_release_built_installation_producer_requires_actual_prepare_receipt(self):
  output=self.root/'installation-output';output.mkdir();request=self.root/'installation-request.json';definition={'job':'fixture','attempt':'one','stage':'run','output_dir':str(output),'params':{},'profile':{'revision':'fixture.r1','sha256':'a'*64}};request.write_text(json.dumps(definition));binary=native_binary('loom-sdk')
  missing=subprocess.run([str(binary),'--installation',str(request)],capture_output=True,text=True,timeout=5);self.assertNotEqual(missing.returncode,0);self.assertFalse((output/'result.json').exists())
  for stage in ('prepare','run'):
   definition['stage']=stage;request.write_text(json.dumps(definition));result=subprocess.run([str(binary),'--installation',str(request)],capture_output=True,text=True,timeout=5);self.assertEqual(result.returncode,0,result.stderr)
  manifest=json.loads((output/'result.json').read_text());self.assertEqual(manifest['entities'][0]['id'],'proof');self.assertEqual((output/'installation.txt').read_bytes(),b'Loom installation smoke\n')
