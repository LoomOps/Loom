"""Check feature ownership and show change impact. Development only, no runtime hooks."""
import argparse
import ast
import fnmatch
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
CODE_ROOTS=['ops','dashboard','executor','deploy','loom_use','integrations','tests']


def candidates(root):
    for name in CODE_ROOTS:
        for path in (root/name).rglob('*'):
            rel=path.relative_to(root)
            if not path.is_file() or set(rel.parts)&{'evidence','__pycache__'}:continue
            if (path.suffix in ('.py','.js','.sh','.yaml','.css','.html','.json','.service') and path.name not in ('build-info.json','workspace.html')) or path.name in ('.dockerignore','requirements.txt') or path.name.startswith('Dockerfile'):
                yield str(rel)


def load(root=ROOT):
    return json.loads((root/'docs/features/catalog.json').read_text())


def check(catalog,root=ROOT):
    errors=[]
    if catalog.get('schema')!='loom-features/v1':errors.append('Unsupported feature catalog schema')
    features=catalog.get('features',[])
    ids=[f['id'] for f in features]
    if len(ids)!=len(set(ids)):errors.append('Duplicate feature IDs')
    source_owners={};test_owners={}
    for f in features:
        for key in ['id','summary','sources','tests','depends_on','spec','contracts','invariants','change_rule','live_validation','interfaces']:
            if key not in f:errors.append(f"{f.get('id')}: missing {key}")
        for interface in f.get('interfaces',[]):
            try:
                file,symbol=interface['entrypoint'].split('#')
                tree=ast.parse((root/file).read_text())
                if symbol not in {n.name for n in tree.body if isinstance(n,(ast.FunctionDef,ast.ClassDef))}:
                    errors.append(f"{f['id']}: missing interface symbol {interface['entrypoint']}")
                if any(v not in ids for v in interface['consumers']):errors.append(f"{f['id']}: unknown interface consumer")
            except (ValueError,OSError,SyntaxError):errors.append(f"{f['id']}: invalid interface entrypoint")
        for dependency in f.get('depends_on',[]):
            if dependency not in ids or dependency==f['id']:errors.append(f"{f['id']}: invalid dependency {dependency}")
        paths=f.get('sources',[])+f.get('tests',[])+f.get('contracts',[])+[f['spec']]
        for p in paths:
            path=root/p
            if Path(p).is_absolute() or '..' in Path(p).parts or not path.is_file():errors.append(f"{f['id']}: missing/unsafe path {p}")
        for p in f.get('sources',[]):source_owners.setdefault(p,[]).append(f['id'])
        for p in f.get('tests',[]):test_owners.setdefault(p,[]).append(f['id'])
        if not f.get('tests') or not f.get('invariants') or not f.get('live_validation'):errors.append(f"{f['id']}: missing behavior/test boundary")
    for p,owners in source_owners.items():
        if len(owners)>1:errors.append(f"Ambiguous ownership {p}: {owners}")
    for p in candidates(root):
        if p not in source_owners and p not in test_owners:errors.append('Unmapped code/test: '+p)
    return errors


def impact(paths,catalog,transitive=False):
    features=catalog['features']
    changed=set(paths)
    direct={f['id'] for f in features if changed.intersection(f['sources']+f['tests']+f['contracts']+[f['spec']]+[i['entrypoint'].split('#')[0] for i in f.get('interfaces',[])])}
    affected=set(direct)
    while True:
        new={f['id'] for f in features if set(f['depends_on'])&affected}
        new |= {consumer for f in features if f['id'] in affected for interface in f.get('interfaces',[]) for consumer in interface['consumers']}
        before=set(affected);affected |= new
        if not transitive or affected==before:break
    found=[f for f in features if f['id'] in affected]
    return {'changed':paths,'direct':sorted(direct),'review':sorted(affected-direct),
            'tests':sorted({p for f in found for p in f['tests']}),
            'specs':[f['spec'] for f in found],
            'unmapped':[p for p in paths if not any(p in f['sources']+f['tests']+f['contracts']+[f['spec']]+[i['entrypoint'].split('#')[0] for i in f.get('interfaces',[])] for f in features)],
            'note':'Review relevant contracts; not every related file needs editing. Use --transitive for cross-cutting contract changes.'}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['check','impact']);p.add_argument('paths',nargs='*');p.add_argument('--transitive',action='store_true');a=p.parse_args()
    catalog=load()
    if a.action=='check':
        errors=check(catalog);print(json.dumps({'valid':not errors,'features':len(catalog['features']),'errors':errors},indent=2));raise SystemExit(bool(errors))
    print(json.dumps(impact(a.paths,catalog,a.transitive),indent=2))
