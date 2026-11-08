#!/usr/bin/env python3
"""Download a versioned Loom guide bundle; never execute downloaded content."""
import argparse,hashlib,io,json,os,re,tempfile,zipfile
from pathlib import Path
from urllib.parse import urlsplit
from urllib.request import Request,build_opener,HTTPRedirectHandler

class NoRedirect(HTTPRedirectHandler):
 def redirect_request(self,*args,**kwargs):return None

def refresh(url,destination,token,development=False):
 parsed=urlsplit(url)
 if parsed.scheme not in ('https','http') or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment:raise ValueError('Use an HTTP(S) Loom base URL without credentials or query')
 if parsed.scheme=='http' and parsed.hostname not in ('localhost','127.0.0.1','::1'):raise ValueError('Remote URLs require HTTPS')
 if not token and not development:raise ValueError('Set LOOM_TOKEN or explicitly choose --development for your dev instance')
 base=url.rstrip('/');headers={'Authorization':'Bearer '+token} if token else {}
 opener=build_opener(NoRedirect)
 def get(path):
  with opener.open(Request(base+path,headers=headers),timeout=30) as response:
   data=response.read(8*1024*1024+1)
   if len(data)>8*1024*1024:raise ValueError('Bundle too large')
   return data
 manifest=json.loads(get('/api/guide'))['manifest']
 server=hashlib.sha256((base+'\0'+hashlib.sha256(token.encode()).hexdigest()).encode()).hexdigest()[:16]
 revision=manifest['revision']
 if not re.fullmatch('[0-9a-f]{64}',revision):raise ValueError('Invalid server revision')
 parent=Path(destination)/server;target=parent/revision
 if target.is_dir():return {'updated':False,'path':str(target),'manifest':manifest}
 data=get('/api/guide/bundle');archive=zipfile.ZipFile(io.BytesIO(data))
 names=archive.namelist()
 if len(names)!=len(set(names)) or sum(i.file_size for i in archive.infolist())>8*1024*1024:raise ValueError('Invalid bundle size or duplicate entries')
 for name in names:
  p=Path(name)
  if p.is_absolute() or '..' in p.parts or '\\' in name or not name or name.endswith('/'):raise ValueError('Invalid bundle path')
 if json.loads(archive.read('manifest.json'))['revision']!=revision:raise ValueError('Deployment changed during download; retry')
 parent.mkdir(parents=True,exist_ok=True)
 with tempfile.TemporaryDirectory(dir=parent,prefix='.download-') as temp:
  staging=Path(temp)/'bundle';staging.mkdir()
  for name in names:
   file=staging/name;file.parent.mkdir(parents=True,exist_ok=True);file.write_bytes(archive.read(name));file.chmod(0o600)
  staging.rename(target)
 return {'updated':True,'path':str(target),'manifest':manifest}

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('url');p.add_argument('--directory',type=Path,default=Path.home()/'.cache/loom/guides');p.add_argument('--development',action='store_true');a=p.parse_args()
 try:print(json.dumps(refresh(a.url,a.directory,os.getenv('LOOM_TOKEN',''),a.development),indent=2))
 except Exception as error:p.exit(1,type(error).__name__+': '+str(error)+'\n')
