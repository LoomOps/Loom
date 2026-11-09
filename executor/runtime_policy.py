"""Container declarations are capabilities, never arbitrary Docker arguments."""
import math
import re

DRIVER_CAPABILITIES=('compute','utility','graphics','display','video','compat32','ngx')
OPTIONS_SCHEMA={'type':'object','properties':{
    'network':{'enum':['none','bridge'],'default':'none'},
    'driver_capabilities':{'type':'array','minItems':1,'uniqueItems':True,
        'items':{'enum':list(DRIVER_CAPABILITIES)},'default':['compute','utility']}},'additionalProperties':False}


def gpu_driver_capabilities(runtime):
    """Only administrator runtime settings choose driver libraries."""
    values=runtime.get('driver_capabilities',['compute','utility'])
    if (not isinstance(values,list) or not values or
        any(not isinstance(v,str) or v not in DRIVER_CAPABILITIES for v in values) or len(values)!=len(set(values))):
        raise ValueError('Invalid runtime driver_capabilities')
    return ','.join(values)


def runtime_network(runtime):
    value=runtime.get('network','none')
    if value not in ('none','bridge'):raise ValueError('Runtime network must be none or bridge')
    return value


def options(value):
    if not isinstance(value,dict) or set(value)-set(OPTIONS_SCHEMA['properties']):raise ValueError('Unknown administrator runtime options')
    return {'network':runtime_network(value),'driver_capabilities':gpu_driver_capabilities(value).split(',')}

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
    return {'image':digest,'cpus':cpus,'memory':memory,'pids':pids,
            'network':runtime_network(policy),'driver_capabilities':gpu_driver_capabilities(policy).split(',')}
