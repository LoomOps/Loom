"""Runtimes workspace curation, exact Profile comparisons and read-only preparation.

Display metadata never changes source, grants, selected execution pins or Jobs.
"""
import json
import time
import component_store
import execution
import job_contract

STAGES=('prepare','runtime','finalize','on_complete','on_error','on_cancel')
SELECTOR={'type':'object','properties':{'name':{'type':'string','minLength':1,'maxLength':160},'scope':{'type':'string','maxLength':1024}},'required':['name'],'additionalProperties':False}
SCHEMAS={
 'curate_runtime_item':{'type':'object','properties':{
  'type':{'enum':['profile','runtime']},'id':{'type':'string','minLength':1,'maxLength':256},'revision':{'type':'integer','minimum':1},
  'label':{'type':'string','minLength':1,'maxLength':100},'categories':{'type':'array','uniqueItems':True,'items':{'enum':list(job_contract.CATEGORIES)}},
  'execution_modes':{'type':'array','uniqueItems':True,'items':{'enum':list(job_contract.MODES)}},
  'group':{'enum':['research','platform-check','other']},'state':{'enum':['active','archived']},'preferred':{'type':'boolean'},
  'primary_dependencies':{'type':'array','maxItems':8,'uniqueItems':True,'items':SELECTOR},
  'stage_dependencies':{'type':'object','propertyNames':{'enum':list(STAGES)},'additionalProperties':{'type':'array','maxItems':8,'uniqueItems':True,'items':SELECTOR}},
  'tool_roles':{'type':'object','maxProperties':64,'additionalProperties':{'type':'object','properties':{
   'role':{'type':'string','minLength':1,'maxLength':80},'stages':{'type':'array','uniqueItems':True,'items':{'enum':list(STAGES)}}},'required':['role','stages'],'additionalProperties':False}},
  'evidence':{'type':'string','minLength':1,'maxLength':2000}},'required':['type','id','evidence'],'additionalProperties':False},
 'inspect_execution_profile':{'type':'object','properties':{'kind':{'type':'string'},'params':{'type':'object'}},'required':['kind'],'additionalProperties':False},
 'compare_execution_profiles':{'type':'object','properties':{'left':{'type':'string'},'right':{'type':'string'}},'required':['left','right'],'additionalProperties':False}}


def init(c):
 c.execute('''CREATE TABLE IF NOT EXISTS runtime_curation(kind TEXT,id TEXT,revision INTEGER,value TEXT,actor TEXT,updated REAL,
 PRIMARY KEY(kind,id,revision))''')


def read(c,kind,identity,revision=0):
 if not c.execute("SELECT 1 FROM sqlite_master WHERE name='runtime_curation' AND type='table'").fetchone():return {}
 row=c.execute('SELECT value FROM runtime_curation WHERE kind=? AND id=? AND revision=?',(kind,identity,revision)).fetchone()
 return json.loads(row[0]) if row else {}


def family(kind,origin):
 if origin['type']=='standalone':return 'profile:'+origin['owner']+':'+origin['id']
 if origin['type']=='component':
  prefix=origin['owner']+'.'+origin['id']+'.'
  suffix='.r'+str(origin['revision'])
  if kind.startswith(prefix) and kind.endswith(suffix):return 'component:'+origin['owner']+':'+origin['id']+':'+kind[len(prefix):-len(suffix)]
 return kind


def curate(c,actor,args,config):
 if 'submit' not in actor['scopes']:raise PermissionError('Submit grant required')
 execution.validate(SCHEMAS['curate_runtime_item'],args)
 identity=args['id'];kind=args['type'];revision=args.get('revision',0)
 import runtime_catalog
 catalog=runtime_catalog.listing(config)
 component_store.mutation(c)
 if kind=='profile':
  if revision or 'primary_dependencies' in args:raise ValueError('Select the exact Profile kind; environment highlights belong to a Runtime')
  import profile_inventory
  rows=profile_inventory.inventory(c,config,actor,catalog)
  item=next((p for p in rows if p['kind']==identity),None)
  if item is None:raise ValueError('Profile not found')
  aliases={p['alias'] for p in item['tools']};stages={s['stage'] for s in item['stages']}
  for stage,selectors in args.get('stage_dependencies',{}).items():
   entry=next((s for s in item['stages'] if s['stage']==stage),None)
   if entry is None:raise ValueError('Unknown dependency stage')
   env=entry['environment'];runtime=next((r for r in catalog['runtimes'] if 'runtime:'+r['id']+':'+str(r['revision'])==env.get('entity')),None)
   if runtime is None and selectors:raise ValueError('No verified inventory for the selected stage')
   runtime_catalog.primary_dependencies(runtime['manifest'] if runtime else {'dependencies':[]},selectors,strict=True)
  for alias,role in args.get('tool_roles',{}).items():
   if alias not in aliases or set(role['stages'])-stages:raise ValueError('Role references an unknown Tool alias or stage')
 else:
  if not revision or set(args)&{'categories','execution_modes','group','preferred','tool_roles','stage_dependencies'}:raise ValueError('Select an exact Runtime revision and Runtime metadata')
  import runtime_catalog
  item=next((r for r in catalog['runtimes'] if (r['id'],r['revision'])==(identity,revision)),None)
  if item is None:raise ValueError('Runtime not found or outside instance grant')
  runtime_catalog.primary_dependencies(item['manifest'],args.get('primary_dependencies',[]),strict=True)
 value={**read(c,kind,identity,revision),**{k:v for k,v in args.items() if k not in ('type','id','revision')}}
 if kind=='profile' and value.get('preferred') and value.get('state')=='archived':raise ValueError('Archived Profile cannot be preferred')
 if kind=='profile' and value.get('preferred'):
  for p in rows:
   if p['kind']!=identity and p['family']==item['family'] and p.get('curation',{}).get('preferred'):raise ValueError('Clear the existing preferred revision first')
 encoded=json.dumps(value,sort_keys=True,separators=(',',':'))
 if read(c,kind,identity,revision)==value:return {**value,'unchanged':True}
 c.execute('INSERT INTO runtime_curation VALUES(?,?,?,?,?,?) ON CONFLICT(kind,id,revision) DO UPDATE SET value=excluded.value,actor=excluded.actor,updated=excluded.updated',
  (kind,identity,revision,encoded,actor['id'],time.time()))
 return value


def definitions(c,config,actor):return component_store.merge(c,config,actor)['profiles']


def compare(c,actor,args,config):
 ps=definitions(c,config,actor)
 if any(k not in ps for k in (args['left'],args['right'])):raise ValueError('Profile not found')
 def normalized(p):
  # Identity/display/attribution are excluded; environment values are never returned.
  return {k:v for k,v in p.items() if k not in ('revision','sha256','usage','owners','submitters')}
 changes=[]
 def walk(a,b,path=''):
  if isinstance(a,dict) and isinstance(b,dict):
   for k in sorted(set(a)|set(b)):
    name=(path+'.' if path else '')+k
    if k not in a or k not in b:changes.append({'path':name,'change':'added' if k not in a else 'removed'})
    else:walk(a[k],b[k],name)
  elif a!=b:changes.append({'path':path,'change':'changed'})
 walk(normalized(ps[args['left']]),normalized(ps[args['right']]))
 return {'left':args['left'],'right':args['right'],'execution_equal':not changes,'changes':changes[:200],'truncated':len(changes)>200}


def inspect_profile(c,actor,args,config):
 import profile_inventory,runtime_catalog
 ps=definitions(c,config,actor);kind=args['kind']
 if kind=='cpu.smoke':
  p={'params_schema':{'type':'object','properties':{'seconds':{'type':'integer','minimum':0,'maximum':20},'fail':{'type':'boolean'}},'required':['seconds','fail'],'additionalProperties':False},'defaults':{},'inputs':{},'outputs':{'observation':'artifact'}}
 elif kind not in ps:raise ValueError('Profile not found')
 else:p=ps[kind]
 catalog=runtime_catalog.listing(config)
 combo=next(x for x in profile_inventory.inventory(c,config,actor,catalog) if x['kind']==kind)
 checks=[]
 for stage in combo['stages']:
  env=stage['environment'];status='unverified' if env['type'] in ('legacy','image') else 'observed' if env['type']=='observed' else 'pinned' if env['type']=='registered' else 'builtin'
  if env['type']=='registered':
   import runtime_client,http.client
   try:
    resolved=runtime_client.call(config['executor'],'resolve_runtime',{'runtime_ref':{'id':env['id'],'revision':env['revision']}})
    if resolved['runtime_ref']['sha256']!=env['sha256']:status='mismatch'
   except (OSError,ValueError,RuntimeError,PermissionError,KeyError,http.client.HTTPException):status='unavailable'
  checks.append({'stage':stage['stage'],'check':'runtime','status':status})
 manifests=[]
 if combo['component']:
  ref=combo['component'];row=c.execute('SELECT manifest FROM components WHERE owner=? AND id=? AND revision=?',tuple(ref[k] for k in ('owner','id','revision'))).fetchone()
  manifests.append((ref['id'],json.loads(row[0])))
 for ref in combo['tools']:
  row=c.execute('SELECT manifest FROM tools WHERE owner=? AND id=? AND revision=?',tuple(ref[k] for k in ('owner','id','revision'))).fetchone()
  manifests.append((ref['alias'],json.loads(row[0])))
 for software,manifest in manifests:
  for check in runtime_catalog.requirement_report(manifest,p,catalog):checks.append({'software':software,**check})
 params=execution.apply_defaults(p,args.get('params',{}));validation={'status':'not_checked'}
 if 'params' in args:
  try:execution.validate(p['params_schema'],params)
  except ValueError as error:validation={'status':'invalid','message':str(error)}
  else:validation={'status':'valid'}
 return {'kind':kind,'profile_sha256':p.get('sha256'),'combination':combo,'checks':checks,'params':validation,
  'params_schema':p['params_schema'],'defaults':p.get('defaults',{}),'inputs':p.get('inputs',{}),'outputs':p.get('outputs'),
  'draft':{'kind':kind,'params':params},'scope':'Read-only static inspection and current installation evidence; no resource reservation, import, GPU validation or Job submission.'}


def software_usage(c):
 """Exact pinned availability in started Jobs; does not claim Tool invocation."""
 import job_work
 from pathlib import Path
 links=job_work.completed_links(c);started={r[0] for r in c.execute('SELECT DISTINCT job FROM attempts WHERE started IS NOT NULL')};index={}
 for identity,encoded in c.execute('SELECT id,spec FROM jobs'):
  if identity not in started:continue
  profile=json.loads(encoded).get('profile',{});refs=[]
  for pin in profile.get('tool_pins',{}).values():refs.append(('tool',pin['owner'],pin['id'],pin['revision']))
  try:
   parts=Path(profile['component_path']).relative_to(component_store.ROOT).parts
   if len(parts)==4 and parts[2]=='revisions':refs.append(('component',parts[0],parts[1],int(parts[3])))
  except (KeyError,ValueError):pass
  for ref in refs:index.setdefault(ref,set()).add(links.get(identity,identity))
 return index
