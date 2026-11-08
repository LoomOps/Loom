"""Run isolated development checks. Does not build, deploy, or submit real Jobs."""
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
    parser.add_argument('--browser',action='store_true',help='Also exercise Firefox against the mock Ops fixture')
    parser.add_argument('--output',type=Path,help='JSON/JUnit report and browser screenshot directory')
    args=parser.parse_args()
    output=args.output or ROOT/'tests/evidence'/datetime.now(ZoneInfo('Asia/Seoul')).strftime('%Y%m%d-%H%M%S')
    output=output.resolve();output.mkdir(parents=True,exist_ok=True)
    suites={'ops':ROOT/'tests/ops','dashboard':ROOT/'tests/dashboard','packaging':ROOT/'tests/deploy'}
    if args.browser:suites['browser']=ROOT/'tests/dashboard/browser'
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
    for name,directory in suites.items():
        report=output/(name+'.json')
        process=subprocess.run([sys.executable,'-B',str(ROOT/'tests/suite.py'),str(directory),str(report)],cwd=ROOT,env=env,capture_output=True,text=True)
        (output/(name+'.log')).write_text(process.stdout+process.stderr)
        result=json.loads(report.read_text()) if report.is_file() else {'tests':0,'successful':False,'error':'Test runner did not produce a report'}
        result['exit_code']=process.returncode
        summary['suites'][name]={key:value for key,value in result.items() if key!='cases'}
        summary['tests']+=result['tests'];summary['successful'] &= process.returncode==0 and result['successful']
        print(f"{name}: {result['tests']} tests · {'PASS' if process.returncode==0 and result['successful'] else 'FAIL'}",flush=True)
        if process.returncode:print(process.stderr,file=sys.stderr)
    summary['browser_requested']=args.browser
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(str(output/'summary.json'))
    return 0 if summary['successful'] else 1

if __name__=='__main__':raise SystemExit(main())
