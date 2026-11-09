"""Read only the authenticated principal's optional host SSH connection descriptor."""
import json,os
from pathlib import Path


def connection(actor):
    path=Path(os.environ.get('LOOM_SSH_INFO','/etc/loom/ssh-info/connections.json'))
    try:rows=json.loads(path.read_text())
    except FileNotFoundError:return {'state':'not_configured'}
    except (OSError,ValueError):return {'state':'unavailable'}
    try:machine=json.loads(path.with_name('machine.json').read_text())
    except (OSError,ValueError):machine={}
    return {**machine,**rows.get(actor['id'],{'state':'not_enabled','next':'Ask a Loom Admin to enable SSH for your account.'})}
