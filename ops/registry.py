"""Versioned typed entities and immutable lineage indexed from completed jobs."""
import ownership
import hashlib
import json
import os
from pathlib import Path


def encode(value):
    return json.dumps(value,sort_keys=True,allow_nan=False)


def init(c):
    ownership.init(c)
    c.executescript("""
    CREATE TABLE IF NOT EXISTS node_control(id INTEGER PRIMARY KEY CHECK(id=1), mode TEXT NOT NULL);
    INSERT OR IGNORE INTO node_control VALUES(1,'active');
    CREATE TABLE IF NOT EXISTS entity_revisions(
      id TEXT NOT NULL, revision INTEGER NOT NULL, job TEXT NOT NULL,
      kind TEXT NOT NULL, owner TEXT NOT NULL, submitter TEXT NOT NULL,
      manifest TEXT NOT NULL, PRIMARY KEY(id,revision));
    CREATE TABLE IF NOT EXISTS entity_locations(
      id TEXT NOT NULL, revision INTEGER NOT NULL, component TEXT NOT NULL,
      machine_id TEXT NOT NULL, storage_id TEXT NOT NULL, ref TEXT NOT NULL,
      status TEXT NOT NULL, PRIMARY KEY(id,revision,component,machine_id,storage_id,ref));
    CREATE TABLE IF NOT EXISTS job_annotations(
      job TEXT NOT NULL, id TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(job,id));
    PRAGMA user_version=4;
    """)


def mode(c):
    return c.execute('SELECT mode FROM node_control WHERE id=1').fetchone()[0]


def stamp(st):
    return st.st_dev,st.st_ino,st.st_size,st.st_mtime_ns,st.st_ctime_ns


def stable_file(stream, before):
    after=os.fstat(stream.fileno())
    if stamp(before)!=stamp(after):
        raise ValueError('Artifact changed during validation')
    return after


def inspect(path, digest=None):
    if not path.is_file():
        return None,digest,'missing'
    with path.open('rb') as f:
        before=os.fstat(f.fileno())
        sha=hashlib.file_digest(f,'sha256').hexdigest()
        size=stable_file(f,before).st_size
    return size,digest or sha,'available' if digest in (None,sha) else 'corrupt'


def prepare(result,bulk,relative,*,facts=None,historical=False):
    prepared={}
    for e in result['entities']:
        path=relative(bulk,e['locator']['ref'])
        members={path:e['sha256']}
        for item in e.get('components',[]):
            members[relative(path.parent,item['ref'])]=item['sha256']
        if e['kind']=='model' and path.name=='model.safetensors':
            for p in sorted(path.parent.rglob('*')):
                if p.is_file():
                    members.setdefault(relative(path.parent,str(p.relative_to(path.parent))),None)
        components, locations = [], []
        for p,sha in members.items():
            if facts is not None and p in facts:
                previous,digest=facts[p]
                current=p.stat()
                size,status=current.st_size,'available' if stamp(previous)==stamp(current) else 'corrupt'
            else:
                size,digest,status=inspect(p,sha)
            if not historical and status!='available':
                raise ValueError('Output component missing or corrupt')
            name=str(p.relative_to(path.parent))
            components.append({'ref':name,'size':size,'sha256':digest})
            locations.append((name,e['locator']['machine_id'],e['locator']['storage_id'],
                              str(p.relative_to(bulk)),status))
        prepared[e['id']]=(components,locations)
    return prepared


def output_members(output):
    ref=output['locator']['ref']
    return {ref:output['sha256'], **{
        str(Path(ref).parent/item['ref']):item['sha256'] for item in output.get('components',[])}}


def index(c,row,result,config,bulk,relative,*,historical=False,prepared=None):
    if prepared is None:
        prepared=prepare(result,bulk,relative,historical=historical)
    local_ids={e['id']:e['kind'] for e in result['entities']}
    spec=json.loads(row['spec'])
    for e in result['entities']:
        eid=row['id']+'.'+e['id']
        if c.execute('SELECT 1 FROM entity_revisions WHERE id=? AND revision=1',(eid,)).fetchone():
            continue
        components,locations=prepared[e['id']]
        relations=[{'kind':r['kind'],'id':row['id']+'.'+r['entity_id'],'revision':1}
                   for r in e.get('relations',[]) if r['entity_id'] in local_ids]
        for name,input_ in spec.get('params',{}).items():
            if isinstance(input_,dict) and set(input_)=={'job','entity'}:
                upstream=c.execute('SELECT attempt FROM jobs WHERE id=?',(input_['job'],)).fetchone()
                if upstream:relations.append({'kind':'consumes','input':name,'id':input_['job']+'.'+input_['entity'],'revision':1,'attempt':upstream['attempt']})
                continue
            if not isinstance(input_,dict) or not {'ref','sha256','storage_id'}<=input_.keys():
                continue
            hits=c.execute("""SELECT e.id,e.revision,e.manifest FROM entity_revisions e
                JOIN entity_locations l ON (e.id,e.revision)=(l.id,l.revision)
                WHERE l.storage_id=? AND l.ref=? AND l.status='available'""",
                (input_['storage_id'],input_['ref']))
            for hit in hits:
                if any(x['sha256']==input_['sha256'] for x in json.loads(hit[2])['components']):
                    relation={'kind':'consumes','input':name,'id':hit[0],'revision':hit[1]}
                    if relation not in relations:
                        relations.append(relation)
            parts=Path(input_['ref']).parts
            if len(parts)>=4 and parts[0]=='runs':
                upstream=c.execute("""SELECT result FROM jobs WHERE id=? AND attempt=? AND
                    json_extract(result,'$.artifact_ready')=1""",
                    (parts[1],parts[2])).fetchone()
                if upstream:
                    for output in json.loads(upstream[0])['entities']:
                        loc=output['locator']
                        if loc['storage_id']!=input_['storage_id']:
                            continue
                        if output_members(output).get(input_['ref'])==input_['sha256']:
                            relation={'kind':'consumes','input':name,'id':parts[1]+'.'+output['id'],'revision':1}
                            if relation not in relations:
                                relations.append(relation)
        manifest={'id':eid,'revision':1,'kind':e['kind'],'origin_machine':e['locator']['machine_id'],
                  'job':row['id'],'attempt':row['attempt'],'provenance':e.get('provenance',{}),
                  'components':components,'relations':relations,
                  **{k:e[k] for k in ('metadata','label','description') if k in e},
                  'metadata_schema':spec.get('profile',{}).get('metadata_schemas',{}).get(e['id'],spec.get('profile',{}).get('metadata_schemas',{}).get('*')),
                  'metrics':[m for m in result['metrics'] if m['entity_id']==e['id']]}
        c.execute('INSERT INTO entity_revisions VALUES(?,?,?,?,?,?,?)',
                  (eid,1,row['id'],e['kind'],row['owner'],row['submitter'],encode(manifest)))
        job_owner=c.execute("SELECT owner FROM item_owners WHERE kind='job' AND id=?",(row['id'],)).fetchone()
        if job_owner:ownership.assign(c,'entity',eid,job_owner[0],row['submitter'])
        c.executemany('INSERT INTO entity_locations VALUES(?,?,?,?,?,?,?)',((eid,1,*loc) for loc in locations))


def view(c,actor,eid=None,revision=1):
    args=()
    where='1=1'
    if eid is None:
        return [ownership.decorate(c,'entity',dict(r)) for r in c.execute(
            'SELECT id,revision,kind,job,owner,submitter FROM entity_revisions WHERE '+where+' ORDER BY id LIMIT 250',args)]
    row=c.execute('SELECT manifest,owner FROM entity_revisions WHERE '+where+' AND id=? AND revision=?',
                  (*args,eid,revision)).fetchone()
    if not row:
        raise PermissionError('Entity outside grant')
    data=json.loads(row[0])
    data['locations']=[dict(r) for r in c.execute(
        'SELECT component,machine_id,storage_id,ref,status FROM entity_locations WHERE id=? AND revision=? ORDER BY component',
        (eid,revision))]
    return ownership.decorate(c,'entity',data,row['owner'])


def reconnect(c,actor,config,bulk,relative,eid,revision,component,ref):
    if type(revision) is not int or not isinstance(component,str) or not isinstance(ref,str):
        raise ValueError('Invalid component location')
    row=c.execute('SELECT manifest,owner FROM entity_revisions WHERE id=? AND revision=?',
                  (eid,revision)).fetchone()
    if not row:
        raise PermissionError('Only the entity owner may reconnect a location')
    expected=next((item for item in json.loads(row['manifest'])['components'] if item['ref']==component),None)
    if expected is None:
        raise ValueError('Unknown entity component')
    size,_,status=inspect(relative(bulk,ref),expected['sha256'])
    if status!='available' or size!=expected['size']:
        raise ValueError('Replacement location differs from immutable component')
    c.execute('BEGIN IMMEDIATE')
    if mode(c)!='active':
        raise PermissionError('Node admission fenced')
    c.execute("""INSERT INTO entity_locations VALUES(?,?,?,?,?,?,?)
      ON CONFLICT(id,revision,component,machine_id,storage_id,ref) DO UPDATE SET status=excluded.status""",
      (eid,revision,component,config['machine_id'],config['storage_id'],ref,'available'))
    return {'id':eid,'revision':revision,'component':component,'ref':ref,'status':'available'}
