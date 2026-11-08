import hashlib
import json
from unittest.mock import patch
import access
import ops
from test_support import IsolatedOps


class AccessContractTests(IsolatedOps):
    def test_credential_expiration_revocation_and_scope(self):
        grant={'scopes':['read'],'owners':[],'notify':[]}
        with ops.db() as c, patch.object(access.time,'time',return_value=1000):
            issued={'credential_id':'old-issued-token','token':'fixture-old-token'}
            digest=hashlib.sha256(issued['token'].encode()).hexdigest()
            c.execute('INSERT INTO principals VALUES(?,?,?)',('guest','user',json.dumps(grant)))
            c.execute('INSERT INTO credentials VALUES(?,?,?,?,NULL)',(issued['credential_id'],'guest',digest,1060))
            digest=hashlib.sha256(issued['token'].encode()).hexdigest()
            actor=access.authenticate(c,[],digest)
            self.assertEqual(actor,{'id':'guest',**grant})
            stored=c.execute('SELECT digest FROM credentials').fetchone()[0]
            self.assertNotEqual(stored,issued['token'])
            with patch.object(access.time,'time',return_value=1061),self.assertRaises(PermissionError):
                access.authenticate(c,[],digest)
            c.execute('UPDATE credentials SET revoked=1001 WHERE id=?',(issued['credential_id'],))
            with self.assertRaises(PermissionError):access.authenticate(c,[],digest)

    def test_no_local_credential_issuance_entrypoint(self):
        self.assertFalse(hasattr(access,'issue'))

    def test_delivery_receipts_are_idempotent_and_owner_scoped(self):
        job=self.submit()
        with ops.db() as c:
            ops.event(c,self.row(job),'execution_failed')
            seq=c.execute('SELECT seq FROM events').fetchone()[0]
            args=(seq,'sent','receipt-1',0)
            with self.assertRaises(PermissionError):access.delivery(c,self.reader,*args)
            first=access.delivery(c,self.owner,*args)
            self.assertEqual(access.delivery(c,self.owner,*args),first)
            self.assertEqual(c.execute('SELECT count(*) FROM delivery_history').fetchone()[0],1)
            with self.assertRaises(ValueError):access.delivery(c,self.owner,seq,'completed','premature',1)
