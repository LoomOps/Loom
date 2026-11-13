"""One Job read model for imported history and executable local Jobs.

Imported records never enter the worker's jobs/attempts tables.
"""
import json
import ownership
import history_store
import job_contract


def entity(record, spec):
    attempts = record.get('attempts', [])
    result = (record.get('result') or {}) if record['state'] not in ('queued','preparing','ready','running') else {}
    identity = record['id']
    properties = {key:record.get(key) for key in ('id','state','created','attempt','owner')}
    properties['status']={'complete':'completed','processing':'finalizing','result_failed':'finalizing_failed','blocked':'queued','interrupted':'failed'}.get(record['state'],record['state'])
    properties['recovery']=record.get('recovery')
    properties.update(job_contract.values(spec))
    properties['category_recorded']=isinstance(spec.get('category'),str) and spec['category'] in job_contract.CATEGORIES
    properties['output_storage']=spec.get('output_storage','data/0')
    properties['consolidated_into']=record.get('consolidated_into')
    properties['consolidates']=spec.get('consolidates',[])
    properties['internal_step_count']=len(spec.get('profile',{}).get('steps',[]))
    properties.update(record_role='loom_job', queued_at=record['created'],
        execution=result.get('execution', {}), artifact_ready=result.get('artifact_ready'),
        reason=result.get('reason'), started_at=attempts[0]['started'] if attempts else None,
        process_exited_at=attempts[-1]['ended'] if attempts else None,
        indexed=record.get('indexed', False),
        verification={'level':spec.get('verification_level')})
    timeline=record.get('work_timeline')
    if timeline:
        properties.update(queued_at=timeline['queued'],started_at=timeline['started'],process_exited_at=timeline['ended'],
                          work_timeline=timeline)
    return {'id':identity, 'kind':'job', 'namespace':'loom',
        'label':spec.get('kind', identity), 'state':record['state'],
        'created':record['created'], 'attempt':record.get('attempt'),
        'properties':properties, 'attempts':attempts, 'loom_job':{'id':identity},
        **({'steps':[{'id':identity+'-'+e['attempt'],
            'label':('Reused execution '+str(i+1)) if e['role']=='source' else 'Consolidation verification',
            'role':e['role'],'state':e['state'],'source_job':e['job'],'source_attempt':e['attempt'],
            'actual_started_at':e['started'],'actual_exited_at':e['ended'],
            'attempts':[{'id':e['attempt'],'state':e['state'],'actual_started_at':e['started'],
                         'actual_exited_at':e['ended']}]}
            for i,e in enumerate(timeline['executions'])]} if timeline else {}),
        'execution_intervals':record.get('execution_intervals',[{'start':a['started'], 'end':a['ended'],
            'count_runtime':True, 'unknown_exit':a['ended'] is None and a['state']!='running'}
            for a in (timeline['executions'] if timeline else attempts)])}


def imported(db, actor, config):
    rows = db.execute("""SELECT * FROM history_records WHERE rowid IN (
        SELECT MAX(rowid) FROM history_records WHERE kind='job_view'
        AND source IN ('queue','loom-ops') GROUP BY id)
        AND id NOT IN (SELECT id FROM jobs) ORDER BY id""")
    result=[]
    for row in rows:
        body=json.loads(row['body'])
        e = body['entity']
        classification = job_contract.values(body.get('record', {}).get('spec', e.get('properties', {})))
        original=body.get('record',{}).get('spec',e.get('properties',{}))
        body['entity'] = {**e, 'properties': {**e.get('properties', {}), **classification,
            'category_recorded':isinstance(original.get('category'),str) and original['category'] in job_contract.CATEGORIES}}
        result.append(ownership.decorate(db,'job',{**body['entity'], 'loom_history':{key:row[key] for key in ('source','kind','id','revision')},
            'migration':{'read_only':True, 'source':row['source'], 'snapshot':row['snapshot']}}))
    return result


def listing(db, actor, config):
    return [{'id':e['id'], 'state':e['properties']['state'],
        'created':e['properties'].get('created_at', e.get('created')),
        'attempt':e.get('attempt'), 'owner':e.get('owner'),'owner_profile':e.get('owner_profile'), **job_contract.values(e['properties']), 'label':e['label'], 'migration':e['migration']}
        for e in imported(db, actor, config)]


def get(db, identity, actor, config):
    if not isinstance(identity,str) or not history_store.KEY.fullmatch(identity):
        raise ValueError('Invalid Job identity')
    item=next((e for e in imported(db,actor,config) if e['id']==identity),None)
    if item is None:
        return None
    key=item['loom_history']
    body=history_store.get_record(db,key['source'],key['kind'],key['id'],key['revision'])['body']
    return {**body.get('record', {'id':item['id'], 'state':item['properties']['state'],
        'label':item['label'], 'properties':item['properties'], 'steps':item.get('steps', [])}),
        **job_contract.values(item['properties']), 'migration':item['migration'],'owner':item.get('owner'),'owner_profile':item.get('owner_profile')}
