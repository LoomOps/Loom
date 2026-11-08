"""Container declarations are capabilities, never arbitrary Docker arguments."""
import math
import re

def declared(data,policy,docker):
    if not policy.get('enabled',False):raise PermissionError('Declared images disabled for this instance')
    image=data['image']
    if not isinstance(image,str) or not image or len(image)>512 or image.startswith('-') or any(c.isspace() for c in image):raise ValueError('Invalid image reference')
    resources=data.get('resources',{})
    if not isinstance(resources,dict) or set(resources)-{'cpus','memory','pids'}:raise ValueError('Unknown resources; mounts and network are host policy')
    cpus=resources.get('cpus',1);memory=resources.get('memory','2g');pids=resources.get('pids',512)
    if type(cpus) not in (int,float) or not math.isfinite(cpus) or not 0<cpus<=policy.get('max_cpus',4):raise ValueError('CPU limit outside instance grant')
    def size(value):
        match=re.fullmatch(r'([1-9][0-9]*)([kmg])',value) if isinstance(value,str) else None
        if not match:raise ValueError('Memory requires k/m/g units')
        return int(match[1])*1024**('kmg'.index(match[2])+1)
    if not 16*1024**2<=size(memory)<=size(policy.get('max_memory','8g')):raise ValueError('Memory limit outside instance grant')
    if type(pids) is not int or not 16<=pids<=policy.get('max_pids',1024):raise ValueError('PID limit outside instance grant')
    # Cache acquisition/build is agent owned. Resolve mutable tags once, then execute ID.
    digest=docker('image','inspect','--format','{{.Id}}',image).strip()
    if not re.fullmatch('sha256:[a-f0-9]{64}',digest):raise ValueError('Image identity unavailable')
    return {'image':digest,'cpus':cpus,'memory':memory,'pids':pids,'network':'none'}
