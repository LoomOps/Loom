"""Explicit, bounded CPU installation Job. Doctor itself never submits work."""
import hashlib
import json
import time
from pathlib import Path

SOURCE = '''import json,sys
from pathlib import Path
r=json.loads(Path(sys.argv[-1]).read_text());p=Path(r['output_dir'])
if sys.argv[1]=='prepare':
 (p/'prepared.txt').write_text('ready')
else:
 assert (p/'prepared.txt').read_text()=='ready'
 (p/'installation.txt').write_text('Loom installation smoke\\n')
 (p/'result.json').write_text(json.dumps({'entities':[{'id':'proof','kind':'artifact','path':'installation.txt','provenance':{'profile':r['profile']['revision'],'producer_sha256':r['profile']['sha256']},'relations':[]}],'metrics':[]}))
'''


def call(setup, plan, group, action, arguments=None):
    reply=setup.read_api(plan,'/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/call','params':{
        'name':group,'arguments':{'action':action,'arguments':arguments or {}}}})
    result=reply.get('result',{})
    if reply.get('error') or result.get('isError'):raise ValueError('Installation smoke operation failed: '+group+'.'+action)
    return json.loads(result['content'][0]['text'])


def verify(setup, plan):
    receipt=json.loads((setup.ROOT/'config/install-smoke.json').read_text())
    if receipt.get('build')!=setup.manifest()['build_id'] or receipt.get('source_sha256')!=hashlib.sha256(SOURCE.encode()).hexdigest():return False
    job=call(setup,plan,'jobs','get',{'id':receipt['job']})
    if job['state']!='complete' or not job.get('result',{}).get('execution',{}).get('process_group_returned'):return False
    if not all(any(s['stage']==stage and s.get('ended') for s in job['stages']) for stage in ('prepare','run')):return False
    entity=call(setup,plan,'results','get',{'id':receipt['job']+'.proof','revision':1})
    output=next(e for e in job['result']['entities'] if e['id']=='proof')
    path=Path(plan['data'])/output['locator']['ref']
    if not path.resolve().is_relative_to(Path(plan['data']).resolve()):return False
    return (entity['job']==receipt['job'] and path.read_text()=='Loom installation smoke\n'
            and hashlib.sha256(path.read_bytes()).hexdigest()==output['sha256'])


def run(setup, plan, timeout=45):
    from tailnet import write
    uid,gid=setup.ROOT.stat().st_uid,setup.ROOT.stat().st_gid
    receipt_path=setup.ROOT/'config/install-smoke.json'
    build=setup.manifest()['build_id'];digest=hashlib.sha256(SOURCE.encode()).hexdigest()
    receipt=json.loads(receipt_path.read_text()) if receipt_path.exists() else {}
    if receipt.get('build')!=build or receipt.get('source_sha256')!=digest:
        # Snapshot the component revision before submission; retries reuse this exact Job.
        profile={'params_schema':{'type':'object','additionalProperties':False},'inputs':{},'outputs':{'proof':'artifact'},
                 'limits':{'timeout_seconds':60,'max_attempts':1},
                 'prepare':{'executor':'container','runtime_id':'python','command':['python3','/component/run.py','prepare'],'lane':'cpu'},
                 'runtime':{'executor':'container','runtime_id':'python','command':['python3','/component/run.py','run'],'lane':'cpu'}}
        registered=call(setup,plan,'components','register',{'id':'installation-smoke-'+build,'manifest':{'profiles':{'check':profile}},'files':{'run.py':SOURCE}})
        receipt={'build':build,'source_sha256':digest,'kind':registered['profiles'][0]}
        write(receipt_path,receipt,uid,gid)
    if not receipt.get('job'):
        job=call(setup,plan,'jobs','submit',{'key':'installation-smoke-'+build+'-'+digest[:12],
            'kind':receipt['kind'],'owner':'owner','notify':'owner','params':{}})
        receipt['job']=job['id'];write(receipt_path,receipt,uid,gid)
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        job=call(setup,plan,'jobs','get',{'id':receipt['job']})
        if job['state']=='complete':
            if not verify(setup,plan):raise ValueError('Smoke Job completed but artifact/resource evidence failed')
            return {'status':'passed','job':receipt['job'],'report':str(receipt_path),
                    'scope':'MCP submit, container preparation/execution, result registration and local artifact digest. External-client download not claimed.'}
        if job['state'] in ('failed','result_failed','cancelled','interrupted'):return {'status':'incomplete','job':receipt['job'],'state':job['state']}
        time.sleep(1)
    return {'status':'incomplete','job':receipt['job'],'state':job['state'],
            'next':'Inspect this Job and rerun smoke to recheck it. The queue was not reordered or cancelled.'}
