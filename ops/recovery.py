"""Durable queue reservations. Agent ownership is separate from event delivery."""
import hashlib
import json
import time
import uuid

RESPONSE_SECONDS=900
IDLE_SECONDS=900
ROUND_SECONDS=1800
MAX_ROUNDS=3


def init(c):
    c.executescript('''
    CREATE TABLE IF NOT EXISTS recovery(
      job TEXT PRIMARY KEY, state TEXT NOT NULL, rounds INTEGER NOT NULL DEFAULT 0,
      agent TEXT, token TEXT, deadline REAL, hard_deadline REAL,
      reason TEXT, updated REAL NOT NULL);
    CREATE TABLE IF NOT EXISTS recovery_history(
      seq INTEGER PRIMARY KEY, job TEXT NOT NULL, action TEXT NOT NULL,
      agent TEXT, round INTEGER NOT NULL, recorded REAL NOT NULL, detail TEXT);
    CREATE TABLE IF NOT EXISTS job_revisions(
      job TEXT NOT NULL, revision INTEGER NOT NULL, spec TEXT NOT NULL,
      reason TEXT NOT NULL, created REAL NOT NULL, PRIMARY KEY(job,revision));
    ''')
    columns={r[1] for r in c.execute('PRAGMA table_info(jobs)')}
    if 'queue_order' not in columns:
        c.execute('ALTER TABLE jobs ADD COLUMN queue_order REAL')
        c.execute('UPDATE jobs SET queue_order=created')


def log(c,job,action,agent,round_,detail=None):
    c.execute('INSERT INTO recovery_history(job,action,agent,round,recorded,detail) VALUES(?,?,?,?,?,?)',
              (job,action,agent,round_,time.time(),json.dumps(detail) if detail else None))


def open_incident(c,row,reason,now=None):
    now=time.time() if now is None else now
    old=c.execute('SELECT * FROM recovery WHERE job=?',(row['id'],)).fetchone()
    rounds=old['rounds'] if old else 0
    # Recovery of finalization does not reserve a GPU that has already returned.
    state='exhausted' if rounds>=MAX_ROUNDS else 'waiting'
    c.execute('''INSERT INTO recovery(job,state,rounds,deadline,reason,updated) VALUES(?,?,?,?,?,?)
        ON CONFLICT(job) DO UPDATE SET state=excluded.state,agent=NULL,token=NULL,
        deadline=excluded.deadline,hard_deadline=NULL,reason=excluded.reason,updated=excluded.updated''',
        (row['id'],state,rounds,now+RESPONSE_SECONDS if state=='waiting' else None,reason,now))
    log(c,row['id'],state,None,rounds,{'reason':reason})


def expire(c,now=None):
    now=time.time() if now is None else now
    for row in c.execute("SELECT * FROM recovery WHERE state IN ('waiting','claimed') AND deadline<=?",(now,)).fetchall():
        c.execute("UPDATE recovery SET state='deferred',token=NULL,updated=? WHERE job=?",(now,row['job']))
        log(c,row['job'],'expired',row['agent'],row['rounds'])


def view(c,job):
    row=c.execute('SELECT * FROM recovery WHERE job=?',(job,)).fetchone()
    if not row:return None
    value=dict(row);value.pop('token',None)
    value['max_rounds']=MAX_ROUNDS
    return value


def permitted(actor,row):
    return bool(actor.get('id'))  # Shared workspace; operation scopes are checked by the API.


def action(c,row,actor,operation,args,validate_patch):
    now=time.time();expire(c,now)
    if not permitted(actor,row):raise PermissionError('Recovery outside delegated owner')
    rec=c.execute('SELECT * FROM recovery WHERE job=?',(row['id'],)).fetchone()
    if rec is None:raise ValueError('No recovery incident')
    fingerprint=hashlib.sha256(json.dumps(args,sort_keys=True).encode()).hexdigest()
    if operation=='resume':
        previous=c.execute("SELECT detail FROM recovery_history WHERE job=? AND action='resume' AND agent=? ORDER BY seq DESC LIMIT 1",(row['id'],actor['id'])).fetchone()
        if previous and previous[0] and json.loads(previous[0]).get('request_sha256')==fingerprint:return view(c,row['id'])
    if operation=='claim':
        if rec['state']=='claimed' and rec['agent']==actor['id']:
            return {**view(c,row['id']),'lease':rec['token']}
        if rec['state']!='waiting':raise ValueError('Reservation expired or already claimed; reactivate deferred jobs first')
        token=uuid.uuid4().hex
        c.execute("UPDATE recovery SET state='claimed',agent=?,token=?,deadline=?,hard_deadline=?,updated=? WHERE job=?",
                  (actor['id'],token,now+IDLE_SECONDS,now+ROUND_SECONDS,now,row['id']))
        log(c,row['id'],'claimed',actor['id'],rec['rounds'])
        return {**view(c,row['id']),'lease':token}
    if operation=='reactivate':
        if rec['state']!='deferred':raise ValueError('Only a deferred reservation may be reactivated')
        from queue_control import append_order
        append_order(c,row['id'])
        c.execute("UPDATE recovery SET state='waiting',agent=NULL,token=NULL,deadline=?,hard_deadline=NULL,updated=? WHERE job=?",(now+RESPONSE_SECONDS,now,row['id']))
        log(c,row['id'],'reactivated',actor['id'],rec['rounds'])
        return view(c,row['id'])
    if operation=='defer':
        if rec['state']=='deferred':return view(c,row['id'])
        if rec['state'] not in ('waiting','claimed'):raise ValueError('No active reservation')
        c.execute("UPDATE recovery SET state='deferred',token=NULL,updated=? WHERE job=?",(now,row['id']))
        log(c,row['id'],'deferred',actor['id'],rec['rounds'])
        return view(c,row['id'])
    if rec['state']!='claimed' or rec['agent']!=actor['id'] or rec['token']!=args.get('lease'):
        raise PermissionError('Valid recovery lease required')
    if operation=='progress':
        c.execute('UPDATE recovery SET deadline=?,updated=? WHERE job=?',(min(now+IDLE_SECONDS,rec['hard_deadline']),now,row['id']))
    elif operation=='resume':
        if rec['rounds']>=MAX_ROUNDS:raise ValueError('Recovery budget exhausted')
        spec=json.loads(row['spec']);patch=args.get('patch',{})
        if not isinstance(patch,dict) or set(patch)-{'params','kind','execution'}:raise ValueError('Recovery may update parameters, runtime profile and execution limits only')
        spec=validate_patch(spec,patch)
        reason=args.get('reason','agent recovery')
        if not isinstance(reason,str) or not 1<=len(reason)<=1000:raise ValueError('Recovery reason required')
        revision=c.execute('SELECT COALESCE(MAX(revision),0)+1 FROM job_revisions WHERE job=?',(row['id'],)).fetchone()[0]
        c.execute('INSERT INTO job_revisions VALUES(?,?,?,?,?)',(row['id'],revision,json.dumps(spec),reason,now))
        finalization=row['state']=='result_failed'
        if finalization and patch:raise ValueError('Result-only retry cannot change execution inputs')
        c.execute("UPDATE jobs SET spec=?,state=?,cancel=0,available_at=0 WHERE id=?",(json.dumps(spec),'processing' if finalization else 'queued',row['id']))
        if finalization:c.execute('UPDATE result_outbox SET retries=0,next_try=0 WHERE job=?',(row['id'],))
        c.execute("UPDATE recovery SET state='pending',token=NULL,updated=? WHERE job=?",(now,row['id']))
    else:raise ValueError('Unknown recovery operation')
    log(c,row['id'],operation,actor['id'],rec['rounds'],{'request_sha256':fingerprint} if operation=='resume' else None)
    return view(c,row['id'])


def started(c,job):
    c.execute("UPDATE recovery SET state='executing',rounds=rounds+1,deadline=NULL,hard_deadline=NULL,updated=? WHERE job=? AND state='pending'",(time.time(),job))


def close(c,job,state):
    c.execute('UPDATE recovery SET state=?,token=NULL,deadline=NULL,updated=? WHERE job=?',(state,time.time(),job))


def reserved(c,spec,queue_order,lane_for):
    lane=lane_for(spec)
    for row in c.execute("SELECT j.spec,j.queue_order,r.reason FROM jobs j JOIN recovery r ON j.id=r.job WHERE r.state IN ('waiting','claimed') AND j.queue_held=0 AND j.cancel=0 AND r.deadline>? AND j.state!='result_failed' AND j.queue_order<=?",(time.time(),queue_order)):
        if lane_for(json.loads(row['spec']))==lane:return True
    return False
