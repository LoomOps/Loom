"""Revision-scoped display metadata. Never rewrites sealed source or Job pins."""
import json
import time
import component_store

TAGS={'sdg':'SDG','training':'Training','evaluation':'Evaluation','validation':'Validation',
      'data-processing':'Data processing','motion-planning':'Motion planning','policy':'Policy',
      'control':'Control','simulation':'Simulation','rendering':'Rendering','sensing':'Sensing',
      'perception':'Perception','metrics':'Metrics','packaging':'Packaging','platform-check':'Platform check','utility':'Utility'}
SCHEMA={'type':'object','properties':{'kind':{'enum':['component','tool']},'owner':{'type':'string'},
    'id':{'type':'string','pattern':component_store.ID.pattern},'revision':{'type':'integer','minimum':1},
    'tags':{'type':'array','items':{'enum':list(TAGS)},'uniqueItems':True,'maxItems':len(TAGS)},
    'label':{'type':'string','minLength':1,'maxLength':160},'evidence':{'type':'string','minLength':1,'maxLength':2000}},
    'required':['kind','owner','id','revision','tags','evidence'],'additionalProperties':False}


def init(c):
    c.execute('''CREATE TABLE IF NOT EXISTS software_metadata(
        kind TEXT,owner TEXT,id TEXT,revision INTEGER,value TEXT,actor TEXT,updated REAL,
        PRIMARY KEY(kind,owner,id,revision))''')


def read(c,kind,owner,identity,revision,manifest=None):
    available=c.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='software_metadata'").fetchone()
    row=c.execute('SELECT value FROM software_metadata WHERE kind=? AND owner=? AND id=? AND revision=?',
                  (kind,owner,identity,revision)).fetchone() if available else None
    if row:return json.loads(row[0])
    tags=(manifest or {}).get('tags',[])
    return {'tags':[t for t in tags if isinstance(t,str) and t in TAGS] if isinstance(tags,list) else [],
            'label':(manifest or {}).get('label')}


def annotate(c,actor,args):
    if 'submit' not in actor['scopes']:raise PermissionError('Submit grant required')
    from execution import validate
    validate(SCHEMA,args)
    if not component_store.ID.fullmatch(args['owner']):raise ValueError('Invalid namespace')
    table={'component':'components','tool':'tools'}[args['kind']]
    if not c.execute('SELECT 1 FROM '+table+' WHERE owner=? AND id=? AND revision=?',
                     (args['owner'],args['id'],args['revision'])).fetchone():raise ValueError('Software revision not found')
    component_store.mutation(c)
    value={k:args[k] for k in ('tags','label','evidence') if k in args}
    encoded=json.dumps(value,sort_keys=True,separators=(',',':'))
    old=c.execute('SELECT value FROM software_metadata WHERE kind=? AND owner=? AND id=? AND revision=?',
                  tuple(args[k] for k in ('kind','owner','id','revision'))).fetchone()
    if old and old[0]==encoded:return {**value,'unchanged':True}
    c.execute('INSERT INTO software_metadata VALUES(?,?,?,?,?,?,?) ON CONFLICT(kind,owner,id,revision) DO UPDATE SET value=excluded.value,actor=excluded.actor,updated=excluded.updated',
              tuple(args[k] for k in ('kind','owner','id','revision'))+(encoded,actor['id'],time.time()))
    return value
