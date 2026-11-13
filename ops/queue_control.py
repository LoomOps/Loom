"""Persisted queue ordering and dispatch holds, independent of execution state."""
import json,time,hashlib
import recovery
import ownership
import execution
import dispatch_control

WAITING={'queued','ready','blocked','preparing'}
POSITION_SCHEMA={'type':'string','enum':['next','tail','before','after']}

def init(c):
 dispatch_control.init(c)
 if 'queue_held' not in {r[1] for r in c.execute('PRAGMA table_info(jobs)')}:
  c.execute('ALTER TABLE jobs ADD COLUMN queue_held INTEGER NOT NULL DEFAULT 0')
 c.execute('''CREATE TABLE IF NOT EXISTS queue_history(
 seq INTEGER PRIMARY KEY,job TEXT NOT NULL,actor TEXT NOT NULL,action TEXT NOT NULL,
 recorded REAL NOT NULL,reason TEXT,before_order TEXT NOT NULL,after_order TEXT NOT NULL)''')

def members(c):
 return c.execute('''SELECT j.* FROM jobs j LEFT JOIN recovery r ON r.job=j.id
 WHERE j.cancel=0 AND (j.state IN ('queued','ready','blocked','preparing','running')
 OR (j.state IN ('failed','interrupted') AND r.state IN ('waiting','claimed')))
 ORDER BY CASE WHEN j.state='running' THEN 0 ELSE 1 END,COALESCE(j.queue_order,j.created),j.id''').fetchall()

def order_value(row):return row['queue_order'] if row['queue_order'] is not None else row['created']

def append_order(c,job):
 value=max(time.time(),max((order_value(r) for r in members(c) if r['id']!=job),default=0)+1)
 c.execute('UPDATE jobs SET queue_order=? WHERE id=?',(value,job))

def reason_value(args):
 reason=args.get('reason','')
 if not isinstance(reason,str) or len(reason)>1000:raise ValueError('Queue reason must be at most 1000 characters')
 return reason

def log(c,job,actor,action,reason,before,after):
 c.execute('INSERT INTO queue_history(job,actor,action,recorded,reason,before_order,after_order) VALUES(?,?,?,?,?,?,?)',
 (job,actor['id'],action,time.time(),reason,json.dumps(before),json.dumps(after)))

def position(c,job):
 waiting=[r['id'] for r in members(c) if r['state']!='running']
 return waiting.index(job)+1 if job in waiting else None

def receipt(c,job):
 row=c.execute('SELECT id,state,queue_held FROM jobs WHERE id=?',(job,)).fetchone()
 return {'id':job,'state':row['state'],'held':bool(row['queue_held']),'position':position(c,job)}

def reorder(c,row,actor,args):
 reason=reason_value(args);where=args.get('position');anchor=args.get('reference')
 if where not in POSITION_SCHEMA['enum']:raise ValueError('Invalid queue position')
 if (where in ('before','after')) != (anchor is not None):raise ValueError('before/after require reference; next/tail do not accept it')
 if anchor is not None and (not isinstance(anchor,str) or anchor==row['id']):raise ValueError('Invalid queue reference')
 rows=members(c);ids=[r['id'] for r in rows];active=[r['id'] for r in rows if r['state']=='running']
 if row['state']=='running' or row['id'] not in ids:
  return {**receipt(c,row['id']),'changed':False,'reason':'not_waiting'}
 rest=[ident for ident in ids if ident!=row['id']]
 if where in ('before','after'):
  if anchor not in rest or anchor in active:raise ValueError('Reference is no longer waiting; refresh the queue')
  index=rest.index(anchor)+(where=='after')
 else:index=len(active) if where=='next' else len(rest)
 rest.insert(index,row['id'])
 if rest==ids:return {**receipt(c,row['id']),'changed':False,'reason':'already_positioned'}
 # Dense integer ranks avoid float midpoint exhaustion. Running Jobs retain the
 # first ranks so a later failure still reserves their previous place.
 c.executemany('UPDATE jobs SET queue_order=? WHERE id=?',[(i+1,ident) for i,ident in enumerate(rest)])
 log(c,row['id'],actor,'reorder',reason,ids,rest)
 return {**receipt(c,row['id']),'changed':True}

def hold(c,row,actor,args,held):
 reason=reason_value(args)
 allowed=row['state'] in WAITING or (not held and bool(row['queue_held']) and row['state'] in ('failed','interrupted'))
 if not allowed:return {**receipt(c,row['id']),'changed':False,'reason':'not_waiting'}
 if bool(row['queue_held'])==held:return {**receipt(c,row['id']),'changed':False,'reason':'already_held' if held else 'already_released'}
 before={'held':bool(row['queue_held']),'position':position(c,row['id'])}
 c.execute('UPDATE jobs SET queue_held=? WHERE id=?',(int(held),row['id']))
 log(c,row['id'],actor,'hold' if held else 'resume',reason,before,{'held':held,'position':before['position']})
 return {**receipt(c,row['id']),'changed':True}

def execution_lane(spec):
 profile=spec.get('profile',{})
 if execution.is_generic(profile):return profile['runtime'].get('lane','cpu')
 return 'gpu' if profile.get('runtime',{}).get('gpu') else 'cpu'

def stage_for(c,row):
 spec=json.loads(row['spec'])
 prepared=bool(row['state'] in ('ready','blocked') and row['attempt'] and c.execute("SELECT 1 FROM stage_runs WHERE job=? AND attempt=? AND stage='prepare' AND state='returned'",(row['id'],row['attempt'])).fetchone())
 return ('prepare' if row['state']=='preparing' or (row['state'] in ('queued','blocked') and spec.get('profile',{}).get('prepare') and not prepared) else 'run'),prepared

def scheduling(c,row,lane_for):
 spec=json.loads(row['spec']);stage,prepared=stage_for(c,row)
 base={'held':bool(row['queue_held']),'stage':stage,'lane':'cpu' if stage=='prepare' else lane_for(spec)}
 def stopped(reason,**extra):return {**base,'eligible':False,'reason':reason,**extra}
 if row['cancel'] or row['state'] in ('complete','cancelled'):return stopped('terminal')
 if row['state'] in ('running','preparing','processing'):return stopped('executing' if row['state']!='processing' else 'finalizing')
 if dispatch_control.status(c)['paused']:return stopped('queue_paused')
 if row['queue_held']:return stopped('held')
 rec=recovery.view(c,row['id'])
 if rec and rec['state'] in ('waiting','claimed','deferred','exhausted'):return stopped('recovery_'+rec['state'])
 if row['state'] not in ('queued','ready','blocked'):return stopped('not_queued')
 if row['available_at']>time.time():return stopped('retry_delay')
 if stage!='prepare' and recovery.reserved(c,spec,order_value(row),lane_for):return stopped('recovery_reservation')
 dependencies=spec.get('profile',{}).get('prepare',{}).get('depends_on',spec.get('depends_on',[])) if stage=='prepare' else spec.get('depends_on',[])
 waiting=[];failed=False
 for ident in dependencies:
  upstream=c.execute("SELECT state,json_extract(result,'$.artifact_ready') FROM jobs WHERE id=?",(ident,)).fetchone()
  if upstream and (upstream['state']=='complete' or (upstream['state'] in ('processing','result_failed') and upstream[1]==1)):continue
  waiting.append(ident)
  failed |= upstream is None or upstream['state'] in ('failed','cancelled','interrupted','blocked','result_failed')
 if waiting:return stopped('dependencies',waiting_for=waiting,dependency_failed=failed)
 for active in c.execute("SELECT id,state,spec FROM jobs WHERE state IN ('running','preparing') AND id!=?",(row['id'],)):
  occupied='cpu' if active['state']=='preparing' else lane_for(json.loads(active['spec']))
  if occupied==base['lane']:return stopped('resource_busy')
 return {**base,'eligible':True,'reason':'ready','waiting_for':[]}

def view(c,actor,lane_for):
 if not c.in_transaction:c.execute("BEGIN")
 rows=[];rank=0
 for row in members(c):
  if row['state']!='running':rank+=1
  owned=recovery.permitted(actor,row)
  schedule=scheduling(c,row,lane_for)
  # Job responsibility is descriptive; all users share queue control.
  rows.append({'id':row['id'],'owner':row['owner'],'state':row['state'],'status':{'blocked':'queued','interrupted':'failed'}.get(row['state'],row['state']),
    'position':None if row['state']=='running' else rank,'held':bool(row['queue_held']),
    'lane':schedule['lane'],'stage':schedule['stage'],
    'reason':schedule['reason'],'eligible':schedule['eligible'],
    'can_reorder':owned and 'submit' in actor['scopes'] and row['state']!='running',
    'can_hold':owned and 'submit' in actor['scopes'] and row['state'] in WAITING,
    'can_resume':owned and 'submit' in actor['scopes'] and bool(row['queue_held']) and row['state']!='running'})
 rows=[ownership.decorate(c,'job',row) for row in rows]
 revision=hashlib.sha256(json.dumps(rows,sort_keys=True).encode()).hexdigest()
 control=dispatch_control.status(c)
 revision=hashlib.sha256(json.dumps({'entries':rows,'dispatch':control},sort_keys=True).encode()).hexdigest()
 import research
 for item in rows:
  record=c.execute('SELECT id,spec FROM jobs WHERE id=?',(item['id'],)).fetchone()
  item['presentation']=research.job(c,record)
  item['presentation']['visible']=item['presentation'].get('scope')=='research' and item['presentation'].get('role')=='main' and not item['presentation'].get('parent_job')
 import workflows
 logical=[];byid={r['id']:r for r in rows}
 for w in workflows.listing(c):
  children=[byid[n['job']] for n in w['executions'] if n['job'] in byid]
  if not children:continue
  held=all(n['held'] for n in children)
  p=w['presentation'];p['visible']=p.get('scope')=='research' and p.get('role')=='main' and not p.get('parent_job')
  logical.append({'id':w['id'],'owner':w['owner'],'state':w['state'],'status':w['status'],
   'position':min((n['position'] for n in children if n['position'] is not None),default=None),
   'held':held,'reason':'held' if held else 'executing' if w['state'] in ('running','preparing') else 'recovery_required' if w['errors'] else children[0]['reason'],
   'eligible':False,'can_reorder':False,'can_hold':any(n['can_hold'] for n in children),'can_resume':any(n['can_resume'] for n in children),'presentation':p})
 return {'revision':revision,'entries':rows,'logical_entries':logical,'dispatch':control}
