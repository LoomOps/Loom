"""Collect a read-only NVIDIA sample alongside the existing Docker snapshot."""
import csv
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

fields = ('index','name','utilization.gpu','memory.used','memory.total',
          'temperature.gpu','power.draw','power.limit')
keys = ('index','name','utilization_percent','used_MiB','total_MiB',
        'temperature_C','power_W','power_limit_W')
sample = {'source':'nvidia-smi', 'observed_at':datetime.now(timezone.utc).isoformat(),
          'status':'unavailable', 'data':None}
try:
    result = subprocess.run(['nvidia-smi','--query-gpu='+','.join(fields),
                             '--format=csv,noheader,nounits'],
                            check=True,capture_output=True,text=True,timeout=5)
    data = []
    for row in csv.reader(result.stdout.splitlines(),skipinitialspace=True):
        data.append({key:value if key=='name' else float(value) if not value.startswith('[') else None
                     for key,value in zip(keys,row,strict=True)})
    sample.update(status='ok',data=data)
except (OSError, subprocess.SubprocessError, ValueError):
    pass
path = Path(sys.argv[1])
with tempfile.NamedTemporaryFile(mode='w',dir=path.parent,delete=False) as out:
    json.dump(sample,out,allow_nan=False)
    tmp = out.name
os.replace(tmp,path)
