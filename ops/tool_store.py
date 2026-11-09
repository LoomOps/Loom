"""Research software revisions, independent of Loom Job lifecycle components.

Snapshots share the existing administrator-granted components store, under .tools;
this introduces no new host mount or execution authority.
"""
import hashlib
import json
from pathlib import Path
import tempfile
import component_store
import ownership


REF = {'type':'object','properties':{'owner':{'type':'string'},'id':{'type':'string'},
       'revision':{'type':'integer','minimum':1}},'required':['id','revision'],'additionalProperties':False}
REFERENCES = {'type':'object','propertyNames':{'pattern':'^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$'},
              'additionalProperties':REF}


def root():
    path=component_store.ROOT/'.tools'
    if path.is_symlink():raise ValueError('Tool store must not be a symlink')
    return path


def init(c):
    c.execute('CREATE TABLE IF NOT EXISTS tools(owner TEXT,id TEXT,revision INTEGER,manifest TEXT,sha256 TEXT,PRIMARY KEY(owner,id,revision))')


def action_schema(action):
    fields={'owner':{'type':'string'}};required=[]
    if action!='list_tools':fields['id']={'type':'string','pattern':component_store.ID.pattern};required=['id']
    if action=='get_tool':fields['revision']={'type':'integer','minimum':1}
    if action=='register_tool':
        fields.update(manifest={'type':'object'},files={'type':'object','additionalProperties':{'type':'string'}},
                      component=REF)
        required.append('manifest')
    return {'type':'object','properties':fields,'required':required,'additionalProperties':False}


def snapshot_path(owner,identity,revision):
    return root()/owner/identity/'revisions'/str(revision)


def checked_files(files):
    if not isinstance(files,dict) or not files:raise ValueError('Tool source files required')
    for ref,content in files.items():
        p=Path(ref)
        if (not ref or p.is_absolute() or '..' in p.parts or str(p)!=ref or ref=='tool.json'
            or not isinstance(content,str) or any(str(parent) in files for parent in p.parents if str(parent)!='.')):
            raise ValueError('Invalid or conflicting Tool source path')


def detail(c,owner,identity,revision=None):
    query='SELECT * FROM tools WHERE owner=? AND id=?';args=[owner,identity]
    if revision is not None:query+=' AND revision=?';args.append(revision)
    row=c.execute(query+' ORDER BY revision DESC LIMIT 1',args).fetchone()
    if not row:raise ValueError('Tool revision not found')
    return {**dict(row),'manifest':json.loads(row['manifest']),**ownership.extension(c,'tool',owner,identity)}


def operate(c,actor,args,config):
    action=args['action'];owner=args.get('owner',actor['id'])
    if not actor.get('id'):raise PermissionError('Authenticated caller required')
    if action=='list_tools':
        query='SELECT owner,id,max(revision) revision FROM tools';values=()
        if 'owner' in args:query+=' WHERE owner=?';values=(owner,)
        return [detail(c,r['owner'],r['id'],r['revision']) for r in c.execute(query+' GROUP BY owner,id ORDER BY owner,id',values).fetchall()]
    identity=args['id']
    if not component_store.ID.fullmatch(owner) or not component_store.ID.fullmatch(identity):raise ValueError('Invalid Tool identity')
    if action=='get_tool':return detail(c,owner,identity,args.get('revision'))
    if action!='register_tool':raise ValueError('Unknown Tool operation')
    if 'submit' not in actor['scopes']:raise PermissionError('Submit grant required')
    manifest=json.loads(json.dumps(args['manifest'],allow_nan=False));files=args.get('files',{})
    if any(k in manifest for k in ('profiles','runtime','prepare','finalize','on_error','on_complete','on_cancel')):
        raise ValueError('Tools implement research functions; Job lifecycle belongs to Components and Profiles')
    if ref:=args.get('component'):
        if files:raise ValueError('Choose Tool files or an exact legacy Component revision')
        source_owner=ref.get('owner',owner)
        row=c.execute('SELECT manifest FROM components WHERE owner=? AND id=? AND revision=?',
                      (source_owner,ref['id'],ref['revision'])).fetchone()
        if not row:raise ValueError('Component revision not found')
        folder=component_store.ROOT/source_owner/ref['id']/'revisions'/str(ref['revision'])
        pins=json.loads(row['manifest']).get('source_pins',{})
        for path,digest in pins.items():
            p=folder/path
            if not p.resolve().is_relative_to(folder.resolve()) or p.is_symlink():raise ValueError('Unsafe legacy source')
            if hashlib.sha256(p.read_bytes()).hexdigest()!=digest:raise ValueError('Component revision changed')
            files[path]=p.read_text()
        manifest['origin']={'kind':'component','owner':source_owner,'id':ref['id'],'revision':ref['revision']}
    checked_files(files)
    manifest['source_pins']={p:hashlib.sha256(v.encode()).hexdigest() for p,v in files.items()}
    digest=hashlib.sha256(json.dumps(manifest,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    c.execute('BEGIN IMMEDIATE')
    latest=c.execute('SELECT revision,sha256 FROM tools WHERE owner=? AND id=? ORDER BY revision DESC LIMIT 1',(owner,identity)).fetchone()
    if latest and latest['sha256']==digest:return {**detail(c,owner,identity,latest['revision']),'unchanged':True}
    revision=c.execute('SELECT coalesce(max(revision),0)+1 FROM tools WHERE owner=? AND id=?',(owner,identity)).fetchone()[0]
    folder=snapshot_path(owner,identity,revision)
    if not folder.resolve().is_relative_to(root().resolve()):raise ValueError('Tool snapshot leaves store')
    snapshot={**files,'tool.json':json.dumps(manifest,sort_keys=True,indent=2)}
    if folder.exists():
        if folder.is_symlink() or any(p.is_symlink() for p in folder.rglob('*')):raise ValueError('Tool snapshot conflict')
        if {str(p.relative_to(folder)):p.read_text() for p in folder.rglob('*') if p.is_file()}!=snapshot:
            raise ValueError('Tool snapshot conflict')
    else:
        folder.parent.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(dir=folder.parent,prefix='.register-') as t:
            stage=Path(t)/'snapshot';stage.mkdir()
            for path,content in snapshot.items():
                p=stage/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(content)
            stage.rename(folder)
    c.execute('INSERT INTO tools VALUES(?,?,?,?,?)',(owner,identity,revision,json.dumps(manifest),digest))
    return detail(c,owner,identity,revision)


def bind(c,references,owner):
    import execution
    execution.validate(REFERENCES,references)
    pins={}
    for alias,ref in references.items():
        row=detail(c,ref.get('owner',owner),ref['id'],ref['revision'])
        pins[alias]={k:row[k] for k in ('owner','id','revision','sha256')}
        pins[alias].update(path=str(snapshot_path(row['owner'],row['id'],row['revision'])),source_pins=row['manifest']['source_pins'])
    return pins


def verify(pins):
    for alias,ref in pins.items():
        if not component_store.ID.fullmatch(alias):raise ValueError('Invalid Tool alias')
        expected=snapshot_path(ref['owner'],ref['id'],ref['revision'])
        if Path(ref['path'])!=expected or not expected.resolve().is_relative_to(root().resolve()):raise ValueError('Tool outside registered store')
        manifest=json.loads((expected/'tool.json').read_text())
        if (hashlib.sha256(json.dumps(manifest,sort_keys=True,separators=(',',':')).encode()).hexdigest()!=ref['sha256']
            or manifest.get('source_pins')!=ref['source_pins']):raise ValueError('Tool manifest changed')
        for path,digest in ref['source_pins'].items():
            p=expected/path
            if p.is_symlink() or not p.resolve().is_relative_to(expected.resolve()) or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:
                raise ValueError('Tool revision changed')
