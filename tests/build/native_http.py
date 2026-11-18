"""Exercise the candidate native binary through HTTP and MCP."""
import os
from pathlib import Path
import sys
import unittest
os.environ['LOOM_TEST_OPS']=str(Path(sys.argv[1]).resolve())
os.environ['LOOM_TEST_SDK']=str(Path(sys.argv[2]).resolve())
os.environ['LOOM_USE_REFERENCE']=str(Path(sys.argv[3]).resolve().parent)
import test_service
large='test_large_catalog_read_does_not_occupy_the_mutation_lane'
names=unittest.defaultTestLoader.getTestCaseNames(test_service.RustServiceTests)
names=[large] if len(sys.argv)>4 and sys.argv[4]=='scale' else [name for name in names if name!=large]
suite=unittest.TestSuite(test_service.RustServiceTests(name) for name in names)
raise SystemExit(0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1)
