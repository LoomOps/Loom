"""Independent owner profiles. Jobs freeze a revision; registration needs no deploy."""
import hashlib
import json
import os
from pathlib import Path
import execution
from runtime_contract import seal

ROOT=Path(os.environ.get('LOOM_PROFILES','/loom/profiles'))


RUNNER_SCHEMA={'type':'object','properties':{
    'executor':{'const':'container'},'command':{'type':'array','minItems':1,'items':{'type':'string','minLength':1}},
    'image':{'type':'string','description':'Cached Docker reference; resolved to image ID when registering.'},
    'runtime_id':{'type':'string','description':'Optional administrator-installed compatibility runtime.'},
    'resources':{'type':'object','properties':{'cpus':{'type':'number','exclusiveMinimum':0},'memory':{'type':'string','pattern':'^[1-9][0-9]*[kmg]$'},'pids':{'type':'integer','minimum':16}},'additionalProperties':False},
    'storage':{'type':'array','uniqueItems':True,'items':{'type':'string','pattern':'^data/[0-9]+$'},'description':'Opt-in RW stores from system.status.data_stores; mounted at /loom/data/N. Omit for only the Job workspace.'},
    'lane':{'enum':['cpu','gpu']},'env':{'type':'object','additionalProperties':{'type':'string'}},
    'timeout':{'type':'integer','minimum':1},'depends_on':{'type':'array','items':{'type':'string'}},'inputs':{'type':'array','items':{'type':'string'}}},
    'required':['executor','command'],'oneOf':[{'required':['image'],'not':{'required':['runtime_id']}},{'required':['runtime_id'],'not':{'required':['image']}}], 'additionalProperties':False}
PROFILE_SCHEMA={'type':'object','properties':{
    'params_schema':{'type':'object','description':'Local JSON Schema for resolved Job params.'},
    'defaults':{'type':'object'},'inputs':{'type':'object','additionalProperties':{'type':'string'},'description':'Input parameter name to allowed data-relative prefix.'},
    'outputs':{'type':['object','null'],'additionalProperties':{'enum':['artifact','dataset','model','experiment']}},
    'metadata_schemas':{'type':'object','additionalProperties':{'type':['object','boolean']},'description':'Output ID or * to local JSON Schema for entity.metadata.'},
    'limits':{'type':'object','properties':{'timeout_seconds':{'type':'integer','minimum':1},'max_attempts':{'type':'integer','minimum':1,'maximum':100},'max_output_bytes':{'type':'integer','minimum':1}},'required':['timeout_seconds','max_attempts'],'additionalProperties':False},
    **{name:RUNNER_SCHEMA for name in ('runtime','prepare','finalize','on_complete','on_error','on_cancel')}},
    'required':['params_schema','inputs','outputs','runtime','limits'],'additionalProperties':False}

def init(c):
    c.execute('CREATE TABLE IF NOT EXISTS execution_profiles(owner TEXT,id TEXT,revision INTEGER,profile TEXT,PRIMARY KEY(owner,id,revision))')

def action_schema(action):
    fields={'owner':{'type':'string'},'id':{'type':'string','pattern':'^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$'}}
    required=['id']
    if action=='register_profile':
        fields.update(profile=PROFILE_SCHEMA,
                      component={'type':'object','properties':{'id':{'type':'string'},'revision':{'type':'integer','minimum':1}},'required':['id','revision'],'additionalProperties':False})
        required.append('profile')
    else:fields['revision']={'type':'integer','minimum':1}
    return {'type':'object','properties':fields,'required':required,'additionalProperties':False}

def bind_runners(profile,config,owner):
    from runtime_client import call
    for name in ('runtime','prepare','finalize','on_complete','on_error','on_cancel'):
        runner=profile.get(name)
        if not runner:continue
        if runner.get('executor')!='container':raise ValueError('Profiles require container execution')
        execution.validate_storage(runner.get('storage',[]))
        if set(runner.get('storage',[]))-set(config.get('executor',{}).get('storage',{})):raise ValueError('Storage outside instance grant')
        if runner.get('lane','cpu')=='gpu' and not config.get('allow_gpu',False):raise PermissionError('GPU execution disabled for this instance')
        if 'image' in runner:
            if 'runtime_id' in runner:raise ValueError('Choose image or runtime_id')
            if 'executor' not in config:raise ValueError('Container executor unavailable')
            resolved=call(config['executor'],'resolve',{'image':runner['image'],'resources':runner.get('resources',{}),'owner':owner})
            runner['image']=resolved['image'];runner['resources']=resolved['resources']
        elif runner.get('runtime_id') not in config.get('runtime_ids',[]):raise ValueError('Select an available container runtime')

def operate(c,actor,args,config):
    from component_store import ID,allowed
    owner=args.get('owner',actor['id']);identity=args['id']
    if not allowed(actor,owner):raise PermissionError('Profile outside delegated owner')
    if not ID.fullmatch(owner) or not ID.fullmatch(identity):raise ValueError('Invalid profile identity')
    if args['action']=='get_profile':
        query='SELECT * FROM execution_profiles WHERE owner=? AND id=?';values=[owner,identity]
        if 'revision' in args:query+=' AND revision=?';values.append(args['revision'])
        row=c.execute(query+' ORDER BY revision DESC LIMIT 1',values).fetchone()
        if not row:raise ValueError('Profile not found')
        return {**dict(row),'profile':json.loads(row['profile'])}
    definition=json.loads(json.dumps(args['profile'],allow_nan=False))
    forbidden={'revision','sha256','schema','owners','submitters','component_path','source_pins','_hook_retry'}
    if forbidden & definition.keys():raise ValueError('Profile identity and component pins are assigned by Loom')
    execution.validate_profile(definition)
    bind_runners(definition,config,owner)
    if component:=args.get('component'):
        row=c.execute('SELECT manifest,profiles FROM components WHERE owner=? AND id=? AND revision=?',(owner,component['id'],component['revision'])).fetchone()
        if not row:raise ValueError('Component revision not found')
        from component_store import ROOT as COMPONENTS
        folder=COMPONENTS/owner/component['id']/'revisions'/str(component['revision'])
        manifest=json.loads(row['manifest'])
        pins=manifest.get('source_pins')
        if pins is None:
            old=next(iter(json.loads(row['profiles']).values()),None)
            if not old:raise ValueError('Component source pins unavailable')
            pins=old['source_pins']
        definition.update(component_path=str(folder),source_pins=pins)
    c.execute('BEGIN IMMEDIATE')
    version=c.execute('SELECT coalesce(max(revision),0)+1 FROM execution_profiles WHERE owner=? AND id=?',(owner,identity)).fetchone()[0]
    kind=f'{owner}.{identity}.r{version}'
    profile=seal({**definition,'schema':execution.REVISION,'revision':kind,'owners':[owner],'submitters':[actor['id'],owner]})
    folder=ROOT/owner/identity/'revisions'
    if not folder.resolve().is_relative_to(ROOT.resolve()):raise ValueError('Profile leaves owner root')
    folder.mkdir(parents=True,exist_ok=True)
    path=folder/f'{version}.json'
    # DB rollback may leave an unreferenced identical candidate. Never replace a revision.
    encoded=json.dumps(profile,sort_keys=True,indent=2)
    if path.exists() and path.read_text()!=encoded:raise ValueError('Profile snapshot conflict')
    if not path.exists():
        with path.open('x') as f:f.write(encoded)
    c.execute('INSERT INTO execution_profiles VALUES(?,?,?,?)',(owner,identity,version,json.dumps(profile)))
    return {'owner':owner,'id':identity,'revision':version,'kind':kind,'sha256':profile['sha256']}

def merge(c,profiles,actor):
    from component_store import allowed
    for row in c.execute('SELECT owner,profile FROM execution_profiles'):
        if actor is not None and not allowed(actor,row['owner']):continue
        p=json.loads(row['profile'])
        if actor is not None and actor['id'] not in p['submitters']:p['submitters'].append(actor['id'])
        profiles[p['revision']]=p
    return profiles
