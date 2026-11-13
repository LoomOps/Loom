"""Versioned, application-independent execution and preparation contracts."""
import storage
import job_work
import job_contract
import copy
import json
import math
import re
import os
from pathlib import Path

from jsonschema import Draft202012Validator, ValidationError, SchemaError

REVISION = 'loom-execution/v2'
USAGE_SCHEMA={'type':'object','properties':{
    'label':{'type':'string','minLength':1,'maxLength':160},
    'description':{'type':'string','maxLength':2000},
    'categories':{'type':'array','minItems':1,'uniqueItems':True,'items':{'enum':list(job_contract.CATEGORIES)}},
    'execution_modes':{'type':'array','minItems':1,'uniqueItems':True,'items':{'enum':list(job_contract.MODES)}}},
    'additionalProperties':False}
POLICY_SCHEMA = {'type':'object','properties':{
    'timeout_seconds':{'type':'integer','minimum':1},
    'max_attempts':{'type':'integer','minimum':1},
    'retry_on':{'type':'array','uniqueItems':True,'items':{'type':'string','enum':[
        'exit_nonzero','timeout','launch_failed','interrupted']}},
    'retry_backoff_seconds':{'type':'integer','minimum':0,'maximum':86400}},
    'additionalProperties':False}


def check_schema(schema):
    # Local contracts never fetch remote schema references at admission time.
    def walk(value):
        if isinstance(value,dict):
            for key,item in value.items():
                if key in ('$ref','$dynamicRef') and (not isinstance(item,str) or not item.startswith('#')):
                    raise ValueError('Only local schema references are supported')
                walk(item)
        elif isinstance(value,list):
            for item in value:walk(item)
    walk(schema)
    try:Draft202012Validator.check_schema(schema)
    except SchemaError as exc:raise ValueError('Invalid parameter schema: '+exc.message) from exc


def validate(schema,value):
    try:
        json.dumps(value,allow_nan=False)
        Draft202012Validator(schema).validate(value)
    except (ValidationError,TypeError,ValueError) as exc:
        raise ValueError('Invalid parameters: '+getattr(exc,'message',str(exc))) from exc


def is_generic(profile):
    return profile.get('schema') == REVISION


def validate_storage(stores):
    if not isinstance(stores,list) or any(not isinstance(s,str) or not re.fullmatch(r'data/[0-9]+',s) for s in stores) or len(stores)!=len(set(stores)):
        raise ValueError('Storage must be unique data/N identifiers')

def validate_profile(profile):
    if not isinstance(profile,dict):raise ValueError('Profile must be an object')
    if 'usage' in profile:validate(USAGE_SCHEMA,profile['usage'])
    if 'presentation' in profile:
        import research
        validate(research.PROFILE_PRESENTATION,profile['presentation'])
        if profile.get('outputs') is not None and set(profile['presentation'].get('output_roles',{}))-set(profile['outputs']):raise ValueError('Presentation names an undeclared output')
    job_work.validate_steps(profile.get('steps',[]))
    for field in ('params_schema','runtime','inputs','limits'):
        if not isinstance(profile.get(field),dict):raise ValueError('Profile requires object '+field)
    if 'outputs' not in profile or profile['outputs'] is not None and not isinstance(profile['outputs'],dict):raise ValueError('Outputs must be a mapping or null')
    if not isinstance(profile.get('metadata_schemas',{}),dict):raise ValueError('Metadata schemas must be a mapping')
    if profile['outputs'] is not None and set(profile.get('metadata_schemas',{}))-set(profile['outputs'])-{'*'}:raise ValueError('Metadata schema names an undeclared output')
    check_schema(profile['params_schema'])
    if profile['params_schema'].get('type')!='object':raise ValueError('Parameters must have an object schema')
    for runner in [profile['runtime']] + [profile[k] for k in ('prepare','finalize','on_error','on_complete','on_cancel') if profile.get(k)]:
        if set(runner)-{'command','env','lane','timeout','depends_on','executor','runtime_id','runtime_ref','runtime_pin','image','resources','inputs','storage'}:raise ValueError('Unknown runtime fields')
        validate_storage(runner.get('storage',[]))
        command=runner['command']
        if not isinstance(command,list) or not command or any(not isinstance(v,str) or not v or '\0' in v for v in command):
            raise ValueError('Runtime requires a trusted argv list')
        if not isinstance(runner.get('env',{}),dict) or any(not isinstance(k,str) or not isinstance(v,str) for k,v in runner.get('env',{}).items()):
            raise ValueError('Runtime environment must contain strings')
        if runner.get('lane','cpu') not in ('cpu','gpu'):raise ValueError('Unknown execution lane')
    for name in ('finalize','on_error','on_complete','on_cancel'):
        if profile.get(name,{}).get('lane','cpu')!='cpu':raise ValueError('Lifecycle callbacks use CPU')
    if profile.get('prepare') and profile['prepare'].get('lane','cpu')!='cpu':raise ValueError('Preparation uses the CPU lane')
    limits=profile['limits']
    for name in ('timeout_seconds','max_attempts'):
        if type(limits[name]) is not int or limits[name]<1:raise ValueError('Invalid execution limit')
    if limits['max_attempts']>100:raise ValueError('Invalid retry limit')
    if 'max_output_bytes' in limits and (type(limits['max_output_bytes']) is not int or limits['max_output_bytes']<1):raise ValueError('Invalid output limit')
    for output_id,schema in profile.get('metadata_schemas',{}).items():
        if not isinstance(output_id,str):raise ValueError('Invalid output schema identity')
        check_schema(schema)
    for prefix in profile.get('inputs',{}).values():
        if not isinstance(prefix,str) or Path(prefix).is_absolute() or '..' in Path(prefix).parts:raise ValueError('Invalid input prefix')
    timeout=profile.get('prepare',{}).get('timeout',limits['timeout_seconds'])
    if type(timeout) is not int or timeout<1 or timeout>limits['timeout_seconds']:raise ValueError('Invalid preparation timeout')


def policy(spec,profile):
    requested=spec.get('execution',{})
    validate(POLICY_SCHEMA,requested)
    limits=profile['limits'] if is_generic(profile) else {'timeout_seconds':profile.get('timeout',30),'max_attempts':profile.get('budget',{}).get('attempts',1)}
    resolved={'timeout_seconds':limits['timeout_seconds'],'max_attempts':1,'retry_on':[],'retry_backoff_seconds':5,**requested}
    if any(resolved[k]>limits[k] for k in ('timeout_seconds','max_attempts')):raise ValueError('Execution policy exceeds installed runtime limits')
    return resolved


def input_dependencies(profile,params):
    return [params[name]['job'] for name in profile.get('inputs',{}) if isinstance(params.get(name),dict) and 'job' in params[name]]


def admit_inputs(profile,params,config,bulk,relative,ident,dependencies):
    for name,prefix in profile.get('inputs',{}).items():
        item=params.get(name)
        if not isinstance(item,dict):raise ValueError('Declared input must be an artifact locator')
        if set(item)=={'job','entity'}:
            ident(item['job']);ident(item['entity'])
            if item['job'] not in dependencies:raise ValueError('Input Job must be a dependency')
            continue
        if set(item)!={'machine_id','storage_id','ref','sha256'}:raise ValueError('Expected locator or upstream Job/entity')
        base=storage.locator(config,bulk,item)
        path=relative(base,item['ref'])
        if not path.is_relative_to(relative(base,prefix)):raise ValueError('Input outside registered prefix')
        digest=item['sha256']
        if not isinstance(digest,str) or len(digest)!=64 or any(c not in '0123456789abcdef' for c in digest):raise ValueError('Invalid input digest')
        if not path.is_file():raise ValueError('Input file not found')


def resolve_inputs(profile,params,config,bulk,relative,file_sha,lookup):
    resolved=copy.deepcopy(params)
    for name,prefix in profile.get('inputs',{}).items():
        item=params[name]
        if 'job' in item:
            upstream=lookup(item['job'])
            result=json.loads(upstream['result']) if upstream and upstream['result'] else {}
            if upstream is None or upstream['state'] not in ('complete','processing','result_failed') or not result.get('artifact_ready'):raise ValueError('Upstream output is not ready')
            entity=next((e for e in result['entities'] if e['id']==item['entity']),None)
            if entity is None:raise ValueError('Upstream entity not found')
            item={**entity['locator'],'sha256':entity['sha256']}
        base=storage.locator(config,bulk,item)
        path=relative(base,item['ref'])
        if not path.is_relative_to(relative(base,prefix)) or file_sha(path)!=item['sha256']:
            raise ValueError('Input changed or outside runtime grant')
        resolved[name]=str(path)
    return resolved


def request_file(folder,spec,config,bulk,relative,file_sha,lookup,stage='run'):
    profile=spec['profile']
    input_profile=profile
    if stage=='prepare' and 'inputs' in profile['prepare']:input_profile={**profile,'inputs':{k:v for k,v in profile.get('inputs',{}).items() if k in profile['prepare']['inputs']}}
    params=resolve_inputs(input_profile,spec['params'],config,bulk,relative,file_sha,lookup)
    request={'schema':'loom-run-request/v1','params':params,'output_dir':str(folder),
             'profile':{'revision':profile['revision'],'sha256':profile['sha256']},
             'execution':policy(spec,profile)}
    if profile.get('steps'):request['steps']={'schema':'loom-job-steps/v1','plan':profile['steps'],'report':'steps.json'}
    path=folder/'request.json'
    temporary=folder/'request.tmp'
    temporary.write_text(json.dumps(request,allow_nan=False))
    temporary.replace(path)
    return path


def command(profile,stage,path):
    runner=profile.get(stage,profile['runtime']) if stage!='run' else profile['runtime']
    return [*runner['command'],str(path)],{**os.environ,**runner.get('env',{})}


def apply_defaults(profile,params):
    def merge(base,override):
        value=copy.deepcopy(base)
        for key,item in override.items():
            value[key]=merge(value[key],item) if isinstance(item,dict) and isinstance(value.get(key),dict) else copy.deepcopy(item)
        return value
    defaults=profile.get('defaults',{})
    if not isinstance(defaults,dict):raise ValueError('Profile defaults must be an object')
    return merge(defaults,params)
