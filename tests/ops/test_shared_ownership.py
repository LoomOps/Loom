import json
import importlib.util
from pathlib import Path
import ops
import ownership
from test_support import IsolatedOps


class SharedOwnershipTests(IsolatedOps):
    def setUp(self):
        super().setUp()
        self.other={**self.owner,'id':'colleague','owners':['colleague'],'notify':['colleague']}
        with ops.db() as c:
            self.person=ownership.register(c,'tester@example.test','Test User')
            self.second=ownership.register(c,'colleague@example.test','Colleague')

    def test_owner_change_does_not_rewrite_execution_or_notify(self):
        job=self.submit();before=self.row(job)
        change={'id':job,'owner':self.person['id']}
        result=ops.operate('jobs',{'action':'assign_owner','arguments':change},self.other,self.config)
        self.assertEqual(result['owner_profile']['display_name'],'Test User')
        self.assertEqual(self.row(job),before)
        view=ops.operate('get_job',{'id':job},self.other,self.config)
        self.assertEqual(view['owner'],self.person['id'])
        self.assertEqual(view['submitter'],'owner');self.assertEqual(view['spec']['owner'],'owner')
        ops.operate('jobs',{'action':'assign_owner','arguments':change},self.owner,self.config)
        with ops.db() as c:self.assertEqual(c.execute('SELECT count(*) FROM ownership_history').fetchone()[0],1)
        with self.assertRaises(PermissionError):ops.operate('assign_job_owner',change,self.reader,self.config)
        ops.operate('cancel_job',{'id':job},self.other,self.config)
        self.assertEqual(self.row(job)['state'],'cancelled')
        with ops.db() as c:self.assertEqual(c.execute('SELECT target FROM events WHERE job=?',(job,)).fetchone()[0],'owner')

    def test_owner_projection_for_registry_and_immutable_history(self):
        job=self.submit()
        manifest={'id':'fixture.model','kind':'model','components':[],'metrics':[],'provenance':{}}
        raw=json.dumps({'original':{'id':'old.model','kind':'model','properties':{}}})
        with ops.db() as c:
            c.execute('INSERT INTO entity_revisions VALUES(?,?,?,?,?,?,?)',('fixture.model',1,job,'model','owner','owner',json.dumps(manifest)))
            c.execute('INSERT INTO history_records VALUES(?,?,?,?,?,?)',('catalog','entity','old.model','1','fixture',raw))
            for kind,identity in ownership.inventory(c):ownership.assign(c,kind,identity,self.person['id'],'operator')
            self.assertEqual(c.execute('SELECT body FROM history_records').fetchone()[0],raw)
        record=ops.operate('get_entity',{'id':'fixture.model','revision':1},self.other,self.config)
        self.assertEqual(record['owner'],self.person['id'])
        self.assertEqual(record['owner_profile']['login'],'tester@example.test')
        updated=ops.operate('results',{'action':'assign_owner','arguments':{'id':'old.model','owner':self.second['id']}},self.other,self.config)
        self.assertEqual(updated['owner'],self.second['id'])
        historical=ops.operate('get_history',{'source':'catalog','kind':'entity','id':'old.model','revision':'1'},self.other,self.config)
        self.assertEqual(historical['body'],json.loads(raw))

    def test_profile_identity_is_stable_and_distinct_from_unix_account(self):
        with ops.db() as c:
            again=ownership.register(c,'TESTER@example.test','Updated Name')
            self.assertEqual(again['id'],self.person['id'])
            self.assertNotEqual(again['id'],'loom')
            self.assertEqual(ownership.profile(c,again['id'])['display_name'],'Updated Name')

    def test_submitter_can_assign_another_registered_owner(self):
        actor={**self.owner,'profile_id':self.person['id']}
        job=self.submit({**self.spec(),'owner':self.second['id']},actor)
        result=ops.operate('get_job',{'id':job},actor,self.config)
        self.assertEqual(result['owner'],self.second['id'])
        self.assertEqual(result['submitter'],'owner')

    def test_migration_preserves_original_records_and_is_repeatable(self):
        job=self.submit()
        instance=self.root/'instance';(instance/'config').mkdir(parents=True)
        (instance/'runtime').symlink_to(self.local,target_is_directory=True)
        for name in ('api','worker'):
            (instance/'config'/f'{name}.json').write_text(json.dumps(self.config))
        spec=importlib.util.spec_from_file_location('assign_owner',Path(__file__).resolve().parents[2]/'deploy/assign-owner.py')
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        before=self.row(job)
        result=module.migrate(instance,'tester@example.test','Test User',self.root/'backup1')
        self.assertEqual(result['assigned']['job'],1);self.assertEqual(self.row(job),before)
        again=module.migrate(instance,'tester@example.test','Test User',self.root/'backup2')
        self.assertEqual(again['assigned']['job'],0)
        self.assertEqual(result['installation_id'],again['installation_id'])
        config=json.loads((instance/'config/api.json').read_text())
        self.assertEqual(config['machine_id'],'fixture')
        self.assertEqual(config['actors'][0]['profile_id'],self.person['id'])
