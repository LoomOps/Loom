"""Allowlisted Docker observations collected on the host and read by Ops."""
import argparse
import copy
import json
import os
import platform
import socket
import subprocess
import tempfile
from datetime import datetime, timezone
from pathlib import Path

FIELDS = """{"id":{{json .Id}},"name":{{json .Name}},"compose_project":{{json (index .Config.Labels "com.docker.compose.project")}},"compose_service":{{json (index .Config.Labels "com.docker.compose.service")}},"job_id":{{json (index .Config.Labels "io.loom.job-id")}},"image":{{json .Config.Image}},"image_id":{{json .Image}},"state":{{json .State.Status}},"health":{{with (index .State "Health")}}{{json .Status}}{{else}}"not_configured"{{end}},"started_at":{{json .State.StartedAt}},"finished_at":{{json .State.FinishedAt}},"exit_code":{{json .State.ExitCode}},"restart_count":{{json .RestartCount}}}"""
FIELDS = FIELDS[:-1] + ',"deployment_channel":{{json (index .Config.Labels "io.loom.channel")}}}'


def docker(prefix, *args):
    result = subprocess.run([*prefix, 'docker', *args], check=True,
                            capture_output=True, text=True, timeout=8)
    return result.stdout


def observation_path(local, name):
    return Path(os.environ.get("LOOM_OBSERVATIONS") or local)/name


def host_observation():
    """Host identity, never the dashboard container's hostname or bridge address."""
    host = {'hostname':socket.gethostname(), 'kernel':platform.release(),
            'os':platform.freedesktop_os_release().get('PRETTY_NAME'),
            'logical_cpus':os.cpu_count(), 'ip':None, 'lan_ips':[]}
    try:
        links = json.loads(subprocess.run(['ip','-j','-4','address','show','up'],
            capture_output=True, text=True, check=True, timeout=3).stdout)
        for link in links:
            addresses = [v['local'] for v in link.get('addr_info',[]) if v.get('scope')=='global']
            if link['ifname']=='tailscale0':
                host['ip'] = next(iter(addresses), None)
            elif not link['ifname'].startswith(('docker','br-','veth','lo')):
                host['lan_ips'].extend(addresses)
        host['ip_kind'] = 'Tailscale' if host['ip'] else 'LAN'
        host['ip'] = host['ip'] or next(iter(host['lan_ips']), None)
    except (OSError, ValueError, subprocess.SubprocessError):
        pass
    return host


def image_metadata(prefix, images):
    """A removed image must not discard valid rows from the same inspect batch."""
    fmt = '{"id":{{json .Id}},"digests":{{json .RepoDigests}}}'
    missing = set()
    try:
        output = docker(prefix, 'image', 'inspect', '--format', fmt, *images)
    except subprocess.CalledProcessError as error:
        # Docker emits valid records on stdout and one missing-image error per ID.
        output = error.stdout or ''
        stderr = error.stderr or ''
        missing = {identity for identity in images if 'No such image: '+identity in stderr}
        if not missing or any('No such image:' not in line for line in stderr.splitlines() if line):
            raise
    values = {v['id']:v['digests'] or [] for v in map(json.loads, output.splitlines())}
    return values, missing


def for_instance(snapshot, node, show_dev=None):
    """Presentation policy is independent of auth and applies before API readers."""
    if show_dev is None:
        try:
            policy = json.loads((node/'config/docker-observations.json').read_text())
            show_dev = policy.get('show_dev_builds') is True
        except (OSError, ValueError, AttributeError):
            show_dev = False
    result = copy.deepcopy(snapshot)
    result['visibility'] = {'show_dev_builds':show_dev}
    def development(row):
        project = row.get('compose_project') or ''
        return (row.get('deployment_channel') == 'dev-build' or project == 'loom-dev-build'
                or project.startswith('loom-dev-build-'))
    result['containers'] = [row for row in result['containers'] if show_dev or not development(row)]
    return result


def write_snapshot(path, result):
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as stream:
        temp = Path(stream.name)
        try:
            json.dump(result, stream)
            stream.flush()
            os.fsync(stream.fileno())
            temp.replace(path)
        finally:
            temp.unlink(missing_ok=True)


def collect(prefix, projects, include_other):
    snapshot = {'observed_at':None, 'status':'unavailable', 'engine':None, 'host':host_observation(),
                'errors':[], 'containers':[]}
    try:
        engine=json.loads(docker(prefix,'version','--format',
                                 '{"version":{{json .Server.Version}},"api_version":{{json .Server.APIVersion}}}'))
        if not engine['version']:
            raise ValueError('Docker server unavailable')
        snapshot['engine']=engine
        args=['ps','-aq','--no-trunc']
        owned_ids=[i for project in projects for i in
                   docker(prefix,*args,'--filter',f'label=com.docker.compose.project={project}').splitlines()]
        all_ids=docker(prefix,'ps','-aq','--no-trunc').splitlines() if include_other else []
        ids=list(dict.fromkeys(all_ids+owned_ids))
    except (OSError,ValueError,subprocess.SubprocessError):
        snapshot['errors'].append('daemon_unavailable')
    else:
        running=[]
        for batch_start in range(0,len(ids),32):
            batch=ids[batch_start:batch_start+32]
            try:
                lines=docker(prefix,'inspect','--type','container','--format',FIELDS,*batch).splitlines()
                for line in lines:
                    row=json.loads(line)
                    project=row['compose_project'] or None
                    owned=project in projects
                    if not owned and not include_other:
                        continue
                    row.update(name=row['name'].lstrip('/'), scope='loom' if owned else 'other',
                               compose_project=project,compose_service=row['compose_service'] or None,
                               job_id=(row['job_id'] or None) if owned else None,stats=None,image_digests=[])
                    if row['state'] not in {'exited','dead'}:
                        row['exit_code']=None
                    for field in ('started_at','finished_at'):
                        if row[field].startswith('0001-'):
                            row[field]=None
                    snapshot['containers'].append(row)
                    if row['state']=='running':
                        running.append(row)
            except (OSError,ValueError,subprocess.SubprocessError):
                snapshot['errors'].append('container_inspect_unavailable')
        images=list(dict.fromkeys(row['image_id'] for row in snapshot['containers']))
        for start in range(0,len(images),32):
            try:
                digests, missing=image_metadata(prefix, images[start:start+32])
                for row in snapshot['containers']:
                    if row['image_id'] in digests:
                        row['image_digests']=digests[row['image_id']]
                        row['image_status']='available'
                    elif row['image_id'] in missing:
                        row['image_status']='removed'
            except (OSError,ValueError,subprocess.SubprocessError):
                snapshot['errors'].append('image_inspect_unavailable')
        for start in range(0,len(running),32):
            group=running[start:start+32]
            try:
                lines=docker(prefix,'stats','--no-stream','--no-trunc','--format','{{json .}}',
                             *(row['id'] for row in group)).splitlines()
                stats={v['ID']:v for v in map(json.loads,lines)}
                for row in group:
                    v=stats.get(row['id'])
                    if v is None:
                        snapshot['errors'].append('container_stats_missing')
                        continue
                    row['stats']={key:v.get(source) for key,source in (
                        ('cpu_percent','CPUPerc'),('memory_usage','MemUsage'),('memory_percent','MemPerc'),
                        ('network_io','NetIO'),('block_io','BlockIO'))}
            except (OSError,ValueError,subprocess.SubprocessError):
                snapshot['errors'].append('container_stats_unavailable')
        snapshot['status']='partial' if snapshot['errors'] else 'ok'
    snapshot['errors']=list(dict.fromkeys(snapshot['errors']))
    snapshot['observed_at']=datetime.now(timezone.utc).isoformat()
    return snapshot


def read_status(path, *, allow_other=False, allow_job=lambda job: False):
    try:
        with path.open('rb') as stream:
            raw=stream.read(2*1024*1024+1)
        if len(raw)>2*1024*1024:
            raise ValueError('Docker snapshot too large')
        result=json.loads(raw)
        age=(datetime.now(timezone.utc)-datetime.fromisoformat(result['observed_at'])).total_seconds()
        if age>120 and result['status']!='unavailable':
            result['status']='stale'
        result['containers']=[row for row in result['containers'] if row['scope']=='loom' or allow_other]
        for row in result['containers']:
            if row['job_id'] and not allow_job(row['job_id']):
                row['job_id']=None
        return result
    except (OSError,ValueError,TypeError,KeyError):
        return {'observed_at':None,'status':'unavailable','engine':None,
                'errors':['snapshot_unavailable'],'containers':[]}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('node',type=Path)
    parser.add_argument('--project',action='append',required=True)
    parser.add_argument('--all-containers',action='store_true')
    parser.add_argument('--sudo',action='store_true')
    args=parser.parse_args()
    runtime=args.node/'runtime'
    if not runtime.is_dir():
        parser.error('Expected an installed node runtime')
    result=collect(['sudo','-n'] if args.sudo else [],args.project,args.all_containers)
    # Private host sample feeds independently filtered instance observations.
    write_snapshot(runtime/'docker-host-status.json', result)
    write_snapshot(runtime/'docker-status.json', for_instance(result, args.node))


if __name__=='__main__':
    main()
