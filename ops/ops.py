"""Local Loom Ops service with a durable, single-worker execution queue."""
import fcntl
import hashlib
import hmac
import json
import job_work
import math
import os
import re
import signal
import sqlite3
import ssl
import threading
import subprocess
import sys
import time
import uuid
from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from functools import lru_cache

# Source checkouts and the image both use the same collector implementation.
sys.path.append(str(Path(__file__).resolve().parents[1]/'dashboard'))
import diagnostics

class DiagnosticHTTPServer(diagnostics.ServerMixin, ThreadingHTTPServer):
    pass

import hooks
import runtime_client
import component_store
import profile_store
import mcp_groups
import recovery
import queue_control
import execution
import access
import ownership
import build_version
import release_updates
import onboarding
import dashboard_read
import docker_status
import history_store
import job_history
import research
import workflows
import job_contract
import profiles
from urllib.parse import urlsplit, parse_qs, unquote
import storage
import registry
from remote_fence import WorkerGrant
from profiles import APP, file_sha, installed, typed_contract, validate_profile

LOCAL = Path('/loom/ops')
BULK = Path('/loom/bulk')
CONFIG = Path(os.environ.get('LOOM_CONFIG_PATH', str(LOCAL / 'config.json')))
ID = re.compile(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,95}')
STOP = False


def ident(value):
    if not isinstance(value, str) or not ID.fullmatch(value):
        raise ValueError('Invalid identifier')
    return value


def dump(value):
    return json.dumps(value, sort_keys=True, allow_nan=False)


def read(path):
    return json.loads(path.read_text())


def relative(root, ref):
    path = Path(ref)
    if path.is_absolute() or '..' in path.parts or not path.parts:
        raise ValueError('Invalid relative locator')
    target = (root / path).resolve()
    if not target.is_relative_to(root.resolve()):
        raise ValueError('Locator outside root')
    return target


def guard(config):
    expected = config['storage_device']
    rows = [row.split() for row in Path('/proc/self/mountinfo').read_text().splitlines()]
    if not any(row[4] == str(BULK) and row[2] == expected for row in rows):
        raise RuntimeError('Expected bulk device is not mounted')
    if read(BULK / '.loom-storage.json') != {'storage_id': config['storage_id']}:
        raise RuntimeError('Storage identity mismatch')


@contextmanager
def db():
    conn = sqlite3.connect(LOCAL / 'ops.sqlite3', timeout=10)
    conn.row_factory = sqlite3.Row
    try:
        with conn:
            yield conn
    finally:
        conn.close()


def init():
    with (LOCAL/'schema.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        _init_schema()


def _init_schema():
    with db() as c:
        version=c.execute('PRAGMA user_version').fetchone()[0]
        if version not in (0,4):
            raise RuntimeError('Unsupported Ops schema')
        c.execute('PRAGMA journal_mode=WAL')
        c.executescript("""
        CREATE TABLE IF NOT EXISTS jobs(
            id TEXT PRIMARY KEY, submitter TEXT NOT NULL, idem TEXT NOT NULL,
            spec TEXT NOT NULL, owner TEXT NOT NULL, notify TEXT NOT NULL,
            state TEXT NOT NULL, attempt TEXT, created REAL NOT NULL,
            result TEXT, cancel INTEGER NOT NULL DEFAULT 0,
            UNIQUE(submitter,idem));
        CREATE INDEX IF NOT EXISTS jobs_ready_order ON jobs(state,created);
        CREATE TABLE IF NOT EXISTS attempts(
            id TEXT PRIMARY KEY, job TEXT NOT NULL, state TEXT NOT NULL,
            started REAL NOT NULL, ended REAL);
        CREATE TABLE IF NOT EXISTS result_outbox(
            job TEXT PRIMARY KEY, attempt TEXT NOT NULL,
            retries INTEGER NOT NULL DEFAULT 0, next_try REAL NOT NULL DEFAULT 0);
        CREATE TABLE IF NOT EXISTS events(
            seq INTEGER PRIMARY KEY, job TEXT NOT NULL, target TEXT NOT NULL,
            code TEXT NOT NULL, created REAL NOT NULL, ack INTEGER NOT NULL DEFAULT 0);
        """)
        columns={r[1] for r in c.execute('PRAGMA table_info(jobs)')}
        if 'available_at' not in columns:c.execute('ALTER TABLE jobs ADD COLUMN available_at REAL NOT NULL DEFAULT 0')
        columns={r[1] for r in c.execute('PRAGMA table_info(events)')}
        if 'attempt' not in columns:c.execute('ALTER TABLE events ADD COLUMN attempt TEXT')
        c.execute('CREATE UNIQUE INDEX IF NOT EXISTS event_attempt_code ON events(job,attempt,code) WHERE attempt IS NOT NULL')
        registry.init(c)
        access.init(c)
        history_store.init(c)
        recovery.init(c)
        queue_control.init(c)
        ownership.init(c)
        component_store.init(c)
        import software_metadata
        software_metadata.init(c)
        import runtime_hub
        runtime_hub.init(c)
        hooks.init(c)
        research.init(c)
        workflows.init(c)
        c.execute("CREATE TABLE IF NOT EXISTS stage_runs(job TEXT,attempt TEXT,stage TEXT,started REAL,ended REAL,state TEXT,PRIMARY KEY(job,attempt,stage))")
        c.execute('CREATE TABLE IF NOT EXISTS attempt_results(job TEXT,attempt TEXT,result TEXT NOT NULL,PRIMARY KEY(job,attempt))')
        c.execute('CREATE TABLE IF NOT EXISTS finalizer_receipts(job TEXT,attempt TEXT,completed REAL NOT NULL,PRIMARY KEY(job,attempt))')


def submission_matches(c, old, requested):
    """A retry identifies the original request, never a later recovery revision."""
    original=c.execute('SELECT spec FROM job_revisions WHERE job=? AND revision=0',(old['id'],)).fetchone()
    frozen=json.loads(original['spec'] if original else old['spec'])
    candidate={**requested,'params':execution.apply_defaults(frozen.get('profile',{}),requested['params'])}
    return (all(frozen.get(field)==candidate[field] for field in ('key','kind','owner','notify','params'))
        and all(frozen.get(field,default)==candidate.get(field,frozen.get('presentation',{}) if field=='presentation' else default) for field,default in
                [('consolidates',[]),('execution',{}),('queue_position','tail'),('output_storage','data/0')])
        and frozen.get('_requested_presentation',frozen.get('presentation'))==requested.get('presentation')
        and job_contract.values(frozen)==job_contract.values(candidate,strict=True)
        and frozen.get('_requested_dependencies',sorted(frozen.get('depends_on',[])))==sorted(candidate.get('depends_on',[])))


def submit(c, actor, spec, config):
    if not isinstance(spec, dict) or set(spec)-{'depends_on','execution','queue_position','output_storage','consolidates','presentation', *job_contract.FIELDS} != {'key','kind','owner','notify','params'}:
        raise ValueError('Invalid job specification')
    if spec.get('queue_position','tail') not in ('next','tail'):raise ValueError('Invalid submission queue_position')
    if 'presentation' in spec:execution.validate(research.PRESENTATION,spec['presentation'])
    spec={**spec,'params':workflows.resolve_inputs(c,spec['params'])} if isinstance(spec.get('params'),dict) else spec
    classification = job_contract.values(spec, strict=True)
    owner, target, key = (ident(spec[k]) for k in ('owner','notify','key'))
    params = spec['params']
    if not isinstance(params, dict):
        raise ValueError('Job params must be an object')
    dependencies=spec.get('depends_on',[])
    if isinstance(dependencies,list):
        expanded=[]
        for dependency in dependencies:
            if isinstance(dependency,str) and workflows.exists(c,dependency):
                expanded.extend(r[0] for r in c.execute('SELECT job FROM workflow_nodes WHERE root=?',(dependency,)))
            else:expanded.append(dependency)
        dependencies=list(dict.fromkeys(expanded)) if all(isinstance(d,str) for d in expanded) else expanded
        spec={**spec,'depends_on':dependencies} if 'depends_on' in spec else spec
    if not isinstance(dependencies,list) or len(dependencies)>64 or len(set(map(ident,dependencies)))!=len(dependencies):
        raise ValueError('Invalid job dependencies')
    requested_dependencies=sorted(dependencies)
    old = c.execute('SELECT id,spec FROM jobs WHERE submitter=? AND idem=?',(actor['id'],key)).fetchone()
    if old:
        if not submission_matches(c,old,spec):
            raise ValueError('Idempotency conflict')
        if registry.mode(c) != 'active':
            raise PermissionError('Job admission fenced')
        return {'id':old['id']}
    requested=json.loads(dump(spec))
    config=component_store.merge(c,config,actor)
    params=execution.apply_defaults(config.get('profiles',{}).get(spec['kind'],{}),params)
    spec={**spec,'params':params,'queue_position':spec.get('queue_position','tail')}
    _,output_id=storage.select(config,BULK,spec.get('output_storage','data/0'),verify=True)
    if spec.get('output_storage','data/0')!='data/0':spec={**spec,'_output_storage_id':output_id}
    dependencies=list(requested_dependencies)
    if 'consolidates' in spec:
        spec={**spec,'_consolidation_sources':job_work.sources(c,spec['consolidates'],owner)}
        dependencies.extend(i for i in spec['consolidates'] if i not in dependencies)
    selected=config.get('profiles',{}).get(spec['kind'],{})
    if execution.is_generic(selected):
        for dependency in execution.input_dependencies(selected,params):
            if dependency not in dependencies:dependencies.append(dependency)
    for item in params.values():
        if not isinstance(item,dict) or not isinstance(item.get('ref'),str):
            continue
        parts=Path(item['ref']).parts
        if len(parts)>=4 and parts[0]=='runs':
            upstream=c.execute('SELECT attempt FROM jobs WHERE id=?',(parts[1],)).fetchone()
            if upstream is not None:
                if upstream['attempt']!=parts[2]:
                    raise ValueError('Input does not name the upstream Attempt')
                if parts[1] not in dependencies:
                    dependencies.append(parts[1])
    for dependency in dependencies:
        predecessor=c.execute('SELECT owner,submitter FROM jobs WHERE id=?',(dependency,)).fetchone()
        if not predecessor:
            raise PermissionError('Dependency outside grant')
    if dependencies:
        spec={**spec,'depends_on':dependencies,'_requested_dependencies':requested_dependencies}
    kind = spec['kind']
    if kind == 'cpu.smoke':
        if set(params) != {'seconds','fail'} or type(params['seconds']) is not int or not 0 <= params['seconds'] <= 20 or type(params['fail']) is not bool:
            raise ValueError('Invalid bounded CPU smoke parameters')
    else:
        if not isinstance(kind,str) or not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,255}',kind):raise ValueError('Invalid profile identity')
        profile = installed(config,kind,actor,params,BULK,relative,ident,dependencies)
        spec = {**spec,'profile':profile}
    for item in params.values():
        if isinstance(item,dict) and isinstance(item.get('ref'),str):
            parts=Path(item['ref']).parts
            if parts and parts[0]=='runs':
                source=c.execute('SELECT owner,submitter FROM jobs WHERE id=?',(parts[1],)).fetchone() if len(parts)>1 else None
                if not source or not recovery.permitted(actor,source):raise PermissionError('Input belongs to another user')
            elif not any(Path(item['ref']).is_relative_to(prefix) for prefixes in config.get('input_grants',{}).values() for prefix in prefixes):
                raise PermissionError('Input outside user storage grant')
    execution.policy(spec,spec.get('profile',{'timeout':30,'budget':{'attempts':2}}))
    spec = {**spec, **classification,'presentation':research.defaults(c,spec),'_requested_presentation':requested.get('presentation')}
    payload = dump(spec)
    if not c.in_transaction:c.execute('BEGIN IMMEDIATE')
    if registry.mode(c) != 'active':
        raise PermissionError('Job admission fenced')
    old = c.execute('SELECT id,spec FROM jobs WHERE submitter=? AND idem=?',(actor['id'],key)).fetchone()
    if old:
        if not submission_matches(c,old,requested):
            raise ValueError('Idempotency conflict')
        return {'id': old['id']}
    if spec.get('consolidates'):
        if job_work.sources(c,spec['consolidates'],owner)!=spec['_consolidation_sources']:
            raise ValueError('Consolidation source Attempt changed')
    job = uuid.uuid4().hex
    c.execute('INSERT INTO jobs(id,submitter,idem,spec,owner,notify,state,created) VALUES(?,?,?,?,?,?,?,?)',
              (job,actor['id'],key,payload,owner,target,'queued',time.time()))
    person=owner if c.execute('SELECT 1 FROM user_profiles WHERE id=?',(owner,)).fetchone() else (
        actor.get('profile_id') if owner==actor['id'] else next(
            (a.get('profile_id') for a in config.get('actors',[]) if a['id']==owner),None))
    if person and c.execute('SELECT 1 FROM user_profiles WHERE id=?',(person,)).fetchone():
        ownership.assign(c,'job',job,person,actor['id'])
    queue_control.append_order(c,job)
    if spec.get('queue_position')=='next':
        queue_control.reorder(c,c.execute('SELECT * FROM jobs WHERE id=?',(job,)).fetchone(),actor,{'position':'next','reason':'Priority submission'})
    c.execute('INSERT INTO job_revisions VALUES(?,?,?,?,?)',(job,0,payload,'submission',time.time()))
    return {'id':job}


def event(c, row, code):
    c.execute('INSERT OR IGNORE INTO events(job,target,code,created,attempt) VALUES(?,?,?,?,?)',
              (row['id'],row['notify'],code,time.time(),row['attempt']))


def public(row):
    data = dict(row)
    data['spec'] = json.loads(data['spec'])
    data['spec'].pop('_requested_dependencies',None)
    data['spec'].pop('_requested_presentation',None)
    data['spec'].pop('_output_storage_id',None)
    data['spec'].pop('_consolidation_sources',None)
    data['result'] = json.loads(data['result']) if data['result'] else None
    if data['state'] in ('queued','preparing','ready','running') and data['result'] is not None:
        # Old installations may retain an earlier Attempt's result on the Job.
        # Preserve that evidence without attributing it to current execution.
        data['previous_result']=data['result'];data['result']=None
    data.update(job_contract.values(data['spec']))
    data['status']={'complete':'completed','processing':'finalizing','result_failed':'finalizing_failed','blocked':'queued','interrupted':'failed'}.get(data['state'],data['state'])
    data.pop('idem')
    return data


def artifact(config, folder, manifest, profile=None, facts=None, output=None):
    output_root,output_id=output or (BULK,config['storage_id'])
    if set(manifest) != {'entities','metrics'}:
        raise ValueError('Invalid result manifest')
    if profile is not None:
        typed_contract(manifest,profile,ident)
    entities = manifest['entities']
    seen = set()
    for entity in entities:
        ident(entity['id'])
        if entity['id'] in seen or entity['kind'] not in {'artifact','dataset','model','experiment'}:
            raise ValueError('Invalid result identity')
        seen.add(entity['id'])
        path = relative(folder, entity.pop('path'))
        if not path.is_file():
            raise ValueError('Missing result artifact')
        with path.open('rb') as stream:
            before=os.fstat(stream.fileno())
            entity['sha256'] = hashlib.file_digest(stream,'sha256').hexdigest()
            os.fsync(stream.fileno())
            if facts is not None:
                facts[path] = (registry.stable_file(stream,before), entity['sha256'])
        if 'components' in entity:
            members = []
            for ref in entity['components']:
                member = relative(path.parent,ref)
                if member==path or not member.is_file() or any(x['ref']==ref for x in members):
                    raise ValueError('Invalid artifact component')
                with member.open('rb') as stream:
                    before=os.fstat(stream.fileno())
                    digest=hashlib.file_digest(stream,'sha256').hexdigest()
                    os.fsync(stream.fileno())
                    if facts is not None:
                        facts[member] = (registry.stable_file(stream,before), digest)
                members.append({'ref':ref,'sha256':digest})
            entity['components']=members
        entity['locator'] = {'machine_id':config['machine_id'],'storage_id':output_id,
                             'ref':str(path.relative_to(output_root))}
    if not entities:
        raise ValueError('No result artifacts')
    for metric in manifest['metrics'] if profile is None else []:
        ident(metric['name'])
        if metric['entity_id'] not in seen or type(metric['value']) not in (int,float) or not math.isfinite(metric['value']):
            raise ValueError('Invalid metric')
    return manifest


def finish(row, state, result=None, config=None, prepared=None):
    if state=='failed':diagnostics.record('worker','finish',code='job_failed')
    with db() as c:
        c.execute('BEGIN IMMEDIATE')
        current=c.execute('SELECT * FROM jobs WHERE id=?',(row['id'],)).fetchone()
        if (current is None or current['attempt']!=row['attempt'] or
                current['state'] not in ('running','preparing','ready','processing')):
            return False
        if state=='complete' and current['state']!='processing':return False
        if current['cancel']:
            state,result='cancelled',{'execution':(result or {}).get('execution',{}),
                                      'process_group_returned':True,'reason':'cancelled'}
        if state == 'complete' and registry.mode(c) != 'active':
            return
        if state == 'complete':
            registry.index(c,row,result,config,BULK,relative,prepared=prepared)
        c.execute('UPDATE jobs SET state=?,result=? WHERE id=?',(state,dump(result) if result else None,row['id']))
        if row['attempt'] and result:
            c.execute('INSERT OR REPLACE INTO attempt_results VALUES(?,?,?)',(row['id'],row['attempt'],dump(result)))
        if (result or {}).get('process_group_returned'):
            c.execute("UPDATE stage_runs SET state=?,ended=COALESCE(ended,?) WHERE job=? AND attempt=? AND state='running'",(state,time.time(),row['id'],row['attempt']))
        c.execute("UPDATE attempts SET state=?,ended=? WHERE id=? AND state='running'",(state,time.time(),row['attempt']))
        c.execute('DELETE FROM result_outbox WHERE job=? AND attempt=?',(row['id'],row['attempt']))
        event(c,row,state)
        hooks.enqueue(c,{**dict(row),'result':dump(result) if result else None},state)
        spec=json.loads(row['spec'])
        policy=execution.policy(spec,spec.get('profile',{'timeout':30,'budget':{'attempts':2}}))
        reason=(result or {}).get('reason','exit_nonzero' if state=='failed' else state)
        attempts=c.execute('SELECT count(*) FROM attempts WHERE job=?',(row['id'],)).fetchone()[0]
        # Explicit automatic retries remain opt-in and consume the same recovery budget.
        existing_recovery=recovery.view(c,row['id'])
        automatic=(not existing_recovery or existing_recovery['rounds']<3) and (state in ('failed','interrupted') and reason in policy['retry_on'] and attempts<min(4,policy['max_attempts'])
                   and (state!='interrupted' or (result or {}).get('process_group_returned',False)))
        if automatic:
            recovery.open_incident(c,row,reason)
            c.execute("UPDATE recovery SET state='pending',deadline=NULL WHERE job=?",(row['id'],))
            c.execute("UPDATE jobs SET state='queued',result=NULL,available_at=? WHERE id=?",(time.time()+policy['retry_backoff_seconds'],row['id']))
            event(c,row,'retry_scheduled')
        elif state in ('failed','interrupted'):
            recovery.open_incident(c,row,reason)
        else:
            recovery.close(c,row['id'],state)


def wait_process_group(child, grace):
    if isinstance(child,runtime_client.Container):return child.poll() is not None
    """Wait for all descendants in the worker-owned session to exit."""
    deadline = time.monotonic() + grace
    while True:
        try:
            os.killpg(child.pid, 0)
        except ProcessLookupError:
            return True
        if time.monotonic() >= deadline:
            return False
        time.sleep(.1)


UnreturnedProcessGroup = runtime_client.Unreturned


def stop_process_group(child):
    if isinstance(child,runtime_client.Container):return child.stop()
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        child.wait(12)
    except subprocess.TimeoutExpired:
        pass
    if not wait_process_group(child, 5):
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(5)
        if not wait_process_group(child, 5):
            raise UnreturnedProcessGroup('Owned process group did not return')
        return 'forced'
    return 'cooperative'


def output_size(folder):
    total=0
    for path in folder.rglob('*'):
        try:
            if path.is_file() and not path.is_symlink():
                total+=path.stat().st_size
        except FileNotFoundError:
            continue
    return total


def mark_returned(c,row,receipt):
    current=c.execute('SELECT state,attempt FROM jobs WHERE id=?',(row['id'],)).fetchone()
    if current is None or current['attempt']!=row['attempt'] or current['state'] not in ('running','preparing'):
        return False
    c.execute("UPDATE attempts SET state='complete',ended=? WHERE id=?",(receipt['ended'],row['attempt']))
    c.execute("UPDATE stage_runs SET state='returned',ended=COALESCE(ended,?) WHERE job=? AND attempt=? AND stage='run'",(receipt['ended'],row['id'],row['attempt']))
    c.execute("UPDATE jobs SET state='processing',result=? WHERE id=?",
              (dump({'execution':receipt['execution']}),row['id']))
    c.execute('INSERT OR IGNORE INTO result_outbox(job,attempt) VALUES(?,?)',(row['id'],row['attempt']))


def returned_receipt(row):
    folder=LOCAL/'returns'
    receipt={'job':row['id'],'attempt':row['attempt'],'ended':time.time(),
             'execution':{'exit_code':0,'process_group_returned':True}}
    with (folder/(row['attempt']+'.tmp')).open('x') as stream:
        stream.write(dump(receipt))
        stream.flush()
        os.fsync(stream.fileno())
    (folder/(row['attempt']+'.tmp')).replace(folder/(row['attempt']+'.json'))
    fd=os.open(folder,os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    return receipt



def execute(row, config, stage="run"):
    spec = json.loads(row['spec'])
    try:
        folder = storage.folder(config,BULK,row,relative)
        guard(config)
        with db() as c:config=component_store.merge(c,config)
        generic=execution.is_generic(spec.get('profile',{}))
        folder.mkdir(parents=True,exist_ok=generic and row.get('state') in ('ready','blocked'))
        environment=None
        if spec['kind'] == 'cpu.smoke':
            command = [sys.executable, __file__, 'cpu', str(folder), dump(spec['params'])]
            timeout = 30
        elif generic:
            profile=spec['profile']
            if config.get('profiles',{}).get(spec['kind'],{}).get('sha256')!=profile['sha256']:raise ValueError('Installed runtime differs from accepted Job')
            execution.validate_profile(profile)
            with db() as c:
                request=execution.request_file(folder,spec,config,BULK,relative,file_sha,
                    lambda job:c.execute('SELECT * FROM jobs WHERE id=?',(job,)).fetchone(),stage=stage)
            command,environment=execution.command(profile,stage,request)
            timeout=execution.policy(spec,profile)['timeout_seconds']
            if stage=='prepare':timeout=min(timeout,profile['prepare'].get('timeout',timeout))
        else:
            raise ValueError('Unsupported frozen profile; register a current execution profile')
        if 'execution' in spec and not generic:timeout=execution.policy(spec,spec.get('profile',{'timeout':30,'budget':{'attempts':2}}))['timeout_seconds']
        runner=spec.get('profile',{}).get('prepare' if stage=='prepare' else 'runtime',{})
        with (folder/('prepare.log' if stage=='prepare' else 'worker.log')).open('ab') as log:
            child = runtime_client.Container(config,row,spec,stage,request,folder,BULK) if runner.get('executor')=='container' else subprocess.Popen(command,stdout=log,stderr=log,start_new_session=True,env=environment)
            with child:
                try:
                    with db() as c:c.execute("INSERT OR REPLACE INTO stage_runs VALUES(?,?,?,?,NULL,'running')",(row['id'],row['attempt'],stage,time.time()))
                    monitor_child(row,config,spec,folder,stage,child,log,timeout,generic)
                except UnreturnedProcessGroup:
                    raise
                except Exception:
                    stop_process_group(child)
                    raise
            with db() as c:c.execute("UPDATE stage_runs SET ended=COALESCE(ended,?),state=? WHERE job=? AND attempt=? AND stage=?",(time.time(),'returned' if child.returncode==0 else 'failed',row['id'],row['attempt'],stage))
            return
    except runtime_client.Busy:
        with db() as c:c.execute("UPDATE jobs SET state='ready',available_at=? WHERE id=?",(time.time()+2,row['id']))
    except UnreturnedProcessGroup:
        raise
    except (OSError,ValueError,KeyError,TypeError,RuntimeError,subprocess.SubprocessError) as exc:
        finish(row,'failed',{'reason':'launch_failed','error_type':type(exc).__name__,'detail':str(exc)[:500]})


def monitor_child(row,config,spec,folder,stage,child,log,timeout,generic):
    try:
        deadline = time.monotonic() + timeout
        output_cap = spec.get('profile',{}).get('limits' if generic else 'budget',{}).get('max_output_bytes')
        next_size_check = 0
        while child.poll() is None:
            with db() as c:
                cancel = c.execute('SELECT cancel FROM jobs WHERE id=?',(row['id'],)).fetchone()[0]
            now=time.monotonic()
            over_budget=False
            if output_cap and now>=next_size_check:
                over_budget=output_size(folder)>output_cap
                next_size_check=now+1
            if STOP or cancel or now >= deadline or over_budget:
                disposition = stop_process_group(child)
                finish(row, 'cancelled' if cancel else 'interrupted' if STOP else 'failed',
                       {'cleanup':disposition,'process_group_returned':True,'reason':'cancelled' if cancel else 'interrupted' if STOP else 'output_budget' if over_budget else 'timeout','stage':stage})
                return
            time.sleep(.1)
        if not wait_process_group(child, 3):
            disposition = stop_process_group(child)
            finish(row,'failed',{'cleanup':disposition,'process_group_returned':True,
                                 'exit_code':child.returncode})
            return
        if output_cap and output_size(folder)>output_cap:
            finish(row,'failed',{'reason':'output_budget','process_group_returned':True})
            return
        if isinstance(child,runtime_client.Container):log.write(child.logs().encode());log.flush()
        if child.returncode:
            finish(row,'failed',{'exit_code':child.returncode,'process_group_returned':True,'reason':'exit_nonzero','stage':stage})
            return
        if stage=='prepare':
            with db() as c:
                if c.execute('SELECT cancel FROM jobs WHERE id=?',(row['id'],)).fetchone()[0]:
                    finish(row,'cancelled',{'process_group_returned':True})
                else:
                    c.execute("UPDATE stage_runs SET state='returned',ended=? WHERE job=? AND attempt=? AND stage='prepare'",(time.time(),row['id'],row['attempt']))
                    c.execute("UPDATE jobs SET state='ready' WHERE id=?",(row['id'],))
            return
        receipt=returned_receipt(row)
        with db() as c:
            mark_returned(c,row,receipt)
    except UnreturnedProcessGroup:
        # Exit this worker before another Work can be claimed. Docker tears
        # down its process namespace; a new worker records the interruption.
        raise
    except (OSError,ValueError,KeyError,TypeError,RuntimeError,subprocess.SubprocessError) as exc:
        disposition=stop_process_group(child)
        finish(row,'failed',{'reason':'launch_failed','error_type':type(exc).__name__,
                            'cleanup':disposition,'process_group_returned':True})


def process_result(row, config):
    try:
        if row['cancel']:
            finish(row,'cancelled',{'execution':{'exit_code':0,'process_group_returned':True}})
            return
        guard(config)
        folder = storage.folder(config,BULK,row,relative)
        spec=json.loads(row['spec'])
        if spec.get('profile',{}).get('finalize'):
            with db() as c:
                rec=recovery.view(c,row['id'])
                finalized=c.execute('SELECT 1 FROM finalizer_receipts WHERE job=? AND attempt=?',(row['id'],row['attempt'])).fetchone()
                published=c.execute("SELECT 1 FROM entity_revisions WHERE job=? AND json_extract(manifest,'$.attempt')=? LIMIT 1",(row['id'],row['attempt'])).fetchone()
            if not finalized and not published:
                run_hook(row,config,'finalize',row.get('retries',0)+100*(rec['rounds'] if rec else 0),required=True)
        with (folder/'result.json').open('rb') as stream:
            os.fsync(stream.fileno())
        facts = {}
        result = artifact(config,folder,read(folder/'result.json'),json.loads(row['spec']).get('profile'),facts,storage.output(config,BULK,spec))
        result['execution'] = {'exit_code':0,'process_group_returned':True}
        if (folder/'container-ticket.json').exists():result['runtime']=read(folder/'container-ticket.json')
        prepared = registry.prepare(result,BULK,relative,facts=facts,config=config)
        if spec.get('profile',{}).get('finalize'):
            # Once finalization produced verified outputs, indexing retries must
            # not execute it again. Old published Attempts are equally conclusive.
            with db() as c:c.execute('INSERT OR IGNORE INTO finalizer_receipts VALUES(?,?,?)',(row['id'],row['attempt'],time.time()))
        fd = os.open(folder,os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
        result['artifact_ready'] = True
        with db() as c:
            c.execute('BEGIN IMMEDIATE')
            current=c.execute('SELECT attempt,state,cancel FROM jobs WHERE id=?',(row['id'],)).fetchone()
            if current is None or current['attempt']!=row['attempt'] or current['state']!='processing':return
            if not current['cancel']:
                registry.index(c,row,result,config,BULK,relative,prepared=prepared,validate_only=True)
                c.execute("UPDATE jobs SET result=? WHERE id=? AND attempt=? AND state='processing' AND cancel=0",
                          (dump(result),row['id'],row['attempt']))
        finish(row,'complete',result,config,prepared)
    except UnreturnedProcessGroup:
        raise
    except (OSError,ValueError,KeyError,TypeError,RuntimeError,sqlite3.Error) as exc:
        with db() as c:
            c.execute('BEGIN IMMEDIATE')
            current=c.execute('SELECT * FROM jobs WHERE id=?',(row['id'],)).fetchone()
            if current is None or current['attempt']!=row['attempt'] or current['state']!='processing':return
            if current['cancel']:
                c.execute("UPDATE jobs SET state='cancelled',result=? WHERE id=?",(dump({'reason':'cancelled','execution':{'exit_code':0,'process_group_returned':True}}),row['id']))
                c.execute('DELETE FROM result_outbox WHERE job=? AND attempt=?',(row['id'],row['attempt']))
                event(c,current,'cancelled');hooks.enqueue(c,dict(current),'cancelled');recovery.close(c,row['id'],'cancelled')
                return
            # A failed revalidation must not publish a new readiness claim.
            # The last immutable Registry revision remains independently readable.
            stored=json.loads(current['result'] or '{}');stored.pop('artifact_ready',None)
            c.execute('UPDATE jobs SET result=? WHERE id=?',(dump(stored),row['id']))
            c.execute('UPDATE result_outbox SET retries=retries+1,next_try=? WHERE job=?',
                      (time.time()+min(300,2**min(8,row['retries'])),row['id']))
            if row['retries'] == 0:
                event(c,row,'result_processing_failed')
            if row['retries'] >= 2:
                c.execute("UPDATE jobs SET state='result_failed' WHERE id=?",(row['id'],))
                event(c,row,'result_retry_required')
                recovery.open_incident(c,row,'finalization_failed')
        print(f"Result processing {row['id']}: {type(exc).__name__}",file=sys.stderr,flush=True)


def run_hook(row,config,stage,retry=0,required=False):
    spec=json.loads(row['spec']);profile=spec['profile'];runner=profile[stage]
    if runner.get('lane','cpu')!='cpu':raise ValueError('Lifecycle hooks use CPU resources')
    folder=storage.folder(config,BULK,row,relative);folder.mkdir(parents=True,exist_ok=True)
    if stage.startswith('on_'):
        folder=folder/'callbacks'/stage/str(retry);folder.mkdir(parents=True,exist_ok=True)
    request=folder/(stage+'-context.json')
    request.write_text(dump({'schema':'loom-run-request/v1','job_id':row['id'],'attempt_id':row['attempt'],
        'stage':stage,'event_id':row['id']+'.'+row['attempt']+'.'+stage,'params':spec['params'],
        'output_dir':str(folder),'result':json.loads(row['result']) if isinstance(row.get('result'),str) else row.get('result'),
        'profile':{'revision':profile['revision'],'sha256':profile['sha256']}}))
    command,env=execution.command(profile,stage,request)
    with (folder/(stage+'.log')).open('ab') as log:
        # A bounded retry uses a new ticket but the same callback idempotency key.
        hook_spec={**spec,'profile':{**profile,'_hook_retry':retry}}
        child=runtime_client.Container(config,row,hook_spec,stage,request,folder,BULK) if runner.get('executor')=='container' else subprocess.Popen(command,stdout=log,stderr=log,start_new_session=True,env=env)
        with child:
            deadline=time.monotonic()+runner.get('timeout',60)
            while child.poll() is None:
                if STOP or time.monotonic()>=deadline:stop_process_group(child);raise RuntimeError('Hook interrupted or timed out')
                time.sleep(.1)
            if not wait_process_group(child,3):stop_process_group(child);raise RuntimeError('Hook descendants did not return')
            if isinstance(child,runtime_client.Container):log.write(child.logs().encode())
            if child.returncode:raise RuntimeError('Hook exited '+str(child.returncode))


def process_hook(item,config):
    row={**dict(item),'id':item['job']}
    try:
        run_hook(row,config,item['stage'],item['retries'])
        with db() as c:c.execute("UPDATE hooks SET state='complete',error=NULL WHERE job=? AND attempt=? AND stage=?",(item['job'],item['attempt'],item['stage']))
    except UnreturnedProcessGroup:
        raise
    except Exception as exc:
        diagnostics.record('worker','hook',exc,code='hook_failed')
        with db() as c:
            c.execute("UPDATE hooks SET retries=retries+1,state=?,next_try=?,error=? WHERE job=? AND attempt=? AND stage=?",('failed' if item['retries']>=2 else 'pending',time.time()+2**item['retries'],str(exc)[:500],item['job'],item['attempt'],item['stage']))
            event(c,row,'hook_failed_'+item['stage'])


def execution_lane(spec):
    return queue_control.execution_lane(spec)


def worker(config):
    global STOP
    def stop(*_):
        global STOP
        STOP = True
    signal.signal(signal.SIGTERM,stop)
    signal.signal(signal.SIGINT,stop)
    with (LOCAL/'worker.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
        grant=WorkerGrant(config['remote_fence']) if config.get('remote_fence') else None
        if grant:
            grant.request('claim')
        (LOCAL/'returns').mkdir(exist_ok=True)
        lanes = ThreadPoolExecutor(max_workers=3)
        jobs = {'cpu':None,'gpu':None,'result':None}
        try:
            if config.get('executor'):
                # An uncertain hook/finalizer must return before any retry or
                # unrelated CPU work. Pending records retain bounded retry IDs.
                with db() as c:
                    pending=c.execute("SELECT id AS job,attempt FROM jobs WHERE state='processing' UNION SELECT job,attempt FROM hooks WHERE state='pending'").fetchall()
                for item in pending:
                    runtime_client.call(config['executor'],'recover',dict(item))
            with db() as c:
                rows=c.execute("SELECT * FROM jobs WHERE state IN ('running','preparing')").fetchall()
            for row in rows:
                try:folder=storage.folder(config,BULK,row,relative)
                except (OSError,ValueError):
                    if config.get('executor'):
                        runtime_client.call(config['executor'],'recover',{'job':row['id'],'attempt':row['attempt']})
                    finish(row,'failed',{'reason':'storage_unavailable','process_group_returned':True})
                    continue
                ticket_path=folder/'container-ticket.json'
                runner='prepare' if row['state']=='preparing' else 'runtime'
                if config.get('executor') and json.loads(row['spec']).get('profile',{}).get(runner,{}).get('executor')=='container':
                    recovered=runtime_client.call(config['executor'],'recover',{'job':row['id'],'attempt':row['attempt']})
                    for entry in recovered:
                        if entry['stage']==('prepare' if row['state']=='preparing' else 'run'):
                            folder.mkdir(parents=True,exist_ok=True);ticket_path.write_text(dump(entry))
                            if not entry['natural_return']:entry['exit_code']=None

                if ticket_path.exists() and config.get('executor'):
                    ticket=read(ticket_path)
                    state=runtime_client.call(config['executor'],'status',{'ticket':ticket['ticket']})
                    if not state['returned']:
                        state=runtime_client.call(config['executor'],'cancel',{'ticket':ticket['ticket']})
                        if not state['returned']:raise UnreturnedProcessGroup('Previous container still owns resources')
                    elif state.get('exit_code')==0 and ticket.get('natural_return',True):
                        if ticket.get('natural_return',True) and ticket['stage']=='prepare':
                            with db() as c:
                                c.execute("UPDATE stage_runs SET state='returned' WHERE job=? AND attempt=? AND stage='prepare'",(row['id'],row['attempt']))
                                c.execute("UPDATE jobs SET state='ready' WHERE id=?",(row['id'],))
                            continue
                        receipt=returned_receipt(row)
                        with db() as c:mark_returned(c,row,receipt)
                        continue
                marker=LOCAL/'returns'/(row['attempt']+'.json')
                try:
                    receipt=read(marker)
                except (OSError,ValueError):
                    receipt={}
                if (isinstance(receipt,dict) and receipt.get('job')==row['id']
                        and receipt.get('attempt')==row['attempt']
                        and type(receipt.get('ended')) in (int,float)
                        and receipt.get('execution')=={'exit_code':0,'process_group_returned':True}):
                    with db() as c:
                        mark_returned(c,row,receipt)
                else:
                    finish(row,'interrupted')
            while not STOP:
                for lane, task in jobs.items():
                    if task is not None and task.done():
                        task.result()
                        jobs[lane] = None
                if jobs['result'] is None:
                    with db() as c:
                        pending=c.execute("""SELECT j.*,o.retries FROM result_outbox o
                            JOIN jobs j ON j.id=o.job AND j.attempt=o.attempt
                            WHERE o.next_try<=? AND j.state='processing' AND
                                  (SELECT mode FROM node_control WHERE id=1)='active'
                            ORDER BY o.next_try,j.created LIMIT 1""",(time.time(),)).fetchone()
                    if pending:
                        with db() as c:recovery.started(c,pending['id'])
                        jobs['result']=lanes.submit(process_result,dict(pending),config)
                    else:
                        with db() as c:callback=hooks.pending(c)
                        if callback:jobs['result']=lanes.submit(process_hook,dict(callback),config)
                if jobs['cpu'] is not None and jobs['gpu'] is not None:
                    time.sleep(.1)
                    continue
                with db() as c:
                    c.execute('BEGIN IMMEDIATE')
                    recovery.expire(c)
                    workflows.refresh(c)
                    row = None
                    if registry.mode(c)=='active':
                        for candidate in c.execute("SELECT * FROM jobs WHERE state IN ('queued','ready','blocked') AND cancel=0 AND available_at<=? ORDER BY COALESCE(queue_order,created),id",(time.time(),)):
                            spec=json.loads(candidate['spec'])
                            decision=queue_control.scheduling(c,candidate,execution_lane)
                            if not decision['eligible']:
                                if decision.get('dependency_failed'):
                                    c.execute("UPDATE jobs SET state='blocked' WHERE id=?",(candidate['id'],))
                                    if candidate['state']!='blocked':event(c,candidate,'dependency_failed')
                                continue
                            stage,prepared=queue_control.stage_for(c,candidate)
                            lane=decision['lane']
                            if jobs[lane] is not None:continue
                            row=candidate
                            break
                    if row:
                        row = dict(row)
                        recovery.started(c,row['id'])
                        if row['state']!='ready' and not (row['state']=='blocked' and prepared):
                            row['attempt'] = uuid.uuid4().hex
                            c.execute("INSERT INTO attempts(id,job,state,started) VALUES(?,?,'running',?)",(row['attempt'],row['id'],time.time()))
                        c.execute("UPDATE jobs SET state=?,attempt=? WHERE id=?",('preparing' if stage=='prepare' else 'running',row['attempt'],row['id']))
                if row:
                    jobs[lane]=lanes.submit(execute,row,config,stage)
                else:
                    time.sleep(.2)
        finally:
            STOP = True
            lanes.shutdown()
            if grant:
                with db() as c:
                    running=c.execute("SELECT 1 FROM jobs WHERE state IN ('preparing','running','processing') UNION SELECT 1 FROM hooks WHERE state='pending' LIMIT 1").fetchone()
                if not running:
                    grant.request('release')


def cpu(folder, params):
    time.sleep(params['seconds'])
    if params['fail']:
        return 1
    (folder/'observation.json').write_text('{"synthetic":true}\n')
    (folder/'result.json').write_text(dump({'entities':[{'id':'observation','kind':'artifact','path':'observation.json'}],
        'metrics':[{'name':'observations','entity_id':'observation','value':1}]}))
    return 0

TOOLS = {
    'submit_workflow': ('submit', {}),
    'curate_research': ('submit', {}),
    'curate_runtime_item': ('submit', {}),
    'inspect_execution_profile': ('read', {}),
    'compare_execution_profiles': ('read', {}),
    'annotate_software': ('submit', {}),
    'runtime_list': ('read', {}),
    'runtime_get': ('read', {'id':'string','revision':'integer'}),
    'list_users': ('read', {}),
    'assign_job_owner': ('submit', {'id':'string','owner':'string'}),
    'assign_entity_owner': ('submit', {'id':'string','owner':'string'}),
    'assign_extension_owner': ('submit', {'kind':'string','namespace':'string','id':'string','owner':'string'}),
    'recover_job': ('submit', {'id':'string','action':'string'}),
    'submit_job': ('submit', {'key':'string','kind':'string','owner':'string','notify':'string','params':'object'}),
    'docker_status': ('read', {}),
    'list_history': ('read', {'kind':'string','offset':'integer','limit':'integer'}),
    'get_history': ('read', {'source':'string','kind':'string','id':'string','revision':'string'}),
    'get_job_logs': ('read', {'id':'string','attempt':'string','offset':'integer','limit':'integer'}),
    'reindex_job': ('submit', {'id':'string','attempt':'string'}),
    'annotate_job': ('submit', {'id':'string','annotation_id':'string','reason':'string','evidence':'object'}),
    'reconnect_location': ('submit', {'id':'string','revision':'integer','component':'string','ref':'string'}),
    'get_job': ('read', {'id':'string'}),
    'cancel_job': ('cancel', {'id':'string'}),
    'queue_view': ('read', {}),
    'reorder_job': ('submit', {'id':'string','position':'string'}),
    'hold_job': ('submit', {'id':'string'}),
    'resume_job': ('submit', {'id':'string'}),
    'list_events': ('events', {}),
    'ack_event': ('events', {'seq':'integer'}),
    'system_status': ('read', {}),
    'ssh_connection': ('read', {}),
    'release_status': ('read', {}),
    'check_releases': ('read', {}),
    'record_delivery': ('events', {'event':'integer','state':'string','receipt':'string','version':'integer'}),
    'list_jobs': ('read', {}),
    'get_job_contract': ('read', {}),
    'list_execution_profiles': ('read', {}),
    'get_principal': ('read', {}),
    'list_registry': ('read', {}),
    'get_entity': ('read', {'id':'string','revision':'integer'}),
}


OPTIONAL_FIELDS = {
    'list_registry':{'view':{'enum':['research','all','operations'],'default':'research'}},
    'reconnect_location':{'storage':storage.SELECTOR},
    'reorder_job': {'reference':{'type':'string'},'reason':{'type':'string','maxLength':1000}},
    'hold_job': {'reason':{'type':'string','maxLength':1000}},
    'resume_job': {'reason':{'type':'string','maxLength':1000}},
    'recover_job':{'lease':{'type':'string'},'patch':{'type':'object'},'reason':{'type':'string'}},
    'list_events': {'kind':{'type':'string','enum':['errors','lifecycle','all'],'default':'errors'},'after':{'type':'integer','minimum':0}},
    'submit_job': {'presentation':research.PRESENTATION,'consolidates':job_work.CONSOLIDATES_SCHEMA,'output_storage':storage.SELECTOR,'queue_position':{'type':'string','enum':['tail','next'],'default':'tail'},'execution':execution.POLICY_SCHEMA,'depends_on': {'type':'array','items':{'type':'string'}}, **job_contract.FIELDS},
    'list_jobs': {'view':{'enum':['research','all','operations'],'default':'research'},'include_consolidated':{'type':'boolean','default':False},'category': job_contract.FIELDS['category'],
                  'execution_mode': {'type':'string','enum':list(job_contract.MODES)},
                  'offset': {'type':'integer','minimum':0,'default':0},
                  'limit': {'type':'integer','minimum':1,'maximum':1000,'default':250}},
}


def tool_schema(name):
    if name=='submit_workflow':return workflows.SCHEMA
    if name=='curate_research':return research.CURATE
    if name in ('curate_runtime_item','inspect_execution_profile','compare_execution_profiles'):
        from runtime_hub import SCHEMAS
        return SCHEMAS[name]
    fields = TOOLS[name][1]
    if name=='assign_extension_owner':
        return {'type':'object','properties':{'kind':{'enum':['component','tool','profile']},
            'namespace':{'type':'string','pattern':'^'+component_store.ID.pattern+'$'},
            'id':{'type':'string','pattern':'^'+component_store.ID.pattern+'$'},
            'owner':{'type':'string','description':'Registered responsible user ID or built-in for Loom-supplied software. Does not restrict sharing.'}},
            'required':list(fields),'additionalProperties':False}
    if name=='reorder_job':
        return {'type':'object','properties':{'id':{'type':'string'},'position':queue_control.POSITION_SCHEMA,**OPTIONAL_FIELDS[name]},'required':['id','position'],'additionalProperties':False}
    return {'type':'object', 'properties':{
        **{k:{'type':v} for k,v in fields.items()}, **OPTIONAL_FIELDS.get(name,{})},
        'required':list(fields), 'additionalProperties':False}


def operate(name, args, actor, config):
    if name in mcp_groups.GROUPS:
        if not isinstance(args,dict) or set(args)-{'action','arguments'}:raise ValueError('Invalid grouped tool request')
        action=args.get('action');payload=args.get('arguments',{})
        operation=mcp_groups.GROUPS[name]['actions'].get(action)
        if not operation or not isinstance(payload,dict):raise ValueError('Unknown action or invalid arguments')
        if operation=='read_guide':return onboarding.read(payload)
        if operation=='describe_action':
            import mcp_docs
            if not payload:return mcp_docs.reference(actor,TOOLS,tool_schema,job_contract.API_REVISION)
            if set(payload)=={'group'}:return mcp_docs.group_reference(payload['group'],actor,TOOLS,tool_schema,job_contract.API_REVISION)
            if set(payload)!={'group','action'}:raise ValueError('Provide group for its action index, group/action for one schema, or neither for all groups')
            return mcp_docs.describe(payload['group'],payload['action'],actor,TOOLS,tool_schema)
        if operation.startswith('profile_'):
            if ('submit' if action=='register_profile' else 'read') not in actor['scopes']:raise PermissionError('Profile scope denied')
            execution.validate(profile_store.action_schema(action),payload)
            with db() as c:return profile_store.operate(c,actor,{'action':action,**payload},config)
        if operation.startswith('tool_'):
            import tool_store
            if ('submit' if action=='register_tool' else 'read') not in actor['scopes']:raise PermissionError('Tool scope denied')
            execution.validate(tool_store.action_schema(action),payload)
            with db() as c:return tool_store.operate(c,actor,{'action':action,**payload},config)
        if operation.startswith('component_'):
            execution.validate(component_store.action_schema(action),payload)
            if ('submit' if action in ('register','write') else 'read') not in actor['scopes']:raise PermissionError('Component scope denied')
            with db() as c:return component_store.operate(c,actor,{'action':action,**payload},config)
        if name=='recovery':payload={**payload,'action':action}
        return operate(operation,payload,actor,config)
    if name=='annotate_software':
        import software_metadata
        with db() as c:return software_metadata.annotate(c,actor,args)
    if name in ('curate_runtime_item','inspect_execution_profile','compare_execution_profiles'):
        import runtime_hub
        if TOOLS[name][0] not in actor['scopes']:raise PermissionError('Scope denied')
        execution.validate(runtime_hub.SCHEMAS[name],args)
        with db() as c:
            return {'curate_runtime_item':runtime_hub.curate,'inspect_execution_profile':runtime_hub.inspect_profile,'compare_execution_profiles':runtime_hub.compare}[name](c,actor,args,config)
    if name in ('submit_workflow','curate_research'):
        if 'submit' not in actor['scopes']:raise PermissionError('Submit scope required')
        guard(config)
        with db() as c:
            if name=='submit_workflow':return workflows.submit(c,actor,args,config,submit)
            c.execute('BEGIN IMMEDIATE')
            if registry.mode(c)!='active':raise PermissionError('Job admission fenced')
            catalog,_=dashboard_read.projection(c,actor,config,LOCAL,bulk=BULK)
            return research.curate(c,actor,args,catalog['entities'])
    if name in ('get_job','cancel_job','hold_job','resume_job') and isinstance(args,dict) and set(args)<={'id','reason'} and args.get('id'):
        with db() as c:
            logical=workflows.get(c,args['id'])
            if logical:logical=ownership.decorate(c,'job',logical)
        if logical:
            execution.validate(tool_schema(name),args)
            required='read' if name=='get_job' else 'cancel' if name=='cancel_job' else 'submit'
            if required not in actor['scopes']:raise PermissionError('Scope denied')
            if name=='get_job':return logical
            results=[operate(name,{'id':node['job'],**({'reason':args['reason']} if 'reason' in args and name!='cancel_job' else {})},actor,config) for node in logical['executions'] if name!='cancel_job' or node['state'] not in ('complete','cancelled')]
            return {'id':logical['id'],'executions':results}
    if name in ('get_job_logs','reindex_job','recover_job','reorder_job') and isinstance(args,dict) and isinstance(args.get('id'),str):
        with db() as c:
            if workflows.exists(c,args['id']):
                if TOOLS[name][0] not in actor['scopes']:raise PermissionError('Scope denied')
                raise ValueError('Select an actual execution ID from jobs.get.executions')
    scope, fields = TOOLS[name]
    if scope not in actor['scopes']:
        raise PermissionError('Scope denied')
    if not isinstance(args,dict) or set(args)-set(OPTIONAL_FIELDS.get(name, {})) != set(fields):
        raise ValueError('Invalid arguments')
    if name == 'release_status':return release_updates.status()
    if name == 'check_releases':return release_updates.request()
    with db() as c:
        if name in ('runtime_list','runtime_get'):
            import runtime_catalog
            result=runtime_catalog.inventory(c,config)
            if name=='runtime_list':return result
            for item in result['runtimes']:
                if (item['id'],item['revision'])==(args['id'],args['revision']):
                    import runtime_client
                    exact=runtime_client.call(config['executor'],'resolve_runtime',{'runtime_ref':{'id':args['id'],'revision':args['revision']}})
                    return {**item,'manifest':exact['manifest'],'manifest_summary':False}
            raise ValueError('Runtime revision unavailable or outside instance grant')
        if name in {'cancel_job','reindex_job','annotate_job','reconnect_location','record_delivery','ack_event','recover_job'} and registry.mode(c) != 'active':
            raise PermissionError('Job admission fenced')
        if name == 'submit_job':
            guard(config)
            return submit(c,actor,args,config)
        if name == 'ssh_connection':
            from ssh_info import connection
            return connection(actor)
        if name == 'list_users':
            return [dict(r) for r in c.execute('SELECT id,provider,login,display_name FROM user_profiles ORDER BY display_name,id')]
        if name == 'assign_extension_owner':
            execution.validate(tool_schema(name),args)
            identity=args['namespace']+':'+args['id']
            if (args['kind'],identity) not in ownership.inventory(c):raise ValueError('Extension not found')
            component_store.mutation(c)
            changed=ownership.assign(c,args['kind'],identity,args['owner'],actor['id'])
            return {'kind':args['kind'],'id':args['id'],'changed':changed,
                    **ownership.extension(c,args['kind'],args['namespace'],args['id'])}
        if name in ('assign_job_owner','assign_entity_owner'):
            kind='job' if name=='assign_job_owner' else 'entity'
            if (kind,args['id']) not in ownership.inventory(c):raise ValueError('Item not found')
            c.execute('BEGIN IMMEDIATE')
            if registry.mode(c)!='active':raise PermissionError('Job admission fenced')
            ownership.assign(c,kind,args['id'],args['owner'],actor['id'])
            return ownership.decorate(c,kind,{'id':args['id']})
        if name == 'get_principal':
            return {**{key:actor[key] for key in ('id','scopes','owners','notify','identity','profile_id','profile','credential_id') if key in actor},'collaboration':'shared'}
        if name == 'get_job_contract':
            return {**job_contract.contract(), 'submit_schema':tool_schema('submit_job'),
                    'list_schema':tool_schema('list_jobs'),'status_values':['queued','preparing','ready','running','finalizing','completed','failed','finalizing_failed','cancelled'],
                    'ownership_policy':{'access':'shared','owner':'responsibility_only','profile_discovery':'system.users','original_provenance':'preserved'},
                    'queue_policy':{'default_position':'tail','placements':['next','tail','before','after'],'scope':'submit','target':'all_jobs','running_preemption':False,'hold_preserves_order':True,'reason_optional':True},
                    'recovery_policy':{'response_seconds':900,'idle_seconds':900,'round_seconds':1800,'max_rounds':3}}
        if name == 'list_execution_profiles':
            import profile_inventory
            combinations={p['kind']:p for p in profile_inventory.inventory(c,config,actor)}
            return [{**p,'usage':combinations[p['kind']]['usage'],'combination':combinations[p['kind']]} for p in profiles.describe(component_store.merge(c,config,actor), actor)]
        if name == 'list_jobs':
            if type(args.get('include_consolidated',False)) is not bool:raise ValueError('Invalid consolidation filter')
            links=job_work.completed_links(c)
            rows = []
            for r in c.execute('SELECT id,state,owner,submitter,created,attempt,spec,queue_order,queue_held FROM jobs'):
                if not recovery.permitted(actor,r):continue
                item = dict(r)
                item['status']={'complete':'completed','processing':'finalizing','result_failed':'finalizing_failed','blocked':'queued','interrupted':'failed'}.get(item['state'],item['state'])
                item['consolidated_into']=links.get(item['id'])
                if item['consolidated_into'] and not args.get('include_consolidated',False):continue
                item['recovery']=recovery.view(c,item['id'])
                spec=json.loads(item.pop('spec'))
                item.update(job_contract.values(spec))
                item['presentation']=research.job(c,{'id':item['id'],'spec':dump(spec)})
                item['presentation']['visible']=item['presentation']['scope']=='research' and item['presentation']['role']=='main' and not item['presentation'].get('parent_job')
                rows.append(ownership.decorate(c,'job',item))
            old=job_history.listing(c,actor,config)
            overlays=research.read(c)
            for item in old:
                item['presentation']={'scope':'research','role':'main',**overlays.get(('job',item['id']),{})}
                item['presentation']['visible']=item['presentation']['scope']=='research' and item['presentation']['role']=='main' and not item['presentation'].get('parent_job')
            logical=workflows.listing(c)
            for item in logical:
                item['presentation'].update(overlays.get(('job',item['id']),{}))
                item['presentation']['visible']=item['presentation'].get('scope')=='research' and item['presentation'].get('role')=='main' and not item['presentation'].get('parent_job')
            selected=research.view(rows+old+logical,args.get('view','research'))
            if args.get('include_consolidated'):
                visible={item['id'] for item in selected}
                selected.extend(item for item in rows if item['id'] not in visible and item.get('consolidated_into') in visible)
            return job_contract.select(selected,args)
        if name == 'queue_view':return queue_control.view(c,actor,execution_lane)
        if name == 'list_registry':
            native = registry.view(c,actor)
            records=native+history_store.catalog_results(c,(item['id'] for item in native))
            proxies=[{**e,'kind':e['kind'],'loom_registry':{'id':e['id']},'loom_job':{'id':e.get('job')}} for e in records]
            research.decorate(c,proxies)
            for record,proxy in zip(records,proxies):record['presentation']=proxy['presentation']
            return research.view(records,args.get('view','research'))
        if name == 'get_entity':
            if not isinstance(args['id'],str) or not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_.-]{0,255}',args['id']):raise ValueError('Invalid entity identity')
            return registry.view(c,actor,args['id'],args['revision'])
        if name in {'list_history','get_history'}:
            if name=='list_history':
                return history_store.list_records(c,**args)
            return history_store.get_record(c,args['source'],args['kind'],args['id'],args['revision'],config,BULK)
        if name == 'docker_status':
            def visible_job(job):
                return c.execute('SELECT 1 FROM jobs WHERE id=?',(job,)).fetchone() is not None
            return docker_status.read_status(docker_status.observation_path(LOCAL,'docker-status.json'),
                allow_other=True,
                allow_job=visible_job)
        if name == 'system_status':
            return {'machine_id':config['machine_id'],'storage_id':config['storage_id'],
                    'build':build_version.status(), 'onboarding':onboarding.identity(), 'updates':release_updates.status(),
                    'principal':{k:actor[k] for k in ('id','scopes','owners','notify','identity','profile_id','profile','credential_id') if k in actor},
                    'installation_id':config.get('installation_id'),'collaboration':'shared',
                    'authentication':{'tailscale':bool(config.get('tailscale_auth',{}).get('enabled')),
                                      'dashboard_sso':bool(config.get('dashboard_auth',{}).get('enabled'))},'api_revision':job_contract.API_REVISION,'job_contract_revision':job_contract.REVISION,'admission':registry.mode(c),'inventory':config['inventory'],'execution_contract_revision':execution.REVISION,'data_stores':{name:{**item,'path':'/loom/'+name} for name,item in storage.stores(config,BULK).items()},'runtime_ids':config.get('runtime_ids',[]),'runtime_catalog':{'discovery':{'group':'components','action':'runtimes'},'dependency_directory':config.get('dependency_directory'),'pinned_revisions':True},'gpu_execution_enabled':config.get('allow_gpu',False),
                    'profiles':{kind:p['revision'] for kind,p in config.get('profiles',{}).items()
}}
        if name == 'record_delivery':
            c.execute('BEGIN IMMEDIATE')
            return access.delivery(c,actor,**args)
        if name == 'list_events':
            kind=args.get('kind','errors');after=args.get('after',0)
            if kind not in ('errors','lifecycle','all') or type(after) is not int or after<0:raise ValueError('Invalid event filter')
            lifecycle=('complete','cancelled','retry_scheduled')
            rows=c.execute("SELECT e.*,COALESCE(d.state,'queued') AS delivery_state,COALESCE(d.version,0) AS delivery_version FROM events e LEFT JOIN delivery d ON d.event=e.seq WHERE e.target=? AND e.ack=0 AND e.seq>? ORDER BY e.seq",(actor['id'],after))
            result=[]
            for r in rows:
                item=dict(r);item['kind']='lifecycle' if item['code'] in lifecycle else 'error'
                item['schema']='loom-event/v2'
                if kind!='all' and (kind=='lifecycle')!=(item['kind']=='lifecycle'):continue
                result.append(item)
                if len(result)==100:break
            return result
        if name == 'ack_event':
            if type(args['seq']) is not int or c.execute('UPDATE events SET ack=1 WHERE seq=? AND target=?',(args['seq'],actor['id'])).rowcount != 1:
                raise PermissionError('Event outside grant')
            return {'acknowledged': True}
        if name == 'reconnect_location':
            return registry.reconnect(c,actor,config,BULK,relative,ident(args['id']),
                                      args['revision'],args['component'],args['ref'],args.get('storage','data/0'))
        if name not in {'get_job','get_job_logs'}:
            c.execute('BEGIN IMMEDIATE')
            if registry.mode(c) != 'active':
                raise PermissionError('Job admission fenced')
        if name=='get_job' and not c.in_transaction:c.execute('BEGIN')
        if name == 'get_job' and (not isinstance(args['id'],str) or not history_store.KEY.fullmatch(args['id'])):
            raise ValueError('Invalid Job identity')
        if name == 'get_job' and not c.execute('SELECT 1 FROM jobs WHERE id=?',(args['id'],)).fetchone():
            historical = job_history.get(c, args['id'], actor, config)
            if historical is not None:
                return historical
        row = c.execute('SELECT * FROM jobs WHERE id=?',(ident(args['id']),)).fetchone()
        if not row or not recovery.permitted(actor,row):
            raise PermissionError('Job outside grant')
        if name == 'reorder_job':return queue_control.reorder(c,row,actor,args)
        if name in ('hold_job','resume_job'):return queue_control.hold(c,row,actor,args,name=='hold_job')
        if name == 'recover_job':
            def validate_patch(frozen,patch):
                candidate={**frozen,**patch}
                if not isinstance(candidate.get('params'),dict):raise ValueError('Recovery params must be an object')
                dependencies=list(frozen.get('_requested_dependencies',frozen.get('depends_on',[])))
                candidate['_requested_dependencies']=list(dependencies)
                dependencies.extend(i for i in frozen.get('consolidates',[]) if i not in dependencies)
                merged=component_store.merge(c,config,actor)
                selected=merged.get('profiles',{}).get(candidate['kind'],{})
                for dependency in execution.input_dependencies(selected,candidate['params']):
                    if dependency not in dependencies:dependencies.append(dependency)
                for item in candidate['params'].values():
                    if isinstance(item,dict) and 'job' in item:
                        upstream=c.execute('SELECT owner,submitter FROM jobs WHERE id=?',(item['job'],)).fetchone()
                        if not upstream or not recovery.permitted(actor,upstream):raise PermissionError('Input belongs to another owner')
                    if isinstance(item,dict) and isinstance(item.get('ref'),str):
                        parts=Path(item['ref']).parts
                        if parts and parts[0]=='runs':
                            upstream=c.execute('SELECT owner,submitter FROM jobs WHERE id=?',(parts[1],)).fetchone() if len(parts)>1 else None
                            if not upstream or not recovery.permitted(actor,upstream):raise PermissionError('Input belongs to another owner')
                            attempt=c.execute('SELECT attempt FROM jobs WHERE id=?',(parts[1],)).fetchone()[0]
                            if len(parts)<4 or parts[2]!=attempt:raise ValueError('Input does not name the upstream Attempt')
                            if parts[1] not in dependencies:dependencies.append(parts[1])
                        elif not any(Path(item['ref']).is_relative_to(prefix) for prefixes in config.get('input_grants',{}).values() for prefix in prefixes):raise PermissionError('Input outside storage grant')
                if candidate['kind']=='cpu.smoke':
                    params=candidate['params']
                    if set(params)!={'seconds','fail'} or type(params['seconds']) is not int or not 0<=params['seconds']<=20 or type(params['fail']) is not bool:raise ValueError('Invalid smoke parameters')
                else:
                    selected=installed(merged,candidate['kind'],actor,candidate['params'],BULK,relative,ident,dependencies)
                    candidate['profile']=selected
                if len(dependencies)>64:raise ValueError('Too many recovery dependencies')
                visited=set();pending=list(dependencies)
                while pending:
                    dependency=ident(pending.pop())
                    if dependency==row['id']:raise ValueError('Recovery would create a dependency cycle')
                    if dependency in visited:continue
                    visited.add(dependency)
                    upstream=c.execute('SELECT spec FROM jobs WHERE id=?',(dependency,)).fetchone()
                    if not upstream:raise ValueError('Recovery dependency is missing')
                    pending.extend(json.loads(upstream['spec']).get('depends_on',[]))
                candidate['depends_on']=dependencies
                execution.policy(candidate,candidate.get('profile',{'timeout':30,'budget':{'attempts':2}}))
                return candidate
            return recovery.action(c,row,actor,args['action'],args,validate_patch)
        if name == 'get_job_logs':
            attempt=ident(args['attempt'])
            offset,limit=args['offset'],args['limit']
            if type(offset) is not int or offset<0 or type(limit) is not int or not 1<=limit<=65536:
                raise ValueError('Invalid log cursor or limit')
            if not c.execute('SELECT 1 FROM attempts WHERE job=? AND id=?',(row['id'],attempt)).fetchone():
                raise PermissionError('Attempt outside job')
            folder=storage.folder(config,BULK,row,relative,attempt)
            ticket=folder/'run-ticket.json'
            if ticket.exists() and config.get('executor'):
                return runtime_client.call(config['executor'],'log_range',{'ticket':read(ticket)['ticket'],'offset':offset,'limit':limit})
            path=folder/'worker.log'
            if not path.is_file():
                return {'available':False,'offset':offset,'next_offset':offset,'text':''}
            with path.open('rb') as stream:
                stream.seek(offset)
                data=stream.read(limit)
                return {'available':True,'offset':offset,'next_offset':stream.tell(),
                        'text':data.decode('utf-8','replace')}
        if name == 'reindex_job':
            if row['attempt']!=ident(args['attempt']):
                raise PermissionError('Result reindex outside execution owner or Attempt')
            if row['state'] not in {'complete','result_failed','processing'}:
                raise ValueError('Reindex requires a returned successful execution')
            pending_result=json.loads(row['result'] or '{}');pending_result.pop('artifact_ready',None)
            c.execute("UPDATE jobs SET state='processing',result=? WHERE id=?",(dump(pending_result),row['id']))
            c.execute("""INSERT INTO result_outbox(job,attempt) VALUES(?,?)
                ON CONFLICT(job) DO UPDATE SET retries=0,next_try=0""",(row['id'],row['attempt']))
            return {'id':row['id'],'attempt':row['attempt'],'state':'processing'}
        if name == 'annotate_job':
            annotation_id=ident(args['annotation_id'])
            if not isinstance(args['reason'],str) or not 0<len(args['reason'])<=500 or not isinstance(args['evidence'],dict):
                raise ValueError('Annotation needs a reason and structured evidence')
            payload=dump({'id':annotation_id,'author':actor['id'],'kind':'operator_note',
                          'reason':args['reason'],'evidence':args['evidence']})
            if len(payload)>8192:
                raise ValueError('Annotation exceeds size bound')
            old=c.execute('SELECT payload FROM job_annotations WHERE job=? AND id=?',(row['id'],annotation_id)).fetchone()
            if old and old[0]!=payload:
                raise ValueError('Annotation identity collision')
            c.execute('INSERT OR IGNORE INTO job_annotations VALUES(?,?,?)',(row['id'],annotation_id,payload))
            return json.loads(payload)
        if name == 'cancel_job' and row['state'] in ('queued','preparing','ready','running','processing','result_failed','blocked','failed','interrupted'):
            if not recovery.permitted(actor,row):
                raise PermissionError('Only delegated owners may cancel')
            c.execute("UPDATE jobs SET cancel=1,state=CASE WHEN state IN ('queued','ready','result_failed','blocked','failed','interrupted') THEN 'cancelled' ELSE state END WHERE id=?",(row['id'],))
            if row['state'] in ('queued','ready','result_failed','blocked','failed','interrupted'):
                event(c,row,'cancelled')
                c.execute("UPDATE attempts SET state='cancelled',ended=? WHERE id=? AND state='running'",(time.time(),row['attempt']))
                recovery.close(c,row['id'],'cancelled')
                if row['attempt']:hooks.enqueue(c,dict(row),'cancelled')
            if row['state']=='result_failed':
                c.execute('DELETE FROM result_outbox WHERE job=?',(row['id'],))
            else:
                c.execute('UPDATE result_outbox SET next_try=0 WHERE job=?',(row['id'],))
            row = c.execute('SELECT * FROM jobs WHERE id=?',(row['id'],)).fetchone()
        response = public(row)
        spec=json.loads(row['spec'])
        response['consolidation']={'sources':spec.get('_consolidation_sources',[]),'consolidated_into':job_work.completed_links(c).get(row['id'])}
        timeline=job_work.timeline(c,row,spec)
        if timeline is not None:response['work_timeline']=timeline
        if spec.get('profile',{}).get('steps'):
            try:
                folder=storage.folder(config,BULK,row,relative) if row['attempt'] else None
                response['internal_steps']=job_work.progress(folder,spec)
            except (OSError,ValueError):
                response['internal_steps']={**job_work.progress(None,spec),'status':'unavailable','reason':'Output storage is unavailable'}
            job_work.attach_timeline(response['internal_steps'],timeline)
        response['stages']=[dict(s) for s in c.execute('SELECT attempt,stage,started,ended,state FROM stage_runs WHERE job=? ORDER BY started',(row['id'],))]
        response['callbacks']=[dict(h) for h in c.execute('SELECT stage,state,retries,error FROM hooks WHERE job=?',(row['id'],))]
        response['recovery']=recovery.view(c,row['id'])
        response['scheduling']={**queue_control.scheduling(c,row,execution_lane),'position':queue_control.position(c,row['id'])}
        response['queue_history']=[dict(h) for h in c.execute('SELECT * FROM queue_history WHERE job=? ORDER BY seq',(row['id'],))]
        pending=c.execute('SELECT retries,next_try FROM result_outbox WHERE job=?',(row['id'],)).fetchone()
        if pending:
            response['result_processing']={'retries':pending['retries'],
                                           'next_try':pending['next_try'] if row['state']=='processing' else None}
        annotations=[json.loads(item[0]) for item in c.execute(
            'SELECT payload FROM job_annotations WHERE job=? ORDER BY id',(row['id'],))]
        if annotations:
            response['annotations']=annotations
        response['presentation']=research.job(c,row)
        response['attempts'] = [{**dict(item),'result':json.loads(item['result']) if item['result'] else None} for item in c.execute(
            'SELECT a.id,a.state,a.started,a.ended,r.result FROM attempts a LEFT JOIN attempt_results r ON r.job=a.job AND r.attempt=a.id WHERE a.job=? ORDER BY a.started,a.id',(row['id'],))]
        return ownership.decorate(c,'job',research.enrich(c,response))


MCP_VERSIONS = ('2025-03-26','2025-06-18','2025-11-25')


class API(BaseHTTPRequestHandler):
    def setup(self):
        self.request.settimeout(10)
        super().setup()

    def log_message(self,*_):
        pass

    def send(self,status,data=None,content_type='application/json',headers=None):
        raw = data if isinstance(data,bytes) else (dump(data) if content_type == 'application/json' else data).encode() if data is not None else b''
        self.send_response(status)
        self.send_header('Content-Type',content_type)
        self.send_header('Content-Length',str(len(raw)))
        self.send_header('Cache-Control','no-store')
        self.send_header('X-Loom-Revision',onboarding.identity()['revision'])
        for name,value in (headers or {}).items():
            self.send_header(name,value)
        self.end_headers()
        self.wfile.write(raw)

    def actor(self):
        actor=self.authenticate_actor()
        with db() as c:
            if actor.get("identity",{}).get("provider")=="tailscale":
                info=actor["identity"];person=ownership.register(c,info["login"],info.get("name"))
                actor={**actor,"profile_id":person["id"]}
            elif actor.get('identity',{}).get('provider')=='dashboard':
                person=ownership.register_identity(c,actor['identity'])
                actor={**actor,'profile_id':person['id']}
                if actor['id'] == actor['identity']['profile_id']:
                    actor={**actor,'id':person['id'],'owners':[person['id']],'notify':[person['id']]}
            if actor.get("profile_id"):
                actor={**actor,"profile":ownership.profile(c,actor["profile_id"])}
        return actor

    def authenticate_actor(self):
        if self.headers.get('Origin'):
            raise PermissionError('Cross-origin request denied')
        header = self.headers.get('Authorization','')
        if not header:
            dashboard_actor = access.dashboard_actor(self.server.config,self.headers)
            if dashboard_actor is not None:
                return dashboard_actor
            actor = access.tailscale_actor(self.server.config,self.headers,self.client_address[0])
            if actor is not None:
                return actor
        if not header and os.environ.get('LOOM_DEV_NO_AUTH') == '1':
            return next(actor for actor in self.server.config['actors'] if actor['id'] == 'owner')
        if not header.startswith('Bearer '):
            raise PermissionError('Bearer token required')
        token = header[7:]
        digest = hashlib.sha256(token.encode()).hexdigest()
        with db() as c:
            return access.authenticate(c,self.server.config['actors'],digest)

    def do_GET(self):
        if self.path == '/':
            self.send(200,{'service':'Loom Ops API','mcp':'/mcp','status':'/api/status'})
        elif self.path == '/mcp':
            self.send(405,{'error':'Stateless Streamable HTTP; use POST'},headers={'Allow':'POST'})
        else:
            self.handle_request()

    def do_POST(self):
        self.handle_request()

    def do_DELETE(self):
        self.send(405,{'error':'No persistent MCP session'},headers={'Allow':'POST'} if self.path=='/mcp' else None)

    def handle_request(self):
        try:
            actor = self.actor()
            if self.path == '/mcp' and self.command == 'POST':
                if self.headers.get_content_type() != 'application/json':
                    self.send(415,{'error':'Expected application/json'});return
                accepted=self.headers.get('Accept','*/*')
                if not any(kind in accepted for kind in ('application/json','text/event-stream','*/*')):
                    self.send(406,{'error':'Accept application/json or text/event-stream'});return
                version=self.headers.get('MCP-Protocol-Version')
                if version and version not in MCP_VERSIONS:
                    self.send(400,{'error':'Unsupported MCP protocol version','supported':MCP_VERSIONS});return
            if self.command == 'POST':
                length = int(self.headers.get('Content-Length','0'))
                if not 0 < length <= 65536:
                    raise ValueError('Invalid body size')
                body = json.loads(self.rfile.read(length))
            else:
                body = {}
            config = self.server.config
            if self.path in ('/api/admin/tokens','/api/admin/tokens/revoke'):
                with db() as c:
                    if self.path.endswith('/revoke') and self.command=='POST':
                        result=access.service_tokens.revoke(c,actor,body)
                    elif self.path=='/api/admin/tokens' and self.command=='GET':
                        result=access.service_tokens.listing(c,actor)
                    elif self.path=='/api/admin/tokens' and self.command=='POST':
                        result=access.service_tokens.issue(c,actor,body)
                    else:raise ValueError('Unsupported token action')
                self.send(200,result);return
            if self.path=='/api/actions' and self.command=='POST':
                self.send(200,operate(body['group'],{'action':body['action'],'arguments':body.get('arguments',{})},actor,config));return
            if self.path == '/mcp' and self.command == 'POST':
                self.mcp(body,actor,config)
                return
            if self.command=='GET' and (self.path=='/api/guide' or self.path.startswith('/api/guide/')):
                name=self.path.removeprefix('/api/guide').lstrip('/')
                if name=='bundle':
                    self.send(200,onboarding.bundle(actor,TOOLS,tool_schema,operate('get_job_contract',{},actor,config)),'application/zip');return
                result=onboarding.read({'id':name} if name else {})
            elif self.path == '/api/updates' and self.command == 'GET':
                result = operate('release_status',{},actor,config)
            elif self.path == '/api/updates/check' and self.command == 'POST':
                result = operate('check_releases',body,actor,config)
            elif self.path == '/api/ssh' and self.command == 'GET':
                result = operate('ssh_connection',{},actor,config)
            elif self.path == '/api/runtimes' and self.command == 'GET':
                result = operate('runtime_list',{},actor,config)
            elif self.path == '/api/status' and self.command == 'GET':
                result = operate('system_status',{},actor,config)
            elif self.command=='POST' and self.path in {
                '/api/jobs/logs','/api/jobs/reindex','/api/jobs/annotate','/api/registry/reconnect'}:
                operation={'/api/jobs/logs':'get_job_logs','/api/jobs/reindex':'reindex_job',
                           '/api/jobs/annotate':'annotate_job','/api/registry/reconnect':'reconnect_location'}[self.path]
                result = operate(operation,body,actor,config)
            elif self.path == '/api/history/list' and self.command == 'POST':
                result = operate('list_history',body,actor,config)
            elif self.path == '/api/history/get' and self.command == 'POST':
                result = operate('get_history',body,actor,config)
            elif self.command == 'GET' and (self.path in {'/api/dashboard/catalog', '/api/dashboard/revision',
                                                          '/api/dashboard/resources'}
                                             or self.path.startswith('/api/dashboard/media/')):
                if 'read' not in actor['scopes']:
                    raise PermissionError('Read grant required')
                if self.path == '/api/dashboard/resources':
                    guard(config)
                    with db() as c:
                        result = dashboard_read.resources(config,BULK,LOCAL,
                            connection=c,actor=actor,lane_for=execution_lane)
                else:
                    with db() as c:
                        c.execute('BEGIN')
                        catalog, media = dashboard_read.projection(c,actor,config,LOCAL,bulk=BULK)
                    if self.path.startswith('/api/dashboard/media/'):
                        self.dashboard_media(media,config)
                        return
                    result = catalog if self.path.endswith('/catalog') else {'revision':catalog['revision']}
            elif self.path == '/api/docker' and self.command == 'GET':
                result = operate('docker_status',{},actor,config)
            elif self.path == '/api/principal' and self.command == 'GET':
                result = operate('get_principal',{},actor,config)
            elif self.path == '/api/jobs/contract' and self.command == 'GET':
                result = operate('get_job_contract',{},actor,config)
            elif self.path == '/api/jobs/queue' and self.command == 'GET':
                result=operate('queue_view',{},actor,config)
            elif self.path == '/api/jobs/profiles' and self.command == 'GET':
                result = operate('list_execution_profiles',{},actor,config)
            elif urlsplit(self.path).path == '/api/jobs' and self.command == 'GET':
                query = parse_qs(urlsplit(self.path).query, keep_blank_values=True)
                if any(len(v) != 1 for v in query.values()):
                    raise ValueError('Repeated Job filter')
                args = {k:int(v[0]) if k in {'offset','limit'} else v[0] for k,v in query.items()}
                if 'include_consolidated' in args:
                    if args['include_consolidated'] not in ('true','false'):raise ValueError('Invalid consolidation filter')
                    args['include_consolidated']=args['include_consolidated']=='true'
                result = operate('list_jobs',args,actor,config)
            elif urlsplit(self.path).path == '/api/registry' and self.command == 'GET':
                query=parse_qs(urlsplit(self.path).query,keep_blank_values=True)
                if any(len(v)!=1 for v in query.values()):raise ValueError('Repeated registry view')
                result=operate('list_registry',{k:v[0] for k,v in query.items()},actor,config)
            elif self.path.startswith('/api/registry/') and self.command == 'GET':
                result = operate('get_entity',{'id':self.path.removeprefix('/api/registry/'),'revision':1},actor,config)
            elif self.path == '/api/jobs/workflow' and self.command == 'POST':
                result=operate('submit_workflow',body,actor,config)
            elif self.path == '/api/research/curate' and self.command == 'POST':
                result=operate('curate_research',body,actor,config)
            elif self.path == '/api/jobs' and self.command == 'POST':
                result = operate('submit_job',body,actor,config)
            elif self.path.startswith('/api/jobs/') and self.command == 'GET':
                result = operate('get_job',{'id':unquote(self.path.removeprefix('/api/jobs/'))},actor,config)
            elif self.path == '/api/jobs/recovery' and self.command=='POST':
                result=operate('recover_job',body,actor,config)
            elif self.path == '/api/jobs/cancel' and self.command == 'POST':
                result = operate('cancel_job',body,actor,config)
            elif self.path == '/api/events/delivery' and self.command == 'POST':
                result = operate('record_delivery',body,actor,config)
            elif self.path == '/api/events/ack' and self.command == 'POST':
                result = operate('ack_event',body,actor,config)
            elif urlsplit(self.path).path == '/api/events' and self.command == 'GET':
                query=parse_qs(urlsplit(self.path).query,keep_blank_values=True)
                if any(len(v)!=1 for v in query.values()):raise ValueError('Repeated event filter')
                result=operate('list_events',{k:int(v[0]) if k=='after' else v[0] for k,v in query.items()},actor,config)
            else:
                self.send(404,{'error':'Unknown endpoint'})
                return
            self.send(200,result)
        except PermissionError as exc:
            diagnostics.record('ops','http',exc,code='permission_denied')
            self.send(403,{'error':str(exc)})
        except (ValueError,KeyError,TypeError) as exc:
            diagnostics.record('ops','http',exc,code='invalid_request')
            self.send(400,{'error':str(exc)})
        except (RuntimeError,OSError) as exc:
            incident=diagnostics.record('ops','http',exc,code='service_unavailable',build=build_version.status().get('build_id','unknown'))
            self.send(503,{'error':'Storage or service unavailable','incident_id':incident})
        except Exception as exc:
            incident=diagnostics.record('ops','http',exc,build=build_version.status().get('build_id','unknown'))
            self.send(500,{'error':'Internal service error','incident_id':incident})

    def dashboard_media(self,media,config):
        guard(config)
        try:
            descriptor,stream = dashboard_read.open_media(media,self.path.rsplit('/',1)[-1],BULK)
        except FileNotFoundError:
            self.send(404,{'error':'Registered media unavailable'})
            return
        if stream is None:
            self.send(410 if descriptor['status']=='intentionally_not_migrated' else 413 if descriptor['status']=='too_large' else 404,descriptor)
            return
        with stream:
            self.send_response(200)
            self.send_header('Content-Type',descriptor['mime'])
            self.send_header('Content-Length',str(descriptor['size']))
            self.send_header('Cache-Control','no-store')
            self.send_header('X-Content-Type-Options','nosniff')
            self.send_header('Content-Security-Policy',"default-src 'none'; sandbox")
            self.end_headers()
            try:
                while chunk := stream.read(65536):
                    self.wfile.write(chunk)
            except (BrokenPipeError,ConnectionResetError):
                pass

    def mcp(self,body,actor,config):
        if not isinstance(body,dict):
            self.send(400,{'jsonrpc':'2.0','id':None,'error':{'code':-32600,'message':'Expected a JSON-RPC object'}})
            return
        method = body.get('method')
        rpc_id = body.get('id')
        if body.get('jsonrpc') != '2.0' or not isinstance(method,str):
            self.send(400,{'jsonrpc':'2.0','id':rpc_id,'error':{'code':-32600,'message':'Invalid JSON-RPC request'}});return
        if 'id' not in body:
            self.send(202)
            return
        def reply(payload):
            accepted=self.headers.get('Accept','application/json')
            if 'text/event-stream' in accepted and 'application/json' not in accepted and '*/*' not in accepted:
                self.send(200,'event: message\ndata: '+dump(payload)+'\n\n','text/event-stream')
            else:
                self.send(200,payload)
        try:
            if method == 'initialize':
                requested=(body.get('params') or {}).get('protocolVersion')
                result = {'protocolVersion':requested if requested in MCP_VERSIONS else MCP_VERSIONS[-1],'capabilities':{'tools':{}},
                          'serverInfo':{'name':'loom-ops','title':'Loom Use','version':build_version.status().get('build_id') or job_contract.API_REVISION},
                          'instructions':'At first connection call system.status and system.guide. Each tools/call response ends with a short Loom version line. Compare it with the previous response; only when contract or guide changes, refresh the affected tools/schemas or guides. A build-only change does not require rereading them. Keep existing Job pins. The first content block remains the operation result.'}
            elif method == 'ping':
                result = {}
            elif method == 'tools/list':
                result = {'tools':mcp_groups.tools(actor,TOOLS)}
            elif method == 'tools/call':
                call = body['params']
                if not isinstance(call,dict) or not isinstance(call.get('name'),str):
                    raise ValueError('Expected tool name and arguments object')
                if call['name'] not in mcp_groups.GROUPS:raise ValueError('Unknown MCP tool; use the advertised tool groups')
                value = operate(call['name'],call.get('arguments',{}),actor,config)
                result = {'content':[{'type':'text','text':dump(value)}]}
            else:
                reply({'jsonrpc':'2.0','id':rpc_id,'error':{'code':-32601,'message':'Unknown method'}})
                return
        except Exception as exc:
            incident=diagnostics.record('ops','mcp',exc,code='tool_error',build=build_version.status().get('build_id','unknown'))
            if method != 'tools/call':
                reply({'jsonrpc':'2.0','id':rpc_id,'error':{'code':-32602,'message':'Invalid params'}});return
            message=str(exc) if isinstance(exc,(ValueError,PermissionError,KeyError,TypeError)) else 'Service unavailable'
            result = {'isError':True,'content':[{'type':'text','text':message}], 'incident_id':incident,
                      'structuredContent':{'error':'tool_error','incident_id':incident}}
        identity=onboarding.identity()
        result['_meta']={'loom':identity}
        if method=='tools/call':
            result['content'].append({'type':'text','text':onboarding.visible_version(identity)})
        reply({'jsonrpc':'2.0','id':rpc_id,'result':result})


def main():
    diagnostics.install_hooks('ops',build=build_version.status().get('build_id','unknown'))
    os.umask(0o077)
    if sys.argv[1] == 'cpu':
        return cpu(Path(sys.argv[2]),json.loads(sys.argv[3]))
    config = read(CONFIG)
    guard(config)
    init()
    if sys.argv[1] == 'worker':
        worker(config)
    elif sys.argv[1] == 'reindex':
        with db() as c:
            for row in c.execute("SELECT * FROM jobs WHERE state='complete' AND result IS NOT NULL ORDER BY created").fetchall():
                registry.index(c,row,json.loads(row['result']),config,BULK,relative,historical=True)
    elif sys.argv[1] == 'api':
        server = DiagnosticHTTPServer(('0.0.0.0',8080),API)
        server.config = config
        if cert:=os.environ.get('LOOM_TLS_CERT'):
            secure=DiagnosticHTTPServer(('0.0.0.0',8443),API)
            secure.config=config
            context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.minimum_version=ssl.TLSVersion.TLSv1_2
            context.load_cert_chain(cert,os.environ['LOOM_TLS_KEY'])
            secure.socket=context.wrap_socket(secure.socket,server_side=True)
            threading.Thread(target=secure.serve_forever,daemon=True).start()
        checker = release_updates.start(build_version.status(), LOCAL)
        try:server.serve_forever()
        finally:checker.close()
    else:
        raise ValueError('Unknown mode')


if __name__ == '__main__':
    raise SystemExit(main())
