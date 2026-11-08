"""Agent-driven Loom installation: inspect, check a plan, apply, verify."""
import argparse,hashlib,io,json,os,platform,re,shutil,socket,subprocess,sys,time,zipfile
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import Request,urlopen

BUNDLE=Path(__file__).resolve().parent
sys.path.insert(0,str(BUNDLE/'platform/deploy' if (BUNDLE/'platform/deploy').exists() else BUNDLE.parent))
ROOT=Path('/loom/ops')
FSTAB=Path('/etc/fstab')
DEFAULT={'data':'/storage/loom/data','dependencies':'/dependencies','mount':None,'repositories':[],
         'gpu':False,'tailscale':False,'api_port':18892,'dashboard_port':18893,'tls_port':18894}
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

def validate(plan):
 if not isinstance(plan,dict) or set(plan)!=set(DEFAULT):raise ValueError('Use the complete example plan; unknown fields are not accepted')
 data=path_value(plan['data']);deps=path_value(plan['dependencies'])
 if data==deps or data.is_relative_to(deps) or deps.is_relative_to(data):raise ValueError('Data and dependency paths must be separate')
 if any(p.is_relative_to('/loom') or Path('/loom').is_relative_to(p) for p in [data,deps]):raise ValueError('Keep external storage/dependencies outside /loom')
 for key in ['gpu']:
  if type(plan[key]) is not bool:raise ValueError('Expected boolean: '+key)
 if type(plan['tailscale']) is not bool:
  from tailnet import validate as validate_tailnet
  validate_tailnet(plan['tailscale'])
 ports=[plan[k] for k in ['api_port','dashboard_port','tls_port']]
 if len(set(ports))!=3 or any(type(n)is not int or not 1024<=n<=65535 for n in ports):raise ValueError('Choose three distinct unprivileged ports')
 m=plan['mount']
 if m is not None:
  if not isinstance(m,dict) or set(m)!={'uuid','target','fstype'}:raise ValueError('Mount requires uuid,target,fstype')
  if not re.fullmatch(r'[A-Fa-f0-9-]{8,64}',m['uuid']) or m['fstype'] not in ['ext4','xfs']:raise ValueError('Unsupported filesystem or UUID')
  target=path_value(m['target'])
  if target==data or not data.is_relative_to(target) or target.is_relative_to('/loom') or deps.is_relative_to(target):raise ValueError('Mount must contain only the selected data location, not /loom or dependencies')
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
 if ROOT.exists() or Path(plan['data']).exists() or Path('/loom/shared/executor/config.json').exists() or Path('/etc/systemd/system/loom-stack.service').exists() or (Path('/loom/distribution')/info['build_id']).exists():raise ValueError('Existing instance/data/executor detected; use the upgrade or migration procedure')
 run('docker','info');run('docker','compose','version')
 if run('docker','ps','-aq','--filter','label=com.docker.compose.project=loom'):raise ValueError('Existing Loom containers detected')
 for port in [plan[k] for k in ['api_port','dashboard_port','tls_port']]:
  with socket.socket() as sock:sock.bind(('127.0.0.1',port))
 if plan['gpu']:
  run('nvidia-smi','-L')
  runtimes=json.loads(run('docker','info','--format','{{json .Runtimes}}'))
  if 'nvidia' not in runtimes:raise ValueError('Configure NVIDIA Container Toolkit before GPU installation')
 if plan['tailscale']:
  if json.loads(run('tailscale','status','--json')).get('BackendState')!='Running':raise ValueError('Join the intended tailnet first')
  serve=json.loads(run('tailscale','serve','status','--json'))
  # Route conflicts are checked when applying this instance's specific Serve paths.
 for repo in plan['repositories']:
  if (Path(plan['dependencies'])/repo['name']).exists():raise ValueError('Dependency destination already exists: '+repo['name'])
 if plan['mount']:mount_preflight(plan['mount'])

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

def verify(plan):
 status=read_api(plan,'/api/status')
 tools=read_api(plan,'/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/list'})
 assert len(tools['result']['tools'])==6
 with urlopen(f"http://127.0.0.1:{plan['dashboard_port']}/healthz",timeout=10) as r:assert r.status==200
 assert run('systemctl','is-active','loom-executor')=='active'
 return {'build':status['build']['build_id'],'mcp_groups':6,'dashboard':'ok','executor':'active','credential_file':str(ROOT/'secrets/client-tokens.json')}

def apply(plan):
 info=manifest()
 if isinstance(plan['tailscale'],dict):
  from tailnet import enrollment,install_client
  if not shutil.which('tailscale'):install_client()
  enrollment_result=enrollment(plan['tailscale'])
  (BUNDLE/'tailnet-progress.json').write_text(json.dumps(enrollment_result,indent=2)+'\n')
  if not enrollment_result['ready']:return {'status':'waiting_for_tailnet','tailnet':enrollment_result}
 preflight(plan,info)
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
 if plan['mount']:mount_volume(plan['mount']);step('storage_mount')
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
 if plan['tailscale'] is True:
  from tailnet import status
  current=status()
  plan['tailscale']={'tailnet':current['tailnet'],'hostname':current['dns'].split('.')[0],'tag':'tag:loom','https_port':443,'bridge_cidr':'10.203.41.0/24','users':{},'ssh':False,'sso':None}
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
RequiresMountsFor={plan['data']} {plan['dependencies']}
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
  try:result=verify(plan);break
  except (OSError,AssertionError):time.sleep(1)
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
 step('verified');return result

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['inspect','check','apply','verify']);p.add_argument('plan',type=Path,nargs='?');a=p.parse_args()
 try:
  if a.action=='inspect':result=inspect()
  else:
   plan=validate(json.loads((a.plan or ROOT/'config/installation-plan.json').read_text()))
   result={'plan':plan,'bundle':manifest()['build_id']} if a.action=='check' else apply(plan) if a.action=='apply' else verify(plan)
  print(json.dumps(result,indent=2))
 except Exception as e:p.exit(1,type(e).__name__+': '+str(e)+'\n')
