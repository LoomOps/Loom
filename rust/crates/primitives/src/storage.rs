use crate::{error::require, store::id, Error, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};
/// Reject links and special files before reading, including FIFOs without a writer.
pub fn open_regular(path: &Path) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    require(
        file.metadata()?.is_file(),
        "artifact",
        "Regular file required",
        400,
    )?;
    Ok(file)
}
pub fn file_hash(path: &Path) -> Result<(u64, String)> {
    use std::os::unix::fs::MetadataExt;
    let mut file = open_regular(path)?;
    let before = file.metadata()?;
    require(before.is_file(), "artifact", "Regular file required", 400)?;
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buf = [0; 1024 * 1024];
    loop {
        let count = file.read(&mut buf)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        hash.update(&buf[..count]);
    }
    let after = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    require(
        !named.file_type().is_symlink()
            && named.ino() == before.ino()
            && named.dev() == before.dev()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
            && before.len() == after.len()
            && before.modified()? == after.modified()?
            && bytes == before.len(),
        "artifact",
        "File changed during verification",
        409,
    )?;
    Ok((bytes, format!("{:x}", hash.finalize())))
}
pub fn atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::input("Missing parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".write-{}", id()));
    let result = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temporary)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        if let Ok(meta) = fs::symlink_metadata(path) {
            require(
                !meta.file_type().is_symlink(),
                "path",
                "Refusing linked destination",
                409,
            )?;
            fs::set_permissions(
                &temporary,
                fs::Permissions::from_mode(meta.permissions().mode()),
            )?;
            let c = std::ffi::CString::new(temporary.as_os_str().as_encoded_bytes())
                .map_err(|_| Error::input("Invalid path"))?;
            use std::os::unix::fs::MetadataExt;
            let rc = unsafe { libc::chown(c.as_ptr(), meta.uid(), meta.gid()) };
            if rc != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        fs::rename(&temporary, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    result
}
pub fn relative(root: &Path, reference: &str) -> Result<PathBuf> {
    require(
        !reference.is_empty() && !reference.contains('\0'),
        "locator",
        "Invalid relative locator",
        400,
    )?;
    let p = Path::new(reference);
    require(
        !p.is_absolute() && p.components().all(|c| matches!(c, Component::Normal(_))),
        "locator",
        "Invalid relative locator",
        400,
    )?;
    let base = root.canonicalize()?;
    let mut candidate = base.clone();
    for part in p.components() {
        candidate.push(part);
        if candidate.exists() || candidate.symlink_metadata().is_ok() {
            require(
                !candidate.symlink_metadata()?.file_type().is_symlink(),
                "locator",
                "Symlink location denied",
                400,
            )?;
            let resolved = candidate.canonicalize()?;
            require(
                resolved.starts_with(&base),
                "locator",
                "Locator outside root",
                403,
            )?;
        }
    }
    Ok(candidate)
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifiedFile {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    stamp: (u64, u64, u64, i64, i64, i64, i64),
}
fn stamp(path: &Path) -> Result<(u64, u64, u64, i64, i64, i64, i64)> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(path)?;
    require(
        m.is_file() && !m.file_type().is_symlink(),
        "artifact",
        "Regular unlinked file required",
        409,
    )?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
impl VerifiedFile {
    pub fn read(path: &Path) -> Result<Self> {
        let before = stamp(path)?;
        let (size, sha256) = file_hash(path)?;
        require(
            before == stamp(path)?,
            "artifact",
            "File changed during validation",
            409,
        )?;
        Ok(Self {
            path: path.into(),
            size,
            sha256,
            stamp: before,
        })
    }
    pub fn revalidate_open(&self, file: &fs::File) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        let actual = (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        );
        require(
            m.is_file() && actual == self.stamp,
            "content_changed",
            "Opened file differs from verified evidence",
            409,
        )
    }
    pub fn revalidate(&self) -> Result<()> {
        require(
            self.stamp == stamp(&self.path)?,
            "artifact",
            "Verified file changed before publication",
            409,
        )
    }
}

/// Walk every path component relative to an already configured root without following links.
pub fn open_registered(root: &Path, reference: &str) -> Result<std::fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let mut fd = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)?;
    let parts = Path::new(reference).components().collect::<Vec<_>>();
    require(!parts.is_empty(), "locator", "File reference required", 400)?;
    for (n, p) in parts.iter().enumerate() {
        let Component::Normal(name) = p else {
            return Err(Error::input("Invalid file reference"));
        };
        let name = std::ffi::CString::new(name.as_encoded_bytes())
            .map_err(|_| Error::input("Invalid file reference"))?;
        let raw = unsafe {
            libc::openat(
                fd.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC
                    | if n + 1 < parts.len() {
                        libc::O_DIRECTORY
                    } else {
                        0
                    },
            )
        };
        if raw < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        fd = unsafe { fs::File::from_raw_fd(raw) };
    }
    require(
        fd.metadata()?.is_file(),
        "locator",
        "Regular file required",
        400,
    )?;
    Ok(fd)
}
