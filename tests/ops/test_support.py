"""Temporary databases only; never connect a test to an installed instance."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import ops

class IsolatedOps(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory(prefix='loom-test-');self.addCleanup(tmp.cleanup)
        self.root=Path(tmp.name);self.local=self.root/'runtime';self.data=self.root/'data'
        self.local.mkdir();self.data.mkdir();(self.local/'returns').mkdir()
        for name,value in [('LOCAL',self.local),('BULK',self.data),('STOP',False)]:
            p=patch.object(ops,name,value);p.start();self.addCleanup(p.stop)
        self.owner={'id':'owner','owners':['owner'],'notify':['owner'],'scopes':['read','submit','cancel','events'],
            'token_sha256':hashlib.sha256(b'fixture-owner').hexdigest()}
        self.reader={'id':'reader','scopes':['read'],'owners':[],'notify':[],
            'token_sha256':hashlib.sha256(b'fixture-reader').hexdigest()}
        self.config={'machine_id':'fixture','storage_id':'fixture-data','inventory':{},'profiles':{},
            'actors':[self.owner,self.reader]}
        ops.init()
    def spec(self,key='job',**params):
        return {'key':key,'kind':'cpu.smoke','owner':'owner','notify':'owner',
            'params':{'seconds':0,'fail':False,**params}}
    def submit(self,spec=None,actor=None):
        with ops.db() as c:return ops.submit(c,actor or self.owner,spec or self.spec(),self.config)['id']
    def row(self,job):
        with ops.db() as c:return dict(c.execute('SELECT * FROM jobs WHERE id=?',(job,)).fetchone())
    def claim(self,job):
        # Worker claims are synthesized here; no scheduler or subprocess starts.
        with ops.db() as c:
            c.execute("UPDATE jobs SET state='running',attempt=? WHERE id=?",('attempt-'+job,job))
            c.execute("INSERT INTO attempts VALUES(?,?,'running',1,NULL)",('attempt-'+job,job))
        return self.row(job)
    def processing(self,row):
        with ops.db() as c:ops.mark_returned(c,row,{'ended':2,'execution':{'exit_code':0,'process_group_returned':True}})
        return {**self.row(row['id']),'retries':0}
