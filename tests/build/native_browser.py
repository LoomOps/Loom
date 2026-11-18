"""Local Firefox acceptance consumes the selected Ops and generated HTML."""
import os
from pathlib import Path
import sys
import unittest
for key,path in zip(['LOOM_TEST_OPS','LOOM_TEST_SDK','LOOM_TEST_WORKSPACE'],sys.argv[1:]):os.environ[key]=str(Path(path).resolve())
import test_native_workspace
names=['test_recent_group_execution_stays_visible_after_the_original_root_completed']
suite=unittest.TestSuite(test_native_workspace.NativeWorkspaceTests(name) for name in names)
raise SystemExit(0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1)
