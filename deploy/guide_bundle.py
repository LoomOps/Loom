"""Versioned local usage guides; independent of machine/agent configuration."""
import io,json,os,re,stat,subprocess,tempfile,uuid,zipfile
from pathlib import Path


def generate(image):
 script="import sys,ops,onboarding,job_contract; actor={'id':'documentation','scopes':['read','submit','cancel','events','control','admin'],'owners':['*'],'notify':[]}; sys.stdout.buffer.write(onboarding.bundle(actor,ops.TOOLS,ops.tool_schema,job_contract.contract()))"
 return subprocess.check_output(['docker','run','--rm','--network','none','--entrypoint','python3',image,'-c',script])


def install(data,parent,version):
 if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*',version) or version=='current':raise ValueError('Invalid guide version')
 parent=Path(parent);target=parent/version
 with zipfile.ZipFile(io.BytesIO(data)) as z:
  entries=z.infolist();names=[x.filename for x in entries]
  if len(names)!=len(set(names)) or sum(x.file_size for x in entries)>8*1024*1024:raise ValueError('Invalid guide bundle size or duplicates')
  for entry in entries:
   p=Path(entry.filename)
   if p.is_absolute() or '..' in p.parts or '\\' in entry.filename or entry.is_dir() or stat.S_ISLNK(entry.external_attr>>16):raise ValueError('Invalid guide bundle path')
  if json.loads(z.read('manifest.json')).get('build_id')!=version:raise ValueError('Guide version mismatch')
  files={name:z.read(name) for name in names}
 parent.mkdir(parents=True,exist_ok=True)
 if target.exists() or target.is_symlink():
  if target.is_symlink() or not target.is_dir():raise ValueError('Guide target is not a version directory')
  existing={str(p.relative_to(target)):p for p in target.rglob('*') if p.is_file() or p.is_symlink()}
  if set(existing)!=set(files) or any(p.is_symlink() or p.read_bytes()!=files[n] for n,p in existing.items()):raise ValueError('Existing guide version differs; retained without overwrite')
  return target
 with tempfile.TemporaryDirectory(dir=parent,prefix='.guide-') as tmp:
  stage=Path(tmp)/version;stage.mkdir(mode=0o755)
  for name,content in files.items():
   p=stage/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(content);p.chmod(0o644)
  stage.rename(target)
 return target


def selected(parent):
 link=Path(parent)/'current'
 if link.is_symlink():return os.readlink(link)
 if link.exists():raise ValueError('Guide current must be a symlink; existing content retained')
 return None


def select(parent,target):
 parent=Path(parent);selected(parent)  # Never replace a user directory.
 if target is None:
  (parent/'current').unlink(missing_ok=True);return
 parent.mkdir(parents=True,exist_ok=True)
 temp=parent/('.current-'+uuid.uuid4().hex)
 try:
  temp.symlink_to(target);os.replace(temp,parent/'current')
 finally:temp.unlink(missing_ok=True)
