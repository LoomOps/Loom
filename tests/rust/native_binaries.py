"""Exact acceptance artifacts; callers may supply candidate binaries explicitly."""
import os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def binary(name):
    return Path(os.environ.get('LOOM_TEST_'+name.removeprefix('loom-').upper(),str(ROOT/'target/debug'/name))).resolve()
