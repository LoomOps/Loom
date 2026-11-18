"""Compile and execute every native Rust test, then real independent protocol fixtures."""
import argparse,hashlib,json,os,re,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
# This 100k-entry correctness fixture is intentionally larger than ordinary rules.
SCALE_CASE_TIMEOUTS={'inventory_projection_is_chunked_and_core_growth_does_not_follow_file_count':120}
def declarations(path):
    return re.findall(r'#\[test\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z_0-9]*)',path.read_text())
def run_case(executable,full,path,timeout=60):
    case={'name':path.stem+'.Rust.'+full.split('::')[-1],'source':str(path),'status':'failed','timeout_seconds':timeout}
    started=time.monotonic()
    try:
        result=subprocess.run([executable,full,'--exact','--nocapture'],capture_output=True,text=True,timeout=timeout)
    except subprocess.TimeoutExpired:
        return {**case,'seconds':time.monotonic()-started,'detail':f'Native test exceeded {timeout} seconds'}
    case['seconds']=time.monotonic()-started
    output=result.stdout+result.stderr
    # libtest returns zero for an ignored test and for a filter that ran nothing.
    if result.returncode==0 and re.search(r'^test result: ok\. 1 passed; 0 failed; 0 ignored;',output,re.M):case['status']='passed'
    elif result.returncode==0 and re.search(r'^test result: ok\. 0 passed; 0 failed; 1 ignored;',output,re.M):case['status']='skipped'
    if case['status']!='passed':case['detail']=output
    return case
def test_index(source, prefix=''):
    """Resolve Rust modules within one Cargo target; names may repeat across targets."""
    index={prefix+name:source for name in declarations(source)}
    text=source.read_text()
    pattern=r'(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub\s+)?mod\s+(\w+)\s*;'
    for match in re.finditer(pattern,text):
        explicit,name=match.groups()
        if explicit:child=source.parent/explicit
        else:
            base=source.parent if source.name in ('lib.rs','main.rs','mod.rs') else source.parent/source.stem
            child=base/(name+'.rs')
            if not child.is_file():child=base/name/'mod.rs'
        if child.is_file():
            for key,path in test_index(child.resolve(),prefix+name+'::').items():
                if key in index:raise ValueError('Duplicate qualified Rust test: '+key)
                index[key]=path
    return index

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--cargo',default=os.environ.get('CARGO','cargo'))
    p.add_argument('--package',action='append',default=[],help='Select Cargo packages; omitted packages are not reported as passing')
    p.add_argument('--filter',default='',help='Select fully qualified libtest names containing this text')
    p.add_argument('--rules-only',action='store_true',help='Skip independent Python protocol fixtures')
    a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    start=time.monotonic();catalog=json.loads((ROOT/'docs/features/catalog.json').read_text())
    refs={s for f in catalog['features'] for s in f['sources']+f['tests']+f['contracts']+[f['spec']]}
    sources={s:hashlib.sha256((ROOT/s).read_bytes()).hexdigest() for s in sorted(refs)}
    selection=[part for name in a.package for part in ('-p',name)] or ['--workspace']
    command=[a.cargo,'test',*selection,'--locked','--no-run','--message-format=json']
    compile=subprocess.run(command,cwd=ROOT,capture_output=True,text=True)
    (a.output/'rust-compile.log').write_text(compile.stderr)
    if compile.returncode:return compile.returncode
    artifacts=[json.loads(line) for line in compile.stdout.splitlines() if line.startswith('{')]
    executables={v['executable']:v for v in artifacts if v.get('reason')=='compiler-artifact' and v.get('profile',{}).get('test') and v.get('executable')}
    errors=[];cases=[];omitted=[]
    for executable,artifact in sorted(executables.items()):
        target=artifact['target'];index=test_index(Path(target['src_path']));seen=set()
        listing=subprocess.run([executable,'--list','--format','terse'],capture_output=True,text=True,timeout=30)
        if listing.returncode:errors.append('Rust test listing failed: '+target['name']);continue
        for line in listing.stdout.splitlines():
            if not line.endswith(': test'):continue
            full=line.removesuffix(': test');path=index.get(full)
            if path is None:errors.append('Unmapped native Rust test: '+target['name']+'::'+full);continue
            seen.add(full)
            identity='loom::'+target['name']+'::'+full
            if a.filter and a.filter not in full:omitted.append(identity);continue
            case=run_case(executable,full,path.relative_to(ROOT),SCALE_CASE_TIMEOUTS.get(full.split('::')[-1],60))
            case.update(id=identity,target=target['name'],qualified_name=full,executed=True)
            cases.append(case)
        errors.extend('Rust test was not listed: '+target['name']+'::'+full for full in index.keys()-seen)
    if not cases:errors.append('Selection did not execute any Rust tests')
    report={'schema':'loom-rust-tests/v2','complete':not a.package and not a.filter and not a.rules_only,
            'selection':{'packages':a.package,'filter':a.filter,'rules_only':a.rules_only},'omitted':omitted,
            'tests':len(cases),'successful':not errors and all(c['status']=='passed' for c in cases),
            'sources':sources,'cases':cases,'errors':errors,'seconds':time.monotonic()-start,'command':command}
    (a.output/'rust.json').write_text(json.dumps(report,indent=2)+'\n')
    if not report['successful']:print(json.dumps({'errors':errors,'failed':[c for c in cases if c['status']!='passed']},indent=2));return 1
    if a.rules_only:
        print(f"rust: {len(cases)} selected native rule/fault cases; protocol fixtures omitted")
        return 0
    build=subprocess.run([a.cargo,'build','--workspace','--locked','--message-format=json'],cwd=ROOT,capture_output=True,text=True)
    (a.output/'rust-build.log').write_text(build.stdout+build.stderr)
    if build.returncode:return build.returncode
    binaries={}
    for line in build.stdout.splitlines():
        if not line.startswith('{'):continue
        artifact=json.loads(line)
        if artifact.get('reason')=='compiler-artifact' and artifact.get('executable') and 'bin' in artifact['target']['kind']:
            name=artifact['target']['name']
            if name.startswith('loom-'):binaries[name]=artifact['executable']
    required={'loom-ops','loom-host','loom-sdk','loom-migrate'}
    if not required<=binaries.keys():raise RuntimeError('Cargo did not return required protocol artifacts')
    (a.output/'native-artifacts.json').write_text(json.dumps(binaries,indent=2)+'\n')
    env={**os.environ,**{'LOOM_TEST_'+name.removeprefix('loom-').upper():path for name,path in binaries.items()}}
    proc=subprocess.run([sys.executable,'-B',str(ROOT/'tests/suite.py'),str(ROOT/'tests/rust'),str(a.output/'native.json')],cwd=ROOT,env=env,capture_output=True,text=True)
    (a.output/'native.log').write_text(proc.stdout+proc.stderr)
    print(f"rust: {len(cases)} native rule/fault cases; protocol exit {proc.returncode}")
    return proc.returncode
if __name__=='__main__':raise SystemExit(main())
