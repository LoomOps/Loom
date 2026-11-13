"""Read-only combinations of exact Profiles, software revisions and environments.

Declared purpose is distinct from observed Job classification. A mounted Tool is
available to a runner; its presence does not prove a particular function was called.
"""
import json

import component_store
import execution
import job_contract
import ownership
import software_metadata
import job_work
import runtime_catalog
import runtime_hub

STAGES=('prepare','runtime','finalize','on_complete','on_error','on_cancel')


def inventory(db,config,actor,catalog=None):
    catalog=runtime_catalog.listing(config) if catalog is None else catalog
    runtime_catalog.decorate(db,config,catalog)
    merged=component_store.merge(db,config,actor)
    definitions=merged['profiles'];origins={};components={};tools={};jobs={}
    for row in db.execute('SELECT * FROM components'):
        ref={k:row[k] for k in ('owner','id','revision')}
        manifest=json.loads(row['manifest'])
        component={**ref,'entity':'component:'+ref['owner']+':'+ref['id']+':r'+str(ref['revision']),
                   'requires':manifest.get('requires',{}),
                   'classification':software_metadata.read(db,'component',ref['owner'],ref['id'],ref['revision'],manifest),'profile_count':len(json.loads(row['profiles']))}
        components[str(component_store.ROOT/ref['owner']/ref['id']/'revisions'/str(ref['revision']))]=component
        for kind in json.loads(row['profiles']):origins[kind]={'type':'component',**ref}
    for row in db.execute('SELECT owner,id,revision,profile FROM execution_profiles'):
        origins[json.loads(row['profile'])['revision']]={'type':'standalone',**{k:row[k] for k in ('owner','id','revision')}}
    for row in db.execute('SELECT owner,id,revision,manifest FROM tools'):
        tools[(row['owner'],row['id'],row['revision'])]=json.loads(row['manifest'])
    for row in db.execute('SELECT id,state,spec,created FROM jobs ORDER BY created,id'):
        spec=json.loads(row['spec']);kind=spec.get('kind')
        if not isinstance(kind,str):continue
        value={'id':row['id'],'state':row['state'],'profile_sha256':spec.get('profile',{}).get('sha256'),'created':row['created']}
        # Do not manufacture "Other" or pilot/full from an absent historical field.
        value['category']=spec.get('category') if isinstance(spec.get('category'),str) and spec['category'] in job_contract.CATEGORIES else None
        value['execution_mode']=spec.get('execution_mode') if isinstance(spec.get('execution_mode'),str) and spec['execution_mode'] in job_contract.MODES else None
        jobs.setdefault(kind,[]).append(value)
    import research
    presentations={r['id']:research.job(db,r) for r in db.execute('SELECT id,spec FROM jobs')}
    consolidated=job_work.completed_links(db)
    executed={r[0] for r in db.execute('SELECT DISTINCT job FROM attempts WHERE started IS NOT NULL')}
    result=[]
    for kind,p in sorted(definitions.items()):
        if kind=='cpu.smoke' or not execution.is_generic(p):continue
        origin=origins.get(kind,{'type':'installation','id':kind,'revision':None})
        component=components.get(p.get('component_path'))
        curated=runtime_hub.read(db,'profile',kind)
        pins=[]
        for alias,pin in p.get('tool_pins',{}).items():
            ref={k:pin[k] for k in ('owner','id','revision','sha256')}
            manifest=tools.get((ref['owner'],ref['id'],ref['revision']),{})
            pins.append({'alias':alias,**ref,'label':manifest.get('label',ref['id']),
                'entity':'tool:'+ref['owner']+':'+ref['id']+':r'+str(ref['revision']),
                'classification':software_metadata.read(db,'tool',ref['owner'],ref['id'],ref['revision'],manifest),
                'requires':manifest.get('requires',{}),'availability':'all_container_stages',
                'invocation':'runner_defined','role':curated.get('tool_roles',{}).get(alias)})
        stages=[]
        for name in STAGES:
            runner=p.get(name)
            if not runner:continue
            pin=runner.get('runtime_pin')
            environment=({'type':'registered','id':pin['id'],'revision':pin['revision'],
                'sha256':pin['sha256'],'image':pin['image'],'dependencies':pin.get('dependencies',[]),
                'validation':{k:v for k,v in pin.get('validation',{}).items() if k in ('status','checks','completed_at')},'entity':'runtime:'+pin['id']+':'+str(pin['revision'])}
                if pin else {'type':'legacy' if runner.get('runtime_id') else 'image',
                    'id':runner.get('runtime_id'),'image':runner.get('image'),'dependencies':None})
            if not pin:environment=runtime_catalog.observed_environment(runner,catalog) or environment
            runtime=next((r for r in catalog['runtimes'] if 'runtime:'+r['id']+':'+str(r['revision'])==environment.get('entity')),None)
            selected=curated.get('stage_dependencies',{}).get(name)
            environment['primary_dependencies']=(runtime_catalog.primary_dependencies(runtime['manifest'] if runtime else {'dependencies':environment['dependencies']},selected) if selected is not None else runtime.get('primary_dependencies',[]) if runtime else runtime_catalog.primary_dependencies({'dependencies':environment['dependencies']}))
            environment.update(runtime_catalog.dependency_preview(environment['dependencies']))
            stages.append({'stage':name,'lane':runner.get('lane','cpu'),'environment':environment,
                'depends_on':runner.get('depends_on',[]),
                'storage':runner.get('storage',[]),'inputs':runner.get('inputs',list(p.get('inputs',{})))})
        observed=[{**j,'pin_matches':j['profile_sha256']==p.get('sha256') if j['profile_sha256'] else None} for j in jobs.get(kind,[])]
        responsible=merged.get('_profile_owners',{}).get(kind)
        if responsible is None:
            owners=p.get('owners',[])
            responsible=ownership.extension(db,'profile',owners[0] if len(owners)==1 else 'unrecorded',kind)
        usage={};usage_status='unrecorded'
        if 'usage' in p:
            try:execution.validate(execution.USAGE_SCHEMA,p['usage'])
            except ValueError:usage_status='invalid'
            else:usage=p['usage'];usage_status='declared'
        if 'categories' in curated or 'execution_modes' in curated:
            usage={**usage,**{k:curated[k] for k in ('categories','execution_modes') if k in curated}};usage_status='curated'
        display=curated.get('label') or usage.get('label')
        if not display and component and component['classification'].get('label'):
            display=component['classification']['label']
            if component['profile_count']>1:display+=' · '+kind[len(origin['owner']+'.'+origin['id']+'.'):].rsplit('.r',1)[0]
        result.append({'family':runtime_hub.family(kind,origin),'curation':curated,'group':curated.get('group','platform-check' if 'platform-check' in (component or {}).get('classification',{}).get('tags',[]) else 'research'),'kind':kind,'sha256':p.get('sha256'),'label':display or (kind.removeprefix(origin.get('owner','')+'.') if origin.get('owner') else kind),
            'description':usage.get('description',''),'usage':usage or None,'usage_status':usage_status,'origin':origin,
            'component':component,'tools':pins,'stages':stages,'limits':p.get('limits',{}),
            'inputs':p.get('inputs',{}),'outputs':p.get('outputs'),
            'steps':p.get('steps',[]),'jobs':observed,**responsible,
            'observed_usage':{'categories':sorted({j['category'] for j in observed if j['category']}),
                              'execution_modes':sorted({j['execution_mode'] for j in observed if j['execution_mode']})}})
    result.insert(0,{'kind':'cpu.smoke','sha256':None,'label':'CPU smoke test',
        'description':'Built-in bounded CPU platform check','usage':{'categories':['other'],'execution_modes':['smoke']},'usage_status':'declared',
        'origin':{'type':'builtin','id':'cpu.smoke','revision':None},'component':None,
        'tools':[],'stages':[{'stage':'runtime','lane':'cpu',
            'environment':{'type':'builtin','id':'Loom worker','dependencies':[]},
            'storage':[],'inputs':[]}],'limits':{},'inputs':{},'outputs':{'observation':'artifact'},'steps':[],
        'jobs':jobs.get('cpu.smoke',[]),'observed_usage':{
            'categories':sorted({j['category'] for j in jobs.get('cpu.smoke',[]) if j['category']}),
            'execution_modes':sorted({j['execution_mode'] for j in jobs.get('cpu.smoke',[]) if j['execution_mode']})},
        **ownership.extension(db,'profile','built-in','cpu.smoke')})
    starts={r[0]:r[1] for r in db.execute('SELECT job,MAX(started) FROM attempts WHERE started IS NOT NULL GROUP BY job')}
    for p in result:
        p.setdefault('family',p['kind']);p.setdefault('group','platform-check');p.setdefault('curation',runtime_hub.read(db,'profile',p['kind']))
        if p['curation'].get('label'):p['label']=p['curation']['label']
        if p['curation'].get('group'):p['group']=p['curation']['group']
        if any(k in p['curation'] for k in ('categories','execution_modes')):
            p['usage']={**(p.get('usage') or {}),**{k:p['curation'][k] for k in ('categories','execution_modes') if k in p['curation']}};p['usage_status']='curated'
        p['params_schema']=definitions.get(p['kind'],{}).get('params_schema',{'type':'object','properties':{'seconds':{'type':'integer'},'fail':{'type':'boolean'}}})
        p['defaults']=definitions.get(p['kind'],{}).get('defaults',{})
        all_jobs=p['jobs']
        for j in all_jobs:j['logical_job']=research.logical_id(j['id'],presentations,consolidated)
        logical={j['logical_job'] for j in all_jobs}
        executed_records=[j for j in all_jobs if j['id'] in executed]
        runs={j['logical_job'] for j in executed_records}
        p['last_execution']=max(({'job':j['logical_job'],'record':j['id'],'started':starts[j['id']],'state':j['state']} for j in executed_records),key=lambda x:x['started'],default=None)
        relevant={j['logical_job'] for j in executed_records if presentations.get(j['id'],{}).get('scope')=='research'}
        p['research_usage_stats']={'executed_jobs':len(relevant),'reuses':max(0,len(relevant)-1)}
        p['usage_stats']={'recorded_jobs':len(all_jobs),'logical_jobs':len(logical),'executed_jobs':len(runs),
            'recorded_executions':len(executed_records),'reuses':max(0,len(runs)-1),
            'consolidated_sources':sum(j['id'] in consolidated for j in all_jobs)}
    return result


def entities(db,config,actor,catalog=None):
    return [{'id':'profile:'+p['kind'],'kind':'execution_profile','namespace':'loom',
        'label':p['label'],'summary':p['description'],'owner_profile':p['owner_profile'],
        'owner_name':p['owner_name'],'owner_kind':p['owner_kind'],
        'loom_execution_profile':p,'properties':{},'relations':[],'artifacts':[]}
        for p in inventory(db,config,actor,catalog)]
