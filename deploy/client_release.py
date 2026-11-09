"""Acquire the exact external Loom Use source package; never configure a client."""
import argparse,hashlib,io,json,os,re,stat,tempfile,zipfile
from pathlib import Path
from urllib.request import urlopen
ROOT=Path(__file__).resolve().parents[1]
LOCK=ROOT/'loom_use/client-release.json'
LIMIT=8*1024*1024
REQUIRED={'VERSION','INSTALL.md','loom_use.py','skills/loom-use/SKILL.md','clients/headers.py'}

def member(name):
 return isinstance(name,str) and bool(name) and not Path(name).is_absolute() and '..' not in Path(name).parts and '\\' not in name and str(Path(name))==name and name!='.package.json' and '__pycache__' not in Path(name).parts

def descriptor():return json.loads(LOCK.read_text())

def prepare(destination,archive=None):
 lock=descriptor();version=lock['version']
 if not re.fullmatch(r'\d+\.\d+\.\d+-alpha\.\d+',version):raise ValueError('Invalid client release')
 expected=f'https://github.com/LoomOps/Loom-Use/releases/download/v{version}/loom-use-{version}.zip'
 if lock['url']!=expected or not re.fullmatch('[a-f0-9]{64}',lock['sha256']):raise ValueError('Invalid pinned client descriptor')
 destination=Path(destination);target=destination/version
 if target.is_symlink():raise ValueError('Refusing a linked package directory')
 if target.exists():
  if (target/'.package.json').is_symlink():raise ValueError('Package receipt must not be a symlink')
  receipt=json.loads((target/'.package.json').read_text())
  if receipt['lock']!=lock:raise ValueError('Package cache differs')
  files=receipt.get('files')
  if not isinstance(files,dict) or not REQUIRED<=files.keys() or not 1<=len(files)<=4096:raise ValueError('Incomplete package receipt')
  paths=list(target.rglob('*'))
  if any(p.is_symlink() for p in paths):raise ValueError('Linked package source')
  actual={str(p.relative_to(target)) for p in paths if p.is_file() and not ('__pycache__' in p.parts and p.suffix=='.pyc')}
  if actual!=set(files)|{'.package.json'}:raise ValueError('Package cache file set changed')
  for name,digest in receipt['files'].items():
   if not member(name) or not isinstance(digest,str) or not re.fullmatch('[a-f0-9]{64}',digest):raise ValueError('Invalid package receipt')
   p=target/name
   if not p.resolve().is_relative_to(target.resolve()) or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:raise ValueError('Package cache changed')
  if (target/'VERSION').read_text().strip()!=version:raise ValueError('Package cache version differs')
  return target
 if archive:data=Path(archive).read_bytes()
 else:
  with urlopen(lock['url'],timeout=30) as response:data=response.read(LIMIT+1)
 if len(data)>LIMIT or hashlib.sha256(data).hexdigest()!=lock['sha256']:raise ValueError('Client release checksum mismatch')
 destination.mkdir(parents=True,exist_ok=True)
 with zipfile.ZipFile(io.BytesIO(data)) as z:
  entries=z.infolist();names=[e.filename for e in entries]
  if not 1<=len(names)<=4096 or len(names)!=len(set(names)) or sum(e.file_size for e in entries)>LIMIT:raise ValueError('Invalid package size')
  for e in entries:
   p=Path(e.filename)
   if not member(e.filename) or e.is_dir() or stat.S_ISLNK(e.external_attr>>16):raise ValueError('Invalid package path')
  if not REQUIRED<=set(names):raise ValueError('Incomplete client package')
  if z.read('VERSION').decode().strip()!=version:raise ValueError('Package version mismatch')
  with tempfile.TemporaryDirectory(dir=destination,prefix='.loom-use-') as temp:
   stage=Path(temp)/'package';stage.mkdir();hashes={}
   for e in entries:
    p=stage/e.filename;p.parent.mkdir(parents=True,exist_ok=True);content=z.read(e.filename);p.write_bytes(content);p.chmod(0o600)
    hashes[e.filename]=hashlib.sha256(content).hexdigest()
   (stage/'.package.json').write_text(json.dumps({'lock':lock,'files':hashes}))
   stage.rename(target)
 return target

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--destination',type=Path,required=True);p.add_argument('--archive',type=Path);a=p.parse_args()
 target=prepare(a.destination,a.archive)
 print(json.dumps({'source':str(target),'read_next':str(target/'INSTALL.md'),'client_configuration':'not_started'},indent=2))
