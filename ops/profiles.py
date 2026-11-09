"""Current execution profile discovery and typed artifact validation."""
import execution
import hashlib
import math
import re
from pathlib import Path

APP = Path(__file__).resolve().parent
HEX = re.compile('[a-f0-9]{64}')

def describe(config, actor):
    if 'submit' not in actor['scopes']:return []
    rows=[{'kind':'cpu.smoke','params_schema':{'type':'object','properties':{
        'seconds':{'type':'integer','minimum':0,'maximum':20},'fail':{'type':'boolean'}},
        'required':['seconds','fail'],'additionalProperties':False},'outputs':{'observation':'artifact'},
        'owner_name':'Built-in','owner_kind':'built-in','owner_profile':{'id':'built-in','provider':'builtin','login':None,'display_name':'Built-in'}}]
    for kind,p in config.get('profiles',{}).items():
        if not execution.is_generic(p):continue
        rows.append({'kind':kind,'revision':p['revision'],'contract':execution.REVISION,'owners':p['owners'],
            'params_schema':p['params_schema'],'outputs':p['outputs'],'defaults':p.get('defaults',{}),
            'metadata_schemas':p.get('metadata_schemas',{}),'limits':p['limits'],'preparation':bool(p.get('prepare')),
            'lane':p['runtime'].get('lane','cpu'),'execution_schema':execution.POLICY_SCHEMA,
            **config.get('_profile_owners',{}).get(kind,{})})
    return rows

def file_sha(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def installed(config,kind,actor,params,bulk,relative,ident,dependencies=()):
    profile=config.get('profiles',{}).get(kind)
    if not profile:raise PermissionError('Installed profile outside grant')
    validate_profile(profile,relative)
    execution.validate(profile['params_schema'],params)
    execution.admit_inputs(profile,params,config,bulk,relative,ident,dependencies)
    return profile

def validate_profile(profile,relative=None,**_):
    if not execution.is_generic(profile):raise ValueError('Unsupported profile schema; register a current execution profile')
    execution.validate_profile(profile)


def typed_contract(manifest, profile, ident):
    entities = manifest['entities']
    declared = {entity['id']: entity['kind'] for entity in entities}
    if profile['outputs'] is not None and declared != profile['outputs']:
        raise ValueError('Result identities differ from installed profile')
    for entity in entities:
        if set(entity)-{'components','metadata','label','description'} != {'id','kind','path','provenance','relations'}:
            raise ValueError('Typed entity requires path, provenance and relations')
        metadata=entity.get('metadata',{})
        if not isinstance(metadata,dict):raise ValueError('Entity metadata must be an object')
        schema=profile.get('metadata_schemas',{}).get(entity['id'],profile.get('metadata_schemas',{}).get('*',{'type':'object'}))
        execution.validate(schema,metadata)
        for field in ('label','description'):
            if field in entity and (not isinstance(entity[field],str) or len(entity[field])>8192):raise ValueError('Invalid entity text')
        provenance = entity['provenance']
        if provenance != {'profile':profile['revision'],'producer_sha256':profile['sha256']}:
            raise ValueError('Invalid producer provenance')
        for relation in entity['relations']:
            expected = {'trained_on':'dataset','evaluates':'model','produces':None}
            target = relation['entity_id']
            kind = relation['kind']
            if (set(relation) != {'kind','entity_id'} or kind not in expected or target not in declared
                    or target == entity['id'] or expected[kind] not in (None,declared[target])):
                raise ValueError('Invalid typed relation')
    for metric in manifest['metrics']:
        if set(metric) != {'name','entity_id','definition','unit','denominator','series'}:
            raise ValueError('Invalid metric fields')
        ident(metric['name'])
        if metric['entity_id'] not in declared or not all(
                isinstance(metric[k],str) and metric[k].strip() for k in ('definition','unit','denominator')):
            raise ValueError('Metric definition, unit and denominator are required')
        previous = -1
        if not 1 <= len(metric['series']) <= 10000:
            raise ValueError('Invalid series length')
        for point in metric['series']:
            if (set(point) != {'step','value'} or type(point['step']) is not int or point['step'] <= previous
                    or type(point['value']) not in (int,float) or not math.isfinite(point['value'])):
                raise ValueError('Invalid metric point')
            previous = point['step']
