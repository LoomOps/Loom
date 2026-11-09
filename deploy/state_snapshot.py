"""Private, identity-preserving state snapshots for update and container rebuild.

External datasets, weights, Tailscale and SSH host identities stay in place. No
fresh-install identities or tokens are issued. Call only after all writers stop.
"""
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import stat
from migrations import save


def sha(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()


def copy_file(source,dest):
    shutil.copy2(source,dest,follow_symlinks=False)
    info=Path(source).lstat()
    if os.geteuid()==0:os.chown(dest,info.st_uid,info.st_gid,follow_symlinks=False)


def restore_file(source,target,root):
    # A failed migration must not redirect restoration through a new symlink.
    if not target.parent.resolve().is_relative_to(Path(root).resolve()):raise ValueError('Restore path leaves the preserved store')
    target.parent.mkdir(parents=True,exist_ok=True)
    if target.is_symlink():target.unlink()
    if source.is_symlink():target.symlink_to(os.readlink(source))
    else:copy_file(source,target)


def copy_tree(source,dest,instance=False):
    def ignore(directory,names):
        return [name for name in names if (instance and Path(directory)==Path(source) and name=='diagnostics') or (Path(directory)/name).is_socket()
                or ((Path(directory)/name).is_dir() and (Path(directory)/name/'compose.env').is_file() and (Path(directory)/name/'runtime/ops.sqlite3').is_file())]
    shutil.copytree(source,dest,symlinks=True,copy_function=copy_file,ignore=ignore)
    if os.geteuid()==0:
        for p in [Path(source),*Path(source).rglob('*')]:
            target=Path(dest)/p.relative_to(source)
            if target.exists() and p.is_dir() and not p.is_symlink():
                info=p.stat();os.chown(target,info.st_uid,info.st_gid)


def databases(root):
    for p in Path(root).rglob('*'):
        if not p.is_file() or p.is_symlink() or p.suffix not in ('.sqlite3','.sqlite','.db'):continue
        if any((parent/'compose.env').is_file() and (parent/'runtime/ops.sqlite3').is_file() for parent in p.parents if parent!=Path(root) and parent.is_relative_to(root)):continue
        with p.open('rb') as stream:header=stream.read(16)
        if header==b'SQLite format 3\x00':yield p


def rows(c,table,columns):
    quote=lambda s:'"'+s.replace('"','""')+'"'
    def value(item):
        return {'blob':item.hex()} if isinstance(item,bytes) else item
    return Counter(hashlib.sha256(json.dumps([value(x) for x in row],ensure_ascii=True,separators=(',',':')).encode()).hexdigest()
                   for row in c.execute('SELECT '+','.join(map(quote,columns))+' FROM '+quote(table)))


def db_inventory(path):
    result={}
    with sqlite3.connect('file:'+str(path)+'?mode=ro',uri=True) as c:
        if c.execute('PRAGMA integrity_check').fetchone()[0]!='ok':raise ValueError('State database integrity failure')
        for (name,) in c.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").fetchall():
            if name=='node_control':continue  # Admission maintenance is intentional.
            columns=[r[1] for r in c.execute('PRAGMA table_info("'+name.replace('"','""')+'")')]
            result[name]={'columns':columns,'rows':dict(rows(c,name,columns))}
    return result


def file_inventory(root,instance=True):
    result={};root=Path(root)
    for p in root.rglob('*'):
        rel=p.relative_to(root)
        if (instance and rel.parts[0] in ('runtime','diagnostics')) or (instance and p.name=='build.env') or (instance and p.name.endswith(('.lock','-wal','-shm','.pid'))):continue
        if instance and any((parent/'compose.env').is_file() and (parent/'runtime/ops.sqlite3').is_file() for parent in p.parents if parent!=root and parent.is_relative_to(root)):continue
        if instance and p.suffix in ('.sqlite3','.sqlite','.db'):continue
        info=p.lstat()
        if p.is_symlink():data={'link':os.readlink(p)}
        elif stat.S_ISREG(info.st_mode):data={'sha256':sha(p)}
        else:continue
        result[str(rel)]={**data,'mode':stat.S_IMODE(info.st_mode),'uid':info.st_uid,'gid':info.st_gid}
    return result


def external_roots(root,platform):
    from tailnet import env_values
    env=env_values(Path(root)/'compose.env')
    return {name:Path(env.get(key,Path(platform)/name)) for name,key in
            (('components','LOOM_COMPONENTS_HOST'),('profiles','LOOM_PROFILES_HOST'))}


def capture(root,archive,platform):
    root=Path(root);archive=Path(archive);base=archive/'preserved';base.mkdir(mode=0o700)
    instance=base/'instance';copy_tree(root,instance,instance=True)
    state={'schema':'loom-preserved-state/v1','root':str(root),'files':file_inventory(root),'databases':{},'extensions':{}}
    for path in databases(root):
        target=instance/path.relative_to(root)
        with sqlite3.connect('file:'+str(path)+'?mode=ro',uri=True) as source,sqlite3.connect(target) as dest:source.backup(dest)
        info=path.stat();target.chmod(stat.S_IMODE(info.st_mode))
        if os.geteuid()==0:os.chown(target,info.st_uid,info.st_gid)
        state['databases'][str(path.relative_to(root))]=db_inventory(target)
        for suffix in ('-wal','-shm'):Path(str(target)+suffix).unlink(missing_ok=True)
    for name,path in external_roots(root,platform).items():
        if path.exists():
            copy_tree(path,base/name)
            state['extensions'][name]={'path':str(path),'files':file_inventory(path,instance=False)}
    save(base/'state.json',state)
    return base


def verify(root,base):
    root=Path(root);state=json.loads((Path(base)/'state.json').read_text())
    if state['root']!=str(root):raise ValueError('Snapshot belongs to another installation')
    problems=[];current=file_inventory(root)
    for name,value in state['files'].items():
        if current.get(name)!=value:problems.append('instance:'+name)
    for rel,tables in state['databases'].items():
        with sqlite3.connect('file:'+str(root/rel)+'?mode=ro',uri=True) as c:
            if c.execute('PRAGMA integrity_check').fetchone()[0]!='ok':problems.append('integrity:'+rel)
            for table,baseline in tables.items():
                try:actual=rows(c,table,baseline['columns'])
                except sqlite3.DatabaseError:problems.append('table:'+table);continue
                if Counter(baseline['rows'])-actual:problems.append('rows:'+table)
    for name,entry in state['extensions'].items():
        current=file_inventory(entry['path'],instance=False)
        for rel,value in entry['files'].items():
            if current.get(rel)!=value:problems.append(name+':'+rel)
    if problems:raise ValueError('Preserved state changed; keep admission fenced: '+', '.join(problems[:20]))
    return {'preserved':True,'databases':len(state['databases']),'files':len(state['files'])}


def restore(root,base):
    """Stopped-writer recovery only; rollback owns the post-update-write guard."""
    root=Path(root);base=Path(base);state=json.loads((base/'state.json').read_text())
    if state['root']!=str(root):raise ValueError('Snapshot belongs to another installation')
    for rel in state['files']:
        source=base/'instance'/rel;target=root/rel
        restore_file(source,target,root)
    for rel in state['databases']:
        if (root/rel).is_symlink() or not (root/rel).parent.resolve().is_relative_to(root.resolve()):raise ValueError('Database restore leaves the preserved instance')
        with sqlite3.connect(base/'instance'/rel) as a,sqlite3.connect(root/rel) as b:a.backup(b)
    # The old migration ledger determines which steps a later retry must inspect.
    target=root/'runtime/migrations.json';old=base/'instance/runtime/migrations.json'
    if old.exists():restore_file(old,target,root)
    else:target.unlink(missing_ok=True)
    for name,entry in state['extensions'].items():
        target_root=Path(entry['path']);saved_root=base/name
        for rel in entry['files']:
            source=saved_root/rel;target=target_root/rel
            restore_file(source,target,target_root)
        # Restore ownership/modes of directories a failed script may have removed.
        for source in [saved_root,*saved_root.rglob('*')]:
            target=target_root/source.relative_to(saved_root)
            if source.is_dir() and not source.is_symlink() and target.is_dir() and not target.is_symlink():
                info=source.stat();target.chmod(stat.S_IMODE(info.st_mode))
                if os.geteuid()==0:os.chown(target,info.st_uid,info.st_gid)


def extension_inventory(base):
    if not base:return {}
    state=json.loads((Path(base)/'state.json').read_text())
    return {entry['path']:file_inventory(entry['path'],instance=False) for entry in state['extensions'].values()}
