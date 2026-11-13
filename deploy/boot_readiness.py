"""Wait for exact Compose bind addresses; never change network or login state."""
import argparse
import ipaddress
import json
import math
import re
import subprocess
import sys
import time


def published_addresses(config):
    addresses=set()
    for service in config.get('services',{}).values():
        for port in service.get('ports',[]) or []:
            if not isinstance(port,dict):raise ValueError('Expected expanded Compose port configuration')
            host=port.get('host_ip')
            if not host:continue
            address=ipaddress.ip_address(host)
            if not address.is_unspecified and not address.is_loopback:addresses.add(str(address))
    return addresses


def local_addresses(timeout):
    result=subprocess.run(['ip','-j','address','show'],capture_output=True,text=True,timeout=timeout,check=True)
    rows=json.loads(result.stdout)
    values=set()
    for interface in rows:
        for item in interface.get('addr_info',[]):
            address=ipaddress.ip_address(item['local'])
            if item.get('tentative') or item.get('dadfailed') or {'tentative','dadfailed'}.intersection(item.get('flags',[])):continue
            values.add(str(address))
            if address.version==6:values.add(str(address)+'%'+interface['ifname'])
    return values


def wait_for_addresses(addresses,timeout=120,*,probe=local_addresses,clock=time.monotonic,sleep=time.sleep):
    if not math.isfinite(timeout) or timeout<0:raise ValueError('A finite nonnegative address timeout is required')
    if not addresses:return
    deadline=clock()+timeout
    while True:
        try:missing=addresses-probe(max(.01,min(5,deadline-clock())))
        except (OSError,ValueError,subprocess.SubprocessError):missing=addresses
        if not missing:return
        remaining=deadline-clock()
        if remaining<=0:raise TimeoutError('Configured listener addresses are not ready: '+', '.join(sorted(missing)))
        sleep(min(1,remaining))


def _longer_timeout(value,minimum):
    if value in ('infinity','0'):return value
    units={'':1,'s':1,'sec':1,'m':60,'min':60,'h':3600,'hr':3600,'d':86400,'ms':.001,'us':.000001}
    compact=re.sub(r'\s+','',value)
    parts=re.findall(r'([0-9]+(?:\.[0-9]+)?)(min|sec|ms|us|hr|[smhd])?',compact)
    if not parts or ''.join(a+b for a,b in parts)!=compact:return value
    seconds=sum(float(a)*units[b] for a,b in parts)
    return value if seconds>=minimum else str(minimum)


def harden_unit(body,*,stack=False):
    """Set boot retry policy without replacing paths, listeners, mounts or overrides."""
    blocks=[]
    for line in body.splitlines():
        if line.strip().startswith('[') and line.strip().endswith(']'):
            blocks.append((line.strip()[1:-1],[line]))
        elif blocks:blocks[-1][1].append(line)
        else:blocks.append(('',[line]))
    timeout='300'
    for section,lines in blocks:
        if section=='Service':
            for line in lines:
                if line.strip().startswith('TimeoutStartSec='):timeout=_longer_timeout(line.split('=',1)[1].strip(),300)
    policy={'Unit':{'StartLimitIntervalSec':'0'},'Service':{'Restart':'on-failure','RestartSec':'15'}}
    if stack:policy['Service']['TimeoutStartSec']=timeout
    for section,settings in policy.items():
        for name,lines in blocks:
            if name==section:lines[:]=[line for line in lines if line.strip().split('=',1)[0] not in settings]
        target=next((lines for name,lines in reversed(blocks) if name==section),None)
        if target is None:target=['['+section+']'];blocks.append((section,target))
        while target and not target[-1].strip():target.pop()
        target.extend(key+'='+value for key,value in settings.items());target.append('')
    return '\n'.join(line for _,lines in blocks for line in lines).rstrip()+'\n'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--timeout',type=float,default=120)
    parser.add_argument('compose_command',nargs=argparse.REMAINDER)
    args=parser.parse_args()
    command=args.compose_command
    if command[:1]==['--']:command=command[1:]
    if not command:parser.error('Provide the configured Compose command after --')
    if not math.isfinite(args.timeout) or args.timeout<=0:parser.error('Timeout must be finite and positive')
    started=time.monotonic()
    try:
        # Never echo expanded Compose configuration, environment or provider output.
        result=subprocess.run([*command,'config','--format','json'],capture_output=True,text=True,
                              timeout=min(20,args.timeout),check=True)
        addresses=published_addresses(json.loads(result.stdout))
        wait_for_addresses(addresses,max(0,args.timeout-(time.monotonic()-started)))
    except ValueError:
        print('Invalid expanded Compose listener configuration; startup was not attempted',file=sys.stderr);return 2
    except (OSError,subprocess.SubprocessError) as exc:
        print(str(exc) if isinstance(exc,TimeoutError) else 'Listener readiness probe failed; startup was not attempted',file=sys.stderr);return 1
    return 0


if __name__=='__main__':raise SystemExit(main())
