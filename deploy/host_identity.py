"""Host service identity and protected platform directories (Linux installer)."""
import grp
import os
from pathlib import Path
import pwd
import stat
import subprocess


def service_identity():
    """Create loom once. Never reinterpret an existing account as another identity."""
    if os.geteuid() != 0:
        raise PermissionError('Run the host installer with sudo to provision loom:loom')
    try:
        group = grp.getgrnam('loom')
    except KeyError:
        subprocess.run(['groupadd', '--system', 'loom'], check=True)
        group = grp.getgrnam('loom')
    try:
        user = pwd.getpwnam('loom')
    except KeyError:
        subprocess.run(['useradd', '--system', '--gid', 'loom', '--home-dir', '/loom',
                        '--no-create-home', '--shell', '/usr/sbin/nologin', 'loom'], check=True)
        user = pwd.getpwnam('loom')
    if user.pw_uid == 0 or group.gr_gid == 0 or user.pw_gid != group.gr_gid:
        raise ValueError('Existing loom account must be non-root with primary group loom')
    return user.pw_uid, group.gr_gid


def identity(uid=None, gid=None):
    if (uid is None) != (gid is None):
        raise ValueError('Supply both --uid and --gid, or neither for loom:loom')
    if uid is None:
        return service_identity()
    if uid <= 0 or gid <= 0:
        raise ValueError('Loom services require a non-root UID/GID')
    return uid, gid


def directory(path, uid, gid, mode=0o750):
    path = Path(path)
    # No symlink ancestors: host installation must not redirect root writes.
    for parent in (path, *path.parents):
        if parent.is_symlink():
            raise ValueError(f'Symlink in installation path: {parent}')
    path.mkdir(parents=True, exist_ok=True)
    os.chown(path, uid, gid)
    path.chmod(mode)
    return path


def platform_layout(root, uid, gid):
    directory(root, 0, gid, 0o755)
    directory(root/'shared', 0, gid, 0o755)
    for name in ('components', 'profiles', 'agents', 'builds'):
        directory(root/name, uid, gid, 0o755)


def own_tree(path, uid, gid, *, skip=()):
    """Own only this tree; do not follow links or cross filesystem boundaries."""
    path = Path(path)
    if path.is_symlink():
        raise ValueError(f'Refusing linked ownership root: {path}')
    device = path.stat().st_dev
    skip = {Path(x).absolute() for x in skip}
    for base, dirs, files in os.walk(path, followlinks=False):
        base = Path(base)
        dirs[:] = [name for name in dirs if not (base/name).is_symlink()
                   and (base/name).absolute() not in skip and (base/name).stat().st_dev == device]
        os.chown(base, uid, gid)
        for name in files:
            file = base/name
            if not file.is_symlink():
                os.chown(file, uid, gid)


def protected_chain(path):
    for item in (Path(path), *Path(path).parents):
        info = item.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise ValueError(f'Privileged service parent must be root-owned and not writable by others: {item}')


def admin_access(path, uid, *, write=True, recursive=False):
    """Explicit operator ACL, without adding the operator to a secret-bearing group."""
    if uid == 0:
        return
    cmd = ['setfacl'] + (['-R'] if recursive else [])
    subprocess.run(cmd + ['-m', f'u:{uid}:{"rwX" if write else "rX"}', str(path)], check=True)
    if Path(path).is_dir():
        subprocess.run(['setfacl', '-m', f'd:u:{uid}:{"rwx" if write else "r-x"}', str(path)], check=True)


def build_owner(path):
    """New build artifacts use the service owner; archived snapshots are not traversed."""
    path=Path(path)
    if os.geteuid()!=0 or not path.is_relative_to('/loom/builds'):return
    try: user=pwd.getpwnam('loom')
    except KeyError:return
    own_tree(path,user.pw_uid,user.pw_gid)


def admin_reexec_for(path):
    import sys
    path=Path(path)
    while not path.exists() and path!=path.parent:path=path.parent
    if os.geteuid()!=0 and not os.access(path,os.W_OK):
        os.execvp('sudo',['sudo','-n',sys.executable,'-B',str(Path(sys.argv[0]).resolve()),*sys.argv[1:]])
