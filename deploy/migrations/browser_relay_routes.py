"""Add missing browser relay prefixes on an existing private installation."""
import argparse,json,sys
from pathlib import Path
from urllib.parse import urlsplit
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import tailnet
from migrations import save

PREFIXES=('/api/search','/api/runtimes','/api/scenarios')

def run(root,phase):
    root=Path(root)
    if (root/'development').exists():return {'status':'done','scope':'development has no managed ingress'}
    access=json.loads((root/'config/access-plan.json').read_text())
    private=urlsplit(access['dashboard_origin'])
    if private.scheme!='https' or not private.hostname or private.netloc==urlsplit(access['origin']).netloc:
        raise ValueError('Expected separate private Dashboard ingress')
    key=private.netloc
    before=json.loads(tailnet.run('tailscale','serve','status','--json'))
    if before.get('AllowFunnel',{}).get(key):raise ValueError('Private Dashboard ingress must not use Funnel')
    handlers=before.get('Web',{}).get(key,{}).get('Handlers',{})
    expected=tailnet.expected_routes(root)
    if any(handlers.get(p,{}).get('Proxy')!=expected[p] for p in ('/','/api')):
        raise ValueError('Private ingress base routes do not belong to this instance')
    if any(p in handlers and handlers[p].get('Proxy')!=expected[p] for p in PREFIXES):
        raise ValueError('Browser relay route conflicts with existing configuration')
    missing=[p for p in PREFIXES if p not in handlers]
    if not missing:return {'status':'done'}
    if phase!='apply':return {'status':'needed','routes':missing}
    backup=root/'runtime/migrations/browser-relay-routes-before.json'
    if not backup.exists():save(backup,before)
    for path in missing:
        tailnet.run('tailscale','serve','--bg','--https='+str(private.port or 443),'--set-path='+path,expected[path])
    return run(root,'verify')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['check','apply','verify']);p.add_argument('--instance',type=Path,required=True);p.add_argument('--platform',type=Path,required=True)
    a=p.parse_args();print(json.dumps(run(a.instance,a.phase)))
