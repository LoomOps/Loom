"""Check feature ownership and show change impact. Development only, no runtime hooks."""
import argparse
import ast
import fnmatch
import json
import hashlib
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
    import jsonschema
    try:jsonschema.Draft202012Validator(json.loads((root/'docs/features/catalog.schema.json').read_text())).validate(catalog)
    except jsonschema.ValidationError as e:return ['Invalid feature schema: '+e.message]
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
        for p in f.get('tests',[]):
            test_owners.setdefault(p,[]).append(f['id'])
            try:
                tree=ast.parse((root/p).read_text())
                if not Path(p).name.startswith('test_') or not any(isinstance(n,(ast.FunctionDef,ast.AsyncFunctionDef)) and n.name.startswith('test_') for n in ast.walk(tree)):
                    errors.append(f"{f['id']}: no discoverable tests in {p}")
            except (OSError,SyntaxError):errors.append(f"{f['id']}: invalid test source {p}")
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


def audit(catalog,evidence,root=ROOT):
    """Map actual suite results to every block. Missing/skipped tests are not passes."""
    reports={}
    for name in ('ops','dashboard','packaging','browser'):
        p=Path(evidence)/(name+'.json')
        if p.is_file():reports[name]=json.loads(p.read_text())
    blocks=[]
    for feature in catalog['features']:
        missing=[];cases=[];stale=[]
        for path in feature['tests']:
            parts=Path(path).parts
            suite='browser' if 'browser' in parts else 'packaging' if parts[1]=='deploy' else parts[1]
            report=reports.get(suite,{})
            found=[c for c in report.get('cases',[]) if c['name'].startswith(Path(path).stem+'.')]
            tree=ast.parse((root/path).read_text())
            expected={Path(path).stem+'.'+cls.name+'.'+fn.name for cls in tree.body if isinstance(cls,ast.ClassDef) for fn in cls.body if isinstance(fn,ast.FunctionDef) and fn.name.startswith('test_')}
            actual={c['name'] for c in found}
            if not found:missing.append(path)
            missing.extend(sorted(expected-actual))
            cases.extend(found)
            recorded=report.get('sources',{})
            for source in set(feature['sources']+feature['tests']+feature['contracts']+[feature['spec']]):
                if recorded.get(source)!=hashlib.sha256((root/source).read_bytes()).hexdigest():stale.append(source)
        failures=[c['name'] for c in cases if c['status']!='passed']
        blocks.append({'id':feature['id'],'passed':not(missing or failures or stale),'tests':len(cases),
                       'missing':missing,'not_passed':failures,'stale_or_unrecorded':sorted(set(stale)),
                       'live_validation':feature['live_validation']})
    return {'valid':not check(catalog,root) and all(b['passed'] for b in blocks),'blocks':blocks,
            'scope':'Recorded automated checks only; listed live boundaries remain separate.'}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('action',choices=['check','impact','audit']);p.add_argument('paths',nargs='*');p.add_argument('--transitive',action='store_true');p.add_argument('--evidence',type=Path);a=p.parse_args()
    catalog=load()
    if a.action=='check':
        errors=check(catalog);print(json.dumps({'valid':not errors,'features':len(catalog['features']),'errors':errors},indent=2));raise SystemExit(bool(errors))
    if a.action=='audit':
        if not a.evidence:p.error('--evidence is required')
        result=audit(catalog,a.evidence);print(json.dumps(result,indent=2));raise SystemExit(not result['valid'])
    print(json.dumps(impact(a.paths,catalog,a.transitive),indent=2))
