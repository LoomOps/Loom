"""Load the exact client release used by this server, never a sibling checkout."""
import importlib.util,os,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('pinned_client_release',ROOT/'deploy/client_release.py')
release=importlib.util.module_from_spec(spec);spec.loader.exec_module(release)
PACKAGE=release.prepare(Path(os.environ.get('LOOM_USE_TEST_CACHE',str(Path.home()/'.cache/loom-use-contract'))))
CLIENTS=PACKAGE/'clients'
sys.path.insert(0,str(CLIENTS))
