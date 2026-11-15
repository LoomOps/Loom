//! Configured, marker-verified storage and immutable package publication.
use crate::{error::require, store::*, Error, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};
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
pub fn store(
    config: &Config,
    bulk: &Path,
    selector: &str,
    expected: Option<&str>,
) -> Result<(PathBuf, String)> {
    let (root, id, mode) = if selector == "data/0" {
        (bulk.to_path_buf(), config.storage_id.clone(), "rw".into())
    } else {
        let s = config
            .data_stores
            .get(selector)
            .ok_or_else(|| Error::input("Unknown storage"))?;
        (
            PathBuf::from(
                s["path"]
                    .as_str()
                    .ok_or_else(|| Error::input("Missing storage path"))?,
            ),
            s["storage_id"].as_str().unwrap_or("").into(),
            s["mode"].as_str().unwrap_or("").to_string(),
        )
    };
    require(
        matches!(mode.as_str(), "rw" | "ro") && !id.is_empty() && root.is_absolute(),
        "storage",
        "Storage grant unavailable",
        409,
    )?;
    if let Some(expected) = expected {
        require(expected == id, "storage", "Storage identity changed", 409)?;
    }
    require(
        root.canonicalize()? == root,
        "storage",
        "Storage root redirected",
        409,
    )?;
    if selector == "data/0" {
        if let Some(device) = config.extra.get("storage_device").and_then(Value::as_str) {
            use std::os::unix::fs::MetadataExt;
            let current = fs::metadata(&root)?.dev();
            require(
                device == format!("{}:{}", libc::major(current), libc::minor(current)),
                "storage_device",
                "Configured primary filesystem device changed",
                409,
            )?;
        }
    }
    let marker: Value = serde_json::from_slice(&fs::read(root.join(".loom-storage.json"))?)?;
    require(
        marker == json!({"storage_id":id}),
        "storage",
        "Storage marker mismatch",
        409,
    )?;
    Ok((root, id))
}
pub fn locator(config: &Config, bulk: &Path, v: &Value) -> Result<PathBuf> {
    require(
        v["machine_id"] == config.machine_id,
        "storage",
        "Input outside machine",
        403,
    )?;
    let ident = v["storage_id"]
        .as_str()
        .ok_or_else(|| Error::input("Storage identity required"))?;
    let mut selectors = vec![];
    if ident == config.storage_id {
        selectors.push("data/0".to_string())
    }
    selectors.extend(
        config
            .data_stores
            .iter()
            .filter(|(k, v)| k.as_str() != "data/0" && v["storage_id"] == ident)
            .map(|(k, _)| k.clone()),
    );
    require(
        selectors.len() == 1,
        "storage",
        "Unknown or ambiguous storage identity",
        409,
    )?;
    let (root, _) = store(config, bulk, &selectors[0], Some(ident))?;
    relative(
        &root,
        v["ref"]
            .as_str()
            .ok_or_else(|| Error::input("Location ref required"))?,
    )
}
pub fn file_hash(path: &Path) -> Result<(u64, String)> {
    use std::os::unix::fs::MetadataExt;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
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
pub fn stage(root: &Path, files: &serde_json::Map<String, Value>) -> Result<PathBuf> {
    fs::create_dir_all(root)?;
    let folder = root.join(format!(".candidate-{}", id()));
    fs::create_dir(&folder)?;
    let result = (|| -> Result<()> {
        for (name, content) in files {
            let path = relative(&folder, name)?;
            let text = content
                .as_str()
                .ok_or_else(|| Error::input("Source content must be text"))?;
            if let Some(p) = path.parent() {
                fs::create_dir_all(p)?;
            }
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o444)
                .open(path)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        fs::File::open(&folder)?.sync_all()?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&folder);
        return Err(e);
    }
    Ok(folder)
}
pub fn publish(candidate: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| Error::input("Missing publication parent"))?;
    fs::create_dir_all(parent)?;
    if destination.exists() {
        fn contents(
            root: &Path,
            dir: &Path,
            out: &mut std::collections::BTreeMap<PathBuf, (u64, String)>,
        ) -> Result<()> {
            for entry in fs::read_dir(dir)? {
                let p = entry?.path();
                let meta = p.symlink_metadata()?;
                require(
                    !meta.file_type().is_symlink(),
                    "immutable",
                    "Revision symlink denied",
                    409,
                )?;
                if meta.is_dir() {
                    contents(root, &p, out)?;
                } else {
                    out.insert(p.strip_prefix(root).unwrap().into(), file_hash(&p)?);
                }
            }
            Ok(())
        }
        let mut old = std::collections::BTreeMap::new();
        let mut new = std::collections::BTreeMap::new();
        contents(destination, destination, &mut old)?;
        contents(candidate, candidate, &mut new)?;
        require(
            old == new,
            "immutable",
            "Existing revision differs from staged package",
            409,
        )?;
        fs::remove_dir_all(candidate)?;
        return Ok(());
    }
    fs::rename(candidate, destination)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
#[derive(Debug, Clone)]
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
    pub fn revalidate(&self) -> Result<()> {
        require(
            self.stamp == stamp(&self.path)?,
            "artifact",
            "Verified file changed before publication",
            409,
        )
    }
}

pub fn output_store(
    config: &Config,
    bulk: &Path,
    selector: &str,
    expected: Option<&str>,
) -> Result<(PathBuf, String)> {
    let mode = config
        .data_stores
        .get(selector)
        .and_then(|s| s["mode"].as_str())
        .unwrap_or(if selector == "data/0" { "rw" } else { "" });
    require(mode == "rw", "storage", "Output store is not writable", 409)?;
    store(config, bulk, selector, expected)
}
pub fn prefix(root: &Path, value: &str) -> Result<PathBuf> {
    if value.is_empty() || value == "." {
        Ok(root.canonicalize()?)
    } else {
        relative(root, value)
    }
}
