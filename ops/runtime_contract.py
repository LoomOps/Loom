"""Immutable execution profile sealing for registered user-owned revisions."""
import hashlib
import json
import execution


def seal(profile):
    value={k:v for k,v in profile.items() if k!='sha256'}
    execution.validate_profile(value)
    value['sha256']=hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    return value
