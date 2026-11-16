//! Streamed, verified producer/import inventories shared by publication and reconciliation.
//! Chunks are authoritative metadata; no per-member rows are inserted into the core DB.
use crate::{error::require, inventory, storage, store::*, Error, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn root(app: &Application) -> PathBuf {
    app.local.join("file-sources")
}
pub fn bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = vec![];
    storage::open_regular(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= max,
        "inventory",
        "Inventory metadata exceeds bound",
        413,
    )?;
    Ok(bytes)
}
fn key(value: &Value) -> Result<&str> {
    let value = value
        .as_str()
        .ok_or_else(|| Error::input("Inventory key required"))?;
    require(
        !value.is_empty()
            && value.len() <= 2048
            && !value.contains('\0')
            && !Path::new(value).is_absolute()
            && Path::new(value)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "inventory",
        "Invalid inventory key",
        400,
    )?;
    Ok(value)
}
fn immutable(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        require(
            hash(bounded(path, inventory::CHUNK_BYTES * 4)?) == hash(bytes),
            "inventory_conflict",
            "Retained inventory differs",
            409,
        )
    } else {
        storage::atomic(path, bytes, 0o600)
    }
}
/// Caller runs this on a bounded I/O worker, never on the execution mutation lane.
pub fn seal(
    app: &Application,
    base: &Path,
    storage_selector: &str,
    declaration: &Value,
) -> Result<Value> {
    require(
        declaration["schema"] == "loom-output-inventory/v1",
        "inventory_schema",
        "Unsupported producer inventory; discover current contracts",
        400,
    )?;
    let chunks = declaration["chunks"]
        .as_array()
        .ok_or_else(|| Error::input("Inventory chunks required"))?;
    require(
        !chunks.is_empty() && chunks.len() <= 4096 && encode(declaration)?.len() <= 1024 * 1024,
        "inventory",
        "Invalid inventory descriptor",
        413,
    )?;
    let (store_root, store_id) = storage::store(&app.config, &app.bulk, storage_selector, None)?;
    require(
        base.starts_with(&store_root),
        "inventory",
        "Inventory leaves selected store",
        403,
    )?;
    let name = hash(encode(&json!([base, store_id, declaration]))?);
    let directory = root(app).join(&name);
    crate::index_store::secure_dir(&directory)?;
    storage::reserve_space(
        &directory,
        inventory::CHUNK_BYTES as u64 * 5,
        app.config
            .extra
            .get("catalog")
            .and_then(|c| c["minimum_free_bytes"].as_u64())
            .unwrap_or(64 * 1024 * 1024),
    )?;
    let descriptor = directory.join("descriptor.json");
    if descriptor.exists() {
        let value: Value = serde_json::from_slice(&bounded(&descriptor, 1024 * 1024)?)?;
        revalidate(
            app,
            &json!({"ref":format!("{name}/descriptor.json"),"sha256":hash(bounded(&descriptor,1024*1024)?)}),
        )?;
        return Ok(
            json!({"schema":"loom-file-source/v1","ref":format!("{name}/descriptor.json"),"sha256":hash(bounded(&descriptor,1024*1024)?),"entries":value["entries"],"bytes":value["bytes"]}),
        );
    }
    let mut sealed = vec![];
    let mut previous = String::new();
    let mut count = 0u64;
    let mut total = 0u64;
    for (index, chunk) in chunks.iter().enumerate() {
        let path = storage::relative(base, key(&chunk["ref"])?)?;
        let bytes = bounded(&path, inventory::CHUNK_BYTES)?;
        require(
            chunk["sha256"] == hash(&bytes),
            "inventory_hash",
            "Producer chunk hash differs",
            409,
        )?;
        let mut output = vec![];
        let mut proofs = vec![];
        let mut first = String::new();
        let mut entries = 0u64;
        for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            require(
                line.len() <= 65536,
                "inventory",
                "Inventory member exceeds bound",
                413,
            )?;
            let entry: Value = serde_json::from_slice(line)?;
            let name = key(&entry["key"])?;
            require(
                name > previous.as_str(),
                "inventory_order",
                "Inventory keys must be unique and sorted",
                409,
            )?;
            previous = name.into();
            if first.is_empty() {
                first = name.into()
            }
            let path = storage::relative(base, key(&entry["path"])?)?;
            let proof = storage::VerifiedFile::read(&path)?;
            require(
                entry["size"].as_u64() == Some(proof.size) && entry["sha256"] == proof.sha256,
                "content_changed",
                "Member differs from declared bytes",
                409,
            )?;
            total = total
                .checked_add(proof.size)
                .ok_or_else(|| Error::input("Inventory byte count overflow"))?;
            let reference = path
                .strip_prefix(&store_root)
                .map_err(|_| Error::input("Inventory member outside store"))?
                .to_string_lossy();
            let normalized = json!({"entry_key":name,"path":entry["path"],"role":entry["role"].as_str().unwrap_or("member"),"size":proof.size,"sha256":proof.sha256,"media_type":entry["media_type"],"copies":[{"type":"storage","machine_id":app.config.machine_id,"storage_id":store_id,"ref":reference}]});
            output.extend(serde_json::to_vec(&normalized)?);
            output.push(b'\n');
            proofs.extend(serde_json::to_vec(&proof)?);
            proofs.push(b'\n');
            require(
                output.len() <= inventory::CHUNK_BYTES
                    && proofs.len() <= inventory::CHUNK_BYTES * 4,
                "inventory",
                "Normalized chunk exceeds bound; use smaller producer chunks",
                413,
            )?;
            entries += 1;
            count += 1;
        }
        require(
            chunk["entries"].as_u64() == Some(entries) && entries > 0,
            "inventory_count",
            "Producer chunk count differs",
            409,
        )?;
        let file = format!("{index:06}.jsonl");
        let verification = format!("{index:06}.proofs");
        immutable(&directory.join(&file), &output)?;
        immutable(&directory.join(&verification), &proofs)?;
        sealed.push(json!({"name":file,"sha256":hash(&output),"entries":entries,"first":first,"last":previous,"proofs":verification,"proof_sha256":hash(&proofs)}));
    }
    require(
        declaration["entries"].as_u64() == Some(count),
        "inventory_count",
        "Producer total count differs",
        409,
    )?;
    let value =
        json!({"schema":"loom-file-source/v1","entries":count,"bytes":total,"chunks":sealed});
    immutable(&descriptor, &serde_json::to_vec(&value)?)?;
    let result = json!({"schema":"loom-file-source/v1","ref":format!("{name}/descriptor.json"),"sha256":hash(bounded(&descriptor,1024*1024)?),"entries":count,"bytes":total});
    revalidate(app, &result)?;
    Ok(result)
}
pub fn descriptor(app: &Application, pin: &Value) -> Result<(PathBuf, Value)> {
    let path = storage::relative(&root(app), key(&pin["ref"])?)?;
    let bytes = bounded(&path, 1024 * 1024)?;
    require(
        pin["sha256"] == hash(&bytes),
        "inventory_hash",
        "Authoritative file-source descriptor differs",
        409,
    )?;
    let value: Value = serde_json::from_slice(&bytes)?;
    require(
        value["schema"] == "loom-file-source/v1",
        "inventory_schema",
        "Unknown file-source schema",
        409,
    )?;
    Ok((path.parent().unwrap().into(), value))
}
pub fn revalidate(app: &Application, pin: &Value) -> Result<()> {
    let (folder, d) = descriptor(app, pin)?;
    for chunk in d["chunks"]
        .as_array()
        .ok_or_else(|| Error::input("Missing source chunks"))?
    {
        let bytes = bounded(
            &storage::relative(&folder, key(&chunk["proofs"])?)?,
            inventory::CHUNK_BYTES * 4,
        )?;
        require(
            hash(&bytes) == chunk["proof_sha256"],
            "inventory_hash",
            "Inventory verification evidence differs",
            409,
        )?;
        for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let proof: storage::VerifiedFile = serde_json::from_slice(line)?;
            proof.revalidate()?;
        }
    }
    Ok(())
}
/// Read one protected proof chunk, not a whole payload or inventory.
pub fn verified_member(
    app: &Application,
    pin: &Value,
    key: &str,
    expected: &Value,
    file: &std::fs::File,
) -> Result<bool> {
    let (folder, d) = descriptor(app, pin)?;
    let Some(chunk) = d["chunks"].as_array().into_iter().flatten().find(|c| {
        c["first"].as_str().is_some_and(|v| key >= v)
            && c["last"].as_str().is_some_and(|v| key <= v)
    }) else {
        return Ok(false);
    };
    let bytes = bounded(
        &storage::relative(
            &folder,
            chunk["proofs"]
                .as_str()
                .ok_or_else(|| Error::input("Proof reference missing"))?,
        )?,
        inventory::CHUNK_BYTES * 4,
    )?;
    require(
        hash(&bytes) == chunk["proof_sha256"],
        "inventory_hash",
        "Inventory proof differs",
        409,
    )?;
    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let proof: storage::VerifiedFile = serde_json::from_slice(line)?;
        if expected["sha256"] == proof.sha256
            && expected["size"].as_u64() == Some(proof.size)
            && proof.revalidate_open(file).is_ok()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub struct Members {
    folder: PathBuf,
    chunks: std::vec::IntoIter<Value>,
    lines: std::vec::IntoIter<Value>,
    skip: Option<String>,
}
impl Members {
    pub fn open(app: &Application, pin: &Value, skip: Option<String>) -> Result<Self> {
        let (folder, d) = descriptor(app, pin)?;
        Ok(Self {
            folder,
            chunks: d["chunks"]
                .as_array()
                .ok_or_else(|| Error::input("Source chunks missing"))?
                .clone()
                .into_iter(),
            lines: vec![].into_iter(),
            skip,
        })
    }
}
impl Iterator for Members {
    type Item = Result<Value>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(line) = self.lines.next() {
                if self.skip.as_ref().is_some_and(|s| line["entry_key"] == *s) {
                    continue;
                }
                return Some(Ok(line));
            }
            let chunk = self.chunks.next()?;
            let result = (|| -> Result<Vec<Value>> {
                let bytes = bounded(
                    &storage::relative(&self.folder, key(&chunk["name"])?)?,
                    inventory::CHUNK_BYTES * 4,
                )?;
                require(
                    hash(&bytes) == chunk["sha256"],
                    "inventory_hash",
                    "Authoritative source chunk differs",
                    409,
                )?;
                bytes
                    .split(|b| *b == b'\n')
                    .filter(|l| !l.is_empty())
                    .map(|l| Ok(serde_json::from_slice(l)?))
                    .collect()
            })();
            match result {
                Ok(lines) => self.lines = lines.into_iter(),
                Err(e) => return Some(Err(e)),
            }
        }
    }
}
pub fn lookup(app: &Application, pin: &Value, entry: &str) -> Result<Value> {
    let (folder, d) = descriptor(app, pin)?;
    let chunk = d["chunks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| {
            c["first"].as_str().is_some_and(|s| entry >= s)
                && c["last"].as_str().is_some_and(|s| entry <= s)
        })
        .ok_or_else(|| Error::new("inventory_member", "Exact member not found", 404))?;
    let bytes = bounded(
        &storage::relative(&folder, key(&chunk["name"])?)?,
        inventory::CHUNK_BYTES * 4,
    )?;
    require(
        hash(&bytes) == chunk["sha256"],
        "inventory_hash",
        "Source chunk differs",
        409,
    )?;
    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        if value["entry_key"] == entry {
            return Ok(value);
        }
    }
    Err(Error::new(
        "inventory_member",
        "Exact member not found",
        404,
    ))
}

impl crate::Application {
    /// Exact revision/member resolution works from authority, without an FTS index.
    pub fn artifact_input(&self, identity: &Value, member: Option<&str>) -> Result<Value> {
        self.artifact_copies(identity, member)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                Error::new(
                    "source_unavailable",
                    "No granted registered artifact copy is available",
                    409,
                )
            })
    }

    pub(crate) fn artifact_copies(
        &self,
        identity: &Value,
        member: Option<&str>,
    ) -> Result<Vec<Value>> {
        use rusqlite::params;
        let id = identity["id"]
            .as_str()
            .ok_or_else(|| Error::input("Artifact ID required"))?;
        let revision = identity["revision"]
            .as_i64()
            .ok_or_else(|| Error::input("Pin the exact artifact revision"))?;
        let (text, job): (String, Option<String>) = self.db.query_row(
            "SELECT manifest,job FROM all_entity_revisions WHERE id=? AND revision=?",
            params![id, revision],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if let Some(job) = job {
            let record = crate::jobs::load(&self.db, &job)?;
            require(
                loom_domain::dependency_available(
                    record.aggregate.state,
                    record
                        .result
                        .as_ref()
                        .is_some_and(|r| r["artifact_ready"] == true),
                ),
                "input",
                "Artifact producer is not ready",
                409,
            )?;
        }
        let value: Value = serde_json::from_str(&text)?;
        let name = member
            .or_else(|| value["path"].as_str())
            .or_else(|| value["components"][0]["ref"].as_str())
            .ok_or_else(|| Error::input("Primary member is unknown"))?;
        let expected = if value["file_set"].is_object() {
            lookup(self, &value["file_set"], name)?
        } else {
            value["components"]
                .as_array()
                .and_then(|a| a.iter().find(|m| m["ref"] == name))
                .cloned()
                .ok_or_else(|| Error::new("inventory_member", "Artifact member not found", 404))?
        };
        let mut copies=crate::store::rows(&self.db,"SELECT machine_id,storage_id,ref FROM all_entity_locations WHERE id=? AND revision=? AND component=? AND status='available' ORDER BY ref",&[&id,&revision,&name])?;
        copies.extend(expected["copies"].as_array().cloned().unwrap_or_default());
        let mut granted = Vec::new();
        for mut copy in copies {
            if storage::locator(&self.config, &self.bulk, &copy).is_ok() {
                copy["sha256"] = expected["sha256"].clone();
                if !granted.contains(&copy) {
                    granted.push(copy);
                }
            }
        }
        require(
            !granted.is_empty(),
            "source_unavailable",
            "No granted registered artifact copy is available",
            409,
        )?;
        Ok(granted)
    }
}
