"""Agent-driven Loom installation: inspect, check a plan, apply, verify."""
import argparse,hashlib,io,json,os,platform,re,shutil,socket,subprocess,sys,time,zipfile,uuid
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import Request,urlopen

BUNDLE=Path(__file__).resolve().parent
sys.path.insert(0,str(BUNDLE/'platform/deploy' if (BUNDLE/'platform/deploy').exists() else BUNDLE.parent))
sys.path.insert(0,str(BUNDLE))
sys.path.insert(0,str(BUNDLE/'platform/dashboard' if (BUNDLE/'platform/dashboard').exists() else BUNDLE.parents[1]/'dashboard'))
import doctor as diagnostics
ROOT=Path('/loom/ops')
FSTAB=Path('/etc/fstab')
DEFAULT={'data':'/loom/data','dependencies':'/dependencies','mount':None,'repositories':[],
         'additional_data':[],'mounts':[],'gpu':False,'tailscale':None,'api_port':18892,'dashboard_port':18893,'tls_port':18894}
TOOLS=['docker','openssl','setfacl','getfacl','findmnt','lsblk','blkid','mount','systemctl']

def run(*args):
 return subprocess.check_output([str(a) for a in args],text=True,stderr=subprocess.PIPE).strip()

def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()

def path_value(value):
 if not isinstance(value,str) or not re.fullmatch(r'/[A-Za-z0-9_./-]+',value):raise ValueError('Use an absolute path without whitespace or shell syntax')
 p=Path(value)
 try:linked=any(x.is_symlink() for x in [p,*p.parents])
 except PermissionError:raise ValueError('Cannot inspect path ownership; inspect with sudo: '+value)
 if '..' in p.parts or p==Path('/') or linked:raise ValueError('Unsafe or linked path: '+value)
 if any(p==Path(v) or p.is_relative_to(v) for v in ['/etc','/usr','/bin','/sbin','/proc','/sys','/dev','/run']):raise ValueError('Choose a data/source location, not a system directory')
 return p

def parse_mount(value):
 fields=value.split(':')
 if len(fields)!=3:raise ValueError('Mount must be UUID:TARGET:FSTYPE')
 return dict(zip(('uuid','target','fstype'),fields))

def planned_mounts(plan):
 values=plan.get('mounts',[])
 if not isinstance(values,list):raise ValueError('mounts must be a list')
 return ([plan['mount']] if plan.get('mount') else [])+values

def validate(plan):
 if not isinstance(plan,dict) or set(plan)-set(DEFAULT) or set(DEFAULT)-{'mounts'}-set(plan):raise ValueError('Use the complete example plan; unknown fields are not accepted')
 data=path_value(plan['data']);deps=path_value(plan['dependencies'])
 if data==deps or data.is_relative_to(deps) or deps.is_relative_to(data):raise ValueError('Data and dependency paths must be separate')
 if (data.is_relative_to('/loom') and not data.is_relative_to('/loom/data')) or Path('/loom').is_relative_to(data):raise ValueError('Within /loom, storage must stay under /loom/data')
 if deps.is_relative_to('/loom') or Path('/loom').is_relative_to(deps):raise ValueError('Keep dependencies outside /loom')
 for key in ['gpu']:
  if type(plan[key]) is not bool:raise ValueError('Expected boolean: '+key)
 if not isinstance(plan['tailscale'],dict):raise ValueError('Full Tailscale identity and Gateway plan required; run setup.py plan')
 from tailnet import validate as validate_tailnet
 validate_tailnet(plan['tailscale'])
 if not plan['tailscale'].get('sso') or not plan['tailscale'].get('access_plan'):raise ValueError('SSO and Sharing Gateway are required')
 if not isinstance(plan['additional_data'],list):raise ValueError('additional_data must be a list of writable storage paths')
 stores=[data]+[path_value(v) for v in plan['additional_data']]
 for i,p in enumerate(stores):
  if i and ((p.exists() and not p.is_dir()) or (p.is_relative_to('/loom') and not p.is_relative_to('/loom/data')) or Path('/loom').is_relative_to(p) or p==deps or p.is_relative_to(deps) or deps.is_relative_to(p)):raise ValueError('Choose a separate data directory, not platform state or dependencies')
  if any(p==q or p.is_relative_to(q) or q.is_relative_to(p) for q in stores[:i]):raise ValueError('Storage paths must not overlap')
 ports=[plan[k] for k in ['api_port','dashboard_port','tls_port']]
 if len(set(ports))!=3 or any(type(n)is not int or not 1024<=n<=65535 for n in ports):raise ValueError('Choose three distinct unprivileged ports')
 targets=[];uuids=set()
 for m in planned_mounts(plan):
  if not isinstance(m,dict) or set(m)!={'uuid','target','fstype'}:raise ValueError('Mount requires uuid,target,fstype')
  if not re.fullmatch(r'[A-Fa-f0-9-]{8,64}',m['uuid']) or m['fstype'] not in ['ext4','xfs']:raise ValueError('Unsupported filesystem or UUID')
  target=path_value(m['target'])
  if not any(p.is_relative_to(target) for p in stores) or (target.is_relative_to('/loom') and not target.is_relative_to('/loom/data')) or Path('/loom').is_relative_to(target) or deps.is_relative_to(target):raise ValueError('Mount must contain a selected data store, not platform state or dependencies')
  if m['uuid'].lower() in uuids or any(target.is_relative_to(p) or p.is_relative_to(target) for p in targets):raise ValueError('Duplicate or overlapping mounts')
  uuids.add(m['uuid'].lower());targets.append(target)
 if not isinstance(plan['repositories'],list):raise ValueError('repositories must be a list')
 names=set()
 for repo in plan['repositories']:
  if not isinstance(repo,dict) or set(repo)!={'name','url','commit'}:raise ValueError('Repository requires name,url,commit')
  if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,63}',repo['name']) or repo['name'] in names or repo['name']=='installations':raise ValueError('Invalid/duplicate repository name')
  u=urlsplit(repo['url'])
  if u.scheme!='https' or not u.hostname or u.username or u.password or u.query or u.fragment:raise ValueError('Use a credential-free HTTPS Git URL')
  if not re.fullmatch(r'[a-fA-F0-9]{40}',repo['commit']):raise ValueError('Pin the repository to a full commit SHA')
  names.add(repo['name'])
 return plan

def manifest():
 info=json.loads((BUNDLE/'manifest.json').read_text())
 if info['schema']!='loom-install-kit/v1':raise ValueError('Unsupported installation bundle')
 for name,digest in info['files'].items():
  p=BUNDLE/name
  if Path(name).is_absolute() or '..' in Path(name).parts or not p.resolve().is_relative_to(BUNDLE.resolve()) or sha(p)!=digest:raise ValueError('Bundle checksum mismatch: '+name)
 return info

def inspect():
 result={'system':platform.system(),'architecture':platform.machine(),'os_release':Path('/etc/os-release').read_text() if Path('/etc/os-release').exists() else None,
         'tools':{name:shutil.which(name) is not None for name in TOOLS+['git','tailscale','nvidia-smi']},'existing_instance':ROOT.exists()}
 for name,command in [('disks',['lsblk','-J','-o','NAME,SIZE,FSTYPE,UUID,MOUNTPOINTS']),('mounts',['findmnt','-J']),('compose',['docker','compose','version'])]:
  try:result[name]=run(*command)
  except (OSError,subprocess.CalledProcessError):result[name]='unavailable'
 result['recommendations']=diagnostics.recommendations(result)
 return result

def mount_preflight(m):
 target=path_value(m['target'])
 if target.exists() and (not target.is_dir() or any(target.iterdir())):raise ValueError('Mountpoint must be empty and unused')
 device=run('blkid','-U',m['uuid'])
 if run('blkid','-s','TYPE','-o','value',device)!=m['fstype']:raise ValueError('Filesystem type mismatch')
 mounted=subprocess.run(['findmnt','-rn','--source',device],capture_output=True,text=True)
 if mounted.returncode==0 and mounted.stdout.strip():raise ValueError('Device is already mounted; select its existing mount path instead')
 for line in FSTAB.read_text().splitlines():
  parts=line.split()
  if parts and not parts[0].startswith('#') and (parts[0] in ['UUID='+m['uuid'],device] or len(parts)>1 and parts[1]==m['target']):raise ValueError('Existing fstab entry conflicts with selected mount')

def preflight(plan,info):
 if os.geteuid()!=0:raise PermissionError('Run apply with sudo')
 if not Path('/run/systemd/system').is_dir():raise ValueError('A booted systemd host is required')
 arch={'x86_64':'amd64','aarch64':'arm64'}.get(platform.machine())
 if info['architecture']!=arch:raise ValueError('Image architecture does not match this host')
 missing=[t for t in TOOLS if not shutil.which(t)]
 if plan['repositories'] and not shutil.which('git'):missing.append('git')
 if plan['tailscale'] and not shutil.which('tailscale'):missing.append('tailscale')
 if missing:raise ValueError('Install host prerequisites first: '+', '.join(missing))
 if ROOT.exists() or (Path(plan['data'])/'.loom-storage.json').exists() or Path('/loom/shared/executor/config.json').exists() or Path('/etc/systemd/system/loom-stack.service').exists() or (Path('/loom/distribution')/info['build_id']).exists():raise ValueError('Existing instance/data/executor detected; use the upgrade or migration procedure')
 run('docker','info');run('docker','compose','version')
 if run('docker','ps','-aq','--filter','label=com.docker.compose.project=loom'):raise ValueError('Existing Loom containers detected')
 for port in [plan[k] for k in ['api_port','dashboard_port','tls_port']]+[diagnostics.service_plan(plan)['host_port']]:
  with socket.socket() as sock:sock.bind(('127.0.0.1',port))
 if plan['gpu']:
  run('nvidia-smi','-L')
  runtimes=json.loads(run('docker','info','--format','{{json .Runtimes}}'))
  if 'nvidia' not in runtimes:raise ValueError('Configure NVIDIA Container Toolkit before GPU installation')
 if plan['tailscale']:
  if not diagnostics.machine_matches(plan,json.loads(run('tailscale','status','--json'))):raise ValueError('Installation plan does not match this Tailscale machine; inspect and regenerate before mutation')
  serve=json.loads(run('tailscale','serve','status','--json'))
  # Route conflicts are checked when applying this instance's specific Serve paths.
 for repo in plan['repositories']:
  if (Path(plan['dependencies'])/repo['name']).exists():raise ValueError('Dependency destination already exists: '+repo['name'])
 for m in planned_mounts(plan):mount_preflight(m)

def mount_volume(m):
 mount_preflight(m)
 target=Path(m['target']);target.mkdir(parents=True,exist_ok=True)
 old=FSTAB.read_bytes()
 backup=FSTAB.with_name(FSTAB.name+'.loom-'+str(time.time_ns()));backup.write_bytes(old);backup.chmod(0o600)
 line=f"UUID={m['uuid']} {target} {m['fstype']} defaults 0 {2 if m['fstype']=='ext4' else 0}\n"
 with FSTAB.open('ab') as f:f.write(b'\n'+line.encode())
 try:run('mount',str(target))
 except BaseException:FSTAB.write_bytes(old);raise
 if run('findmnt','-rn','-o','UUID','--mountpoint',str(target)).lower()!=m['uuid'].lower():raise ValueError('Mounted filesystem UUID mismatch')

def read_api(plan,path,body=None):
 token=json.loads((ROOT/'secrets/client-tokens.json').read_text())['owner']
 request=Request(f"http://127.0.0.1:{plan['api_port']}"+path,data=json.dumps(body).encode() if body is not None else None,headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
 with urlopen(request,timeout=10) as response:return json.load(response)

def verify_core(plan):
 status=read_api(plan,'/api/status')
 tools=read_api(plan,'/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/list'})
 if len(tools['result']['tools'])!=6:raise ValueError('Expected six MCP groups')
 with urlopen(f"http://127.0.0.1:{plan['dashboard_port']}/healthz",timeout=10) as r:
  if r.status!=200:raise ValueError('Dashboard health failed')
 if run('systemctl','is-active','loom-executor')!='active':raise ValueError('Executor not active')
 return {'build':status['build']['build_id'],'mcp_groups':6,'dashboard':'ok','executor':'active','credential_file':str(ROOT/'secrets/client-tokens.json')}

def apply(plan):
 validate(plan);diagnostics.service_plan(plan)
 info=manifest()
 preflight(plan,info)
 from access_setup import load as access_load,inspect as access_inspect
 access_plan=access_load(plan['tailscale']['access_plan'])
 access_inspect(access_plan)
 from access_setup import register_machine,private_access
 private_access(access_plan,plan['tailscale']['sso']['network_users'])
 register_machine(access_plan)
 if isinstance(plan['tailscale'],dict):
  from tailnet import enrollment,install_client
  if not shutil.which('tailscale'):install_client()
  enrollment_result=enrollment(plan['tailscale'])
  (BUNDLE/'tailnet-progress.json').write_text(json.dumps(enrollment_result,indent=2)+'\n')
  if not enrollment_result['ready']:return {'status':'waiting_for_tailnet','tailnet':enrollment_result}
 progress={'build':info['build_id'],'completed':[]}
 def step(name):
  progress['completed'].append(name);(BUNDLE/'install-progress.json').write_text(json.dumps(progress,indent=2)+'\n')
 # Verify image identities before any platform installation.
 if info.get('image_transport','archive')=='archive':run('docker','load','--input',BUNDLE/'images.tar')
 elif info['image_transport']!='local':raise ValueError('Unknown image transport')
 for image in info['images'].values():
  if run('docker','image','inspect',image['tag'],'--format','{{.Id}}')!=image['id']:raise ValueError('Loaded image identity mismatch')
 if plan['gpu']:run('docker','run','--rm','--network','none','--gpus','all','--entrypoint','nvidia-smi',info['images']['python']['id'],'-L')
 step('images')
 for m in planned_mounts(plan):mount_volume(m)
 if planned_mounts(plan):step('storage_mount')
 deps=Path(plan['dependencies']);deps.mkdir(parents=True,exist_ok=True);(deps/'installations').mkdir(exist_ok=True)
 for repo in plan['repositories']:
  target=deps/repo['name'];run('git','clone','--no-checkout','--',repo['url'],target)
  run('git','-C',target,'checkout','--detach',repo['commit'])
  run('git','-C',target,'submodule','update','--init','--recursive')
  assert run('git','-C',target,'rev-parse','HEAD').lower()==repo['commit'].lower()
  record=deps/'installations'/repo['name']/repo['commit'];record.mkdir(parents=True)
  (record/'source.json').write_text(json.dumps(repo,indent=2)+'\n')
 step('dependencies')
 src=BUNDLE/'platform'
 run(sys.executable,src/'deploy/install.py','--data',plan['data'],'--api-port',plan['api_port'],'--dashboard-port',plan['dashboard_port'],'--tls-port',plan['tls_port'],'--project','loom','--ops-image',info['images']['ops']['tag'],'--dashboard-image',info['images']['dashboard']['tag'])
 step('host_layout')
 configure_identity_storage(plan,access_plan)
 (ROOT/'config/installation-plan.json').write_text(json.dumps(plan,indent=2)+'\n')
 if isinstance(plan['tailscale'],dict):
  from tailnet import configure,status
  configure(ROOT,plan['tailscale'],status()['dns'])
  if plan['tailscale'].get('access_plan'):
   from access_setup import load as access_load,inspect as access_inspect,configure as access_configure
   access_plan=access_load(plan['tailscale']['access_plan'])
   access_inspect(access_plan)
   access_configure(ROOT,access_plan)
 # Keep the boot-time Compose code independent of the downloaded/extracted kit.
 dest=Path('/loom/distribution')/info['build_id'];shutil.copytree(src,dest)
 for p in [dest.parent,dest,*dest.rglob('*')]:os.chown(p,0,0);p.chmod(0o755 if p.is_dir() else 0o644)
 args=[sys.executable,dest/'deploy/executor-attach.py',ROOT,'--instance','alpha','--data',plan['data'],'--install-service']
 for path in plan['additional_data']:args.extend(['--additional-data',path])
 if plan['gpu']:args.append('--gpu')
 run(*args)
 config=Path('/loom/shared/executor/config.json');values=json.loads(config.read_text());values['runtimes']['python']['image']=info['images']['python']['id'];config.write_text(json.dumps(values,indent=2)+'\n');config.chmod(0o600)
 run('systemctl','restart','loom-executor')
 step('executor')
 import guide_bundle
 guides=Path('/loom/loom_use')
 guide_bundle.install((BUNDLE/'loom-agent-kit.zip').read_bytes(),guides,info['build_id'])
 guide_bundle.select(guides,info['build_id'])
 unit=f'''[Unit]
Description=Loom Compose stack
Requires=docker.service loom-executor.service
After=docker.service loom-executor.service network-online.target
RequiresMountsFor={plan['data']} {plan['dependencies']} {' '.join(plan['additional_data'])}
[Service]
Type=oneshot
RemainAfterExit=yes
Environment=LOOM_INSTANCE=/loom/ops LOOM_PROJECT=loom
ExecStart=/bin/bash {dest}/deploy/compose.sh up -d --no-build
ExecStop=/bin/bash {dest}/deploy/compose.sh stop
TimeoutStartSec=180
TimeoutStopSec=120
[Install]
WantedBy=multi-user.target
'''
 Path('/etc/systemd/system/loom-stack.service').write_text(unit)
 run('systemctl','daemon-reload');run('systemctl','enable','--now','loom-stack.service');step('compose')
 for _ in range(30):
  try:result=verify_core(plan);break
  except (OSError,ValueError):time.sleep(1)
 else:raise RuntimeError('Startup verification failed; inspect docker compose logs')
 if result['build']!=info['build_id']:raise ValueError('Running version differs from installer')
 if plan['tailscale']:
  from tailnet import serve,status
  tailplan=plan['tailscale'] if isinstance(plan['tailscale'],dict) else {'https_port':443}
  serve(ROOT,tailplan,status()['dns'])
  step('tailscale')
  if isinstance(plan['tailscale'],dict) and plan['tailscale'].get('access_plan'):
   from access_setup import publish,register_machine
   register_machine(access_plan)
   publish(access_plan)
   step('access_enrollment')
 (ROOT/'config/installation-plan.json').write_text(json.dumps(plan,indent=2)+'\n')
 result=diagnostics.doctor(sys.modules[__name__],plan)
 if result['ready']:step('verified')
 return {'status':'services_ready' if result['ready'] else 'incomplete','doctor':result,'gateway':access_plan['origin'],'next':'Read INSTALL.md, then complete doctor stages and setup.py smoke; do not hand off until acceptance passes.'}

def configure_identity_storage(plan, access):
 from tailnet import write
 uid,gid=ROOT.stat().st_uid,ROOT.stat().st_gid
 api=json.loads((ROOT/'config/api.json').read_text())
 api.update(machine_id=access['machine_id'],machine_name=access['name'])
 write(ROOT/'config/api.json',api,uid,gid)
 paths=[plan['data'],*plan['additional_data']]
 store_ids=[]
 for value in paths:
  path=path_value(value);path.mkdir(parents=True,exist_ok=True)
  marker=path/'.loom-storage.json'
  if marker.is_symlink():raise ValueError('Linked storage identity marker')
  if not marker.exists():
   import secrets
   write(marker,{'storage_id':secrets.token_hex(12)},uid,gid)
  stored=json.loads(marker.read_text())
  if set(stored)!={'storage_id'} or not isinstance(stored['storage_id'],str) or not stored['storage_id']:raise ValueError('Invalid storage identity')
  store_ids.append(stored['storage_id'])
  # Grant the service identity without taking ownership of existing user files.
  run('setfacl','-R','-P','-m',f'u:{uid}:rwX',path)
  run('setfacl','-R','-P','-m',f'd:u:{uid}:rwx',path)
  # Owner permissions take precedence over a named ACL for the same UID.
  run('find','-P',path,'-uid',str(uid),'(','-type','f','-o','-type','d',')','-exec','chmod','u+rwX','{}','+')
  for parent in path.parents:
   if parent!=Path('/') and not parent.stat().st_mode & 0o001:run('setfacl','-m',f'u:{uid}:--x',parent)
 overlay={'services':{'loom-ops':{'environment':{'LOOM_DATA_PATHS':':'.join('/loom/data/'+str(i) for i in range(len(paths)))},
  'volumes':[{'type':'bind','source':p,'target':'/loom/data/'+str(i),'read_only':False,'bind':{'create_host_path':False}} for i,p in enumerate(paths)]}}}
 write(ROOT/'compose-storage.yaml',overlay,uid,gid)
 write(ROOT/'config/storage-plan.json',{'primary':plan['data'],'read_write':paths},uid,gid)
 api['data_stores']={f'data/{i}':{'path':f'/loom/data/{i}','mode':'rw','storage_id':store_ids[i]} for i in range(len(paths))}
 if len(set(store_ids))!=len(store_ids):raise ValueError('Duplicate storage identity')
 write(ROOT/'config/api.json',api,uid,gid)


def make_plan(output,admin_email,data,additional_data,gpu,credential_file,mounts=None):
 """Generate all linked plans from inspected identity and explicitly chosen inputs."""
 from tailnet import status,write
 current=status()
 if current['state']!='Running' or not current['dns']:raise ValueError('Complete Tailscale login before generating the installation plan')
 if not re.fullmatch(r'[^\s@]+@[^\s@]+\.[^\s@]+',admin_email):raise ValueError('The login email of the Tailscale account that will administer Loom is required')
 output=output.resolve();access_file=output.with_name(output.stem+'-access.json')
 if output.exists() or access_file.exists():raise ValueError('Plan already exists; review it rather than silently replacing identities')
 raw=json.loads(run('tailscale','status','--json'));own=raw['Self']
 origin='https://'+current['dns'];private=origin+':8443';subject='tailscale:'+admin_email
 access={'machine_id':'loom-'+str(uuid.uuid4()),'name':own.get('HostName') or current['dns'].split('.')[0],
 'tailnet':current['tailnet'],'device_id':current['device_id'],'addresses':own['TailscaleIPs'],
 'identity_addresses':[],'ports':[8443],'ssh_accounts':[], 'origin':origin+':10000','dashboard_origin':private,
 'host_port':18904,'bind':'0.0.0.0','port':8081,'admins':[{'issuer':private+'/identity','subject':subject}],
 'credential_file':str(credential_file.resolve()),'database':'/loom/ops/admission/requests.sqlite3',
 'enrollment_mode':'sharing','sharing_hourly_limit':20,'sharing_pending_limit':200}
 plan={**DEFAULT,'data':data,'additional_data':additional_data,'mounts':mounts or [],'gpu':gpu,'tailscale':{
 'tailnet':current['tailnet'],'hostname':current['dns'].split('.')[0],'tag':'tag:loom','https_port':8443,
 'bridge_cidr':'10.203.41.0/24','users':{},'ssh':False,'sso':{'provider':'tailscale',
 'network_users':{admin_email:{'subject':subject}},'session_seconds':604800,'link_tailscale_email':True},'access_plan':str(access_file)}}
 validate(plan)
 write(access_file,access);write(output,plan)
 return {'plan':str(output),'access_plan':str(access_file),'gateway':access['origin'],
         'next':'Read INSTALL.md and steps/01-plan.md, then run setup.py check with this plan. Confirm the Docker subnet is unused.'}


def resume_network(plan):
 """Recover publication after installation; never reinstall a running database or executor."""
 validate(plan);diagnostics.service_plan(plan)
 saved=json.loads((ROOT/'config/installation-plan.json').read_text())
 if saved!=plan:raise ValueError('Resume requires the original installation plan')
 for stage in ('configuration','services'):
  result=diagnostics.doctor(sys.modules[__name__],plan,stage)
  if not result['ready']:return {'status':'incomplete','doctor':result}
 from tailnet import serve,status
 from access_setup import load,inspect,publish,register_machine,private_access
 access=load(plan['tailscale']['access_plan']);inspect(access)
 private_access(access,plan['tailscale']['sso']['network_users'])
 serve(ROOT,plan['tailscale'],status()['dns']);register_machine(access);publish(access)
 return diagnostics.handoff(sys.modules[__name__],plan)


if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('action',choices=['inspect','plan','check','apply','verify','doctor','handoff','resume-network','smoke'])
 p.add_argument('plan',type=Path,nargs='?');p.add_argument('--stage',choices=diagnostics.STAGES)
 p.add_argument('--output',type=Path);p.add_argument('--admin-email',help='Tailscale login email of the account that will administer Loom (not a contact email or OS username)');p.add_argument('--data',default=DEFAULT['data'])
 p.add_argument('--additional-data',action='append',default=[],help='Additional RW store; repeat in data/1, data/2 order');p.add_argument('--mount',action='append',default=[],metavar='UUID:TARGET:FSTYPE',help='Explicit existing filesystem to mount; repeat if needed');p.add_argument('--gpu',action='store_true')
 p.add_argument('--credential-file',type=Path,default=Path('/loom/private-inputs/tailscale-management.json'));a=p.parse_args()
 try:
  if a.action=='inspect':result=inspect()
  elif a.action=='plan':
   if not a.output or not a.admin_email:raise ValueError('plan requires --output and --admin-email')
   result=make_plan(a.output,a.admin_email,a.data,a.additional_data,a.gpu,a.credential_file,[parse_mount(v) for v in a.mount])
  else:
   plan=json.loads((a.plan or ROOT/'config/installation-plan.json').read_text())
   if a.action in ('doctor','verify'):result=diagnostics.doctor(sys.modules[__name__],plan,a.stage)
   elif a.action=='smoke':
    validate(plan);diagnostics.service_plan(plan)
    import smoke
    result=smoke.run(sys.modules[__name__],plan)
   elif a.action=='handoff':result=diagnostics.handoff(sys.modules[__name__],plan)
   else:
    validate(plan);diagnostics.service_plan(plan)
    result={'valid':True,'build':manifest()['build_id'],'re_read':'INSTALL.md','next_document':'steps/02-host.md'} if a.action=='check' else apply(plan) if a.action=='apply' else resume_network(plan)
  print(json.dumps(result,indent=2))
  if result.get('ready') is False or result.get('status') in ('incomplete','waiting_for_tailnet'):raise SystemExit(2)
 except Exception as e:p.exit(1,type(e).__name__+(': '+str(e) if isinstance(e,ValueError) else ': inspect local configuration; provider output is not printed')+'\n')
