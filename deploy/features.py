"""Check feature ownership and show change impact. Development only, no runtime hooks."""
import argparse
import ast
import re
import json
import hashlib
from collections import defaultdict
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
CODE_ROOTS=['ops','dashboard','executor','deploy','loom_use','integrations','tests','rust','build_support']


def candidates(root):
    for name in ('Cargo.toml','Cargo.lock','rust-toolchain.toml','.bazelrc','.bazelversion','MODULE.bazel','MODULE.bazel.lock','BUILD.bazel'):
        if (root/name).is_file():yield name
    for name in CODE_ROOTS:
        for path in (root/name).rglob('*'):
            rel=path.relative_to(root)
            if not path.is_file() or set(rel.parts)&{'evidence','__pycache__'}:continue
            if (path.suffix in ('.py','.js','.sh','.yaml','.css','.html','.json','.service','.rs','.sql','.toml','.bzl') and path.name not in ('build-info.json','workspace.html')) or path.name in ('.dockerignore','requirements.txt','BUILD.bazel') or path.name.startswith('Dockerfile'):
                yield str(rel)


def test_cases(path):
    if path.suffix=='.rs':
        return {path.stem+'.Rust.'+name for name in re.findall(r'#\[test\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z_0-9]*)',path.read_text())}
    tree=ast.parse(path.read_text())
    if not path.name.startswith('test_'):return set()
    return {path.stem+'.'+cls.name+'.'+fn.name for cls in tree.body if isinstance(cls,ast.ClassDef) for fn in cls.body if isinstance(fn,ast.FunctionDef) and fn.name.startswith('test_')}

def load(root=ROOT):
    return json.loads((root/'docs/features/catalog.json').read_text())


def check(catalog,root=ROOT,discovered=None):
    errors=[]
    if discovered is None:discovered={}
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
                text=(root/file).read_text()
                if Path(file).suffix=='.rs':
                    symbols=set(re.findall(r'\b(?:fn|struct|enum|trait)\s+([A-Za-z_][A-Za-z_0-9]*)',text))
                else:
                    tree=ast.parse(text);symbols={n.name for n in tree.body if isinstance(n,(ast.FunctionDef,ast.ClassDef))}
                if symbol not in symbols:errors.append(f"{f['id']}: missing interface symbol {interface['entrypoint']}")
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
                if p not in discovered:discovered[p]=test_cases(root/p)
                if not discovered[p]:errors.append(f"{f['id']}: no discoverable tests in {p}")
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
    discovered={}
    errors=check(catalog,root,discovered)
    if errors:return {'valid':False,'errors':errors,'blocks':[]}
    reports={}
    indexed=defaultdict(list)
    for name in ('ops','dashboard','packaging','browser','rust','native'):
        p=Path(evidence)/(name+'.json')
        if not p.is_file():continue
        report=reports[name]=json.loads(p.read_text())
        for case in report.get('cases',[]):
            # Old Python reports carry a discovery module. New reports and
            # Rust carry an exact path, so another file cannot satisfy a case.
            source=case.get('source') or case['name'].split('.',1)[0]
            indexed[name,source].append(case)
    refs={p for f in catalog['features'] for p in f['sources']+f['tests']+f['contracts']+[f['spec']]}
    fingerprints={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in refs}
    blocks=[]
    for feature in catalog['features']:
        missing=[];cases=[];stale=set();suite_errors=set();suites=set()
        for path in feature['tests']:
            parts=Path(path).parts
            suite='rust' if Path(path).suffix=='.rs' else 'native' if parts[1]=='rust' else 'browser' if 'browser' in parts else 'packaging' if parts[1]=='deploy' else parts[1]
            suites.add(suite)
            found=indexed[suite,path]+indexed[suite,Path(path).stem]
            if not found:missing.append(path)
            missing.extend(sorted(discovered[path]-{c['name'] for c in found}))
            cases.extend(found)
        refs=set(feature['sources']+feature['tests']+feature['contracts']+[feature['spec']])
        for suite in suites:
            report=reports.get(suite,{})
            if report.get('successful') is not True:suite_errors.add(suite)
            recorded=report.get('sources',{})
            stale.update(p for p in refs if recorded.get(p)!=fingerprints[p])
        failures=[c['name'] for c in cases if c['status']!='passed']
        blocks.append({'id':feature['id'],'passed':not(missing or failures or stale or suite_errors),'tests':len(cases),
                       'missing':missing,'not_passed':failures,'stale_or_unrecorded':sorted(stale),
                       'suite_errors':sorted(suite_errors),'live_validation':feature['live_validation']})
    return {'valid':all(b['passed'] for b in blocks),'errors':[],'blocks':blocks,
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
