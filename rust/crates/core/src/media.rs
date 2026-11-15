//! Authenticated registered-file reads. Byte I/O runs outside the application lane.
use crate::{
    error::require,
    http::{HttpReply, HttpRequest},
    storage,
    store::*,
    Error, Result,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::Read,
    os::fd::{AsRawFd, FromRawFd},
    path::PathBuf,
};
#[derive(Debug)]
pub struct MediaRead {
    pub root: PathBuf,
    pub reference: String,
    pub sha256: String,
    pub size: u64,
    pub mime: &'static str,
}
pub fn mime(reference: &str) -> Option<(&'static str, u64)> {
    let suffix = std::path::Path::new(reference)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    let mime = match suffix.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "json" => "application/json",
        "jsonl" => "application/x-ndjson",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" | "yaml" | "yml" | "toml" | "csv" => "text/plain",
        _ => return None,
    };
    Some((
        mime,
        if mime.starts_with("image/") || mime.starts_with("video/") {
            64 * 1024 * 1024
        } else {
            256 * 1024
        },
    ))
}
pub fn file_dtos(
    identity: &Value,
    manifest: &Value,
    locations: &[Value],
    config: &Config,
) -> (Value, Value) {
    let mut files = vec![];
    let mut media = vec![];
    for l in locations {
        let reference = l["ref"].as_str().unwrap_or("");
        let component = l["component"].as_str().unwrap_or("");
        let source = manifest["components"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["ref"] == component);
        let size = source.and_then(|s| s["size"].as_u64());
        let selector = if l["storage_id"] == config.storage_id {
            Some("data/0")
        } else {
            config
                .data_stores
                .iter()
                .find(|(_, s)| s["storage_id"] == l["storage_id"])
                .map(|(k, _)| k.as_str())
        };
        let id = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!([
                identity,
                l["component"],
                l["storage_id"],
                l["ref"]
            ]))
            .unwrap_or_default(),
        );
        let preview = mime(reference);
        let status = if l["machine_id"] != config.machine_id || selector.is_none() {
            "not_connected"
        } else if source.is_none() {
            "missing"
        } else {
            l["status"].as_str().unwrap_or("unavailable")
        };
        let label = std::path::Path::new(reference)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(reference);
        files.push(json!({"id":id,"label":label,"source_ref":component,"target_ref":reference,"path":format!("{}/{}",selector.unwrap_or("references"),reference),"status":status,"size":size,"preview_id":preview.map(|_|id.clone())}));
        if let Some((mime, limit)) = preview {
            media.push(json!({"id":id,"label":label,"mime":mime,"source_ref":component,"target_ref":reference,"status":if status=="available"&&size.is_some_and(|s|s>limit){"too_large"}else{status},"size":size}));
        }
    }
    (json!(files), json!(media))
}
impl Application {
    pub fn media_lookup(&self, r: HttpRequest) -> Result<MediaRead> {
        require(
            r.method == "GET",
            "method",
            "Registered files require GET",
            405,
        )?;
        crate::auth::authenticate(&self.db, &self.config, &r.headers, &r.peer, r.dev)?
            .require("read")?;
        let key = r
            .path
            .split('?')
            .next()
            .unwrap_or("")
            .strip_prefix("/api/dashboard/media/")
            .ok_or_else(|| Error::input("Media route required"))?;
        require(
            key.len() <= 8192,
            "media",
            "Invalid registered-file key",
            400,
        )?;
        let parts: Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(key)
                .map_err(|_| Error::input("Invalid registered-file key"))?,
        )?;
        require(
            parts.as_array().is_some_and(|a| a.len() == 4),
            "media",
            "Invalid registered-file identity",
            400,
        )?;
        let identity = &parts[0];
        let id = identity["id"]
            .as_str()
            .ok_or_else(|| Error::input("Entity identity required"))?;
        let (manifest, locations) = if identity["source"].is_string() {
            self.historical_files(identity)?
        } else {
            let revision = identity["revision"]
                .as_i64()
                .ok_or_else(|| Error::input("Entity revision required"))?;
            let text: String = self.db.query_row(
                "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
                rusqlite::params![id, revision],
                |r| r.get(0),
            )?;
            (
                serde_json::from_str(&text)?,
                rows(
                    &self.db,
                    "SELECT * FROM all_entity_locations WHERE id=? AND revision=?",
                    &[&id, &revision],
                )?,
            )
        };
        let component = manifest["components"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["ref"] == parts[1])
            .ok_or_else(|| Error::new("media", "Registered component not found", 404))?;
        let exists = locations.iter().any(|l| {
            l["component"] == parts[1]
                && l["storage_id"] == parts[2]
                && l["ref"] == parts[3]
                && l["machine_id"] == self.config.machine_id
                && l["status"] == "available"
        });
        require(exists, "media", "Registered location unavailable", 404)?;
        let reference = parts[3]
            .as_str()
            .ok_or_else(|| Error::input("File reference required"))?;
        let (_, limit) = mime(reference).ok_or_else(|| Error::input("File type has no preview"))?;
        let size = component["size"]
            .as_u64()
            .ok_or_else(|| Error::input("Registered file size required"))?;
        require(
            size <= limit,
            "media",
            "Registered file exceeds preview bound",
            413,
        )?;
        storage::locator(
            &self.config,
            &self.bulk,
            &json!({"machine_id":self.config.machine_id,"storage_id":parts[2],"ref":reference}),
        )?;
        let selector = if parts[2] == self.config.storage_id {
            "data/0"
        } else {
            self.config
                .data_stores
                .iter()
                .find(|(_, s)| s["storage_id"] == parts[2])
                .map(|(k, _)| k.as_str())
                .ok_or_else(|| Error::input("Unknown storage"))?
        };
        let (configured, _) =
            storage::store(&self.config, &self.bulk, selector, parts[2].as_str())?;
        Ok(MediaRead {
            root: configured,
            reference: reference.into(),
            sha256: component["sha256"]
                .as_str()
                .ok_or_else(|| Error::input("Registered hash required"))?
                .into(),
            size,
            mime: mime(reference).unwrap().0,
        })
    }
}
pub fn read(request: MediaRead) -> Result<HttpReply> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(&request.root)?;
    let parts = std::path::Path::new(&request.reference)
        .components()
        .map(|c| match c {
            std::path::Component::Normal(n) => std::ffi::CString::new(n.as_encoded_bytes())
                .map_err(|_| Error::input("Invalid path")),
            _ => Err(Error::input("Invalid path")),
        })
        .collect::<Result<Vec<_>>>()?;
    let mut stream = None;
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                part.as_ptr(),
                libc::O_RDONLY
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | if last { 0 } else { libc::O_DIRECTORY },
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if last {
            stream = Some(file)
        } else {
            directory = file
        }
    }
    let mut stream = stream.ok_or_else(|| Error::input("File path required"))?;
    let facts = stream.metadata()?;
    require(
        facts.is_file() && facts.len() == request.size,
        "media",
        "Registered file changed",
        409,
    )?;
    let mut bytes = Vec::with_capacity(request.size as usize);
    stream
        .by_ref()
        .take(request.size + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() as u64 == request.size && hash(&bytes) == request.sha256,
        "media",
        "Registered file hash changed",
        409,
    )?;
    Ok(HttpReply {
        status: 200,
        content_type: request.mime,
        body: bytes,
    })
}
