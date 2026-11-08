"""Stdio transport for the installed Ops JSON-RPC endpoint."""
import json
import os
from pathlib import Path
import stat
import sys
from urllib.parse import urlsplit
from urllib.request import Request, build_opener, HTTPRedirectHandler

LIMIT = 2 * 1024 * 1024


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class OpsTransport:
    def __init__(self, config_path):
        config = json.loads(config_path.read_text())
        self.endpoint = config['endpoint']
        url = urlsplit(self.endpoint)
        if (url.path != '/mcp' or url.username or url.password or url.query or url.fragment
                or not url.hostname
                or (url.scheme != 'https' and not (url.scheme == 'http' and url.hostname in {'127.0.0.1', '::1'}))):
            raise ValueError('Expected the installed TLS or loopback MCP endpoint')
        self.token = None
        self.protocol_version = None
        if config.get('credential'):
            with Path(config['credential']).open() as stream:
                info = os.fstat(stream.fileno())
                if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
                    raise PermissionError('Expected an operator-owned private credential file')
                self.token = json.load(stream)['token']
        elif config.get('authentication') != 'tailscale':
            raise ValueError('Set credential or authentication=tailscale')
        self.opener = build_opener(NoRedirect())

    def rpc(self, message):
        headers={'Content-Type':'application/json','Accept':'application/json, text/event-stream'}
        if self.token:headers['Authorization']='Bearer '+self.token
        if self.protocol_version:headers['MCP-Protocol-Version']=self.protocol_version
        request = Request(self.endpoint, data=json.dumps(message).encode(),headers=headers)
        with self.opener.open(request, timeout=20) as response:
            if response.status == 202 and 'id' not in message:
                return None
            content = response.read(LIMIT + 1)
            media=response.headers.get_content_type()
        if len(content) > LIMIT:
            raise ValueError('MCP response exceeds transport bound')
        if media=='text/event-stream':
            replies=[]
            for event in content.decode().replace('\r\n','\n').split('\n\n'):
                data='\n'.join(line[5:].lstrip(' ') for line in event.split('\n') if line.startswith('data:'))
                if data:
                    item=json.loads(data)
                    if item.get('id')==message.get('id') and ('result' in item or 'error' in item):replies.append(item)
            if len(replies)!=1:raise ValueError('Expected exactly one MCP response')
            reply=replies[0]
        elif media=='application/json':
            reply=json.loads(content)
        else:
            raise ValueError('Unexpected MCP response type')
        if message.get('method')=='initialize' and 'result' in reply:
            self.protocol_version=reply['result']['protocolVersion']
        return reply


def serve(transport):
    while raw := sys.stdin.buffer.readline(LIMIT + 1):
        if len(raw) > LIMIT:
            raise ValueError('MCP input exceeds transport bound')
        message = json.loads(raw)
        try:
            reply = transport.rpc(message)
        except (OSError, ValueError) as error:
            if 'id' not in message:
                continue
            reply = {'jsonrpc': '2.0', 'id': message['id'],
                     'error': {'code': -32000, 'message': 'Ops transport: ' + type(error).__name__}}
        if reply is not None:
            print(json.dumps(reply, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    serve(OpsTransport(Path(sys.argv[1])))
