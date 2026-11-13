"""Instance-granted Runtime discovery and immutable Profile bindings."""
import http.client
import json
import runtime_client

REF_SCHEMA={'type':'object','properties':{'id':{'type':'string','pattern':'^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$'},
    'revision':{'type':'integer','minimum':1}},'required':['id','revision'],'additionalProperties':False}

def dependency_preview(dependencies):
    """Bound repeated Profile projections; full inventories stay on the Runtime."""
    if dependencies is None:return {'dependencies':None,'dependency_count':None,'dependencies_complete':False}
    priorities=['isaacsim','isaaclab','python','isaac-python','torch','lerobot','numpy','python-fcl','nvidia driver','mujoco','tensorflow']
    def key(d):
        name=d['name'].lower().replace('_','-')
        return (priorities.index(name) if name in priorities else len(priorities),name,d['version'],d.get('scope',''))
    selected=sorted(dependencies,key=key)[:16]
    return {'dependencies':[{k:d[k] for k in ('name','version','kind','scope') if k in d} for d in selected],
        'dependency_count':len(dependencies),'dependencies_complete':len(selected)==len(dependencies)}



def primary_dependencies(manifest, selectors=None, *, strict=False):
    """Representative versions, never a claim that every installed package is used."""
    deps=manifest.get('dependencies') or []
    selectors=manifest.get('primary_dependencies') if selectors is None else selectors
    selected=[]
    if selectors is not None:
        from execution import validate
        from runtime_hub import SELECTOR
        validate({'type':'array','maxItems':8,'items':SELECTOR},selectors)
        for selector in selectors:
            found=[d for d in deps if d['name'].casefold()==selector['name'].casefold() and ('scope' not in selector or d.get('scope')==selector['scope'])]
            if not found or len({d['version'] for d in found})>1:
                if strict:raise ValueError('Representative dependency is missing or has ambiguous versions; select its scope')
                continue
            selected.append(found[0])
    else:
        # Fallback suggestions until an owner selects highlights. Never enumerate
        # bundled/OS packages simply because a Runtime has no declared highlights.
        names=['isaacsim','isaaclab','python','isaac-python','torch','lerobot','tensorflow','mujoco']
        for name in names:
            found=[d for d in deps if d['name'].casefold()==name and not d.get('deprecated')]
            if found and len({d['version'] for d in found})==1:selected.append(found[0])
    return [{k:d[k] for k in ('name','version','scope','kind','origin') if k in d} for d in selected[:8]]


def decorate(db,config,catalog):
    import runtime_hub
    for item in catalog.get('runtimes',[]):
        curated=runtime_hub.read(db,'runtime',item['id'],item['revision'] or 0)
        item['curation']=curated
        item['primary_dependencies']=primary_dependencies(item['manifest'],curated.get('primary_dependencies'))
        item['primary_source']='declared' if 'primary_dependencies' in curated or 'primary_dependencies' in item['manifest'] else 'suggested'
        item['validation_scope']=item['manifest'].get('validation',{}).get('scope')
    return catalog


def requirement_report(manifest,profile,catalog=None):
    """Explain requirements for each declared stage/interpreter context."""
    from execution import validate
    from runtime_hub import STAGES
    requirements=manifest.get('requires',{})
    if not isinstance(requirements,dict) or any(not isinstance(k,str) or not isinstance(v,str) for k,v in requirements.items()):raise ValueError('requires must map dependency names to exact version strings')
    rows=[{'stage':'runtime','name':name,'version':version} for name,version in requirements.items()]
    extra=manifest.get('stage_requires',[])
    validate({'type':'array','maxItems':128,'items':{'type':'object','properties':{'stage':{'enum':list(STAGES)},'name':{'type':'string','minLength':1},'version':{'type':'string','minLength':1},'scope':{'type':'string','minLength':1}},'required':['stage','name','version'],'additionalProperties':False}},extra)
    rows+=extra;result=[]
    for req in rows:
        runner=profile.get(req['stage']);pin=(runner or {}).get('runtime_pin');deps=(pin or {}).get('dependencies');versions=set();basis='pinned' if pin else 'unverified'
        if not runner:status='missing_stage'
        else:
            if deps is None and catalog is not None:
                env=observed_environment(runner,catalog)
                deps=env.get('dependencies') if env else None
                if env:basis='current_observation'
            if deps is None:status='unverified'
            else:
                versions={d['version'] for d in deps if d['name'].casefold()==req['name'].casefold() and ('scope' not in req or d.get('scope')==req['scope'])}
                status='missing' if not versions else 'ambiguous' if len(versions)>1 else 'matched' if versions=={req['version']} else 'mismatch'
        result.append({**req,'check':'dependency','status':status,'actual_versions':sorted(versions),'basis':basis})
    return result


def listing(config):
    if 'executor' not in config:
        return {'status':'unavailable','reason':'Executor not configured','runtimes':[]}
    try:
        value=runtime_client.call(config['executor'],'list_runtimes',{},timeout=3)
        if not isinstance(value,dict) or not isinstance(value.get('runtimes'),list):raise ValueError('Unsupported Runtime discovery')
        return {'status':'ok',**value}
    except (OSError,ValueError,RuntimeError,http.client.HTTPException) as error:
        return {'status':'unavailable','reason':type(error).__name__,'runtimes':[]}


def bind(runner, config):
    from execution import validate
    validate(REF_SCHEMA,runner['runtime_ref'])
    result=runtime_client.call(config['executor'],'resolve_runtime',{'runtime_ref':runner['runtime_ref']})
    pin=result['runtime_ref'];manifest=result['manifest']
    if {k:pin.get(k) for k in ('id','revision')} != runner['runtime_ref']:
        raise ValueError('Executor returned a different Runtime identity')
    # Read-only sealed fields are assigned here, never accepted from registration callers.
    runner['runtime_pin']={**pin,'image':manifest['runtime']['image'],
        'dependencies':manifest.get('dependencies',[]),'validation':manifest.get('validation',{})}


def profiles_used(db,config):
    from component_store import merge
    return merge(db,config).get('profiles',{})


def inventory(db,config,catalog=None):
    catalog=listing(config) if catalog is None else catalog
    profiles=profiles_used(db,config)
    jobs=[]
    import job_work
    links=job_work.completed_links(db)
    started={r[0] for r in db.execute('SELECT DISTINCT job FROM attempts WHERE started IS NOT NULL')}
    for row in db.execute('SELECT id,spec FROM jobs'):
        spec=json.loads(row['spec']);jobs.append((row['id'],spec.get('profile',{})))
    def exact(runner,item,proof=False):
        if proof and not runner.get('runtime_pin'):return False
        ref=runner.get('runtime_pin') or runner.get('runtime_ref',{})
        return (ref.get('id'),ref.get('revision'))==(item['id'],item['revision']) and (not ref.get('sha256') or ref['sha256']==item['sha256'])
    for item in catalog['runtimes']:
        def matches(runner):
            return exact(runner,item) or runner.get('runtime_id') in item.get('aliases',[]) or (runner.get('image')==item['manifest']['runtime']['image'] and not item['manifest']['runtime'].get('mounts')) if not item.get('legacy') else runner.get('runtime_id')==item['id']
        stages=lambda p:[stage for stage in ('runtime','prepare','finalize','on_complete','on_error','on_cancel') if matches(p.get(stage,{}))]
        item['used_by_profiles']=[{'kind':kind,'stages':stages(profile),'binding':'pinned' if any(exact(profile.get(s,{}),item) for s in stages(profile)) else 'current_configuration'} for kind,profile in profiles.items() if stages(profile)]
        # Retained field means association, never proof that today's observation
        # revision was used in a historical Job.
        item['used_by_jobs']=[identity for identity,profile in jobs if stages(profile)]
        item['executed_jobs']=sorted({links.get(identity,identity) for identity,profile in jobs if identity in started and any(exact(profile.get(s,{}),item,proof=True) for s in ('runtime','prepare','finalize','on_complete','on_error','on_cancel'))})
        item['usage_stats']={'configured_profiles':len(item['used_by_profiles']),'associated_records':len(item['used_by_jobs']),'executed_jobs':len(item['executed_jobs']),'basis':'exact_runtime_revision_started_attempts'}
    return decorate(db,config,catalog)


def check_requirements(manifest,profile):
    """Opt-in declared requirements retain registration-time validation."""
    if any(row['status']!='matched' for row in requirement_report(manifest,profile)):
        raise ValueError('Runtime dependency requirements do not match their declared stage and scope')


def observed_environment(runner,catalog):
    """Read-only current-environment observation, never a backfilled execution pin."""
    for item in catalog.get('runtimes',[]):
        manifest=item['manifest'];image=manifest.get('runtime',{}).get('image')
        match=runner.get('runtime_id') in item.get('aliases',[]) or (runner.get('image')==image and not manifest.get('runtime',{}).get('mounts'))
        if item.get('legacy') or not match:continue
        return {'type':'observed','id':item['id'],'label':manifest.get('label',item['id']),'revision':item['revision'],
            'image':image,'dependencies':manifest.get('dependencies',[]),'entity':'runtime:'+item['id']+':'+str(item['revision']),
            'observation_scope':manifest.get('validation',{}).get('scope'),'original_runtime_id':runner.get('runtime_id')}
    return None
