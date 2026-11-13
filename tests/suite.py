"""Internal unittest runner with machine-readable and JUnit results."""
import argparse
import json
import hashlib
from pathlib import Path
import sys
import time
import unittest
import xml.etree.ElementTree as ET

class Result(unittest.TextTestResult):
    def __init__(self,*args,**kwargs):
        super().__init__(*args,**kwargs);self.records={};self.started={}
    def startTest(self,test):
        super().startTest(test);self.started[test.id()]=time.monotonic()
        self.records[test.id()]={'name':test.id(),'status':'passed'}
    def record(self,test,status,detail=''):
        self.records[test.id()]={'name':test.id(),'status':status,'detail':detail}
    def addFailure(self,test,err):
        super().addFailure(test,err);self.record(test,'failed',self._exc_info_to_string(err,test))
    def addError(self,test,err):
        super().addError(test,err);self.record(test,'error',self._exc_info_to_string(err,test))
    def addSkip(self,test,reason):
        super().addSkip(test,reason);self.record(test,'skipped',reason)
    def addExpectedFailure(self,test,err):
        super().addExpectedFailure(test,err);self.record(test,'expected_failure',self._exc_info_to_string(err,test))
    def addUnexpectedSuccess(self,test):
        super().addUnexpectedSuccess(test);self.record(test,'unexpected_success','Unexpected success of an expected-failure test')
    def addSubTest(self,test,subtest,err):
        super().addSubTest(test,subtest,err)
        if err:self.record(subtest,'failed',self._exc_info_to_string(err,test));self.record(test,'failed','Subtest failed')
    def stopTest(self,test):
        self.records[test.id()]['seconds']=time.monotonic()-self.started[test.id()];super().stopTest(test)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('directory',type=Path);p.add_argument('report',type=Path);a=p.parse_args()
    root=Path(__file__).resolve().parents[1]
    component=a.directory.resolve().relative_to(root/'tests').parts[0]
    sys.path[:0]=[str(a.directory.resolve()),str(root/component)]
    suite=unittest.defaultTestLoader.discover(str(a.directory),pattern='test_*.py')
    catalog=json.loads((root/'docs/features/catalog.json').read_text())
    refs={p for f in catalog['features'] for p in f['sources']+f['tests']+f['contracts']+[f['spec']]}
    tracked=[root/p for p in sorted(refs)]
    sources={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in tracked}
    started=time.monotonic();result=unittest.TextTestRunner(verbosity=2,resultclass=Result).run(suite)
    report={'sources':sources,'tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),
        'skipped':len(result.skipped),'expected_failures':len(result.expectedFailures),
        'unexpected_successes':len(result.unexpectedSuccesses),
        'seconds':time.monotonic()-started,'successful':result.wasSuccessful(),
        'cases':list(result.records.values())}
    a.report.parent.mkdir(parents=True,exist_ok=True);a.report.write_text(json.dumps(report,indent=2)+'\n')
    xml=ET.Element('testsuite',name=a.directory.name,tests=str(result.testsRun),failures=str(len(result.failures)+len(result.unexpectedSuccesses)),errors=str(len(result.errors)),skipped=str(len(result.skipped)+len(result.expectedFailures)),time=str(report['seconds']))
    for case in report['cases']:
        item=ET.SubElement(xml,'testcase',name=case['name'],time=str(case.get('seconds',0)))
        if case['status']!='passed':ET.SubElement(item,{'failed':'failure','error':'error','skipped':'skipped','expected_failure':'skipped','unexpected_success':'failure'}[case['status']]).text=case.get('detail','')
    ET.ElementTree(xml).write(a.report.with_suffix('.xml'),encoding='utf-8',xml_declaration=True)
    raise SystemExit(0 if result.wasSuccessful() else 1)
