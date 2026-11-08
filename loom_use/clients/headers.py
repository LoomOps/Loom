"""Read an Admin-issued token. No issuance, sudo, renewal or network access."""
import json
import os
from pathlib import Path
import stat
import sys


def read_token(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd) as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
            raise PermissionError('Credential must be owned by this user and private (0600)')
        value = json.loads(stream.read(8192))
    token = value.get('token')
    if not isinstance(token, str) or not token or len(token) > 4096 or any(ord(c) < 33 or ord(c) > 126 for c in token):
        raise ValueError('Invalid credential')
    return token


if __name__ == '__main__':
    try:
        # stdout is consumed only by the MCP client, never a diagnostic report.
        print(json.dumps({'Authorization': 'Bearer ' + read_token(Path(sys.argv[1]))}))
    except (OSError, ValueError, IndexError):
        print('Loom credential unavailable; obtain a token from Dashboard Admin.', file=sys.stderr)
        raise SystemExit(1)
