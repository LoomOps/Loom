"""Ordering/holds, privacy and transactions on isolated durable SQLite databases."""
from concurrent.futures import ThreadPoolExecutor
import json
import threading
import time
from unittest.mock import patch
import ops,queue_control,recovery
from test_support import IsolatedOps

class QueueControlTests(IsolatedOps):
 def action(self,action,job=None,actor=None,**args):
  return ops.operate('jobs',{'action':action,'arguments':{**({'id':job} if job else {}),**args}},actor or self.owner,self.config)
 def ids(self):return [x['id'] for x in self.action('queue')['entries']]
 def schedule(self,job):return ops.operate('get_job',{'id':job},self.owner,self.config)['scheduling']
 def test_fifo_placements_hold_and_reopen_persist(self):
  a,b,c=[self.submit(self.spec(x)) for x in 'abc']
  self.assertEqual(self.ids(),[a,b,c])
  for target,where,ref,expected in [(c,'next',None,[c,a,b]),(c,'tail',None,[a,b,c]),(c,'before',b,[a,c,b]),(a,'after',b,[c,b,a])]:
   self.action('reorder',target,position=where,**({'reference':ref} if ref else {}));self.assertEqual(self.ids(),expected)
  self.action('hold',b,reason='user coordination');before=self.row(b)['queue_order']
  self.assertEqual(self.schedule(b)['reason'],'held')
  ops.init();self.assertEqual(self.ids(),[c,b,a]);self.assertTrue(self.row(b)['queue_held'])
  self.action('resume',b);self.assertEqual(self.row(b)['queue_order'],before)
  self.assertFalse(self.action('resume',b)['changed'])
  history=ops.operate('get_job',{'id':b},self.owner,self.config)['queue_history']
  self.assertEqual([h['action'] for h in history],['hold','resume']);self.assertEqual(history[0]['actor'],'owner')
 def test_submit_next_idempotent_after_later_reorder(self):
  a=self.submit(self.spec('a'));spec={**self.spec('urgent'),'queue_position':'next'};b=self.submit(spec)
  self.assertEqual(self.ids(),[b,a]);self.action('reorder',b,position='tail')
  self.assertEqual(self.submit(spec),b);self.assertEqual(self.ids(),[a,b])
  self.assertEqual(self.submit({**self.spec('a'),'queue_position':'tail'}),a)
  for where in ['before',None,{},True]:
   with self.subTest(where=where),self.assertRaises(ValueError):self.submit({**self.spec('bad'),'queue_position':where})
  with self.assertRaises(ValueError):self.submit({**spec,'queue_position':'tail'})
 def test_users_can_coordinate_all_jobs_but_readonly_credentials_cannot_write(self):
  other={**self.owner,'id':'other','owners':['other'],'notify':['other']}
  a=self.submit({**self.spec('private'),'owner':'other','notify':'other'},other);b=self.submit(self.spec('mine'))
  self.action('reorder',b,position='before',reference=a);self.assertEqual(self.ids(),[b,a])
  for action,extra in [('hold',{}),('resume',{}),('reorder',{'position':'tail'}),('get',{})]:
   with self.subTest(action=action):self.assertEqual(self.action(action,a,**extra)['id'],a)
  visible=self.action('queue',actor=self.reader)['entries'];self.assertEqual(len(visible),2)
  self.assertNotIn('params',json.dumps(visible));self.assertNotIn('private',json.dumps(visible))
  self.assertTrue(all(not r['can_reorder'] for r in visible))
  with self.assertRaises(PermissionError):self.action('hold',b,actor=self.reader)
  delegate={**self.owner,'id':'delegate'};self.assertTrue(self.action('hold',b,actor=delegate)['changed'])
 def test_running_job_is_not_preempted_and_stale_anchor_is_rejected(self):
  a,b=[self.submit(self.spec(x)) for x in 'ab'];self.claim(a)
  for action,args in [('hold',{}),('reorder',{'position':'tail'})]:
   self.assertFalse(self.action(action,a,**args)['changed'])
  with self.assertRaises(ValueError):self.action('reorder',b,position='before',reference=a)
  self.action('reorder',b,position='next');self.assertEqual(self.ids(),[a,b]);self.assertEqual(self.schedule(b)['reason'],'resource_busy')
  self.assertEqual(self.row(a)['state'],'running')
 def test_reorder_cannot_evade_dependencies_and_failed_dependencies_skip(self):
  a=self.submit(self.spec('source'));b=self.submit({**self.spec('consumer'),'depends_on':[a]});c=self.submit(self.spec('independent'))
  self.action('reorder',b,position='next');self.assertEqual(self.schedule(b)['reason'],'dependencies');self.assertTrue(self.schedule(c)['eligible'])
  with ops.db() as db:db.execute("UPDATE jobs SET state='failed' WHERE id=?",(a,))
  self.assertTrue(self.schedule(b)['dependency_failed']);self.assertTrue(self.schedule(c)['eligible'])
 def test_recovery_reservation_can_be_overtaken_without_changing_lease(self):
  a,b=[self.submit(self.spec(x)) for x in 'ab'];row=self.claim(a);ops.finish(row,'failed',{'process_group_returned':True})
  rec=ops.operate('recover_job',{'id':a,'action':'claim'},self.owner,self.config)
  self.assertEqual(self.schedule(b)['reason'],'recovery_reservation')
  self.action('reorder',b,position='next');self.assertTrue(self.schedule(b)['eligible'])
  with ops.db() as db:self.assertEqual(recovery.view(db,a)['agent'],rec['agent'])
  self.action('reorder',b,position='tail');self.assertFalse(self.schedule(b)['eligible'])
  with ops.db() as db:db.execute('UPDATE recovery SET deadline=? WHERE job=?',(time.time()-1,a));recovery.expire(db)
  self.assertTrue(self.schedule(b)['eligible'])
 def test_held_preparation_failure_does_not_reserve_and_resume_is_not_repair(self):
  a,b=[self.submit(self.spec(x)) for x in 'ab'];self.action('hold',a)
  # Model failure of already-running preparation after a hold request.
  row=self.claim(a);ops.finish(row,'failed',{'process_group_returned':True})
  self.assertTrue(self.schedule(b)['eligible'])
  self.action('resume',a);self.assertEqual(self.row(a)['state'],'failed')
  self.assertEqual(self.schedule(b)['reason'],'recovery_reservation')
 def test_mutations_reject_fenced_instance_and_bad_inputs(self):
  a,b=[self.submit(self.spec(x)) for x in 'ab']
  for args in [{'position':'next','reference':b},{'position':'before'},{'position':'before','reference':a},{'position':'before','reference':'missing'},{'position':'tail','reason':[]}]:
   with self.subTest(args=args),self.assertRaises(ValueError):self.action('reorder',a,**args)
  with ops.db() as db:db.execute("UPDATE node_control SET mode='fenced'")
  for action,args in [('hold',{}),('resume',{}),('reorder',{'position':'tail'})]:
   with self.assertRaises(PermissionError):self.action(action,a,**args)
 def test_concurrent_order_changes_leave_unique_durable_order(self):
  jobs=[self.submit(self.spec(str(i))) for i in range(12)]
  with ThreadPoolExecutor(max_workers=6) as pool:list(pool.map(lambda job:self.action('reorder',job,position='next'),jobs))
  self.assertEqual(set(self.ids()),set(jobs))
  with ops.db() as db:
   ranks=[r[0] for r in db.execute('SELECT queue_order FROM jobs')];self.assertEqual(len(set(ranks)),len(jobs))
  ops.init();self.assertEqual(set(self.ids()),set(jobs))
 def test_inflight_claim_wins_lock_before_reorder_or_hold(self):
  a,b=[self.submit(self.spec(x)) for x in 'ab']
  # Hold the same BEGIN IMMEDIATE used by worker claims; callers must re-read
  # state after lock acquisition instead of applying a stale waiting snapshot.
  for action,args in [('reorder',{'position':'tail'}),('hold',{})]:
   with ops.db() as db:
    db.execute('BEGIN IMMEDIATE');db.execute("UPDATE jobs SET state='running' WHERE id=?",(a,))
    with ThreadPoolExecutor(max_workers=1) as pool:
     future=pool.submit(self.action,action,a,**args)
     time.sleep(.05);self.assertFalse(future.done());db.commit()
     self.assertFalse(future.result(3)['changed'])
   self.assertEqual(self.row(a)['queue_held'],0)
 def test_old_schema_migration_preserves_jobs_and_order(self):
  a=self.submit();rank=self.row(a)['queue_order']
  with ops.db() as db:db.execute('ALTER TABLE jobs DROP COLUMN queue_held');db.execute('DROP TABLE queue_history')
  ops.init();self.assertEqual(self.row(a)['queue_order'],rank);self.assertFalse(self.row(a)['queue_held'])
