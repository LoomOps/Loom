"""Configure discovery only; never installs a release or restarts services."""
import argparse
import getpass
import json
import os
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'ops'))
from release_updates import settings


def configure(root, repository='LoomOps/loom', channel='alpha', interval=21600, enabled=True, token=None):
    root = Path(root)
    if not (root / 'compose.env').is_file():
        raise ValueError('Choose an installed instance')
    env = {'LOOM_RELEASE_REPOSITORY': repository, 'LOOM_RELEASE_CHANNEL': channel,
           'LOOM_RELEASE_INTERVAL': str(interval), 'LOOM_RELEASE_CHECK_ENABLED': '1' if enabled else '0'}
    settings({}, env)
    if token is not None and (not token or len(token) > 4096 or any(c.isspace() for c in token)):
        raise ValueError('Invalid credential')
    owner = (root / 'runtime').stat()
    directory = root / 'secrets/releases'
    if directory.is_symlink():
        raise ValueError('Credential directory must not be a symlink')
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chown(directory, owner.st_uid, owner.st_gid)
    if token is not None:
        path = directory / 'github-token'
        temp = directory / '.github-token.tmp'
        fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'w') as stream:
            stream.write(token + '\n')
        os.chown(temp, owner.st_uid, owner.st_gid)
        temp.replace(path)
    config = root / 'release-check.env'
    temp = root / '.release-check.env.tmp'
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'w') as stream:
        stream.write(''.join(f'{key}={value}\n' for key, value in env.items()))
    os.chown(temp, owner.st_uid, owner.st_gid)
    temp.replace(config)
    return {'configured': True, 'repository': repository, 'channel': channel, 'interval': interval,
            'enabled': enabled, 'credential_present': (directory / 'github-token').is_file(),
            'restart_required': True, 'automatic_install': False}


if __name__ == '__main__':
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument('--root', type=Path, default=Path('/loom/ops'))
    cli.add_argument('--repository', default='LoomOps/loom')
    cli.add_argument('--channel', choices=['alpha', 'beta', 'rc', 'stable'], default='alpha')
    cli.add_argument('--interval', type=int, default=21600)
    cli.add_argument('--disable', action='store_true')
    credentials = cli.add_mutually_exclusive_group()
    credentials.add_argument('--credential', action='store_true', help='Read a Contents:read GitHub token without echo')
    credentials.add_argument('--token-stdin', action='store_true', help='Read the token from a pipe; never a command argument')
    args = cli.parse_args()
    token = sys.stdin.read().strip() if args.token_stdin else getpass.getpass('GitHub read token: ') if args.credential else None
    print(json.dumps(configure(args.root, args.repository, args.channel, args.interval, not args.disable, token)))
