"""Additive research presentation. Never changes execution or original evidence."""
import json
import time
import hashlib

PRESENTATION = {'type':'object','properties':{
    'scope':{'enum':['research','operations']},
    'role':{'enum':['main','support','test']},
    'label':{'type':'string','minLength':1,'maxLength':200},
    'experiment':{'type':'string','minLength':1,'maxLength':256},
    'request_ref':{'type':'string','minLength':1,'maxLength':1024},
    'parent_job':{'type':'string','minLength':1,'maxLength':256},
    'output_roles':{'type':'object','maxProperties':256,'additionalProperties':{'enum':['main','support','test']}}
},'additionalProperties':False}
CHANGE={'type':'object','properties':{'kind':{'enum':['job','entity','component','tool','profile','runtime']},
    'id':{'type':'string','minLength':1,'maxLength':512},'presentation':PRESENTATION},
    'required':['kind','id','presentation'],'additionalProperties':False}
CURATE={'type':'object','properties':{'key':{'type':'string','minLength':1,'maxLength':128},
    'reason':{'type':'string','minLength':1,'maxLength':2000},'dry_run':{'type':'boolean','default':True},
    'changes':{'type':'array','minItems':1,'maxItems':1000,'items':CHANGE}},
    'required':['key','reason','changes'],'additionalProperties':False}
PROFILE_PRESENTATION={'type':'object','properties':{k:PRESENTATION['properties'][k]
    for k in ('scope','role','output_roles')},'additionalProperties':False}


def defaults(c,spec):
    """Freeze the declared purpose at submission; no name-based classification."""
    value={'scope':'operations','role':'test'} if spec['kind']=='cpu.smoke' else {'scope':'research','role':'main'}
    profile=spec.get('profile',{})
    # Several experiment-shaped receipts do not establish which is the requested
    # scientific result. Keep them attached until the producer declares roles.
    experiments=[name for name,kind in (profile.get('outputs') or {}).items() if kind=='experiment']
    if len(experiments)>1:value['output_roles']={name:'support' for name in experiments}
    import software_metadata
    for row in c.execute('SELECT * FROM components'):
        if spec['kind'] not in json.loads(row['profiles']):continue
        suffix='/'+row['owner']+'/'+row['id']+'/revisions/'+str(row['revision'])
        if not profile.get('component_path','').endswith(suffix):continue
        tags=software_metadata.read(c,'component',row['owner'],row['id'],row['revision'],json.loads(row['manifest']))['tags']
        if 'platform-check' in tags:value={'scope':'operations','role':'test'}
    for declared in (profile.get('presentation',{}),spec.get('presentation',{})):
        roles={**value.get('output_roles',{}),**declared.get('output_roles',{})}
        value.update(declared)
        if roles:value['output_roles']=roles
    outputs=profile.get('outputs',{'observation':'artifact'} if spec['kind']=='cpu.smoke' else None)
    if outputs is not None and set(value.get('output_roles',{}))-set(outputs):raise ValueError('Presentation names an undeclared output')
    return value


def init(c):
    c.execute('CREATE TABLE IF NOT EXISTS research_overlays(kind TEXT,id TEXT,presentation TEXT NOT NULL,actor TEXT NOT NULL,reason TEXT NOT NULL,updated REAL NOT NULL,PRIMARY KEY(kind,id))')
    c.execute('CREATE TABLE IF NOT EXISTS research_batches(actor TEXT,key TEXT,request TEXT NOT NULL,created REAL NOT NULL,PRIMARY KEY(actor,key))')


def read(c):
    if not c.execute("SELECT 1 FROM sqlite_master WHERE name='research_overlays'").fetchone():return {}
    return {(r['kind'],r['id']):json.loads(r['presentation']) for r in c.execute('SELECT * FROM research_overlays')}


def job(c, row, overlays=None):
    overlays=read(c) if overlays is None else overlays
    spec=json.loads(row['spec'])
    default={'scope':'operations','role':'test'} if spec.get('kind')=='cpu.smoke' else {'scope':'research','role':'main'}
    return {**default,**spec.get('presentation',{}),**overlays.get(('job',row['id']),{})}


def logical_id(identity, presentations, consolidated):
    seen=set()
    while identity not in seen:
        seen.add(identity)
        parent=presentations.get(identity,{}).get('parent_job') or consolidated.get(identity)
        if not parent:return identity
        identity=parent
    return identity


def decorate(c, entities):
    overlays=read(c);jobs={r['id']:job(c,r,overlays) for r in c.execute('SELECT id,spec FROM jobs')}
    import job_work,workflows
    consolidated=job_work.completed_links(c)
    published={}
    for w in workflows.listing(c):
        for alias in w['result']['outputs'].values():published[(alias['job'],alias['entity'])]=w['id']
    for e in entities:
        native=e.get('loom_job',{}).get('id')
        p=dict(jobs.get(e['id'] if e['kind']=='job' else native,{'scope':'research','role':'main'}))
        if e.get('loom_workflow'):p.update(e['loom_workflow'].get('presentation',{}))
        if e['kind']!='job' and native in jobs:
            local=e['id'].removeprefix(native+'.')
            p['role']=p.get('output_roles',{}).get(local,p['role'])
        if e.get('properties',{}).get('record_role')=='step':p['role']='support'
        if e.get('properties',{}).get('consolidated_into'):
            p.update(parent_job=e['properties']['consolidated_into'],role='support')
        if native and e['kind']!='job':
            parent=consolidated.get(native)
            if parent:p.update(parent_job=parent,role='support')
        if e['kind']!='job' and native and (native,e['id'].removeprefix(native+'.')) in published:
            p.update(role='main',parent_job=published[(native,e['id'].removeprefix(native+'.'))])
        profile=e.get('loom_execution_profile')
        extension=e.get('loom_extension')
        if profile:
            tags=(profile.get('component') or {}).get('classification',{}).get('tags',[])
            uses=[j for j in profile.get('jobs',[]) if jobs.get(j['id'],{}).get('scope')=='research']
            p.update(scope='operations' if profile.get('kind')=='cpu.smoke' or 'platform-check' in tags else 'research',role='main' if uses else 'support')
            p['related_jobs']=sorted({jobs[j['id']].get('parent_job',j.get('logical_job',j['id'])) for j in uses})
        elif extension:
            tags=extension.get('classification',{}).get('tags',[])
            p.update(scope='operations' if 'platform-check' in tags else 'research')
        keykind='entity' if e.get('loom_registry') or e.get('loom_history') and e['kind']!='job' else {'execution_profile':'profile'}.get(e['kind'],e['kind'])
        p.update(overlays.get((keykind,e['id']),{}))
        p['visible']=p.get('scope')=='research' and p.get('role')=='main' and (e['kind']!='job' or not p.get('parent_job'))
        e['presentation']=p
        if p.get('label'):e['label']=p['label']
        if not p['visible'] and 'contribution' in e:e['contribution']={**e['contribution'],'eligible':False}
    started={r[0] for r in c.execute('SELECT DISTINCT job FROM attempts WHERE started IS NOT NULL')}
    for e in entities:
        if not e.get('loom_execution_profile'):continue
        p=e['loom_execution_profile'];used={logical_id(j['id'],jobs,consolidated) for j in p.get('jobs',[]) if j['id'] in started and jobs.get(j['id'],{}).get('scope')=='research'}
        p['research_usage_stats']={'executed_jobs':len(used),'reuses':max(0,len(used)-1)}
    visible_profiles=[e['loom_execution_profile'] for e in entities if e.get('loom_execution_profile') and e['presentation']['visible']]
    for e in entities:
        related=[]
        if e.get('loom_runtime'):
            kinds={p['kind'] for p in e['loom_runtime'].get('used_by_profiles',[])}
            related=[p for p in visible_profiles if p['kind'] in kinds]
        elif e.get('loom_extension'):
            item=e['loom_extension']
            for p in visible_profiles:
                refs=([p['component']] if e['kind']=='component' and p.get('component') else p.get('tools',[]) if e['kind']=='tool' else [])
                if any((r['owner'],r['id'])==(item['owner'],item['id']) for r in refs):related.append(p)
        else:continue
        e['research_context']={'profiles':sorted({p.get('family',p['kind']) for p in related}),
            'jobs':sorted({logical_id(j['id'],jobs,consolidated) for p in related for j in p.get('jobs',[]) if jobs.get(j['id'],{}).get('scope')=='research'})}
    return entities


def view(items, mode='research'):
    if mode not in ('research','all','operations'):raise ValueError('Invalid research view')
    return [e for e in items if mode=='all' or (e.get('presentation',{}).get('scope')=='operations' if mode=='operations' else e.get('presentation',{}).get('visible',True))]


def enrich(c, record):
    """Expose reviewed constituent execution; do not rewrite its recorded outcome."""
    import job_work
    overlays=read(c);children=[identity for (kind,identity),p in overlays.items()
        if kind=='job' and p.get('parent_job')==record['id']]
    if not children:return record
    nodes=[];intervals=[];consolidated=job_work.completed_links(c)
    for identity in [record['id'],*children]:
        row=c.execute('SELECT * FROM jobs WHERE id=?',(identity,)).fetchone()
        if not row:continue
        p=json.loads(row['spec'])
        attempts=[dict(a) for a in c.execute('SELECT id,state,started,ended FROM attempts WHERE job=? ORDER BY started,id',(identity,))]
        nodes.append({'id':identity,'job':identity,'label':overlays.get(('job',identity),{}).get('label',p['kind']),
            'state':row['state'],'attempts':attempts,'profile':p['kind']})
        if identity in consolidated and consolidated[identity]==record['id']:continue
        intervals.extend(job_work.execution_intervals(c,row,job_work.timeline(c,row,p)))
    if nodes:
        record={**record,'executions':nodes,'execution_intervals':intervals,
            'recorded_execution_state':record['state'],'grouping_authority':'reviewed additive presentation; recorded outcome preserved'}
    return record


def curate(c, actor, args, inventory):
    import execution
    execution.validate(CURATE,args)
    if 'submit' not in actor['scopes']:raise PermissionError('Submit scope required')
    changes=args['changes'];known={(('entity' if e.get('loom_registry') or e.get('loom_history') and e['kind']!='job' else {'execution_profile':'profile'}.get(e['kind'],e['kind'])),e['id']) for e in inventory}
    keys=[(r['kind'],r['id']) for r in changes]
    if len(set(keys))!=len(keys) or any(k not in known for k in keys):raise ValueError('Unknown or repeated research identity')
    current=read(c);base={('job',e['id']):e.get('presentation',{}) for e in inventory if e['kind']=='job'};proposed={**base,**current,**{(r['kind'],r['id']):r['presentation'] for r in changes}}
    jobids={e['id'] for e in inventory if e['kind']=='job'}
    for (kind,identity),p in proposed.items():
        parent=p.get('parent_job')
        if parent and (parent not in jobids or kind=='job' and parent==identity):raise ValueError('Invalid main Job binding')
        seen={identity};cursor=parent
        while cursor:
            if cursor in seen:raise ValueError('Research Job binding cycle')
            seen.add(cursor);cursor=proposed.get(('job',cursor),{}).get('parent_job')
    raw=json.dumps({'reason':args['reason'],'changes':changes},sort_keys=True,allow_nan=False)
    old=c.execute('SELECT request FROM research_batches WHERE actor=? AND key=?',(actor['id'],args['key'])).fetchone()
    if old and old[0]!=raw:raise ValueError('Research curation key conflict')
    if args.get('dry_run',True):return {'dry_run':True,'changes':changes,'sha256':hashlib.sha256(raw.encode()).hexdigest()}
    if old:return {'applied':False,'replayed':True,'count':len(changes)}
    for r in changes:
        c.execute('INSERT INTO research_overlays VALUES(?,?,?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET presentation=excluded.presentation,actor=excluded.actor,reason=excluded.reason,updated=excluded.updated',
            (r['kind'],r['id'],json.dumps(r['presentation'],sort_keys=True),actor['id'],args['reason'],time.time()))
    c.execute('INSERT INTO research_batches VALUES(?,?,?,?)',(actor['id'],args['key'],raw,time.time()))
    return {'applied':True,'count':len(changes)}
