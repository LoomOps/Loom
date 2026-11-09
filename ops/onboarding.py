"""Versioned, image-local guidance and a credential-free scoped handoff bundle."""
import hashlib,io,json,zipfile
from functools import lru_cache
from pathlib import Path
import build_version,job_contract,mcp_groups,mcp_docs

ROOT=Path(__file__).resolve().parent
GUIDES=ROOT.parent/'loom_use'
TITLES={'entry':'Server entry','quickstart':'First use','jobs':'Jobs and pipelines',
        'components':'Components and dependencies','recovery':'Completion and error recovery','updates':'Version updates',
        'operator':'Administrator installation and operations','inventory':'Package inventory'}
CONTRACT_FILES=('storage.py','mcp_groups.py','mcp_docs.py','ops.py','job_contract.py','execution.py',
                'component_store.py','tool_store.py','profile_store.py','runtime_contract.py','queue_control.py','recovery.py','onboarding.py')

def digest(value):return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':')).encode()).hexdigest()

def packaged_files():
 return [GUIDES/(name+'.md') for name in TITLES]+[GUIDES/name for name in
         ('LICENSE','THIRD_PARTY_NOTICES.md','README.md','client-release.json','skills/loom-use/SKILL.md')]

@lru_cache(maxsize=8)
def revisions(signature):
 return (digest({name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in CONTRACT_FILES}),
         digest({str(p.relative_to(GUIDES)):hashlib.sha256(p.read_bytes()).hexdigest() for p in packaged_files()}))

def identity():
 paths=[ROOT/name for name in CONTRACT_FILES]+packaged_files()
 signature=tuple((str(p),p.stat().st_mtime_ns,p.stat().st_size) for p in paths)
 contract,guide=revisions(signature)
 values={'build_id':build_version.status()['build_id'],'api_revision':job_contract.API_REVISION,
         'contract_revision':contract,'guide_revision':guide,'client_release':json.loads((GUIDES/'client-release.json').read_text())}
 return {**values,'revision':digest(values)}

def manifest():
 return {**identity(),'schema':'loom-onboarding/v1','package':'loom_use','title':'Loom Use','entry':'entry',
         'mcp':{'endpoint':'/mcp','tool':'system','action':'guide'},
         'http':{'index':'/api/guide','bundle':'/api/guide/bundle'},
         'updates':{'mode':'check_on_connect_and_response','push':False,'changes_running_jobs':False}}

def read(arguments):
 if not isinstance(arguments,dict) or set(arguments)-{'id','known_revision'}:raise ValueError('Invalid guide arguments')
 if any(not isinstance(v,str) for v in arguments.values()):raise ValueError('Guide arguments must be strings')
 current=manifest();name=arguments.get('id')
 if name is not None and name not in TITLES:raise ValueError('Unknown guide')
 if arguments.get('known_revision')==current['revision']:return {'manifest':current,'not_modified':True}
 if name is not None:return {'manifest':current,'id':name,'title':TITLES[name],'content':(GUIDES/(name+'.md')).read_text()}
 return {'manifest':current,'documents':[{'id':name,'title':title,'path':name+'.md'} for name,title in TITLES.items()]}

def bundle(actor,known,schema_for,contract):
 buffer=io.BytesIO()
 with zipfile.ZipFile(buffer,'w',zipfile.ZIP_DEFLATED) as archive:
  for p in packaged_files():archive.writestr(str(p.relative_to(GUIDES)),p.read_bytes())
  payloads={'manifest.json':manifest(),'mcp-tools.json':{'tools':mcp_groups.tools(actor,known)},
            'mcp-reference.json':mcp_docs.reference(actor,known,schema_for,job_contract.API_REVISION),
            'job-contract.json':contract}
  for name,value in payloads.items():archive.writestr(name,json.dumps(value,ensure_ascii=False,indent=2)+'\n')
 return buffer.getvalue()


def visible_version(value):
 return (f"Loom {value['build_id']} | contract {value['contract_revision'][:12]}"
         f" | guide {value['guide_revision'][:12]}")
