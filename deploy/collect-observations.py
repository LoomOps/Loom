"""Root-owned host collector; publish only into its protected observation tree."""
import json
import os
from pathlib import Path
import subprocess
import sys
import docker_status
import host_metrics


def collect(root):
    config=json.loads((root/'services/observations.json').read_text())
    output=root/'observations'
    raw=docker_status.collect([], [x['project'] for x in config['instances'].values()], True)
    subprocess.run([sys.executable,'-B',str(root/'services/collect-dashboard-gpus.py'),
                    str(output/'dashboard-gpus.json')],check=True)
    gpu=json.loads((output/'dashboard-gpus.json').read_text())
    try: previous=json.loads((output/'dashboard-host-metrics.json').read_text())
    except (OSError,ValueError): previous=None
    host=host_metrics.collect(previous)
    docker_status.write_snapshot(output/'dashboard-host-metrics.json',host)
    for name, instance in config['instances'].items():
        target=output/name
        data=docker_status.for_instance(raw,Path('/unused'),show_dev=instance['show_dev_builds'])
        for row in data['containers']:
            row['scope']='loom' if row['compose_project']==instance['project'] else 'other'
            if row['scope']=='other': row['job_id']=None
        for filename,value in [('docker-status.json',data),('dashboard-gpus.json',gpu),('dashboard-host-metrics.json',host)]:
            path=target/filename
            docker_status.write_snapshot(path,value)
            os.chown(path,0,instance['gid']);path.chmod(0o640)

if __name__=='__main__': collect(Path(sys.argv[1]))
