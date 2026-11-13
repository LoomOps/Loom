"""One public Job with independently scheduled, immutable execution nodes."""
import json
import time
import uuid
import hashlib
import research

NAME={'type':'string','pattern':'^[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}$'}
SCHEMA={'type':'object','properties':{
    'key':{'type':'string','minLength':1,'maxLength':128},'label':{'type':'string','minLength':1,'maxLength':200},
    'owner':{'type':'string'},'notify':{'type':'string'},'presentation':research.PRESENTATION,
    'category':{'enum':['sdg','data_preparation','fine_tuning','evaluation','pipeline','other']},
    'nodes':{'type':'array','minItems':1,'maxItems':64,'items':{'type':'object','properties':{
        'id':NAME,'label':{'type':'string','minLength':1,'maxLength':200},
        'depends_on':{'type':'array','maxItems':64,'uniqueItems':True,'items':NAME},
        'spec':{'type':'object'}},'required':['id','label','spec'],'additionalProperties':False}},
    'outputs':{'type':'object','maxProperties':256,'additionalProperties':{'type':'object','properties':{'node':NAME,'entity':{'type':'string'}},'required':['node','entity'],'additionalProperties':False}}},
    'required':['key','label','owner','notify','nodes'],'additionalProperties':False}


def init(c):
    c.execute('CREATE TABLE IF NOT EXISTS job_workflows(id TEXT PRIMARY KEY,submitter TEXT,key TEXT,request TEXT NOT NULL,owner TEXT NOT NULL,notify TEXT NOT NULL,created REAL NOT NULL,state TEXT NOT NULL,UNIQUE(submitter,key))')
    if 'observed_errors' not in {r[1] for r in c.execute('PRAGMA table_info(job_workflows)')}:c.execute("ALTER TABLE job_workflows ADD COLUMN observed_errors TEXT NOT NULL DEFAULT '[]'")
    c.execute('CREATE TABLE IF NOT EXISTS workflow_nodes(root TEXT,node TEXT,label TEXT,job TEXT UNIQUE NOT NULL,PRIMARY KEY(root,node))')


def exists(c,identity):
    if not c.execute("SELECT 1 FROM sqlite_master WHERE name='job_workflows'").fetchone():return None
    return c.execute('SELECT * FROM job_workflows WHERE id=?',(identity,)).fetchone()


def resolve_inputs(c,params):
    result={}
    for key,value in params.items():
        if isinstance(value,dict) and set(value)=={'job','entity'} and (root:=exists(c,value['job'])):
            request=json.loads(root['request']);alias=request.get('outputs',{}).get(value['entity'])
            if not alias:raise ValueError('Unknown logical Job output')
            node=c.execute('SELECT job FROM workflow_nodes WHERE root=? AND node=?',(root['id'],alias['node'])).fetchone()
            value={'job':node['job'],'entity':alias['entity']}
        result[key]=value
    return result


def submit(c,actor,args,config,submit_node):
    import execution,registry
    execution.validate(SCHEMA,args)
    if not c.in_transaction:c.execute('BEGIN IMMEDIATE')
    if registry.mode(c)!='active':raise PermissionError('Job admission fenced')
    raw=json.dumps(args,sort_keys=True,allow_nan=False)
    old=c.execute('SELECT * FROM job_workflows WHERE submitter=? AND key=?',(actor['id'],args['key'])).fetchone()
    if old:
        if old['request']!=raw:raise ValueError('Workflow idempotency conflict')
        return {'id':old['id'],'nodes':{r['node']:r['job'] for r in c.execute('SELECT * FROM workflow_nodes WHERE root=?',(old['id'],))}}
    nodes={n['id']:n for n in args['nodes']}
    if len(nodes)!=len(args['nodes']):raise ValueError('Duplicate execution node')
    pending=dict(nodes);ordered=[]
    while pending:
        available=[n for n in pending if all(d in {x['id'] for x in ordered} for d in pending[n].get('depends_on',[]))]
        if not available:raise ValueError('Unknown dependency or execution cycle')
        for n in available:ordered.append(pending.pop(n))
    for alias in args.get('outputs',{}).values():
        if alias['node'] not in nodes:raise ValueError('Unknown output node')
    root=uuid.uuid4().hex;assigned={}
    c.execute('SAVEPOINT workflow_submission')
    try:
        c.execute('INSERT INTO job_workflows(id,submitter,key,request,owner,notify,created,state) VALUES(?,?,?,?,?,?,?,?)',(root,actor['id'],args['key'],raw,args['owner'],args['notify'],time.time(),'queued'))
        for n in ordered:
            spec=dict(n['spec'])
            if set(spec)&{'key','owner','notify','presentation','consolidates'}:raise ValueError('Execution identity and purpose are inherited from logical Job')
            params=spec.get('params',{})
            if not isinstance(params,dict) or not isinstance(spec.get('depends_on',[]),list):raise ValueError('Invalid node parameters or dependencies')
            params={k:({'job':assigned[v['node']],'entity':v['entity']} if isinstance(v,dict) and set(v)=={'node','entity'} and v['node'] in n.get('depends_on',[]) else v) for k,v in params.items()}
            spec.update(key='workflow-'+hashlib.sha256((actor['id']+':'+args['key']+':'+n['id']).encode()).hexdigest(),
                owner=args['owner'],notify=args['notify'],params=params,
                depends_on=list(dict.fromkeys(spec.get('depends_on',[])+[assigned[d] for d in n.get('depends_on',[])])),
                presentation={'scope':'research',**args.get('presentation',{}),'parent_job':root,'role':'support','label':n['label']})
            result=submit_node(c,actor,spec,config);assigned[n['id']]=result['id']
            c.execute('INSERT INTO workflow_nodes VALUES(?,?,?,?)',(root,n['id'],n['label'],result['id']))
        for alias in args.get('outputs',{}).values():
            spec=json.loads(c.execute('SELECT spec FROM jobs WHERE id=?',(assigned[alias['node']],)).fetchone()[0])
            outputs=spec.get('profile',{}).get('outputs',{'observation':'artifact'} if spec['kind']=='cpu.smoke' else {})
            if alias['entity'] not in outputs:raise ValueError('Output alias outside pinned Profile contract')
        c.execute('RELEASE workflow_submission')
    except BaseException:
        c.execute('ROLLBACK TO workflow_submission');c.execute('RELEASE workflow_submission');raise
    return {'id':root,'nodes':assigned}


def get(c,identity):
    root=exists(c,identity)
    if root is None:return None
    req=json.loads(root['request']);nodes=[];intervals=[]
    for n in c.execute('SELECT n.*,j.state,j.spec,j.result,j.attempt FROM workflow_nodes n JOIN jobs j ON j.id=n.job WHERE n.root=? ORDER BY n.rowid',(identity,)):
        spec=json.loads(n['spec']);result=json.loads(n['result'] or '{}')
        attempts=[dict(a) for a in c.execute('SELECT id,state,started,ended FROM attempts WHERE job=? ORDER BY started,id',(n['job'],))]
        import job_work
        row=c.execute('SELECT * FROM jobs WHERE id=?',(n['job'],)).fetchone()
        intervals.extend(job_work.execution_intervals(c,row))
        dependency_errors=[]
        if n['state']=='blocked':
            for dependency in spec.get('depends_on',[]):
                upstream=c.execute("SELECT state,json_extract(result,'$.artifact_ready') FROM jobs WHERE id=?",(dependency,)).fetchone()
                if upstream is None or upstream['state'] in ('failed','cancelled','interrupted','blocked') or upstream['state']=='result_failed' and upstream[1]!=1:dependency_errors.append(dependency)
        nodes.append({'dependency_errors':dependency_errors,'id':n['node'],'job':n['job'],'label':n['label'],'state':n['state'],'attempt':n['attempt'],
            'attempts':attempts,'profile':spec['kind'],'profile_sha256':spec.get('profile',{}).get('sha256'),
            'depends_on':spec.get('depends_on',[]),'artifact_ready':result.get('artifact_ready',False)})
    states={n['state'] for n in nodes}
    if states <= {'complete'}:state='complete'
    elif 'running' in states:state='running'
    elif 'preparing' in states:state='preparing'
    elif 'processing' in states:state='processing'
    elif 'ready' in states:state='ready'
    elif states & {'failed','interrupted','result_failed'} or any(n['dependency_errors'] for n in nodes):state='failed'
    elif states <= {'complete','cancelled'}:state='cancelled'
    else:state='queued'
    p={'scope':'research','role':'main',**req.get('presentation',{})}
    return {'id':identity,'label':req['label'],'owner':root['owner'],'notify':root['notify'],'submitter':root['submitter'],
        'created':root['created'],'state':state,'status':{'complete':'completed','processing':'finalizing'}.get(state,state),'category':req.get('category','pipeline'),
        'execution_mode':None,'attempt':None,'attempts':[],
        'executions':nodes,'execution_intervals':intervals,'presentation':p,
        'result':{'artifact_ready':state=='complete' and all(n['artifact_ready'] for n in nodes),'outputs':{
            key:{'job':next(n['job'] for n in nodes if n['id']==v['node']),'entity':v['entity']} for key,v in req.get('outputs',{}).items()}},
        'errors':sorted({n['job'] for n in nodes if n['state'] in ('failed','interrupted','result_failed')}|{d for n in nodes for d in n['dependency_errors']}),
        'spec':{'kind':'workflow','params':{},'presentation':p},'workflow':True}


def listing(c):
    if not c.execute("SELECT 1 FROM sqlite_master WHERE name='job_workflows'").fetchone():return []
    return [get(c,r[0]) for r in c.execute('SELECT id FROM job_workflows ORDER BY created,id')]


def entities(c):
    result=[]
    for w in listing(c):
        starts=[x['start'] for x in w['execution_intervals'] if x['start'] is not None]
        ends=[x['end'] for x in w['execution_intervals'] if x['end'] is not None]
        result.append({'id':w['id'],'kind':'job','label':w['label'],'namespace':'loom','state':w['state'],
            'created':w['created'],'owner':w['owner'],'attempts':[],'loom_job':{'id':w['id']},'loom_workflow':w,
            'execution_intervals':w['execution_intervals'],
            'properties':{'record_role':'loom_job','category':w['category'],'state':w['state'],'created_at':w['created'],
                'started_at':min(starts) if starts else None,'process_exited_at':max(ends) if ends and w['state'] in ('complete','failed','cancelled') else None,
                'internal_step_count':len(w['executions'])}})
    return result


def refresh(c):
    for value in listing(c):
        old=exists(c,value['id'])
        errors=json.dumps(sorted(value['errors']))
        if old['state']==value['state'] and old['observed_errors']==errors:continue
        new_failure=bool(set(value['errors'])-set(json.loads(old['observed_errors'])))
        c.execute('UPDATE job_workflows SET state=?,observed_errors=? WHERE id=?',(value['state'],errors,value['id']))
        code={'complete':'complete','failed':'execution_failed','cancelled':'cancelled'}.get(value['state'])
        if not code and new_failure:code='execution_failed'
        if code:
            # Real node Attempts stay on their executions; a workflow event has no fabricated Attempt.
            c.execute('INSERT INTO events(job,target,code,created,attempt) VALUES(?,?,?,?,NULL)',(value['id'],value['notify'],code,time.time()))
