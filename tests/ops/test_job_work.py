import json
import os
from pathlib import Path
import job_work
import ops
import dashboard_read
from test_support import IsolatedOps


class LogicalJobTests(IsolatedOps):
    def test_logical_timeline_uses_source_receipts_without_rewriting_attempts(self):
        sources=[self.completed('time-source-'+str(i)) for i in range(2)]
        job=self.submit({**self.spec('time-canonical'),'consolidates':sources})
        row=self.claim(job);spec=json.loads(row['spec'])
        spec['profile']={'sha256':'f'*64,'steps':[{'id':'first','label':'First source'}]}
        folder=self.data/'runs'/job/row['attempt'];folder.mkdir(parents=True)
        (folder/'steps.json').write_text(json.dumps({'schema':'loom-job-steps/v1','profile_sha256':'f'*64,
            'steps':[{'id':'first','state':'reused','source_job':sources[0],'source_attempt':'attempt-'+sources[0]}]}))
        with ops.db() as c:
            for identity,start,end in [(sources[0],100,200),(sources[1],250,400),(job,500,530)]:
                c.execute('UPDATE jobs SET created=? WHERE id=?',(start-10,identity))
                c.execute('UPDATE attempts SET started=?,ended=? WHERE job=?',(start,end,identity))
            c.execute("UPDATE jobs SET state='complete',spec=?,result=? WHERE id=?",(json.dumps(spec),json.dumps({'artifact_ready':True}),job))
            before=[tuple(a) for a in c.execute('SELECT * FROM attempts ORDER BY id')]
        detail=ops.operate('get_job',{'id':job},self.reader,self.config)
        self.assertEqual(detail['work_timeline']['started'],100)
        self.assertEqual(detail['work_timeline']['ended'],530)
        self.assertEqual(detail['attempts'][0]['started'],500)
        self.assertEqual(detail['internal_steps']['steps'][0]['source_execution']['ended'],200)
        with ops.db() as c:
            entities=dashboard_read.projection(c,self.reader,self.config,self.local,self.data)[0]['entities']
            entity=next(e for e in entities if e['id']==job)
            self.assertEqual(entity['properties']['started_at'],100)
            self.assertEqual(entity['properties']['queued_at'],90)
            self.assertEqual(entity['properties']['process_exited_at'],530)
            self.assertEqual(len(entity['execution_intervals']),3)
            self.assertEqual(len(entity['steps']),3)
            self.assertEqual(entity['steps'][0]['actual_started_at'],100)
            self.assertEqual(entity['steps'][0]['source_attempt'],'attempt-'+sources[0])
            self.assertEqual(entity['attempts'][0]['started'],500)
            self.assertEqual([tuple(a) for a in c.execute('SELECT * FROM attempts ORDER BY id')],before)
            c.execute("UPDATE jobs SET state='running' WHERE id=?",(job,))
            running=c.execute('SELECT * FROM jobs WHERE id=?',(job,)).fetchone()
            self.assertIsNone(job_work.timeline(c,running,spec)['ended'])

    def completed(self,key):
        job=self.submit(self.spec(key));row=self.claim(job)
        with ops.db() as c:
            c.execute("UPDATE jobs SET state='complete',result=? WHERE id=?",(json.dumps({'artifact_ready':True,'entities':[]}),job))
            c.execute("UPDATE attempts SET state='complete',ended=2 WHERE job=?",(job,))
        return job

    def test_consolidation_preserves_originals_until_verified_and_can_be_audited(self):
        sources=[self.completed('source-'+str(i)) for i in range(2)]
        before=[self.row(i) for i in sources]
        spec={**self.spec('canonical'),'consolidates':sources}
        job=self.submit(spec)
        self.assertEqual(self.submit(spec),job)
        with self.assertRaises(ValueError):self.submit({**spec,'consolidates':sources[::-1]})
        self.assertEqual(len(ops.operate('list_jobs',{},self.owner,self.config)),3)
        detail=ops.operate('get_job',{'id':job},self.owner,self.config)
        self.assertEqual([s['job'] for s in detail['consolidation']['sources']],sources)
        self.assertNotIn('_consolidation_sources',detail['spec'])
        self.assertTrue(set(sources)<=set(detail['spec']['depends_on']))
        self.claim(job)
        with ops.db() as c:c.execute("UPDATE jobs SET state='complete',result=? WHERE id=?",(json.dumps({'artifact_ready':True}),job))
        self.assertEqual([r['id'] for r in ops.operate('list_jobs',{},self.reader,self.config)],[job])
        self.assertEqual(len(ops.operate('list_jobs',{'include_consolidated':True},self.reader,self.config)),3)
        for i,old in zip(sources,before):
            self.assertEqual(self.row(i),old)
            self.assertEqual(ops.operate('get_job',{'id':i},self.reader,self.config)['consolidation']['consolidated_into'],job)
        with self.assertRaises(ValueError):self.submit({**self.spec('duplicate'),'consolidates':sources})

    def test_reject_live_unverified_and_different_owner_sources_without_submission(self):
        live=self.submit(self.spec('live'))
        with self.assertRaises(ValueError):self.submit({**self.spec('a'),'consolidates':[live]})
        with ops.db() as c:c.execute("UPDATE jobs SET state='complete' WHERE id=?",(live,))
        with self.assertRaises(ValueError):self.submit({**self.spec('a'),'consolidates':[live]})
        source=self.completed('done')
        with ops.db() as c:c.execute("UPDATE jobs SET owner='someone-else' WHERE id=?",(source,))
        with self.assertRaises(PermissionError):self.submit({**self.spec('a'),'consolidates':[source]})
        for ids in [[],[live,live],['../outside'],[None],'bad']:
            with self.subTest(ids=ids),self.assertRaises(ValueError):self.submit({**self.spec('b'),'consolidates':ids})
        with self.assertRaises(ValueError):ops.operate('list_jobs',{'include_consolidated':'true'},self.reader,self.config)

    def test_failed_canonical_never_hides_or_reexecutes_sources(self):
        source=self.completed('source')
        job=self.submit({**self.spec('canonical'),'consolidates':[source]})
        with ops.db() as c:c.execute("UPDATE jobs SET state='failed' WHERE id=?",(job,))
        self.assertEqual(len(ops.operate('list_jobs',{},self.reader,self.config)),2)
        with self.assertRaises(ValueError):self.submit({**self.spec('second'),'consolidates':[source]})
        self.assertEqual(self.row(source)['state'],'complete')

    def test_pinned_steps_report_is_bounded_and_cannot_override_execution(self):
        spec={'profile':{'sha256':'f'*64,'steps':[{'id':'verify','label':'Verify preserved source'}]},
              '_consolidation_sources':[{'job':'a'*32,'attempt':'original'}]}
        folder=self.data/'progress';folder.mkdir()
        self.assertEqual(job_work.progress(folder,spec)['status'],'planned')
        report={'schema':'loom-job-steps/v1','profile_sha256':'f'*64,'steps':[{'id':'verify','state':'reused','source_job':'a'*32,'source_attempt':'original'}]}
        path=folder/'steps.json';path.write_text(json.dumps(report))
        self.assertEqual(job_work.progress(folder,spec)['steps'][0]['state'],'reused')
        for bad in [{**report,'profile_sha256':'wrong'},{**report,'steps':report['steps']*2},
                    {**report,'steps':[dict(report['steps'][0],source_attempt='different')]},
                    {**report,'steps':[{'id':'unplanned','state':'completed'}]}]:
            path.write_text(json.dumps(bad));self.assertEqual(job_work.progress(folder,spec)['status'],'unavailable')
        path.write_bytes(b'x'*(job_work.LIMIT+1));self.assertEqual(job_work.progress(folder,spec)['status'],'unavailable')
        path.unlink();path.symlink_to('/etc/passwd');self.assertEqual(job_work.progress(folder,spec)['status'],'unavailable')
        path.unlink();os.mkfifo(path);self.assertEqual(job_work.progress(folder,spec)['status'],'unavailable')
        with self.assertRaises(ValueError):job_work.validate_steps(spec['profile']['steps']*2)
        job_work.validate_steps(spec['profile']['steps'])

    def test_started_job_detail_reads_progress_from_pinned_output_store(self):
        for selector in ('data/0','data/1'):
            with self.subTest(storage=selector):
                root=self.data if selector=='data/0' else self.root/'secondary'
                root.mkdir(exist_ok=True)
                identity='fixture-data' if selector=='data/0' else 'secondary-data'
                (root/'.loom-storage.json').write_text(json.dumps({'storage_id':identity}))
                self.config['data_stores']={'data/1':{'path':str(root),'storage_id':identity,'mode':'rw'}} if selector=='data/1' else {}
                job=self.submit({**self.spec('progress-'+selector[-1]),'output_storage':selector})
                row=self.claim(job);spec=json.loads(row['spec'])
                spec['profile']={'sha256':'f'*64,'steps':[{'id':'verify','label':'Verify'}]}
                with ops.db() as c:c.execute('UPDATE jobs SET spec=? WHERE id=?',(json.dumps(spec),job))
                folder=root/'runs'/job/row['attempt'];folder.mkdir(parents=True)
                report={'schema':'loom-job-steps/v1','profile_sha256':spec['profile']['sha256'],
                        'steps':[{'id':'verify','state':'completed'}]}
                (folder/'steps.json').write_text(json.dumps(report))
                detail=ops.operate('get_job',{'id':job},self.reader,self.config)
                self.assertEqual(detail['state'],'running')
                self.assertEqual(detail['internal_steps']['status'],'reported')
                self.assertEqual(detail['internal_steps']['steps'][0]['state'],'completed')
