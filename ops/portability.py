"""Separate offline metadata export, payload transfer and explicit activation."""
import argparse
from contextlib import closing
import fcntl
import hashlib
import json
import os
import sqlite3
import tempfile
from pathlib import Path

from host_layout import config_path, write_json
from ops import relative
from registry import inspect


def binding(root):
    env=dict(line.split('=',1) for line in (root/'compose.env').read_text().splitlines())
    cfg=json.loads(config_path(root).read_text())
    bulk=Path(env['OPS_BULK']).resolve(strict=True)
    dev=bulk.stat().st_dev
    if (f'{os.major(dev)}:{os.minor(dev)}'!=cfg['storage_device']
            or json.loads((bulk/'.loom-storage.json').read_text())!={'storage_id':cfg['storage_id']}):
        raise ValueError('Target storage binding mismatch')
    return bulk,cfg


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f,'sha256').hexdigest()


class PayloadMismatch(ValueError):
    pass


def copy_verified(source,dest,expected):
    dest.parent.mkdir(parents=True,exist_ok=True)
    if dest.exists():
        if source.stat().st_size!=expected['size'] or sha(source)!=expected['sha256']:
            raise PayloadMismatch('Source payload differs')
        if not dest.is_file() or dest.stat().st_size!=expected['size'] or sha(dest)!=expected['sha256']:
            raise ValueError('Existing target payload differs')
        return
    fd,name=tempfile.mkstemp(prefix='.incoming-',dir=dest.parent)
    temp=Path(name)
    try:
        with os.fdopen(fd,'wb') as o,source.open('rb') as i:
            digest=hashlib.sha256()
            size=0
            while chunk:=i.read(1024*1024):
                o.write(chunk)
                digest.update(chunk)
                size+=len(chunk)
            if size!=expected['size'] or digest.hexdigest()!=expected['sha256']:
                raise PayloadMismatch('Copied payload differs')
            o.flush()
            os.fsync(o.fileno())
        os.link(temp,dest)
    finally:
        temp.unlink(missing_ok=True)


def integrity(db):
    if db.execute('PRAGMA integrity_check').fetchone()[0]!='ok':
        raise ValueError('Database integrity failure')


def clear_credentials(db):
    """A payload archive carries provenance, never usable client credentials."""
    tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    for name in ('credentials','resident_tokens'):
        if name in tables:db.execute('DELETE FROM '+name)


def entity_view(db,kind):
    name='all_entity_'+kind
    return name if db.execute("SELECT 1 FROM sqlite_master WHERE name=?",(name,)).fetchone() else 'entity_'+kind


def require_single_store(db, config):
    """Schema 1 has a single payload root; reject silent secondary-store loss."""
    if db.execute("SELECT 1 FROM "+entity_view(db,"locations")+" WHERE machine_id=? AND storage_id<>? AND status='available' LIMIT 1",
                  (config['machine_id'],config['storage_id'])).fetchone():
        raise ValueError('Archive schema 1 supports one storage root; use a reviewed multi-store migration')
    for (spec,) in db.execute("SELECT spec FROM jobs WHERE state NOT IN ('complete','cancelled')"):
        if json.loads(spec).get('output_storage','data/0')!='data/0':
            raise ValueError('Pending Jobs require a secondary store; use a reviewed multi-store migration')


def catalog_metadata(db,state,config):
    tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    if 'catalog_manifests' not in tables:return []
    base=Path(config.get('catalog',{}).get('manifest_directory',state/'catalog-manifests'))
    files=[]
    for descriptor,digest in db.execute('SELECT descriptor,sha256 FROM catalog_manifests'):
        file=relative(base,descriptor)
        if sha(file)!=digest:raise ValueError('Authoritative inventory descriptor changed')
        value=json.loads(file.read_bytes());names=[(descriptor,digest)]
        for chunk in value['chunks']:names.append((str(Path(descriptor).parent/chunk['name']),chunk['sha256']))
        for name,digest in names:
            file=relative(base,name)
            if sha(file)!=digest:raise ValueError('Authoritative inventory chunk changed')
            files.append({'ref':name,'sha256':digest,'size':file.stat().st_size})
    return files


def source_metadata(db,state):
    tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    sources={}
    for table in ('entity_revisions','asset_revisions'):
        if table not in tables:continue
        for (text,) in db.execute('SELECT manifest FROM '+table+" WHERE json_type(manifest,'$.file_set')='object'"):
            pin=json.loads(text)['file_set'];name=pin['ref'];file=relative(state/'file-sources',name)
            if sha(file)!=pin['sha256']:raise ValueError('Source inventory descriptor changed')
            body=json.loads(file.read_bytes())
            for ref in [name]+[str(Path(name).parent/chunk[key]) for chunk in body['chunks'] for key in ('name','proofs') if key in chunk]:
                path=relative(state/'file-sources',ref)
                sources[ref]={'ref':ref,'sha256':sha(path),'size':path.stat().st_size}
    return list(sources.values())

def streamed_payloads(db,state,config):
    """Only sealed current-contract members; never discover unregistered payloads."""
    tables={r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    for table in ('entity_revisions','asset_revisions'):
        if table not in tables:continue
        for entity,text in db.execute('SELECT id,manifest FROM '+table+" WHERE json_type(manifest,'$.file_set')='object'"):
            pin=json.loads(text)['file_set'];descriptor=relative(state/'file-sources',pin['ref'])
            if sha(descriptor)!=pin['sha256']:raise ValueError('File-source descriptor changed')
            body=json.loads(descriptor.read_bytes())
            for chunk in body['chunks']:
                file=relative(descriptor.parent,chunk['name'])
                if sha(file)!=chunk['sha256']:raise ValueError('File-source chunk changed')
                with file.open('rb') as stream:
                    for line in stream:
                        if len(line)>65536:raise ValueError('File-source entry exceeds bound')
                        member=json.loads(line)
                        copies=[c for c in member.get('copies',[]) if c.get('machine_id')==config['machine_id'] and c.get('storage_id')==config['storage_id'] and c.get('type')=='storage']
                        if not copies:raise ValueError('Streamed export requires an explicit single-store copy; use a reviewed multi-store migration')
                        yield entity,{'ref':copies[0]['ref'],'size':member['size'],'sha256':member['sha256']}


def payload_files(archive,data):
    yield from data['files']
    if pin:=data.get('payload_inventory'):
        file=relative(archive,pin['ref'])
        if file.stat().st_size!=pin['size'] or sha(file)!=pin['sha256']:raise ValueError('Payload index differs')
        with file.open('rb') as stream:
            count=0
            for line in stream:
                if len(line)>65536:raise ValueError('Payload index entry exceeds bound')
                count+=1
                if count>pin['entries']:raise ValueError('Payload index count differs')
                yield json.loads(line)
            if count!=pin['entries']:raise ValueError('Payload index truncated')

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('command',choices=('export','transfer','restore','activate','rollback-source'))
    p.add_argument('node',type=Path)
    p.add_argument('archive',type=Path)
    p.add_argument('--mode',choices=('relocate','clone'),default='relocate')
    p.add_argument('--exclude-prefix',action='append',default=[])
    p.add_argument('--include-entity',action='append',default=[])
    args=p.parse_args()
    os.umask(0o077)
    root,archive=args.node.resolve(),args.archive.resolve()
    bulk,cfg=binding(root)
    db_path=root/'runtime/ops.sqlite3'
    if args.command=='transfer':
        data=json.loads((archive/'manifest.json').read_text())
        for file in payload_files(archive,data):
            if file['status']!='available':
                continue
            source=relative(bulk,file['ref'])
            dest=relative(archive/'payload',file['ref'])
            copy_verified(source,dest,file)
        return
    with (root/'runtime/worker.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.command=='export':
            archive.mkdir()
            with sqlite3.connect(db_path) as db:
                integrity(db)
                db.execute('BEGIN IMMEDIATE')
                require_single_store(db,cfg)
                if db.execute("SELECT 1 FROM jobs WHERE state IN ('preparing','running','processing','result_failed')").fetchone():
                    raise ValueError('Source still has a running job or unexported result')
                if db.execute('SELECT mode FROM node_control').fetchone()[0]!='active':
                    raise ValueError('Source is not active')
                db.execute("UPDATE node_control SET mode='fenced'")
            with sqlite3.connect(db_path) as source, sqlite3.connect(archive/'ops.sqlite3') as out:
                source.backup(out)
                clear_credentials(out)
                out.commit()
                out.execute('PRAGMA wal_checkpoint(TRUNCATE)')
                out.execute('PRAGMA journal_mode=DELETE')
                integrity(out)
                inventories=catalog_metadata(source,root/'runtime',cfg)
                source_files=source_metadata(source,root/'runtime')
                for entry in source_files:copy_verified(relative(root/'runtime/file-sources',entry['ref']),relative(archive/'file-sources',entry['ref']),entry)
                base=Path(cfg.get('catalog',{}).get('manifest_directory',root/'runtime/catalog-manifests'))
                for entry in inventories:copy_verified(relative(base,entry['ref']),relative(archive/'catalog-manifests',entry['ref']),entry)
                rows=source.execute("SELECT DISTINCT ref FROM "+entity_view(source,"locations")+" WHERE machine_id=? AND storage_id=?",
                                    (cfg['machine_id'],cfg['storage_id'])).fetchall()
                included={ref for (ref,) in source.execute('SELECT ref FROM '+entity_view(source,'locations')+' WHERE id IN ('+
                           ','.join('?' for _ in args.include_entity)+')',args.include_entity)} if args.include_entity else None
                if args.include_entity and not included:
                    raise ValueError('Selected entities have no local payloads')
                files=[]
                for (ref,) in rows:
                    entity=source.execute("SELECT e.manifest,l.component FROM "+entity_view(source,"locations")+" l JOIN "+entity_view(source,"revisions")+" e ON (e.id,e.revision)=(l.id,l.revision) WHERE l.machine_id=? AND l.storage_id=? AND l.ref=?",
                        (cfg['machine_id'],cfg['storage_id'],ref)).fetchone()
                    expected=next(x for x in json.loads(entity[0])['components'] if x['ref']==entity[1])
                    size,_,status=inspect(relative(bulk,ref),expected['sha256'])
                    if size!=expected['size'] and status=='available':
                        status='corrupt'
                    if (included is not None and ref not in included) or any(Path(ref).is_relative_to(prefix) for prefix in args.exclude_prefix):
                        status='intentionally_not_migrated'
                    files.append({'ref':ref,'size':expected['size'],'sha256':expected['sha256'],'status':status})
                payload_index=archive/'payload-index.jsonl';count=0
                with payload_index.open('wb') as stream:
                    for entity,file in streamed_payloads(source,root/'runtime',cfg):
                        excluded=(bool(args.include_entity) and entity not in args.include_entity) or any(Path(file['ref']).is_relative_to(prefix) for prefix in args.exclude_prefix)
                        if excluded:status='intentionally_not_migrated'
                        else:
                            size,_,status=inspect(relative(bulk,file['ref']),file['sha256'])
                            if size!=file['size'] and status=='available':status='corrupt'
                        stream.write((json.dumps({**file,'status':status},separators=(',',':'))+'\n').encode());count+=1
                        if count>1000000:raise ValueError('Use a smaller explicit payload export scope')
                payload_pin={'ref':'payload-index.jsonl','sha256':sha(payload_index),'size':payload_index.stat().st_size,'entries':count} if count else None
                if not count:payload_index.unlink()
            (archive/'manifest.json').write_text(json.dumps({'schema':2 if count else 1,'source_node':str(root),
                'machine_id':cfg['machine_id'],'storage_id':cfg['storage_id'],'db_sha256':sha(archive/'ops.sqlite3'),
                'files':files,'catalog_manifests':inventories,'file_sources':source_files,'payload_inventory':payload_pin},indent=2))
        elif args.command=='restore':
            if db_path.exists():
                raise ValueError('Restore requires an unused target runtime')
            data=json.loads((archive/'manifest.json').read_text())
            if data['db_sha256']!=sha(archive/'ops.sqlite3'):
                raise ValueError('DB snapshot hash mismatch')
            if args.mode=='relocate' and cfg['machine_id']!=data['machine_id']:
                raise ValueError('Relocation preserves logical machine ID')
            if args.mode=='clone' and cfg['machine_id']==data['machine_id']:
                raise ValueError('Clone requires a fresh machine ID')
            if data.get('payload_inventory') and (cfg['machine_id']!=data['machine_id'] or cfg['storage_id']!=data['storage_id']):
                raise ValueError('Streamed inventories preserve logical machine/store IDs; reconcile verified copies explicitly before a clone')
            base=Path(cfg.get('catalog',{}).get('manifest_directory',root/'runtime/catalog-manifests'))
            for entry in data.get('catalog_manifests',[]):copy_verified(relative(archive/'catalog-manifests',entry['ref']),relative(base,entry['ref']),entry)
            for entry in data.get('file_sources',[]):copy_verified(relative(archive/'file-sources',entry['ref']),relative(root/'runtime/file-sources',entry['ref']),entry)
            locations=[]
            for file in payload_files(archive,data):
                ref,status=file['ref'],file['status']
                if status=='available':
                    src=relative(archive/'payload',ref)
                    if not src.is_file():
                        status='missing'
                    else:
                        try:
                            copy_verified(src,relative(bulk,ref),file)
                        except PayloadMismatch:
                            status='corrupt'
                locations.append((cfg['machine_id'],cfg['storage_id'],status,data['machine_id'],data['storage_id'],ref))
            temp=db_path.with_suffix('.incoming')
            with closing(sqlite3.connect((archive/'ops.sqlite3').as_uri()+'?mode=ro',uri=True)) as source, closing(sqlite3.connect(temp)) as target:
                source.backup(target)
                clear_credentials(target)
                integrity(target)
                target.execute("UPDATE node_control SET mode='readonly'")
                target.executemany("""INSERT OR IGNORE INTO entity_locations
                  SELECT id,revision,component,?,?,ref,? FROM entity_locations
                  WHERE machine_id=? AND storage_id=? AND ref=?""",locations)
                if target.execute("SELECT 1 FROM sqlite_master WHERE name='asset_locations'").fetchone():
                    target.executemany("INSERT OR IGNORE INTO asset_locations SELECT id,revision,component,?,?,ref,? FROM asset_locations WHERE machine_id=? AND storage_id=? AND ref=?",locations)
                    target.execute("UPDATE asset_locations SET status='external' WHERE storage_id<>? AND status='available'",(cfg['storage_id'],))
                # Old locations remain provenance; their accessibility is not verified here.
                target.execute("UPDATE entity_locations SET status='external' WHERE storage_id<>? AND status='available'",(cfg['storage_id'],))
                target.commit()
                target.execute('PRAGMA wal_checkpoint(TRUNCATE)')
                target.execute('PRAGMA journal_mode=DELETE')
                integrity(target)
            temp.replace(db_path)
            (root/'runtime/restore.json').write_text(json.dumps({'archive':str(archive),'mode':args.mode,
                'manifest_sha256':sha(archive/'manifest.json')}))
        else:
            data=json.loads((archive/'manifest.json').read_text())
            source_path=Path(data['source_node'])/'runtime/ops.sqlite3'
            if args.command=='activate':
                receipt=json.loads((root/'runtime/restore.json').read_text())
                if receipt['archive']!=str(archive):
                    raise ValueError('Restore receipt mismatch')
                # Serialize activation and rollback against this archive.
                with (archive/'activation.lock').open('a') as activation:
                    fcntl.flock(activation,fcntl.LOCK_EX)
                    with sqlite3.connect(source_path.as_uri()+'?mode=ro',uri=True) as c:
                        if c.execute('SELECT mode FROM node_control').fetchone()[0]!='fenced':
                            raise ValueError('Source not fenced')
                    marker=archive/'active-target.json'
                    if marker.exists() and json.loads(marker.read_text())!={'db':str(db_path)}:
                        raise ValueError('Another target owns activation')
                    with sqlite3.connect(db_path) as c:
                        mode=c.execute('SELECT mode FROM node_control').fetchone()[0]
                        if mode=='active' and marker.exists():
                            return
                        if mode!='readonly':
                            raise ValueError('Target is not in read-only restore mode')
                        if not marker.exists():
                            write_json(marker,{'db':str(db_path)})
                            fd=os.open(archive,os.O_RDONLY|os.O_DIRECTORY)
                            try: os.fsync(fd)
                            finally: os.close(fd)
                        c.execute("UPDATE node_control SET mode='active' WHERE mode='readonly'")
            else:
                if db_path!=source_path:
                    raise ValueError('Wrong source for rollback')
                with (archive/'activation.lock').open('a') as activation:
                    fcntl.flock(activation,fcntl.LOCK_EX)
                    marker=archive/'active-target.json'
                    if marker.exists():
                        target=Path(json.loads(marker.read_text())['db'])
                        with (target.parent/'worker.lock').open('a') as guard:
                            fcntl.flock(guard,fcntl.LOCK_EX|fcntl.LOCK_NB)
                            with sqlite3.connect(target) as c:
                                c.execute("UPDATE node_control SET mode='fenced'")
                        marker.rename(archive/'rolled-back-target.json')
                    with sqlite3.connect(db_path) as c:
                        c.execute("UPDATE node_control SET mode='active' WHERE mode='fenced'")


if __name__=='__main__':
    main()
