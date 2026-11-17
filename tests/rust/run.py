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
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--cargo',default=os.environ.get('CARGO','cargo'));a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    start=time.monotonic();catalog=json.loads((ROOT/'docs/features/catalog.json').read_text());refs={s for f in catalog['features'] for s in f['sources']+f['tests']+f['contracts']+[f['spec']]};sources={s:hashlib.sha256((ROOT/s).read_bytes()).hexdigest() for s in sorted(refs)}
    compile=subprocess.run([a.cargo,'test','--workspace','--locked','--no-run','--message-format=json'],cwd=ROOT,capture_output=True,text=True);(a.output/'rust-compile.log').write_text(compile.stderr)
    if compile.returncode:return compile.returncode
    executables=[json.loads(line) for line in compile.stdout.splitlines() if line.startswith('{')];executables={v['executable'] for v in executables if v.get('reason')=='compiler-artifact' and v.get('profile',{}).get('test') and v.get('executable')}
    index={};errors=[];cases=[]
    for path in sorted((ROOT/'tests/rust').glob('*.rs')):
        for fn in declarations(path):
            if fn in index:errors.append('Ambiguous Rust test declaration: '+fn)
            index[fn]=path
    seen=set()
    for executable in sorted(executables):
        listing=subprocess.run([executable,'--list','--format','terse'],capture_output=True,text=True,timeout=30)
        if listing.returncode:errors.append('Rust test listing failed');continue
        for line in listing.stdout.splitlines():
            if not line.endswith(': test'):continue
            full=line.removesuffix(': test');fn=full.split('::')[-1];path=index.get(fn)
            if path is None:errors.append('Unmapped native Rust test: '+full);continue
            seen.add(fn);cases.append(run_case(executable,full,path.relative_to(ROOT),SCALE_CASE_TIMEOUTS.get(fn,60)))
    errors.extend('Rust test was not executed: '+fn for fn in index.keys()-seen)
    report={'tests':len(cases),'successful':not errors and all(c['status']=='passed' for c in cases),'sources':sources,'cases':cases,'errors':errors,'seconds':time.monotonic()-start}
    (a.output/'rust.json').write_text(json.dumps(report,indent=2)+'\n')
    if not report['successful']:print(json.dumps(report,indent=2));return 1
    build=subprocess.run([a.cargo,'build','--workspace','--locked'],cwd=ROOT,capture_output=True,text=True);(a.output/'rust-build.log').write_text(build.stdout+build.stderr)
    if build.returncode:return build.returncode
    # Black-box tests must load binaries from this compilation, not a borrowed installation.
    proc=subprocess.run([sys.executable,'-B',str(ROOT/'tests/suite.py'),str(ROOT/'tests/rust'),str(a.output/'native.json')],cwd=ROOT,capture_output=True,text=True)
    (a.output/'native.log').write_text(proc.stdout+proc.stderr)
    print(f"rust: {len(cases)} native rule/fault cases; protocol exit {proc.returncode}")
    return proc.returncode
if __name__=='__main__':raise SystemExit(main())
