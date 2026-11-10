"""Local-operator dispatch pause; submissions and running resource ownership differ."""
import time
import uuid


def init(c):
    c.execute('CREATE TABLE IF NOT EXISTS dispatch_control(id INTEGER PRIMARY KEY CHECK(id=1),paused INTEGER NOT NULL,token TEXT,reason TEXT,recorded REAL)')
    c.execute('INSERT OR IGNORE INTO dispatch_control VALUES(1,0,NULL,NULL,NULL)')


def status(c, private=False):
    if not c.execute("SELECT 1 FROM sqlite_master WHERE name='dispatch_control'").fetchone():return {'paused':False}
    row=c.execute('SELECT paused,token,reason,recorded FROM dispatch_control WHERE id=1').fetchone()
    result={'paused':bool(row[0]),'reason':row[2],'recorded':row[3]}
    if private:result['token']=row[1]
    return result


def change(c, paused, token, reason=''):
    try:uuid.UUID(token)
    except (ValueError,TypeError,AttributeError):raise ValueError('A stable UUID pause key is required') from None
    if not isinstance(reason,str) or len(reason)>1000:raise ValueError('Invalid dispatch pause reason')
    c.execute('BEGIN IMMEDIATE')
    init(c);current=status(c,True)
    if current['paused']:
        if current['token']!=token:raise ValueError('Another operator pause owns the queue')
        if paused:return current
    elif not paused:
        if current['token']!=token:raise ValueError('Pause key does not match the recorded operation')
        return current
    mode=c.execute('SELECT mode FROM node_control WHERE id=1').fetchone()[0]
    if mode not in ('active','maintenance'):raise ValueError('Cannot change queue of a transferred/read-only instance')
    if paused:
        if mode!='active':raise ValueError('Instance has an unrelated maintenance fence')
        if c.execute("SELECT 1 FROM jobs WHERE state IN ('preparing','running','processing') LIMIT 1").fetchone():
            raise ValueError('Wait for running Jobs and result processing to return before pausing for deployment')
        if c.execute("SELECT 1 FROM sqlite_master WHERE name='hooks'").fetchone() and c.execute("SELECT 1 FROM hooks WHERE state='pending' LIMIT 1").fetchone():
            raise ValueError('Wait for lifecycle callbacks before deployment')
        # Older workers honor this fence; new workers also retain dispatch pause
        # after the updater reopens admission for a staged continuation Job.
        c.execute("UPDATE node_control SET mode='maintenance' WHERE id=1")
    elif mode=='maintenance':c.execute("UPDATE node_control SET mode='active' WHERE id=1")
    c.execute('UPDATE dispatch_control SET paused=?,token=?,reason=?,recorded=? WHERE id=1',(int(paused),token,reason,time.time()))
    return status(c,True)
