"""Administrator-managed execution environments, separate from Job/ticket databases.

Only the host installer writes this catalog. The executor exposes grant-filtered
discovery and exact immutable pins; it never accepts build commands from Jobs.
"""
import hashlib
import json
from pathlib import Path
import re
import sqlite3
import time

ID = re.compile(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}')


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def digest(value):
    return hashlib.sha256(encoded(value).encode()).hexdigest()


def connect(root, write=False):
    path = Path(root) / 'runtimes.sqlite3'
    if path.is_symlink():
        raise ValueError('Runtime database must not be linked')
    if not write and not path.exists():
        return None
    existed=path.exists()
    c = sqlite3.connect(path if write else path.absolute().as_uri()+'?mode=ro', uri=not write, timeout=10)
    if write and not existed:path.chmod(0o600)
    c.row_factory = sqlite3.Row
    if write:
        c.executescript('''
          CREATE TABLE IF NOT EXISTS runtime_revisions(
            id TEXT, revision INTEGER, manifest TEXT, sha256 TEXT,
            created REAL, PRIMARY KEY(id,revision));
          CREATE TABLE IF NOT EXISTS runtime_grants(
            instance TEXT, id TEXT, revision INTEGER,
            PRIMARY KEY(instance,id,revision));
          CREATE TABLE IF NOT EXISTS runtime_installations(
            id TEXT PRIMARY KEY, runtime TEXT, state TEXT, evidence TEXT, updated REAL);
        ''')
    return c


def dependencies_root(config, instance):
    value = config['instances'][instance].get('dependencies')
    if not isinstance(value, str) or not Path(value).is_absolute():
        raise ValueError('This instance has no configured dependency directory; attach its installation plan first')
    path = Path(value)
    if path.resolve() != path or not path.is_dir():
        raise ValueError('Configured dependency directory is missing or redirected')
    return path


def register(root, identity, manifest, instances):
    if not isinstance(identity, str) or not ID.fullmatch(identity):
        raise ValueError('Invalid Runtime ID')
    if not instances or any(not isinstance(i,str) or not ID.fullmatch(i) for i in instances):
        raise ValueError('Runtime requires explicit instance grants')
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', manifest.get('runtime',{}).get('image','')):
        raise ValueError('Runtime requires a local immutable image ID')
    if manifest.get('validation',{}).get('status') != 'passed':
        raise ValueError('Runtime requires successful execution validation')
    validate_highlights(manifest)
    content = encoded(manifest)
    with connect(root, True) as c:
        c.execute('BEGIN IMMEDIATE')
        # Identical content is retryable; a different environment gets a new revision.
        row = c.execute('SELECT revision FROM runtime_revisions WHERE id=? AND sha256=?',
                        (identity,digest(manifest))).fetchone()
        revision = row[0] if row else c.execute('SELECT coalesce(max(revision),0)+1 FROM runtime_revisions WHERE id=?',(identity,)).fetchone()[0]
        if not row:
            c.execute('INSERT INTO runtime_revisions VALUES(?,?,?,?,?)',
                      (identity,revision,content,digest(manifest),time.time()))
        c.executemany('INSERT OR IGNORE INTO runtime_grants VALUES(?,?,?)',
                      [(i,identity,revision) for i in instances])
    return {'id':identity,'revision':revision,'sha256':digest(manifest)}


def validate_highlights(manifest):
    selectors=manifest.get('primary_dependencies',[])
    if not isinstance(selectors,list) or len(selectors)>8:raise ValueError('Select at most eight representative dependencies')
    for selector in selectors:
        if not isinstance(selector,dict) or set(selector)-{'name','scope'} or not isinstance(selector.get('name'),str) or not selector['name'] or ('scope' in selector and not isinstance(selector['scope'],str)):
            raise ValueError('Representative dependencies require a name and optional scope')
        found=[d for d in manifest.get('dependencies',[]) if d['name'].casefold()==selector['name'].casefold() and ('scope' not in selector or d.get('scope')==selector['scope'])]
        if not found or len({d['version'] for d in found})!=1:raise ValueError('Representative dependency is missing or ambiguous; select its scope')


def detail(root, instance, ref, verify_inventory=False):
    if not isinstance(ref,dict) or set(ref)-{'id','revision','sha256'} or not ID.fullmatch(str(ref.get('id',''))) or type(ref.get('revision')) is not int or ref['revision']<1:
        raise ValueError('Select an exact Runtime id/revision')
    c = connect(root)
    if c is None:
        raise PermissionError('Runtime catalog unavailable')
    try:
        row = c.execute('''SELECT r.* FROM runtime_revisions r JOIN runtime_grants g
          ON (r.id=g.id AND r.revision=g.revision)
          WHERE g.instance=? AND r.id=? AND r.revision=?''',
          (instance,ref['id'],ref['revision'])).fetchone()
        if not row:
            raise PermissionError('Runtime revision outside instance grant')
        value = {**dict(row),'manifest':json.loads(row['manifest']),'legacy':False}
        if digest(value['manifest']) != value['sha256'] or ref.get('sha256',value['sha256']) != value['sha256']:
            raise ValueError('Runtime revision integrity mismatch')
        if verify_inventory:
            for path,expected in value['manifest'].get('reconciliation',{}).get('source_pins',{}).items():
                p=Path(path)
                if not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest()!=expected:
                    raise ValueError('Observed dependency metadata changed; reconcile before binding this Runtime')
        return value
    finally:
        c.close()


def listing(root, config, instance, image_identity=None):
    rows = [];installations=[]
    c = connect(root)
    if c is not None:
        try:
            refs=c.execute('SELECT id,revision FROM runtime_grants WHERE instance=? ORDER BY id,revision DESC',(instance,)).fetchall()
            for row in c.execute('SELECT * FROM runtime_installations ORDER BY updated DESC'):
                evidence=json.loads(row['evidence'])
                if evidence.get('instance')==instance:
                    installations.append({k:row[k] for k in ('id','runtime','state','updated')} | {
                        k:evidence[k] for k in ('path','recipe_sha256','error','container_returned','runtime_ref') if k in evidence})
        finally:
            c.close()
        rows=[detail(root,instance,dict(ref)) for ref in refs]
        for item in rows:
            evidence=item['manifest'].get('installation',{})
            for name in ('source_pins','context_pins'):
                if name in evidence:
                    evidence[name+'_count']=len(evidence.pop(name))
                    item['manifest_summary']=True
    # Reconciled observations bind current administrator policy and measured
    # metadata. Old Profile/Job runtime_id fields and execution behavior stay intact.
    for identity in config['instances'][instance].get('runtimes',[]):
        runtime=config.get('runtimes',{}).get(identity)
        if not runtime:continue
        candidates=[item for item in rows if item['manifest'].get('reconciliation',{}).get('runtime_id')==identity
                    and item['manifest']['reconciliation'].get('policy_sha256')==digest(runtime)]
        selected=max(candidates,key=lambda x:(x.get('created',0),x['revision'])) if candidates else None
        valid=False
        if selected:
            observed=selected['manifest']['reconciliation']
            try:
                image=image_identity(runtime['image']) if image_identity else runtime['image']
                valid=image==observed.get('image_id') and all(Path(p).is_file() and hashlib.sha256(Path(p).read_bytes()).hexdigest()==sha for p,sha in observed.get('source_pins',{}).items())
            except (OSError,RuntimeError):valid=False
        if valid:
            selected.setdefault('aliases',[]).append(identity)
        else:
            rows.append({'id':identity,'revision':None,'legacy':True,'sha256':None,
                'manifest':{'label':identity,'runtime':runtime,'dependencies':None,'validation':{'status':'unrecorded'}}})
    for item in rows:
        evidence=item['manifest'].get('reconciliation',{})
        if 'source_pins' in evidence:
            evidence['metadata_files']=len(evidence.pop('source_pins'));item['manifest_summary']=True
    return {'configured_dependencies':config['instances'][instance].get('dependencies'),
            'database':'runtimes.sqlite3','runtimes':rows,'installations':installations[:100],
            'installation_count':len(installations),'installations_truncated':len(installations)>100}


def installation(root, identity, runtime, state, evidence):
    with connect(root,True) as c:
        c.execute('INSERT INTO runtime_installations VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET state=excluded.state,evidence=excluded.evidence,updated=excluded.updated',
                  (identity,runtime,state,encoded(evidence),time.time()))


def validation_fenced(root):
    c=connect(root)
    if c is None:return False
    try:
        return any((e:=json.loads(row[0])).get('validation_gpu') and e.get('container_returned') is False
                   for row in c.execute('SELECT evidence FROM runtime_installations'))
    finally:c.close()
