"""Run the API and durable worker in one Loom Ops container."""
import sys
import os
import signal
import subprocess
import time


def main():
    stopping = False

    def stop(*_):
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    api_env = {**os.environ, 'LOOM_CONFIG_PATH': os.environ['LOOM_API_CONFIG']}
    worker_env = {**os.environ, 'LOOM_CONFIG_PATH': os.environ['LOOM_WORKER_CONFIG']}
    api = subprocess.Popen([sys.executable, '/app/ops.py', 'api'], env=api_env)
    worker = None
    failed = False
    try:
        command = [sys.executable, '/app/ops.py', 'worker']
        worker = subprocess.Popen(command, env=worker_env)
        while not stopping:
            if api.poll() is not None or worker.poll() is not None:
                failed = True
                break
            time.sleep(.2)
    finally:
        # Keep the API available while the worker returns its execution grant.
        for child in (worker, api):
            if child is None:
                continue
            if child.poll() is None:
                child.terminate()
            child.wait()
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
