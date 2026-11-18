"""Run the pinned native graph inside its deployment-compatible build platform."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
from urllib.request import urlopen

ROOT=Path(__file__).resolve().parents[1]

def executable(cache,info):
    target=cache/'tools/bazel';target.parent.mkdir(parents=True,exist_ok=True)
    if target.is_file() and hashlib.sha256(target.read_bytes()).hexdigest()==info['bazel_sha256']:return target
    selected=os.environ.get('LOOM_BAZEL_BINARY')
    temporary=target.with_suffix('.pending')
    try:
        if selected:shutil.copyfile(selected,temporary)
        else:
            url=f"https://github.com/bazelbuild/bazel/releases/download/{info['bazel_version']}/bazel-{info['bazel_version']}-linux-x86_64"
            with urlopen(url,timeout=60) as response,temporary.open('wb') as output:
                total=0
                while chunk:=response.read(1024*1024):
                    total+=len(chunk)
                    if total>128*1024*1024:raise ValueError('Bazel download exceeded its size bound')
                    output.write(chunk)
        if hashlib.sha256(temporary.read_bytes()).hexdigest()!=info['bazel_sha256']:raise ValueError('Pinned Bazel checksum mismatch')
        temporary.chmod(0o755);temporary.replace(target)
    finally:temporary.unlink(missing_ok=True)
    return target

def source_inputs():
    import snapshot
    return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in snapshot.sources()}

def run(arguments,cache):
    info=json.loads((ROOT/'build_support/platform.json').read_text())
    if platform.system()!='Linux' or platform.machine() not in ('x86_64','AMD64'):raise ValueError('This build platform supports Linux x86_64; use the current Cargo runner on other hosts')
    cache=Path(cache).resolve()
    if cache==ROOT or ROOT.is_relative_to(cache):raise ValueError('Build cache must not contain the source checkout')
    marker=cache/'platform.json'
    if marker.exists() and json.loads(marker.read_text())!=info:raise ValueError('Build platform differs from this cache; choose a new --cache directory')
    cache.mkdir(parents=True,exist_ok=True);marker.write_text(json.dumps(info,indent=2)+'\n')
    for name in ('home','output','disk','native','web','logs'):(cache/name).mkdir(parents=True,exist_ok=True)
    executable(cache,info)
    # Read-only source, no Docker socket, installation mounts, credentials or host home.
    shell='''set -eu
/cache/tools/bazel --output_user_root=/cache/output "$@" --symlink_prefix=/cache/links/ --disk_cache=/cache/disk
targets=""
for arg in "$@"; do case "$arg" in //*) targets="$targets $arg" ;; esac; done
/cache/tools/bazel --output_user_root=/cache/output cquery "kind('rust_binary rule', deps(set($targets)))" --output=files --symlink_prefix=/cache/links/ --disk_cache=/cache/disk > /cache/selected-paths.txt
execroot=$(/cache/tools/bazel --output_user_root=/cache/output info execution_root --symlink_prefix=/cache/links/ --disk_cache=/cache/disk)
: > /cache/native/selected.txt
for source in $(cat /cache/selected-paths.txt); do
    source="$execroot/$source"
    name=$(basename "$source")
    case "$name" in loom-ops|loom-host|loom-sdk|loom-migrate)
        if ! cmp -s "$source" "/cache/native/$name"; then install -m 755 "$source" "/cache/native/$name"; fi
        printf '%s\n' "$name" >> /cache/native/selected.txt ;;
    esac
done
bin=$(/cache/tools/bazel --output_user_root=/cache/output info bazel-bin --symlink_prefix=/cache/links/ --disk_cache=/cache/disk)
if [ -f "$bin/dashboard/generated/workspace.html" ]; then
    if ! cmp -s "$bin/dashboard/generated/workspace.html" /cache/web/workspace.html; then install -m 644 "$bin/dashboard/generated/workspace.html" /cache/web/workspace.html; fi
fi
'''
    command=['docker','run','--rm','--user',f'{os.getuid()}:{os.getgid()}','--env','HOME=/cache/home','--env','USER=loom-build','--volume',f'{ROOT}:/src:ro','--volume',f'{cache}:/cache:rw','--workdir','/src',info['builder'],'/bin/sh','-c',shell,'loom-build',*arguments]
    inputs=source_inputs()
    subprocess.run(command,check=True)
    if source_inputs()!=inputs:raise RuntimeError("Source inputs changed during the build; rerun before accepting artifacts")
    names=set((cache/'native/selected.txt').read_text().splitlines())
    files={name:hashlib.sha256((cache/'native'/name).read_bytes()).hexdigest() for name in sorted(names)}
    receipt={**info,'schema':'loom-native-build/v1','sources':inputs,'files':files,'scope':'pinned build platform; acceptance and release provenance are separate'}
    (cache/'native/platform.json').write_text(json.dumps(receipt,indent=2)+'\n')
    return receipt

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache',type=Path,default=ROOT/'target/build-cache/bookworm')
    parser.add_argument('arguments',nargs=argparse.REMAINDER)
    args=parser.parse_args()
    if not args.arguments:parser.error('Pass a Bazel build/test command and explicit targets')
    if args.arguments[0] not in ('build','test'):parser.error('Only build/test commands are supported; no deployment is performed')
    result=run(args.arguments,args.cache)
    print(json.dumps({k:v for k,v in result.items() if k!='sources'} | {'source_files':len(result['sources']),'receipt':str(args.cache.resolve()/'native/platform.json')},indent=2))

if __name__=='__main__':main()
