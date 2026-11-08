"""Configure and diagnose a resident Codex connection using an Admin-issued token."""
import argparse
import getpass
import json
import os
from pathlib import Path
import re
import shlex
import stat
import sys
import tempfile
import tomllib

from headers import read_token
from stdio import OpsTransport


def atomic_private(path, text):
    if path.is_symlink():
        raise PermissionError('Refusing a symlink destination')
    fd, temporary = tempfile.mkstemp(prefix='.loom-', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            stream.write(text); stream.flush(); os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)


def private_directory(path):
    path = path.expanduser().absolute()
    if any(p.is_symlink() for p in [path, *path.parents]):
        raise PermissionError('Use a directory without symlink ancestors')
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = path.stat()
    if info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise PermissionError('Credential directory must be user-owned and private (0700)')
    return path


def entry(directory, endpoint):
    return {'url': endpoint, 'http_headers_helper': shlex.join([
        sys.executable, str(directory/'headers.py'), str(directory/'credential.json')])}


def config_text(original, name, expected):
    """Replace only a flat connection table, preserving all unrelated TOML."""
    before = tomllib.loads(original)
    existing = before.get('mcp_servers', {}).get(name)
    if existing is not None and existing.get('url') != expected['url']:
        raise ValueError('Connection name already targets another server; choose another --name')
    starts = list(re.finditer(r'^\[([^\n]+)\][ \t]*(?:#.*)?$', original, re.M))
    selected = []
    for i, match in enumerate(starts):
        # Parse headers instead of guessing quoting or dotted names.
        try: table = tomllib.loads(match.group(0)+'\n').get('mcp_servers', {})
        except tomllib.TOMLDecodeError: continue
        if name in table:
            selected.append((match.start(), starts[i+1].start() if i+1 < len(starts) else len(original)))
    remaining = original
    for start, end in reversed(selected): remaining = remaining[:start]+remaining[end:]
    addition = '\n[mcp_servers.'+json.dumps(name)+']\n'
    addition += ''.join(key+' = '+json.dumps(value, ensure_ascii=False)+'\n' for key,value in expected.items())
    result = remaining.rstrip()+'\n'+addition
    wanted = json.loads(json.dumps(before))
    wanted.setdefault('mcp_servers', {})[name] = expected
    if tomllib.loads(result) != wanted:
        raise ValueError('Cannot safely edit this configuration; move the matching MCP table to user config first')
    return result


def verify(directory):
    read_token(directory/'credential.json')
    transport = OpsTransport(directory/'client.json')
    def rpc(method, params=None):
        reply = transport.rpc({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})
        if not reply or 'error' in reply or reply['result'].get('isError'):
            raise ValueError('MCP verification failed')
        return reply['result']
    rpc('initialize', {'protocolVersion':'2025-03-26','capabilities':{},'clientInfo':{'name':'loom-client-doctor','version':'1'}})
    transport.rpc({'jsonrpc':'2.0','method':'notifications/initialized'})
    names = [t['name'] for t in rpc('tools/list')['tools']]
    if 'system' not in names: raise ValueError('Loom system tool is unavailable')
    result = rpc('tools/call', {'name':'system','arguments':{'action':'status'}})
    status = json.loads(result['content'][0]['text'])
    rpc('tools/call', {'name':'system','arguments':{'action':'guide'}})
    principal = status['principal']
    if not principal.get('credential_id'):
        raise ValueError('Use an Admin-issued agent token, not an installation credential')
    return {'status':'verified','endpoint':transport.endpoint,'owner':principal['id'],
            'machine_id':status['machine_id'],'credential_id':principal['credential_id'],'scopes':principal['scopes'],'tools':names}


def configure(directory, config, name, endpoint, token):
    original = config.read_text() if config.exists() else ''
    updated = config_text(original, name, entry(directory, endpoint))
    # Validate endpoint before writing the supplied secret.
    with tempfile.TemporaryDirectory(prefix='loom-client-') as temp:
        candidate = Path(temp)
        atomic_private(candidate/'credential.json', json.dumps({'token':token})+'\n')
        atomic_private(candidate/'client.json', json.dumps({'endpoint':endpoint,'credential':str(candidate/'credential.json')}))
        report = verify(candidate)
    directory = private_directory(directory)
    atomic_private(directory/'.gitignore', '*\n')
    atomic_private(directory/'headers.py', Path(__file__).with_name('headers.py').read_text())
    atomic_private(directory/'credential.json', json.dumps({'token':token})+'\n')
    atomic_private(directory/'client.json', json.dumps({'endpoint':endpoint,'credential':str(directory/'credential.json')}))
    config.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if config.is_symlink(): raise PermissionError('Refusing a symlink Codex config')
    # Refuse to overwrite a concurrent client configuration edit.
    if (config.read_text() if config.exists() else '') != original:
        raise ValueError('Codex configuration changed; retry setup')
    if original: atomic_private(config.with_name('config.toml.loom-backup'), original)
    atomic_private(config, updated)
    return {**report,'credential_file':str(directory/'credential.json'),'config':str(config),
            'next':'Reconnect this MCP server in Codex. No token renewal is required.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('configure','doctor'))
    parser.add_argument('--name', default='loom-use', help='MCP connection name (default: loom-use)')
    parser.add_argument('--endpoint')
    parser.add_argument('--directory', type=Path, help='Private credential directory; default: CODEX_HOME/loom/<name>')
    args = parser.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,80}', args.name): parser.error('Use a simple connection name')
    home = Path(os.environ.get('CODEX_HOME', str(Path.home()/'.codex'))).expanduser().absolute()
    directory = (args.directory or home/'loom'/args.name).expanduser().absolute()
    config = home/'config.toml'
    if args.action == 'configure':
        if not args.endpoint: parser.error('--endpoint is required')
        if not sys.stdin.isatty(): parser.error('Run in an interactive terminal; the token is entered with echo disabled')
        token = getpass.getpass('Token issued by Loom Dashboard Admin: ').strip()
        report = configure(directory, config, args.name, args.endpoint, token)
    else:
        report = verify(directory)
        configured = tomllib.loads(config.read_text()).get('mcp_servers', {}).get(args.name)
        if configured != entry(directory, report['endpoint']):
            raise ValueError('Codex MCP configuration does not match the private credential helper')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    try: main()
    except (OSError, ValueError, KeyError) as error:
        # Do not print server payloads, request headers or secret-bearing exceptions.
        print('Client setup incomplete ('+type(error).__name__+'). Check the endpoint, Admin-issued token and local configuration.', file=sys.stderr)
        raise SystemExit(1)
