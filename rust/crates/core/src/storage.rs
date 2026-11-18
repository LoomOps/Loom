//! Configured, marker-verified storage and immutable package publication.
use crate::{error::require, store::*, Error, Result};
pub use loom_primitives::storage::relative;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
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
    let mut bytes = Vec::new();
    open_regular(&root.join(".loom-storage.json"))?
        .take(65537)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= 65536,
        "storage",
        "Storage marker exceeds bound",
        409,
    )?;
    let marker: Value = serde_json::from_slice(&bytes)?;
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
pub use loom_primitives::storage::{atomic, file_hash, open_regular};
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
pub use loom_primitives::storage::VerifiedFile;
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

pub use loom_primitives::storage::open_registered;
/// Metadata/log workers reserve space without inspecting payload trees.
pub fn reserve_space(root: &Path, additional: u64, reserve: u64) -> Result<()> {
    let name = std::ffi::CString::new(root.as_os_str().as_encoded_bytes())
        .map_err(|_| Error::input("Invalid metadata root"))?;
    let mut info = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    require(
        unsafe { libc::statvfs(name.as_ptr(), info.as_mut_ptr()) } == 0,
        "metadata_space",
        "Cannot inspect metadata capacity",
        503,
    )?;
    let info = unsafe { info.assume_init() };
    require(
        info.f_bavail.saturating_mul(info.f_frsize) >= additional.saturating_add(reserve),
        "metadata_space",
        "Configured metadata disk reserve would be exhausted",
        409,
    )
}
