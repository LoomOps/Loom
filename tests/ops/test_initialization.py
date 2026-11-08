import multiprocessing as mp
from test_support import IsolatedOps
import ops
class InitializationTests(IsolatedOps):
 def test_api_and_worker_can_initialize_new_schema_concurrently(self):
  (self.local/'ops.sqlite3').unlink()
  children=[mp.get_context('fork').Process(target=ops.init) for _ in range(2)]
  for p in children:p.start()
  for p in children:p.join(10);self.assertEqual(p.exitcode,0)
  with ops.db() as c:self.assertIn('available_at',{r[1] for r in c.execute('pragma table_info(jobs)')})
