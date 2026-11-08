"""Host CPU and temperature observations for the existing dashboard collector."""
import argparse
import json
import os
import time
from datetime import datetime, timezone
from pathlib import Path
from docker_status import write_snapshot


def cpu_usage(previous, current):
    if not previous or previous.get('boot_id') != current['boot_id']:
        return None, None
    delta = [b-a for a,b in zip(previous['ticks'],current['ticks'])]
    elapsed = current['monotonic']-previous['monotonic']
    if len(delta)!=8 or any(v<0 for v in delta) or sum(delta)<=0 or elapsed<=0:
        return None, None
    # guest and guest_nice are already counted in user/nice; use the first 8 fields.
    return 100*(sum(delta)-delta[3]-delta[4])/sum(delta), elapsed


def temperatures(root=Path('/sys/class/hwmon')):
    readings=[];cpu=[]
    for chip in sorted(root.glob('hwmon*')):
        try: name=(chip/'name').read_text().strip()
        except OSError: continue
        for sensor in sorted(chip.glob('temp*_input')):
            try:
                value=float(sensor.read_text())/1000
                label_path=sensor.with_name(sensor.name.replace('_input','_label'))
                label=label_path.read_text().strip() if label_path.exists() else sensor.stem
            except (OSError,ValueError):continue
            # Firmware may publish disconnected inputs as a negative sentinel.
            if not 0 <= value <= 150:continue
            readings.append({'chip':name,'label':label,'temperature_C':value})
            if name in ('k10temp','coretemp') and (label in ('Tctl','Tdie') or label.startswith('Package id')):
                cpu.append((0 if label=='Tctl' or label.startswith('Package id') else 1,value))
    chosen=min(cpu,key=lambda item:item[0])[1] if cpu else None
    return readings,chosen


def collect(previous=None,proc=Path('/proc'),hwmon=Path('/sys/class/hwmon')):
    observed=datetime.now(timezone.utc).isoformat()
    result={'observed_at':observed,'sections':{}}
    sections=result['sections']
    try:
        ticks=list(map(int,(proc/'stat').read_text().splitlines()[0].split()[1:9]))
        sample={'ticks':ticks,'monotonic':time.monotonic(),'boot_id':(proc/'sys/kernel/random/boot_id').read_text().strip()}
        usage,window=cpu_usage((previous or {}).get('_cpu_sample'),sample)
        data={'utilization_percent':usage,'sample_window_s':window,'logical_cpus':os.cpu_count(),
              'load_1m':float((proc/'loadavg').read_text().split()[0])}
        sections['cpu']={'status':'ok' if usage is not None else 'warming','observed_at':observed,
                         'source':'host:/proc/stat','data':data}
        result['_cpu_sample']=sample
    except (OSError,ValueError,IndexError,KeyError):
        sections['cpu']={'status':'unavailable','data':None,'observed_at':observed}
    readings,cpu_temp=temperatures(hwmon)
    sections['sensors']={'status':'ok' if readings else 'unavailable','data':readings,
                         'source':'host:hwmon','observed_at':observed}
    if sections['cpu']['data'] is not None:sections['cpu']['data']['temperature_C']=cpu_temp
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);args=parser.parse_args()
    try:previous=json.loads(args.output.read_text())
    except (OSError,ValueError):previous=None
    write_snapshot(args.output,collect(previous))

if __name__=='__main__':main()
