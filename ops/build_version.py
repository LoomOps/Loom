"""Build identity and live checkout changes; never infer an image version from a tag alias."""
import hashlib
import json
import os
from pathlib import Path

_cache = (None, None)


def status():
    global _cache
    root=Path(__file__).resolve().parent
    try:
        info=json.loads((root/'build-info.json').read_text())
    except (OSError,ValueError):
        return {'build_id':'unversioned','live_source':os.environ.get('LOOM_LIVE_SOURCE')=='1'}
    live=os.environ.get('LOOM_LIVE_SOURCE')=='1'
    modified=False
    if live:
        paths={ref:root/ref for ref in info['files']}
        current={str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()
                 and not {'evidence','__pycache__','.git','node_modules','secrets'}.intersection(p.relative_to(root).parts)
                 and p.name not in {'build-info.json','client-tokens.json'}
                 and p.suffix not in {'.pyc','.sqlite3','.log','.pem','.env'}
                 and not p.name.endswith(('.pending.html','private.json'))}
        if info.get('component')=='ops':
            use=root.parent/'loom_use'
            current.update('../loom_use/'+str(p.relative_to(use)) for p in use.rglob('*') if p.is_file()
                           and not {'__pycache__','.git','secrets'}.intersection(p.relative_to(use).parts)
                           and p.suffix not in {'.pyc','.env'})
        signature=(info['build_id'],tuple(sorted(current)),tuple((ref,p.stat().st_mtime_ns,p.stat().st_size) if p.is_file() else (ref,None)
                                         for ref,p in paths.items()))
        if signature!=_cache[0]:
            modified=current!=set(paths) or any(not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest()!=info['files'][ref]
                         for ref,p in paths.items())
            _cache=(signature,modified)
        else:
            modified=_cache[1]
    return {key:info[key] for key in ('build_id','channel','created_at','source_sha256','git_commit')} | {
        'image_build_id':os.environ.get('LOOM_IMAGE_BUILD_ID',info['build_id']),
        'live_source':live,'source_modified':modified,'source_scope':info.get('component')}
