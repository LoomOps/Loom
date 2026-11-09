"""User-owned editable components with immutable per-submission revisions."""
import hashlib
import json
import os
from pathlib import Path
import re
import execution
import profile_store
import ownership
from runtime_contract import seal

ROOT=Path(os.environ.get('LOOM_COMPONENTS','/loom/components'))
ID=re.compile(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}')

def init(c):
    profile_store.init(c)
    c.execute('''CREATE TABLE IF NOT EXISTS components(owner TEXT,id TEXT,revision INTEGER,
        manifest TEXT,profiles TEXT,PRIMARY KEY(owner,id,revision))''')

def allowed(actor,owner):return bool(actor.get('id'))

def action_schema(action):
    fields={'owner':{'type':'string','description':'Stable namespace. Register/write/get default to the authenticated principal; list defaults to all shared namespaces. Use built-in explicitly for common components. Display responsibility is returned in owner_profile.'}}
    required=[]
    if action!='list':fields['id']={'type':'string','pattern':ID.pattern};required=['id']
    if action=='register':
        fields.update(manifest={'type':'object','description':'profiles: name → execution v2 profile.'},files={'type':'object','additionalProperties':{'type':'string'},'description':'Relative path → UTF-8 source; omit to use saved editable sources.'});required.append('manifest')
    if action=='write':
        fields.update(path={'type':'string','description':'Relative source path.'},content={'type':'string'});required+=['path','content']
    return {'type':'object','properties':fields,'required':required,'additionalProperties':False}

def operate(c,actor,args,config):
    action=args['action'];owner=args.get('owner',actor['id'])
    if not allowed(actor,owner):raise PermissionError('Component outside delegated owner')
    if action=='list':
        query='SELECT owner,id,max(revision) AS revision FROM components'
        values=()
        if 'owner' in args:query+=' WHERE owner=?';values=(owner,)
        return [{**dict(r),**ownership.extension(c,'component',r['owner'],r['id'])} for r in c.execute(query+' GROUP BY owner,id ORDER BY owner,id',values)]
    identity=args.get('id','')
    if not isinstance(identity,str) or not ID.fullmatch(identity) or not ID.fullmatch(owner):raise ValueError('Invalid component identity')
    if action=='get':
        row=c.execute('SELECT * FROM components WHERE owner=? AND id=? ORDER BY revision DESC LIMIT 1',(owner,identity)).fetchone()
        if not row:raise ValueError('Component not found')
        return {**dict(row),**ownership.extension(c,'component',owner,identity),'manifest':json.loads(row['manifest']),'profiles':json.loads(row['profiles'])}
    if action=='write':
        if 'submit' not in actor['scopes']:raise PermissionError('Submit grant required')
        ref=args['path'];content=args['content']
        if not isinstance(ref,str) or not ref or Path(ref).is_absolute() or '..' in Path(ref).parts or not isinstance(content,str):raise ValueError('Invalid source file')
        base=ROOT/owner/identity/'src';target=base/ref
        if not target.resolve().is_relative_to(base.resolve()) or not base.resolve().is_relative_to(ROOT.resolve()):raise ValueError('Source leaves owner root')
        target.parent.mkdir(parents=True,exist_ok=True);target.write_text(content)
        return {'owner':owner,'id':identity,'path':ref,'bytes':len(content.encode()),**ownership.extension(c,'component',owner,identity)}
    if action!='register':raise ValueError('Unknown component operation')
    if 'submit' not in actor['scopes']:raise PermissionError('Submit grant required')
    manifest=args['manifest'];files=args.get('files',{})
    if not files:
        base=ROOT/owner/identity/'src'
        if not base.resolve().is_relative_to(ROOT.resolve()):raise ValueError('Source outside owner root')
        files={}
        for path in base.rglob('*'):
            if path.is_file():
                if not path.resolve().is_relative_to(base.resolve()):raise ValueError('Source link leaves component')
                files[str(path.relative_to(base))]=path.read_text()
    if not isinstance(manifest,dict) or not isinstance(files,dict) or not files:raise ValueError('Manifest and source files required')
    for ref,content in files.items():
        if not isinstance(ref,str) or Path(ref).is_absolute() or '..' in Path(ref).parts or not ref or not isinstance(content,str):raise ValueError('Invalid component source')
    c.execute('BEGIN IMMEDIATE')
    version=c.execute('SELECT COALESCE(MAX(revision),0)+1 FROM components WHERE owner=? AND id=?',(owner,identity)).fetchone()[0]
    base=ROOT/owner/identity;folder=base/'revisions'/str(version)
    if not folder.resolve().is_relative_to(ROOT.resolve()):raise ValueError('Invalid component root')
    profiles={}
    for name,definition in manifest.get('profiles',{}).items():
        if not ID.fullmatch(name):raise ValueError('Invalid profile name')
        profile={**definition,'schema':execution.REVISION,'revision':f'{owner}.{identity}.{name}.r{version}',
            'component_path':str(folder),'source_pins':{ref:hashlib.sha256(content.encode()).hexdigest() for ref,content in files.items()},'submitters':[actor['id'],owner],'owners':[owner]}
        profile_store.bind_runners(profile,config,owner)
        profiles[profile['revision']]=seal(profile)
    manifest={**manifest,'source_pins':{ref:hashlib.sha256(content.encode()).hexdigest() for ref,content in files.items()}}
    folder.mkdir(parents=True,exist_ok=False)
    for ref,content in files.items():
        path=folder/ref;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(content)
    (folder/'component.json').write_text(json.dumps(manifest,indent=2))
    # The editable source is a convenience checkout, never the running revision.
    work=base/'src';work.mkdir(exist_ok=True)
    for ref,content in files.items():
        path=work/ref
        if not path.resolve().is_relative_to(work.resolve()):raise ValueError('Editable source leaves component')
        path.parent.mkdir(parents=True,exist_ok=True);path.write_text(content)
    (base/'component.json').write_text(json.dumps(manifest,indent=2))
    c.execute('INSERT INTO components VALUES(?,?,?,?,?)',(owner,identity,version,json.dumps(manifest),json.dumps(profiles)))
    return {'owner':owner,'id':identity,'revision':version,'profiles':list(profiles),**ownership.extension(c,'component',owner,identity)}

def merge(c,config,actor=None):
    profiles=dict(config.get('profiles',{}))
    labels=dict(config.get('_profile_owners',{}))
    for row in c.execute('SELECT owner,id,profiles FROM components'):
        if actor is not None and not allowed(actor,row['owner']):continue
        for kind,p in json.loads(row['profiles']).items():
            if actor is not None and actor['id'] not in p['submitters']:p={**p,'submitters':[*p['submitters'],actor['id']]}
            profiles[kind]=p
            labels[kind]=ownership.extension(c,'component',row['owner'],row['id'])
    for row in c.execute('SELECT owner,id,profile FROM execution_profiles'):
        labels[json.loads(row['profile'])['revision']]=ownership.extension(c,'profile',row['owner'],row['id'])
    return {**config,'profiles':profile_store.merge(c,profiles,actor),'_profile_owners':labels}
