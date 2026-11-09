"""Explicit idle-instance update/rollback. Never replaces data, profiles or components."""
import argparse,contextlib,fcntl,hashlib,json,os,shutil,sqlite3,subprocess,time
from pathlib import Path
from tailnet import env_values,write
import guide_bundle
import migrations
import state_snapshot

ACTIVE=('preparing','running','processing')

def run(*args,**kw):return subprocess.check_output([str(a) for a in args],text=True,stderr=subprocess.PIPE,**kw).strip()
def sql(path):return sqlite3.connect(f'file:{path}?mode=rw',uri=True,timeout=15)
def digest_db(path):
 with sql(path) as c:return hashlib.sha256('\n'.join(line for line in c.iterdump() if not line.startswith('INSERT INTO "node_control"')).encode()).hexdigest()
def backup_db(source,destination):
 with sql(source) as a,sqlite3.connect(destination) as b:a.backup(b)
 os.chmod(destination,0o600)

def idle_gate(path):
 """Same SQLite transaction as dispatch: no check-then-fence race."""
 with sql(path) as c:
  c.execute('BEGIN IMMEDIATE')
  if c.execute("SELECT 1 FROM jobs WHERE state IN ('preparing','running','processing') LIMIT 1").fetchone():raise ValueError('Active Jobs: wait for execution and result handling to finish')
  if c.execute("SELECT name FROM sqlite_master WHERE name='hooks'").fetchone() and c.execute("SELECT 1 FROM hooks WHERE state='pending' LIMIT 1").fetchone():raise ValueError('Pending lifecycle callbacks must finish before update')
  mode=c.execute('SELECT mode FROM node_control WHERE id=1').fetchone()[0]
  if mode!='active':raise ValueError('Instance is already fenced: '+mode)
  c.execute("UPDATE node_control SET mode='maintenance' WHERE id=1")

def ungate(path):
 with sql(path) as c:c.execute("UPDATE node_control SET mode='active' WHERE id=1 AND mode='maintenance'")

def inspect(root,release):
 root,release=Path(root),Path(release)
 if (root/'development').exists():raise ValueError('Release updater requires an image-backed instance; dev uses source mounts')
 info=json.loads((release/'result.json').read_text());manifest=json.loads((release/'manifest.json').read_text())
 if info['status']!='built' or info['build_id']!=manifest['build_id']:raise ValueError('Completed matching release required')
 for rel,digest in manifest['files'].items():
  p=release/'source'/rel
  if Path(rel).is_absolute() or '..' in Path(rel).parts or not p.resolve().is_relative_to((release/'source').resolve()) or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:raise ValueError('Release source mismatch: '+rel)
 for role,tag in info['images'].items():
  if run('docker','image','inspect',tag,'--format','{{.Id}}')!=info['image_ids'][role]:raise ValueError('Release image mismatch: '+role)
 for name in ['compose.env','network.env','build.env']:
  if not (root/name).is_file():raise ValueError('Missing installed configuration: '+name)
 # Reject unsupported authentication configuration before changing services.
 p=root/'secrets/sso/config.json'
 if p.exists() and json.loads(p.read_text()).get('mode')!='network':raise ValueError('Configure native Tailscale bindings before updating; see docs/TAILSCALE-SSO.md')
 db_path=root/'runtime/ops.sqlite3'
 if db_path.exists():
  with sql(db_path) as c:
   for row in c.execute("SELECT id,spec FROM jobs WHERE state NOT IN ('complete','cancelled')"):
    spec=json.loads(row[1])
    if spec.get('kind')!='cpu.smoke' and spec.get('profile',{}).get('schema')!='loom-execution/v2':
     raise ValueError('Pending Job uses an unsupported profile: '+row[0]+'; finish or cancel it before updating')
 effective=json.loads(run('bash',release/'source/deploy/compose.sh','config','--format','json',env={**os.environ,'LOOM_INSTANCE':str(root),'LOOM_PROJECT':'loom-update-check','LOOM_BUILD_ENV':str(release/'build.env')}))
 for role in ('ops','dashboard'):
  if role in info['images'] and effective['services']['loom-'+role]['image']!=info['images'][role]:raise ValueError('User override selects a different '+role+' image')
 if 'loom-gateway' in effective['services'] and effective['services']['loom-gateway']['image']!=info['images']['dashboard']:raise ValueError('Gateway must use the matching Dashboard image')
 return info

def compose(source,root,project,*args):
 return run('bash',source/'deploy/compose.sh',*args,env={**os.environ,'LOOM_INSTANCE':str(root),'LOOM_PROJECT':project})

def check_entry(root):
 """Verify the installed user entry, not just healthy containers."""
 from urllib.request import urlopen
 from urllib.parse import urlsplit
 from tailnet import expected_routes
 root=Path(root)
 required=['config/tailnet-plan.json','config/access-plan.json','secrets/sso/config.json','secrets/admission/config.json']
 if any(not (root/name).is_file() for name in required):raise OSError('Incomplete native identity/Gateway configuration')
 tail=json.loads((root/required[0]).read_text());access=json.loads((root/required[1]).read_text())
 sso=json.loads((root/required[2]).read_text());admission=json.loads((root/required[3]).read_text())
 private=urlsplit(access['dashboard_origin']);public=urlsplit(access['origin'])
 if private.netloc==public.netloc or (private.port or 443)!=tail['https_port']:raise OSError('Gateway/private port mismatch')
 if sso.get('origin')!=access['dashboard_origin'] or not sso.get('network_users'):raise OSError('Dashboard native identity mismatch')
 if not admission.get('separate_gateway') or any(admission.get(k)!=access[k] for k in ['origin','dashboard_origin','machine_id']):raise OSError('Gateway runtime configuration mismatch')
 env={**env_values(root/'compose.env'),**env_values(root/'network.env')}
 with urlopen('http://127.0.0.1:'+env['OPS_DASHBOARD_PORT']+'/session',timeout=3) as r:session=json.load(r)
 if session.get('sso') is not True or session.get('entry_url')!=access['origin']:raise OSError('Running Dashboard does not use configured Gateway/SSO')
 with urlopen('http://127.0.0.1:'+str(access['host_port'])+'/api/session',timeout=3) as r:gateway=json.load(r)
 if gateway.get('machine')!=access['machine_id']:raise OSError('Wrong Gateway installation')
 state=json.loads(run('tailscale','serve','status','--json'))
 pub=f'{public.hostname}:{public.port or 443}';priv=f'{private.hostname}:{private.port or 443}'
 if state.get('Web',{}).get(pub,{}).get('Handlers')!={'/':{'Proxy':'http://127.0.0.1:'+str(access['host_port'])}}:raise OSError('Public Gateway routes expose the wrong service')
 handlers=state.get('Web',{}).get(priv,{}).get('Handlers',{})
 if any(handlers.get(path,{}).get('Proxy')!=target for path,target in expected_routes(root).items()):raise OSError('Private Dashboard/API relay routes are incomplete')
 if not state.get('AllowFunnel',{}).get(pub) or state.get('AllowFunnel',{}).get(priv):raise OSError('Gateway/private Funnel boundary mismatch')

def check_health(source,root,project,expected=None):
 from urllib.request import urlopen,Request
 env={**env_values(root/'compose.env'),**env_values(root/'network.env')}
 failure='No readiness response'
 for _ in range(45):
  try:
   token=json.loads((root/'secrets/client-tokens.json').read_text())['owner']
   requests=[Request('http://127.0.0.1:'+env['OPS_PORT']+'/api/status',headers={'Authorization':'Bearer '+token}),Request('http://127.0.0.1:'+env['OPS_DASHBOARD_PORT']+'/healthz')]
   for request in requests:
    with urlopen(request,timeout=2) as r:
     if r.status!=200:raise OSError('Health check failed')
     if request.full_url.endswith('/api/status'):
      status=json.load(r)
      if expected and status['build']['build_id']!=expected:raise OSError('Unexpected running build')
   if not (root/'development').exists():check_entry(root)
   if not (root/'development').exists():
    import storage_parity
    try:storage_parity.inspect(root,project,status)
    except (OSError,ValueError,KeyError,subprocess.SubprocessError) as error:raise OSError('Storage parity check failed: '+type(error).__name__) from None
   if (root/'config/ssh.json').exists():
    from ssh_setup import doctor as ssh_doctor
    if not ssh_doctor(root)['ready']:raise OSError('Managed SSH readiness failed')
   return
  except OSError as error:failure=str(error);time.sleep(1)
 raise RuntimeError('Updated services failed readiness checks: '+failure)

def helper_files(platform,source):
 result={}
 for name in ('server.py','runtime_policy.py'):
  dest=platform/'shared/executor'/name
  if dest.exists():result[dest]=source/'executor'/name
 for name in ('docker_status.py','host_metrics.py','remote_fence.py','collect-dashboard-gpus.py','collect-observations.py'):
  dest=platform/'shared/services'/name
  if dest.exists():result[dest]=source/('deploy' if name.startswith('collect-') else 'ops')/name
 dest=Path('/usr/local/lib/loom-ssh/ssh_host.py')
 if (platform/'ops/config/ssh.json').exists() and dest.exists() and (source/'deploy/ssh_host.py').is_file():result[dest]=source/'deploy/ssh_host.py'
 return result

def restart_ssh(root):
 p=root/'config/ssh.json'
 if p.exists():
  from ssh_setup import configure_service,paths
  state,_,name=paths(root);configure_service(state,name)
  run('systemctl','daemon-reload');run('systemctl','restart',name)

def instance_paths(platform,root):
 # Shared helpers affect every local instance; fence each before changing them.
 paths={root}
 ids=run('docker','ps','-q','--filter','label=com.docker.compose.service=loom-ops').split()
 if ids:
  for item in json.loads(run('docker','inspect',*ids)):
   for mount in item.get('Mounts',[]):
    if mount.get('Destination')=='/loom/ops':paths.add(Path(mount['Source']).parent)
 return sorted(paths)

def previous_source(root,platform):
 version=env_values(Path(root)/'build.env').get('LOOM_BUILD_ID','')
 if not version or '/' in version or version in ('.','..'):raise ValueError('Installed version identity is missing')
 for candidate in [Path(platform)/'distribution'/version,Path(platform)/'builds/alpha'/version/'source']:
  if (candidate/'deploy/compose.sh').is_file():return candidate
 raise ValueError('Keep the installed release source under distribution/<version> before updating')

def plan(root,release,mode='update'):
 if mode not in ('update','rebuild'):raise ValueError('Unknown upgrade mode')
 info=inspect(root,release)
 return {**info,'mode':mode,'upgrade':migrations.plan(root,release)}

def apply(root,release,project,platform=Path('/loom'),mode='update'):
 if mode not in ('update','rebuild'):raise ValueError('Unknown upgrade mode')
 root,release,platform=Path(root),Path(release),Path(platform);info=inspect(root,release)
 upgrade=migrations.plan(root,release)
 installed=previous_source(root,platform)
 if (platform/'distribution'/info['build_id']).exists():raise ValueError('Distribution version already installed')
 archive=platform/'updates'/str(time.time_ns());archive.mkdir(parents=True,mode=0o700)
 with (platform/'updates/.lock').open('a') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
  # Another administrator may have finished an update before this lock arrived.
  info=inspect(root,release);upgrade=migrations.plan(root,release);installed=previous_source(root,platform)
  paths=instance_paths(platform,root) if helper_files(platform,release/'source') else [root];gated=[];record={'schema':'loom-update/v1','root':str(root),'project':project,'release':str(release),'build':info['build_id'],'mode':mode,'upgrade':upgrade,'state':'preparing','gated':[],'helpers':{},'files':{},'databases':{}}
  guides=platform/'loom_use'
  record['guides']={'parent':str(guides),'previous':guide_bundle.selected(guides),'version':info['build_id']}
  guide_bundle.install(guide_bundle.generate(info['images']['ops']),guides,info['build_id'])
  src=release/'source';old_source=archive/'previous-source'
  shutil.copytree(installed,old_source,ignore=shutil.ignore_patterns('.git','evidence','legacy','__pycache__'))
  record['previous_source']=str(old_source)
  try:
   for instance in paths:
    db=instance/'runtime/ops.sqlite3';idle_gate(db);gated.append(db);record['gated']=[str(x) for x in gated];write(archive/'record.json',record)
   ex=platform/'shared/executor/executor.sqlite3'
   if ex.exists():
    with sql(ex) as c:
     if c.execute("SELECT 1 FROM tickets WHERE state!='returned' LIMIT 1").fetchone():raise ValueError('Executor has unreturned tickets')
   # Stop writers before taking snapshots or replacing shared helper code.
   record['state']='backing_up';write(archive/'record.json',record)
   compose(old_source,root,project,'stop')
   helpers=helper_files(platform,src)
   if helpers:run('systemctl','stop','loom-executor.service','loom-observations.timer','loom-observations.service')
   db=root/'runtime/ops.sqlite3';dest=archive/'ops.sqlite3';backup_db(db,dest);record['databases'][str(db)]=str(dest)
   preserved=state_snapshot.capture(root,archive,platform);record['preserved_state']=str(preserved)
   for path in state_snapshot.databases(root):
    record['databases'].setdefault(str(path),str(preserved/'instance'/path.relative_to(root)))
   for dest,new in helpers.items():
    backup=archive/('helper-'+dest.name);shutil.copy2(dest,backup);record['helpers'][str(dest)]=str(backup)
   for rel in ['build.env']:
    dest=root/rel;backup=archive/rel;shutil.copy2(dest,backup);record['files'][str(dest)]=str(backup)
   record['state']='applying';write(archive/'record.json',record)
   record['migrations']=migrations.apply(upgrade,root,release,platform)
   write(archive/'record.json',record)
   dest=platform/'distribution'/info['build_id']
   if dest.exists():raise ValueError('Distribution version already installed')
   shutil.copytree(src,dest)
   for target,new in helpers.items():shutil.copyfile(new,target);os.chmod(target,0o644)
   st=(root/'build.env').stat();write(root/'build.env',(release/'build.env').read_text(),st.st_uid,st.st_gid)
   # Existing boot unit follows the newly installed, immutable Compose source.
   unit=Path('/etc/systemd/system/loom-stack.service')
   if unit.exists():
    backup=archive/'loom-stack.service';shutil.copy2(unit,backup);record['files'][str(unit)]=str(backup);write(archive/'record.json',record)
    import re
    unit.write_text(re.sub(r'/\S+/deploy/compose\.sh',str(dest/'deploy/compose.sh'),unit.read_text()));run('systemctl','daemon-reload')
   if helpers:run('systemctl','start','loom-executor.service','loom-observations.timer')
   restart_ssh(root)
   args=['up','-d','--no-build']+(['--force-recreate'] if mode=='rebuild' else [])
   compose(dest,root,project,*args);check_health(dest,root,project,info['build_id'])
   record['preservation']=state_snapshot.verify(root,preserved)
   guide_bundle.select(guides,info['build_id'])
   record.update(state='complete',source=str(dest),previous_source=str(old_source))
  except BaseException:
   if record['state']=='preparing':
    for db in gated:ungate(db)
   record['state']='failed';write(archive/'record.json',record)
   # Leave explicit recovery evidence; never restart a failed update silently.
   raise
  finally:
   # Fingerprint while still fenced. New Jobs accepted after ungating must never
   # become part of the rollback baseline and then disappear on restoration.
   record['post_databases']={db:digest_db(Path(db)) for db in record['databases']};write(archive/'record.json',record)
   record['post_files']=state_snapshot.file_inventory(root)
   record['post_extensions']=state_snapshot.extension_inventory(record.get('preserved_state'))
   record['post_managed_files']={target:hashlib.sha256(Path(target).read_bytes()).hexdigest() for group in ('files','helpers') for target in record[group]}
   write(archive/'record.json',record)
   for db in gated:ungate(db) if record['state']=='complete' or record['state']=='preparing' else None
  return {'updated':info['build_id'],'mode':mode,'record':str(archive/'record.json'),'preserved':'accounts, tokens, installation identity, overrides, components, tools, profiles, history, data'}

def resume(record_path):
 """Resume an owned failed update without issuing another installation identity."""
 path=Path(record_path);record=json.loads(path.read_text())
 if record['state']=='complete':return {'updated':record['build'],'already_complete':True,'record':str(path)}
 if record['state']!='failed' or not record.get('preserved_state'):raise ValueError('No resumable preservation snapshot; use rollback')
 root=Path(record['root']);release=Path(record['release']);platform=path.parent.parent.parent;project=record['project']
 with (platform/'updates/.lock').open('a') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
  for db,digest in record['post_databases'].items():
   if digest_db(Path(db))!=digest:raise ValueError('Database changed after failure; review recovery before resuming')
  if state_snapshot.file_inventory(root)!=record['post_files']:raise ValueError('Installation changed after failure; review recovery before resuming')
  if state_snapshot.extension_inventory(record.get('preserved_state'))!=record.get('post_extensions',{}):raise ValueError('Extensions changed after failure; review recovery before resuming')
  for name in record['gated']:
   with sql(Path(name)) as c:
    if c.execute('SELECT mode FROM node_control WHERE id=1').fetchone()[0]!='maintenance':raise ValueError('Resume requires the retained maintenance fence')
  info=inspect(root,release);src=release/'source';dest=platform/'distribution'/info['build_id']
  compose(Path(record['previous_source']),root,project,'stop')
  try:
   record['migrations']=migrations.apply(record['upgrade'],root,release,platform)
   if dest.exists():
    for rel,digest in json.loads((release/'manifest.json').read_text())['files'].items():
     if hashlib.sha256((dest/rel).read_bytes()).hexdigest()!=digest:raise ValueError('Incomplete or modified release copy; use rollback')
   else:shutil.copytree(src,dest)
   for target,new in helper_files(platform,src).items():shutil.copyfile(new,target);os.chmod(target,0o644)
   owner=(root/'build.env').stat();write(root/'build.env',(release/'build.env').read_text(),owner.st_uid,owner.st_gid)
   unit=Path('/etc/systemd/system/loom-stack.service')
   if unit.exists():
    import re
    unit.write_text(re.sub(r'/\S+/deploy/compose\.sh',str(dest/'deploy/compose.sh'),unit.read_text()));run('systemctl','daemon-reload')
   if record['helpers']:run('systemctl','start','loom-executor.service','loom-observations.timer')
   restart_ssh(root)
   compose(dest,root,project,'up','-d','--no-build',*(['--force-recreate'] if record.get('mode')=='rebuild' else []))
   check_health(dest,root,project,info['build_id'])
   record['preservation']=state_snapshot.verify(root,record['preserved_state'])
   guide_bundle.select(record['guides']['parent'],record['guides']['version'])
   record.update(state='complete',source=str(dest));write(path,record)
  finally:
   record['post_databases']={db:digest_db(Path(db)) for db in record['databases']}
   record['post_files']=state_snapshot.file_inventory(root)
   record['post_extensions']=state_snapshot.extension_inventory(record.get('preserved_state'))
   record['post_managed_files']={target:hashlib.sha256(Path(target).read_bytes()).hexdigest() for group in ('files','helpers') for target in record[group]}
   write(path,record)
  if record['state']=='complete':
   for name in record['gated']:ungate(Path(name))
 return {'updated':record['build'],'resumed':True,'record':str(path)}

def rollback(record_path):
 path=Path(record_path)
 with (path.parent.parent/'.lock').open('a') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
  return _rollback(path)

def _rollback(record_path):
 path=Path(record_path);record=json.loads(path.read_text());root=Path(record['root']);project=record['project'];source=Path(record.get('source',record['release']+'/source'))
 if record['state'] not in ('complete','failed'):raise ValueError('Update record is not rollback-ready')
 # Do not lose work accepted since update. New DB writes require manual migration review.
 for db,digest in record['post_databases'].items():
  if digest_db(Path(db))!=digest:raise ValueError('Database changed after update; automatic restore would lose new records')
 if 'post_files' in record and state_snapshot.file_inventory(root)!=record['post_files']:raise ValueError('Installation files changed after update; review a data-preserving recovery')
 if 'post_extensions' in record and state_snapshot.extension_inventory(record.get('preserved_state'))!=record['post_extensions']:raise ValueError('Extensions changed after update; review a data-preserving recovery')
 for target,digest in record.get('post_managed_files',{}).items():
  if hashlib.sha256(Path(target).read_bytes()).hexdigest()!=digest:raise ValueError('Managed service files changed after update; review recovery')
 gated=[];acquired=[];mutated=False
 try:
  for name in record['gated']:
   db=Path(name)
   with sql(db) as c:mode=c.execute('SELECT mode FROM node_control WHERE id=1').fetchone()[0]
   if mode=='active':idle_gate(db);acquired.append(db)
   elif mode!='maintenance':raise ValueError('Unexpected admission state')
   gated.append(db)
  mutated=True
  compose(source,root,project,'stop')
  for db,digest in record['post_databases'].items():
   if digest_db(Path(db))!=digest:raise ValueError('Database changed while starting rollback; records retained')
  if record['helpers']:run('systemctl','stop','loom-executor.service','loom-observations.timer','loom-observations.service')
  for group in ('files','helpers'):
   for target,backup in record[group].items():shutil.copy2(backup,target)
  for db,backup in record['databases'].items():
   if Path(db).is_symlink() or not Path(db).parent.resolve().is_relative_to(root.resolve()):raise ValueError('Database restore leaves the preserved instance')
   with sqlite3.connect(backup) as a,sql(db) as b:a.backup(b)
  if record.get('preserved_state'):state_snapshot.restore(root,record['preserved_state'])
  if '/etc/systemd/system/loom-stack.service' in record['files']:run('systemctl','daemon-reload')
  if record['helpers']:run('systemctl','start','loom-executor.service','loom-observations.timer')
  restart_ssh(root)
  previous=Path(record['previous_source']) if record.get('previous_source') else Path(__file__).resolve().parents[1]
  compose(previous,root,project,'up','-d','--no-build');check_health(previous,root,project)
  if record.get('preserved_state'):state_snapshot.verify(root,record['preserved_state'])
  if record.get('guides'):guide_bundle.select(record['guides']['parent'],record['guides']['previous'])
  for db in gated:ungate(db)
  record['state']='rolled_back';write(path,record)
 except BaseException:
  if not mutated:
   for db in acquired:ungate(db)
  raise
 return {'rolled_back':True,'record':str(path)}

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['plan','apply','resume','rollback']);p.add_argument('--instance',type=Path,default=Path('/loom/ops'));p.add_argument('--project',default='loom');p.add_argument('--release',type=Path);p.add_argument('--record',type=Path);p.add_argument('--mode',choices=['update','rebuild'],default='update');a=p.parse_args()
 if a.action=='rollback':result=rollback(a.record)
 elif a.action=='resume':result=resume(a.record)
 elif a.action=='plan':result=plan(a.instance,a.release,a.mode)
 else:result=apply(a.instance,a.release,a.project,mode=a.mode)
 print(json.dumps(result,indent=2))
