"""Logical Job continuity and producer-reported internal progress.

These records never release resources, dispatch work or change execution outcomes.
"""
import json
import os
import re
import stat

CONSOLIDATES_SCHEMA={'type':'array','minItems':1,'maxItems':64,'uniqueItems':True,
    'items':{'type':'string','pattern':'^[a-f0-9]{32}$'},
    'description':'Explicitly consolidate completed producer Jobs into this logical Job. Original execution/history and outputs remain intact.'}
STEPS_SCHEMA={'type':'array','maxItems':256,'items':{'type':'object','properties':{
    'id':{'type':'string','pattern':'^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$'},
    'label':{'type':'string','minLength':1,'maxLength':200}},'required':['id','label'],'additionalProperties':False}}
STATES={'pending','running','completed','reused','failed','skipped'}
LIMIT=256*1024


def validate_steps(plan):
    if not isinstance(plan,list) or len(plan)>256:raise ValueError('Invalid internal step plan')
    ids=set()
    for step in plan:
        if (not isinstance(step,dict) or set(step)!={'id','label'} or
            not isinstance(step['id'],str) or not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}',step['id']) or
            step['id'] in ids or not isinstance(step['label'],str) or not 1<=len(step['label'])<=200):
            raise ValueError('Invalid or duplicate internal step')
        ids.add(step['id'])


def sources(c, ids, owner, job=None):
    if not isinstance(ids,list) or not 1<=len(ids)<=64 or any(not isinstance(i,str) or not re.fullmatch('[a-f0-9]{32}',i) for i in ids) or len(set(ids))!=len(ids):
        raise ValueError('Invalid consolidated source Jobs')
    # A second live/finished canonical submission must not claim the same work.
    for other in c.execute("SELECT id,spec FROM jobs WHERE state!='cancelled'"):
        if other['id']!=job and set(json.loads(other['spec']).get('consolidates',[])) & set(ids):
            raise ValueError('Source Job already belongs to a consolidation')
    result=[]
    for identity in ids:
        row=c.execute('SELECT owner,state,attempt,result FROM jobs WHERE id=?',(identity,)).fetchone()
        if row is None or row['owner']!=owner:raise PermissionError('Consolidation source Owner differs or Job is missing')
        if row['state']!='complete' or not json.loads(row['result'] or '{}').get('artifact_ready'):
            raise ValueError('Consolidation sources must have completed verified outputs')
        result.append({'job':identity,'attempt':row['attempt']})
    return result


def completed_links(c):
    links={}
    for row in c.execute("SELECT id,spec,result FROM jobs WHERE state='complete'"):
        if json.loads(row['result'] or '{}').get('artifact_ready'):
            for source in json.loads(row['spec']).get('consolidates',[]):links[source]=row['id']
    return links


def progress(folder,spec):
    plan=spec.get('profile',{}).get('steps',[])
    if not plan:return None
    result={'schema':'loom-job-steps/v1','authority':'producer_reported; Job execution and artifact verification remain authoritative',
            'status':'planned','steps':[{**s,'state':'pending'} for s in plan]}
    if folder is None:return result
    try:
        directory=os.open(folder,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
        try:fd=os.open('steps.json',os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=directory)
        finally:os.close(directory)
        with os.fdopen(fd,'rb') as stream:
            before=os.fstat(stream.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_size>LIMIT:raise ValueError('Invalid progress file')
            raw=stream.read(LIMIT+1)
            if len(raw)>LIMIT:raise ValueError('Oversized progress file')
        value=json.loads(raw)
        if (not isinstance(value,dict) or value.get('schema')!='loom-job-steps/v1' or
            value.get('profile_sha256')!=spec['profile']['sha256'] or not isinstance(value.get('steps'),list)):
            raise ValueError('Invalid progress identity')
        reported={};allowed={s['id'] for s in plan}
        sources={s['job']:s['attempt'] for s in spec.get('_consolidation_sources',[])}
        for step in value['steps']:
            if (not isinstance(step,dict) or set(step)-{'id','state','detail','source_job','source_attempt'} or
                step.get('id') not in allowed or step['id'] in reported or step.get('state') not in STATES or
                not isinstance(step.get('detail',''),str) or len(step.get('detail',''))>1000):raise ValueError('Invalid step report')
            if step['state']=='reused' and (step.get('source_job') not in sources or step.get('source_attempt')!=sources[step['source_job']]):
                raise ValueError('Reused step must name a frozen source Job/Attempt')
            reported[step['id']]=step
        result.update(status='reported',steps=[{**s,'state':'pending',**reported.get(s['id'],{})} for s in plan])
    except FileNotFoundError:pass
    except (OSError,ValueError,TypeError,KeyError):result.update(status='unavailable',reason='Invalid or unreadable producer progress')
    return result
