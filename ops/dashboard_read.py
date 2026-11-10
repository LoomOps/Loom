"""Read-only projection for the Loom Dashboard."""
import hashlib
import json
import job_work
import os
import shutil
import stat
from collections import Counter
from contextlib import ExitStack
from datetime import datetime, timezone
from pathlib import Path

from docker_status import observation_path
import history_store
import recovery
import queue_control
import job_history
import job_contract
import storage as data_storage
import registry
import ownership

MEDIA = {'.png':'image/png', '.jpg':'image/jpeg', '.jpeg':'image/jpeg',
         '.webp':'image/webp', '.mp4':'video/mp4', '.webm':'video/webm'}
TEXT_MEDIA = {'.md':'text/markdown', '.markdown':'text/markdown', '.json':'application/json',
              '.jsonl':'application/x-ndjson', '.txt':'text/plain', '.log':'text/plain',
              '.yaml':'text/plain', '.yml':'text/plain', '.toml':'text/plain', '.csv':'text/plain'}
MEDIA.update(TEXT_MEDIA)
MAX_TEXT_BYTES = 256 * 1024
MAX_MEDIA_BYTES = 64 * 1024 * 1024


def host(config, local=None):
    result = {'name':config['machine_id'], 'hostname':None, 'machine_id':config['machine_id'],
              'inventory':config.get('inventory',{}), 'status':'unavailable'}
    if local is not None:
        try:
            snapshot = json.loads(observation_path(local,'docker-status.json').read_text())
            observed = snapshot['observed_at']
            age = (datetime.now(timezone.utc)-datetime.fromisoformat(observed)).total_seconds()
            fields = ('hostname','ip','ip_kind','lan_ips','os','kernel','logical_cpus')
            result.update({key:snapshot['host'].get(key) for key in fields})
            result.update(name=result['hostname'] or result['name'], observed_at=observed,
                          status='ok' if 0 <= age <= 120 else 'stale')
        except (OSError, ValueError, TypeError, KeyError):
            pass
    return result


def add_media(index, identity, locations):
    descriptors = []
    for location in locations:
        ref = location['source_ref']
        mime = MEDIA.get(Path(ref).suffix.lower())
        if mime is None:
            continue
        key = hashlib.sha256(json.dumps([identity, ref], separators=(',', ':')).encode()).hexdigest()
        descriptor = {'id':key, 'label':Path(ref).name, 'mime':mime,
                      'status':location['status'], 'source_ref':ref}
        descriptor.update(size=location.get('size'), target_ref=location.get('target_ref'))
        limit=MAX_TEXT_BYTES if mime in TEXT_MEDIA.values() else MAX_MEDIA_BYTES
        if location.get('size', 0) > limit and descriptor['status'] == 'available':
            descriptor['status'] = 'too_large'
        index[key] = (descriptor, location)
        descriptors.append(descriptor)
    return descriptors


def file_entries(index, identity, locations, storage):
    entries=[]
    for location in locations:
        ref=location['source_ref']
        key=hashlib.sha256(json.dumps([identity,ref],separators=(',', ':')).encode()).hexdigest()
        preview=index.get(key)
        target=location.get('target_ref')
        entries.append({'id':key,'label':Path(ref).name,'source_ref':ref,'target_ref':target,
                        'path':location.get('storage_label',storage)+'/'+target if target else 'references/'+ref.lstrip('/'),
                        'status':location['status'],'size':location.get('size'),
                        'preview_id':key if preview else None})
    return entries


def storage_label(bulk):
    if bulk is not None:
        for value in os.environ.get('LOOM_DATA_PATHS','').split(':'):
            try:
                if value and Path(value).samefile(bulk):return 'data/'+Path(value).name
            except OSError:pass
    return 'data'


def extension_profile(kind,profile):
    return {'kind':kind,'stages':[{'stage':stage,'lane':runner.get('lane','cpu'),
        'runtime':runner.get('runtime_id') or runner.get('image'),'command':runner.get('command',[])}
        for stage in ('prepare','runtime','finalize','on_complete','on_error','on_cancel')
        if (runner:=profile.get(stage))], 'tools':{alias:{k:pin.get(k) for k in ('owner','id','revision','sha256')}
            for alias,pin in profile.get('tool_pins',{}).items()},'limits':profile.get('limits',{})}


def extension_inventory(db,actor,config):
    """Read immutable software revisions and actual bindings; never infer readiness."""
    import tool_store,component_store
    tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    entries=[];consumers={}
    def references(profile,owner,component=None):
        for alias,pin in profile['tools'].items():
            key=(pin['owner'],pin['id'],pin['revision'])
            consumers.setdefault(key,[]).append({'kind':profile['kind'],'alias':alias,'owner':owner,'component':component})
    for kind,table in (('tool','tools'),('component','components')):
        if table not in tables:continue
        for row in db.execute('SELECT owner,id,max(revision) revision FROM '+table+' GROUP BY owner,id').fetchall():
            value=(tool_store.detail(db,row['owner'],row['id'],row['revision']) if kind=='tool' else
                   component_store.operate(db,actor,{'action':'get','owner':row['owner'],'id':row['id']},config))
            revisions=[]
            for item in db.execute('SELECT * FROM '+table+' WHERE owner=? AND id=? ORDER BY revision DESC',(row['owner'],row['id'])).fetchall():
                manifest=json.loads(item['manifest'])
                raw_profiles=json.loads(item['profiles']) if kind=='component' else {}
                profiles=[extension_profile(key,p) for key,p in raw_profiles.items()]
                pins=manifest.get('source_pins')
                if pins is None:
                    # Older registrations sealed hashes in Profiles only. Read
                    # that evidence without rewriting the original manifest.
                    recorded=[p['source_pins'] for p in raw_profiles.values() if 'source_pins' in p]
                    pins=recorded[0] if recorded and all(p==recorded[0] for p in recorded) else None
                for profile in profiles:references(profile,row['owner'],kind+':'+row['owner']+':'+row['id']+':r'+str(row['revision']))
                revisions.append({'revision':item['revision'],'sha256':item['sha256'] if kind=='tool' else None,
                    'label':manifest.get('label',row['id']),'description':manifest.get('description',''),
                    'source_pins':pins or {},'source_pins_recorded':pins is not None,'profiles':profiles,'origin':manifest.get('origin')})
            value['revisions']=revisions
            entries.append({'id':kind+':'+row['owner']+':'+row['id']+':r'+str(row['revision']),
                'kind':kind,'namespace':'loom','label':value['manifest'].get('label',row['id']),
                'summary':value['manifest'].get('description',''),'properties':{'revision':row['revision']},
                'loom_extension':value,'relations':[],'artifacts':[],
                **ownership.extension(db,kind,row['owner'],row['id'])})
    if 'execution_profiles' in tables:
        for row in db.execute('SELECT owner,profile FROM execution_profiles'):
            profile=json.loads(row['profile']);references(extension_profile(profile['revision'],profile),row['owner'])
    for entity in entries:
        if entity['kind']=='tool':
            for revision in entity['loom_extension']['revisions']:
                revision['used_by']=consumers.get((entity['loom_extension']['owner'],entity['loom_extension']['id'],revision['revision']),[])
    return entries


def projection(db, actor, config, local, bulk=None):
    storage=storage_label(bulk)
    snapshot = db.execute("""SELECT snapshot FROM history_records
        WHERE source='catalog' ORDER BY rowid DESC LIMIT 1""").fetchone()
    # A clean installation has no legacy catalog; native Jobs/results still render.
    if snapshot is None:snapshot=(None,)
    entities, media, sources = [], {}, set()
    imported_jobs = job_history.imported(db, actor, config)
    imported_ids = {item['id'] for item in imported_jobs}
    for row in db.execute("""SELECT source,kind,id,revision,body FROM history_records
            WHERE snapshot=? AND source IN ('catalog','queue') ORDER BY source,kind,id""", (snapshot[0],)):
        identity = {key:row[key] for key in ('source','kind','id','revision')}
        body = json.loads(row['body'])
        if row['source'] == 'catalog':
            original = body['original']
            if original['id'] in imported_ids:
                continue
            locations = body['locations']
            source = original.get('source')
            if isinstance(source, str):
                sources.add(source)
            entities.append({**original, 'loom_history':identity, 'payload_status':body['payload_status'],
                             'loom_location_counts':dict(Counter(item['status'] for item in locations)),
                             'loom_media':add_media(media, list(identity.values()), locations),
                             'loom_files':file_entries(media,list(identity.values()),locations,storage)})
    consolidations=job_work.completed_links(db)
    for row in db.execute("""SELECT * FROM jobs ORDER BY created,id"""):
        if not recovery.permitted(actor,row):continue
        item = dict(row)
        item['recovery']=recovery.view(db,row['id']) if 'recovery' in {x[0] for x in db.execute("SELECT name FROM sqlite_master WHERE type='table'")} else None
        spec = json.loads(item.pop('spec'))
        raw_result = item.pop('result')
        result = json.loads(raw_result) if raw_result else {}
        attempts = [dict(value) for value in db.execute(
            'SELECT id,state,started,ended FROM attempts WHERE job=? ORDER BY started,id',(row['id'],))]
        item['consolidated_into']=consolidations.get(row['id'])
        item['work_timeline']=job_work.timeline(db,row,spec)
        item.update(result=result, attempts=attempts, indexed=bool(db.execute(
            'SELECT 1 FROM entity_revisions WHERE job=? LIMIT 1',(row['id'],)).fetchone()))
        entities.append(job_history.entity(item, spec))
    entities.extend(imported_jobs)
    for record in registry.view(db, actor):
        item = registry.view(db, actor, record['id'], record['revision'])
        components = {entry['ref']:entry for entry in item['components']}
        locations=[]
        configured=data_storage.stores(config,bulk or Path('/loom/bulk'))
        for entry in item.pop('locations'):
            if entry['machine_id']!=config['machine_id']:continue
            for name,store in configured.items():
                if entry['storage_id']==store['storage_id']:
                    locations.append({**components[entry['component']],'source_ref':entry['component'],
                        'target_ref':entry['ref'],'status':entry['status'],'storage_label':storage if name=='data/0' and not config.get('data_stores') else name,
                        '_storage_root':store['path']})
                    break
        typed_metrics = item.pop('metrics')
        metrics = [{'id':metric['name'], 'label':metric['name'], 'kind':'series',
                    'unit':metric.get('unit'), 'definition':metric.get('definition'),
                    'aggregation':metric.get('denominator'), 'x_label':'Step',
                    'observations':[{'x':point['step'], 'value':point['value']}
                                    for point in metric.get('series',[])]} for metric in typed_metrics]
        entities.append({**item, **record, 'metrics':metrics, 'loom_metrics':typed_metrics,
                         'properties':{**item.get('metadata',{}),**item['provenance']}, 'loom_registry':{'id':record['id'], 'revision':record['revision']},
                         'loom_job':{'id':record['job']}, 'namespace':'loom',
                         'label':item.get('label',record['id']),
                         'loom_location_counts':dict(Counter(entry['status'] for entry in locations)),
                         'loom_media':add_media(media, ['registry',record['id'],record['revision']], locations),
                         'loom_files':file_entries(media,['registry',record['id'],record['revision']],locations,storage)})
    entities=[ownership.decorate(db,'job' if e['kind']=='job' else 'entity',e) for e in entities]
    # Research Tools and Job lifecycle Components are separate registry entries.
    # Historical catalog entries and their original classification remain intact.
    entities.extend(extension_inventory(db,actor,config))
    queue = queue_control.view(db,actor,queue_control.execution_lane)
    revision = hashlib.sha256(json.dumps({'entities':entities,'queue':queue}, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    return {'entities':entities, 'queue':queue, 'sources':[{'id':source, 'status':'ok'} for source in
                       sorted(sources | {'catalog','queue','loom-ops'})], 'revision':revision,
            'host':host(config, local), 'job_contract':job_contract.contract()}, media


def open_media(index, key, bulk):
    if key not in index:
        raise FileNotFoundError
    descriptor, location = index[key]
    if descriptor['status'] != 'available':
        return descriptor, None
    bulk=Path(location.get('_storage_root',bulk))
    path = history_store.under(bulk, location['target_ref'])
    parts = path.relative_to(bulk.resolve()).parts
    if not parts:
        raise FileNotFoundError
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    with ExitStack() as parents:
        directory = os.open(bulk,directory_flags)
        parents.callback(os.close,directory)
        for part in parts[:-1]:
            directory = os.open(part,directory_flags,dir_fd=directory)
            parents.callback(os.close,directory)
        fd = os.open(parts[-1],os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,dir_fd=directory)
    stream = os.fdopen(fd, 'rb')
    try:
        facts = os.fstat(fd)
        text=descriptor['mime'] in TEXT_MEDIA.values()
        limit=MAX_TEXT_BYTES if text else MAX_MEDIA_BYTES
        if (not stat.S_ISREG(facts.st_mode) or not (0 if text else 1) <= facts.st_size <= limit
                or facts.st_size != location.get('size')
                or hashlib.file_digest(stream, 'sha256').hexdigest() != location.get('sha256')):
            stream.close()
            return {**descriptor, 'status':'corrupt'}, None
        stream.seek(0)
        return {**descriptor, 'size':facts.st_size}, stream
    except BaseException:
        stream.close()
        raise


def resources(config, bulk, local, *, connection=None, actor=None, lane_for=None):
    observed = datetime.now(timezone.utc).isoformat()
    sections = {name:{'status':'unavailable', 'data':None}
                for name in ('cpu','memory','gpus','storage','sensors','leases')}
    def measured(name, source, data):
        sections[name] = {'status':'ok', 'source':source, 'observed_at':observed, 'data':data}

    try:
        load = float(Path('/proc/loadavg').read_text().split()[0])
        measured('cpu','linux:/proc/loadavg',{'load_1m':load, 'logical_cpus':os.cpu_count()})
        values = {key.rstrip(':'):int(value)*1024 for key,value,*_ in
                  (line.split() for line in Path('/proc/meminfo').read_text().splitlines())}
        measured('memory','linux:/proc/meminfo',{'total_bytes':values['MemTotal'],
                 'available_bytes':values['MemAvailable'], 'used_bytes':values['MemTotal']-values['MemAvailable'],
                 'swap_used_bytes':values['SwapTotal']-values['SwapFree']})
    except (OSError, ValueError, KeyError):
        pass
    # Compose declares which data mounts this instance exposes. Old instances
    # retain the single output store until their deployment is updated.
    storage_paths = os.environ.get('LOOM_DATA_PATHS', str(bulk)).split(os.pathsep)
    disks = []
    for index, path in enumerate(storage_paths):
        disk = {'path':f'data/{index}', 'mount':path, 'status':'unavailable'}
        try:
            if not Path(path).is_dir():
                raise FileNotFoundError(path)
            total, used, free = shutil.disk_usage(path)
            disk.update(status='ok', total_bytes=total, used_bytes=used, free_bytes=free)
        except OSError:
            disk['reason'] = 'data_directory_unavailable'
        disks.append(disk)
    measured('storage','linux:statvfs',disks)
    if not any(disk['status'] == 'ok' for disk in disks):
        sections['storage']['status'] = 'unavailable'
    try:
        gpu = json.loads(observation_path(local,'dashboard-gpus.json').read_text())
        age = (datetime.now(timezone.utc) - datetime.fromisoformat(gpu['observed_at'])).total_seconds()
        sections['gpus'] = {**gpu, 'status':gpu['status'] if 0 <= age <= 90 else 'stale'}
    except (OSError, ValueError, KeyError, TypeError):
        pass
    try:
        sample = json.loads(observation_path(local,'dashboard-host-metrics.json').read_text())
        for name in ('cpu','sensors'):
            section = sample['sections'][name]
            age = (datetime.now(timezone.utc)-datetime.fromisoformat(section['observed_at'])).total_seconds()
            sections[name] = {**section, 'status':section['status'] if 0 <= age <= 90 else 'stale'}
    except (OSError, ValueError, KeyError, TypeError):
        pass
    if connection is not None and actor is not None and lane_for is not None:
        rows = connection.execute("""SELECT id,state,spec,attempt FROM jobs
            WHERE state IN ('running','preparing','ready','queued','blocked') ORDER BY created,id""").fetchall()
        registered = []
        for row in rows:
            if row['state'] not in ('running','preparing'):
                continue
            spec = json.loads(row['spec'])
            lane = 'cpu' if row['state']=='preparing' else lane_for(spec)
            registered.append({'job_id':row['id'], 'label':spec.get('kind',row['id']),
                'attempt':row['attempt'], 'resource':lane.upper()+' 실행 슬롯', 'units':1})
        measured('leases','loom:queue/worker-lanes',{'registered':registered,
            'queued_jobs':sum(row['state'] in ('queued','ready','blocked') for row in rows),
            'scope':'visible_node_jobs', 'mechanism':'worker_lanes'})
    return {'host':host(config, local), 'observed_at':observed, 'sections':sections}
