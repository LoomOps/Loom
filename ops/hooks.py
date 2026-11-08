"""Durable lifecycle callbacks. Notification failure never re-executes GPU work."""
import json
import time

def init(c):
    c.execute('''CREATE TABLE IF NOT EXISTS hooks(job TEXT,attempt TEXT,stage TEXT,state TEXT,
      retries INTEGER NOT NULL DEFAULT 0,next_try REAL NOT NULL DEFAULT 0,error TEXT,
      PRIMARY KEY(job,attempt,stage))''')
    columns={r[1] for r in c.execute('PRAGMA table_info(hooks)')}
    for name in ('spec_snapshot','result_snapshot','owner_snapshot','notify_snapshot'):
        if name not in columns:c.execute('ALTER TABLE hooks ADD COLUMN '+name+' TEXT')

def enqueue(c,row,state):
    stage={'complete':'on_complete','failed':'on_error','interrupted':'on_error','cancelled':'on_cancel'}.get(state)
    if stage and json.loads(row['spec']).get('profile',{}).get(stage):
        c.execute("INSERT OR IGNORE INTO hooks(job,attempt,stage,state,spec_snapshot,result_snapshot,owner_snapshot,notify_snapshot) VALUES(?,?,?,'pending',?,?,?,?)",(row['id'],row['attempt'],stage,row['spec'],row.get('result'),row['owner'],row['notify']))

def pending(c):
    return c.execute("SELECT h.*,COALESCE(h.spec_snapshot,j.spec) AS spec,h.result_snapshot AS result,COALESCE(h.owner_snapshot,j.owner) AS owner,COALESCE(h.notify_snapshot,j.notify) AS notify FROM hooks h JOIN jobs j ON h.job=j.id WHERE h.state='pending' AND h.next_try<=? ORDER BY h.next_try LIMIT 1",(time.time(),)).fetchone()
