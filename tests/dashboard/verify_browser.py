"""Exercise the dashboard in Firefox against an actual isolated Loom Ops node."""
import argparse
import base64
import json
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


FIREFOX = Path("/snap/firefox/current/usr/lib/firefox")


def request(url, body=None, method=None, token=None):
    data = json.dumps(body).encode() if body is not None else None
    headers = {"Content-Type": "application/json"}
    if token is not None:
        headers["Authorization"] = "Bearer " + token
    with urlopen(Request(url, data=data, method=method, headers=headers), timeout=20) as response:
        return json.load(response)






def main(exercise):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--tokens", type=Path)
    parser.add_argument("--inactive-tokens", type=Path)
    parser.add_argument("--screenshot", type=Path)
    args = parser.parse_args()
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    with tempfile.TemporaryDirectory(prefix="loom-dashboard-firefox-") as profile:
        driver = subprocess.Popen([str(FIREFOX / "geckodriver"), "--port", str(port), "--profile-root", profile],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        base = f"http://127.0.0.1:{port}"
        session = None
        try:
            for _ in range(30):
                try:
                    request(base + "/status")
                    break
                except URLError:
                    if driver.poll() is not None:
                        raise
                    time.sleep(0.1)
            created = request(base + "/session", {"capabilities": {"alwaysMatch": {
                "browserName": "firefox", "moz:firefoxOptions": {"args": ["-headless"], "binary": str(FIREFOX / "firefox")}
            }}})["value"]
            session = base + "/session/" + created["sessionId"]
            def wd(path, body=None):
                return request(session + path, body)["value"]
            tokens = json.load(sys.stdin) if str(args.tokens) == '-' else json.loads(args.tokens.read_text()) if args.tokens else {}
            inactive = json.loads(args.inactive_tokens.read_text()) if args.inactive_tokens else {}
            result = exercise(wd, args.base, tokens, args.screenshot, inactive)
            result["firefox"] = created["capabilities"]["browserVersion"]
            print(json.dumps(result, indent=2))
        finally:
            if session:
                try:
                    request(session, method="DELETE")
                except URLError:
                    pass
            if driver.poll() is None:
                driver.terminate()
                driver.wait(timeout=10)


if __name__ == "__main__":
    raise SystemExit("Run tests/run.py --browser or verify_workspace.py")
