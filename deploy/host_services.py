"""Install protected host helpers and system units, independent of a login session."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
from host_identity import directory, protected_chain

SOURCE=Path(__file__).resolve().parents[1]


def code(platform):
    target=directory(platform/'shared/services',0,0,0o755)
    protected_chain(target)
    for name,source in {
        'docker_status.py':SOURCE/'ops/docker_status.py',
        'host_metrics.py':SOURCE/'ops/host_metrics.py',
        'remote_fence.py':SOURCE/'ops/remote_fence.py',
        'collect-dashboard-gpus.py':SOURCE/'deploy/collect-dashboard-gpus.py',
        'collect-observations.py':SOURCE/'deploy/collect-observations.py',
    }.items():
        dest=target/name
        if dest.is_symlink():raise ValueError(f'Linked service source: {dest}')
        shutil.copyfile(source,dest);os.chown(dest,0,0);dest.chmod(0o644)
    return target


def unit(name,body):
    path=Path('/etc/systemd/system')/name
    if path.is_symlink():raise ValueError(f'Linked system unit: {path}')
    path.write_text(body);os.chown(path,0,0);path.chmod(0o644)


def install_observations(platform, instance, project, uid, gid, show_dev=False, *, activate=True):
    services=code(platform)
    config_file=services/'observations.json'
    config=json.loads(config_file.read_text()) if config_file.exists() else {'instances':{}}
    if '/' in project or project in ('.','..') or not project:raise ValueError('Invalid project')
    config['instances'][project]={'project':project,'gid':gid,'show_dev_builds':show_dev}
    config_file.write_text(json.dumps(config,indent=2)+'\n');config_file.chmod(0o600)
    directory(platform/'shared/observations',0,0,0o755)
    target=directory(platform/'shared/observations'/project,0,gid,0o750)
    env=instance/'observations.env'
    env.write_text(f'LOOM_OBSERVATIONS_HOST={target}\n');os.chown(env,uid,gid);env.chmod(0o600)
    unit('loom-observations.service',f'''[Unit]
Description=Loom host observations
After=docker.service
[Service]
Type=oneshot
UMask=0077
TimeoutStartSec=60
ExecStart=/usr/bin/python3 -B {services}/collect-observations.py {platform}/shared
''')
    unit('loom-observations.timer','''[Unit]
Description=Refresh Loom host observations
[Timer]
OnBootSec=10s
OnUnitInactiveSec=30s
AccuracySec=2s
[Install]
WantedBy=timers.target
''')
    if activate:
        subprocess.run(['systemctl','daemon-reload'],check=True)
        subprocess.run(['systemctl','enable','--now','loom-observations.timer'],check=True)
        subprocess.run(['systemctl','start','loom-observations.service'],check=True)


def install_authority(platform, bind, port, *, activate=True):
    services=code(platform);state=platform/'shared/authority'
    # Existing authority state/credentials are required; never initialize a grant here.
    for name in ('authority.sqlite3','policy.json','cert.pem','key.pem'):
        if not (state/name).is_file():raise ValueError(f'Missing authority state: {name}')
    unit('loom-authority.service',f'''[Unit]
Description=Loom handoff authority
After=network-online.target tailscaled.service
Wants=network-online.target
[Service]
Type=simple
User=loom
Group=loom
UMask=0077
Restart=on-failure
RestartSec=1
ExecStart=/usr/bin/python3 -B {services}/remote_fence.py {state}/authority.sqlite3 {state}/policy.json --bind {bind} --port {port} --tls-cert {state}/cert.pem --tls-key {state}/key.pem
[Install]
WantedBy=multi-user.target
''')
    if activate:
        subprocess.run(['systemctl','daemon-reload'],check=True)
        subprocess.run(['systemctl','enable','--now','loom-authority.service'],check=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--platform',type=Path,default=Path('/loom'))
    sub=p.add_subparsers(dest='action',required=True)
    obs=sub.add_parser('observations');obs.add_argument('instance',type=Path);obs.add_argument('--project',required=True);obs.add_argument('--uid',type=int,required=True);obs.add_argument('--gid',type=int,required=True);obs.add_argument('--show-dev',action='store_true')
    authority=sub.add_parser('authority');authority.add_argument('--bind',required=True);authority.add_argument('--port',type=int,default=19924)
    a=p.parse_args()
    if a.action=='observations':install_observations(a.platform,a.instance,a.project,a.uid,a.gid,a.show_dev)
    else:install_authority(a.platform,a.bind,a.port)
