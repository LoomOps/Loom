"""Run isolated development/native checks. No operational deployment or research Jobs."""
import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
from zoneinfo import ZoneInfo

ROOT=Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-only',action='store_true',help='Omit native rules/protocol suites; browser fixtures still build their native candidate')
    parser.add_argument('--browser',action='store_true',help='Also exercise Firefox against the mock Ops fixture')
    parser.add_argument('--output',type=Path,help='JSON/JUnit report and browser screenshot directory')
    parser.add_argument('--suite',action='append',choices=['ops','dashboard','packaging','native','browser'],help='Run only these suites; default is the full candidate set')
    parser.add_argument('--rust-package',action='append',default=[],help='Select a Cargo package for native rule checks')
    parser.add_argument('--rust-filter',default='',help='Select qualified Rust test names')
    parser.add_argument('--python-pattern',default='test_*.py',help='unittest file pattern within selected Python suites')
    args=parser.parse_args()
    selected=set(args.suite or ['ops','dashboard','packaging','native'])
    if args.browser:selected.add('browser')
    if args.rust_package or args.rust_filter:
        if 'native' not in selected:parser.error('Rust selection requires the native suite')

    output=args.output or ROOT/'tests/evidence'/datetime.now(ZoneInfo('Asia/Seoul')).strftime('%Y%m%d-%H%M%S')
    output=output.resolve();output.mkdir(parents=True,exist_ok=True)
    suites={'ops':ROOT/'tests/ops','dashboard':ROOT/'tests/dashboard','packaging':ROOT/'tests/deploy'}
    if 'browser' in selected:suites['browser']=ROOT/'tests/dashboard/browser'
    suites={name:path for name,path in suites.items() if name in selected}
    # Build into a temporary file and compare; don't rewrite a running UI.
    import tempfile
    with tempfile.TemporaryDirectory(prefix='loom-test-build-') as temp:
        rendered=Path(temp)/'workspace.html'
        build=subprocess.run([sys.executable,'-B',str(ROOT/'dashboard/workspace/build.py'),str(ROOT/'dashboard/workspace/template.html'),'--output',str(rendered)],capture_output=True,text=True)
        if build.returncode:
            print(build.stderr,file=sys.stderr);return 1
        current=ROOT/'dashboard/public/workspace.html'
        if rendered.read_bytes()!=current.read_bytes():
            print('Generated dashboard is stale. Run dashboard/workspace/build.py before browser verification.',file=sys.stderr);return 1
    summary={'suites':{},'successful':True,'tests':0,'scope':'isolated fixtures; no deployment, production DB or GPU execution'}
    env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','LOOM_TEST_SCREENSHOTS':str(output/'screenshots')}
    if not args.reference_only and 'native' in selected:
        rust_args=[part for name in args.rust_package for part in ('--package',name)]
        if args.rust_filter:rust_args+=['--filter',args.rust_filter]
        narrowed=bool(args.rust_package or args.rust_filter)
        if narrowed:rust_args+=['--rules-only']
        rust=subprocess.run([sys.executable,'-B',str(ROOT/'tests/rust/run.py'),'--output',str(output),*rust_args],cwd=ROOT,env=env,capture_output=True,text=True)
        (output/'rust-run.log').write_text(rust.stdout+rust.stderr)
        print(rust.stdout,flush=True)
        for name in (('rust',) if narrowed else ('rust','native')):
            report=output/(name+'.json');value=json.loads(report.read_text()) if report.exists() else {'tests':0,'successful':False}
            summary['suites'][name]={'tests':value['tests'],'successful':value['successful']};summary['tests']+=value['tests'];summary['successful'] &= value['successful']
        summary['successful'] &= rust.returncode==0
    artifacts=output/'native-artifacts.json'
    if 'browser' in selected:
        if not artifacts.is_file() or args.reference_only or ('native' not in selected) or args.rust_package or args.rust_filter:
            cargo=os.environ.get('CARGO','cargo')
            compiled=subprocess.run([cargo,'build','-p','loom-ops','-p','loom-sdk','--locked','--message-format=json'],cwd=ROOT,env=env,capture_output=True,text=True)
            (output/'browser-native-build.log').write_text(compiled.stdout+compiled.stderr)
            if compiled.returncode:return compiled.returncode
            binaries={v['target']['name']:v['executable'] for line in compiled.stdout.splitlines() if line.startswith('{') for v in [json.loads(line)] if v.get('reason')=='compiler-artifact' and v.get('executable') and 'bin' in v['target']['kind']}
        else:binaries=json.loads(artifacts.read_text())
        if not {'loom-ops','loom-sdk'}<=binaries.keys():raise RuntimeError('Missing compiled browser candidate')
        env.update({'LOOM_TEST_'+name.removeprefix('loom-').upper():path for name,path in binaries.items()})
    summary['reference_only']=args.reference_only
    for name,directory in suites.items():
        report=output/(name+'.json')
        process=subprocess.run([sys.executable,'-B',str(ROOT/'tests/suite.py'),str(directory),str(report),'--pattern',args.python_pattern],cwd=ROOT,env=env,capture_output=True,text=True)
        (output/(name+'.log')).write_text(process.stdout+process.stderr)
        result=json.loads(report.read_text()) if report.is_file() else {'tests':0,'successful':False,'error':'Test runner did not produce a report'}
        result['exit_code']=process.returncode
        summary['suites'][name]={key:value for key,value in result.items() if key not in ('cases','sources')}
        summary['tests']+=result['tests'];summary['successful'] &= process.returncode==0 and result['successful']
        print(f"{name}: {result['tests']} tests · {'PASS' if process.returncode==0 and result['successful'] else 'FAIL'}",flush=True)
        if process.returncode:print(process.stderr,file=sys.stderr)
    summary['browser_requested']='browser' in selected
    summary['complete']=not args.reference_only and not args.rust_package and not args.rust_filter and args.python_pattern=='test_*.py' and {'ops','dashboard','packaging','native'}.issubset(selected)
    summary['selection']={'suites':sorted(selected),'rust_packages':args.rust_package,'rust_filter':args.rust_filter,'python_pattern':args.python_pattern}
    if summary['tests']==0:summary['successful']=False
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(str(output/'summary.json'))
    return 0 if summary['successful'] else 1

if __name__=='__main__':raise SystemExit(main())
