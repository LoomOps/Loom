"""Serve the live checkout, rebuild workspace edits and reload Python changes."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent


def signature(paths):
    return tuple((str(p), p.stat().st_mtime_ns, p.stat().st_size) for p in sorted(paths) if p.is_file())


def main():
    gateway=os.environ.get('LOOM_UI_ROLE')=='gateway'
    entry='gateway.py' if gateway else 'server.py'
    stopping = False

    def stop(*_):
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    process = None
    last = None
    server_version = None
    try:
        while not stopping:
            current = signature([*ROOT.joinpath('workspace').glob('*'), *ROOT.joinpath('branding').glob('*'), ROOT/'theme/theme.js'])
            if current != last and not gateway:
                staged = ROOT / 'public/workspace.pending.html'
                result = subprocess.run([sys.executable, '-B', str(ROOT/'workspace/build.py'),
                    str(ROOT/'workspace/template.html'), '--output', str(staged)])
                if result.returncode == 0:
                    os.replace(staged, ROOT/'public/workspace.html')
                last = current
            version = signature([ROOT/'server.py',ROOT/'sso.py',ROOT/'admission.py',ROOT/'sharing.py',ROOT/'admission_http.py',ROOT/'gateway.py'])
            if process is None or version != server_version:
                if process is not None and process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
                process = subprocess.Popen([sys.executable, '-B', str(ROOT/entry)])
                server_version = version
            if process.poll() is not None:
                return process.returncode or 1
            time.sleep(.5)
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
