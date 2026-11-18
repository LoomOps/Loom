"""Build immutable development images. Imports/help never touch installations."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def build(destination, *, instance=None):
    import snapshot
    info = snapshot.capture(Path(destination))
    folder = Path(info['snapshot']);source = folder/'source';source.mkdir()
    with tarfile.open(folder/'source.tar.gz') as archive:
        archive.extractall(source, filter='data')
    engine = ['docker']
    if subprocess.run(engine+['info'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode:
        raise RuntimeError('Docker access required; configure it explicitly before building')
    result = {**info,'status':'building','image_ids':{}}
    try:
        for name in ('ops','dashboard'):
            context = source if name=='ops' else source/'dashboard'
            dockerfile = source/name/('Dockerfile.rust' if name=='ops' else 'Dockerfile')
            subprocess.run(engine+['build','-t',info['images'][name],'-f',str(dockerfile),str(context)],check=True)
            result['image_ids'][name] = subprocess.check_output(engine+['image','inspect',info['images'][name],'--format','{{.Id}}'],text=True).strip()
        import native_artifacts
        result['native'] = native_artifacts.extract(result['image_ids']['ops'],folder/'native',engine)
        result['status'] = 'built'
        if instance is not None:
            # Explicit alias selection only; this never starts or restarts services.
            for name,tag in info['images'].items():
                subprocess.run(engine+['tag',tag,'loom-'+name+':dev-build'],check=True)
            snapshot.select(info,Path(instance))
    except BaseException:
        result['status'] = 'failed';raise
    finally:
        (folder/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    return result


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--destination',type=Path,default=ROOT/'target/dev-builds')
    p.add_argument('--select-instance',type=Path,help='Explicitly select the built alias for an existing dev instance; no activation')
    a=p.parse_args();print(json.dumps(build(a.destination,instance=a.select_instance),indent=2))


if __name__=='__main__':main()
