"""Layout helpers for a host instance with separate credentials, config and SQLite."""
import json
import os
from pathlib import Path


def create_layout(root):
    root.mkdir(mode=0o700)
    for name in ('config','runtime','secrets','cache','logs'):
        (root/name).mkdir(mode=0o700)


def config_path(root):
    return root/'config/api.json'


def write_json(path,data):
    temp=path.with_name(path.name+'.tmp')
    fd=os.open(temp,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'w') as stream:
        json.dump(data,stream,indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    temp.replace(path)


def write_config(root,data):
    write_json(root/'config/api.json',data)
    write_json(root/'config/worker.json',{key:value for key,value in data.items() if key!='actors'})
