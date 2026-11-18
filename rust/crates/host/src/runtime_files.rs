//! Explicit administrator inventories under the configured dependency directory.
//! Runtime file pages/ranges are instance-granted capabilities, never host paths from callers.
use crate::{executor::Config, runtime};
use loom_primitives::{
    error::require,
    file_set, storage,
    store::{hash, now},
    Error, Result,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn dependency_root(config: &Config, instance: &str) -> Result<PathBuf> {
    let account = config
        .instances
        .get(instance)
        .ok_or_else(|| Error::new("instance", "Unknown instance", 403))?;
    let root = account
        .extra
        .get("dependencies")
        .or_else(|| config.extra.get("dependencies"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("dependencies", "No configured dependency grant", 403))?;
    let path = PathBuf::from(root);
    require(
        path.is_absolute() && path.canonicalize()? == path,
        "dependencies",
        "Dependency root missing or redirected",
        409,
    )?;
    Ok(path)
}
fn stamp(root: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let m = root.metadata()?;
    Ok(hash(format!("{}:{}:{}", root.display(), m.dev(), m.ino())))
}
/// Only creates a new Runtime metadata revision after all selected bytes are verified.
pub fn register_inventory(
    root: &Path,
    config: &Config,
    instance: &str,
    request: &Value,
) -> Result<Value> {
    let runtime = runtime::detail(root, instance, &request["runtime_ref"])?;
    let base = dependency_root(config, instance)?;
    let scopes = request["roots"]
        .as_array()
        .ok_or_else(|| Error::input("Select explicit dependency-relative roots"))?;
    require(
        !scopes.is_empty() && scopes.len() <= 16,
        "inventory_scope",
        "Select one to sixteen explicit package roots",
        400,
    )?;
    let maximum = request["max_entries"]
        .as_u64()
        .unwrap_or(10000)
        .min(1000000);
    let max_bytes = request["max_bytes"].as_u64().unwrap_or(64 * 1024 * 1024);
    let deadline = Instant::now()
        + Duration::from_secs(request["max_seconds"].as_u64().unwrap_or(30).clamp(1, 3600));
    let folder = uuid::Uuid::now_v7().to_string();
    let directory = root.join("runtime-files").join(&folder);
    loom_primitives::index_store::secure_dir(&directory)?;
    let mut entries = 0u64;
    let mut bytes = 0u64;
    let mut skipped = 0u64;
    let mut chunks = vec![];
    let mut buffer = vec![];
    let mut count = 0u64;
    let mut first = String::new();
    let mut last = String::new();
    let mut selected = vec![];
    let flush = |buffer: &mut Vec<u8>,
                 count: &mut u64,
                 chunks: &mut Vec<Value>,
                 first: &mut String,
                 last: &str|
     -> Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        let name = format!("{:06}.jsonl", chunks.len());
        storage::atomic(&directory.join(&name), buffer, 0o600)?;
        chunks.push(
            json!({"name":name,"sha256":hash(&buffer),"entries":*count,"first":first,"last":last}),
        );
        buffer.clear();
        *count = 0;
        first.clear();
        Ok(())
    };
    let mut scopes = scopes.clone();
    scopes.sort_by_key(|s| s["prefix"].as_str().unwrap_or("").to_string());
    for scope in scopes {
        let prefix = scope["prefix"]
            .as_str()
            .ok_or_else(|| Error::input("Dependency-relative prefix required"))?;
        require(
            !prefix.is_empty() && prefix != ".",
            "inventory_scope",
            "Do not inventory the entire dependency directory implicitly",
            400,
        )?;
        if scope["dependency"].is_object() {
            let expected = loom_primitives::items::dependency_ref(&runtime, &scope["dependency"]);
            require(
                runtime["manifest"]["dependencies"]
                    .as_array()
                    .is_some_and(|deps| {
                        deps.iter().any(|d| {
                            loom_primitives::items::dependency_ref(&runtime, d) == expected
                        })
                    }),
                "dependency_scope",
                "Inventory dependency is not declared by the exact Runtime",
                409,
            )?;
        }
        let source = storage::relative(&base, prefix)?;
        require(
            !selected
                .iter()
                .any(|p: &PathBuf| source.starts_with(p) || p.starts_with(&source)),
            "inventory_scope",
            "Inventory roots overlap",
            400,
        )?;
        selected.push(source.clone());
        let mut stack = std::collections::BTreeSet::new();
        stack.insert(
            source
                .to_str()
                .ok_or_else(|| Error::input("Runtime filenames must be UTF-8"))?
                .to_string(),
        );
        let mut visited = 0u64;
        while let Some(path) = stack.pop_first() {
            visited += 1;
            require(
                visited <= maximum.saturating_add(16),
                "inventory_budget",
                "Traversal entry budget exhausted",
                409,
            )?;
            let path = PathBuf::from(path);
            require(
                Instant::now() < deadline,
                "inventory_budget",
                "Runtime inventory time budget exhausted",
                409,
            )?;
            let metadata = path.symlink_metadata()?;
            if metadata.file_type().is_symlink() {
                skipped += 1;
                continue;
            }
            if metadata.is_dir() {
                let mut children = fs::read_dir(&path)?
                    .take(maximum as usize + 1)
                    .map(|e| e.map(|e| e.path()))
                    .collect::<std::io::Result<Vec<_>>>()?;
                require(
                    children.len() as u64 <= maximum,
                    "inventory_budget",
                    "Directory exceeds the explicit entry budget",
                    409,
                )?;
                children.sort();
                for child in children {
                    stack.insert(
                        child
                            .to_str()
                            .ok_or_else(|| Error::input("Runtime filenames must be UTF-8"))?
                            .to_string(),
                    );
                }
                continue;
            }
            require(
                metadata.is_file(),
                "runtime_inventory",
                "Unsupported runtime file type",
                409,
            )?;
            require(
                entries < maximum && bytes.saturating_add(metadata.len()) <= max_bytes,
                "inventory_budget",
                "Runtime inventory entry/byte budget exhausted",
                409,
            )?;
            let proof = storage::VerifiedFile::read(&path)?;
            let relative = path
                .strip_prefix(&base)
                .map_err(|_| Error::input("Dependency file leaves root"))?
                .to_str()
                .ok_or_else(|| Error::input("Runtime filenames must be UTF-8"))?;
            require(
                last.is_empty() || relative > last.as_str(),
                "runtime_inventory",
                "Runtime inventory order changed",
                409,
            )?;
            let entry = json!({"entry_key":relative,"path":relative,"size":proof.size,"sha256":proof.sha256,"role":"dependency","dependency":scope["dependency"],"proof":proof});
            let encoded = serde_json::to_vec(&entry)?;
            require(
                encoded.len() <= 65536,
                "runtime_inventory",
                "Runtime entry exceeds bound",
                413,
            )?;
            if buffer.len() + encoded.len() + 1 > 524288 {
                flush(&mut buffer, &mut count, &mut chunks, &mut first, &last)?;
            }
            if first.is_empty() {
                first = relative.into()
            }
            last = relative.into();
            buffer.extend(encoded);
            buffer.push(b'\n');
            count += 1;
            entries += 1;
            bytes += metadata.len();
        }
    }
    flush(&mut buffer, &mut count, &mut chunks, &mut first, &last)?;
    let descriptor = json!({"schema":"loom-runtime-files/v1","root_identity":stamp(&base)?,"entries":entries,"bytes":bytes,"skipped_links":skipped,"complete":skipped==0,"chunks":chunks,"recorded":now(),"source_runtime":request["runtime_ref"]});
    let encoded = serde_json::to_vec(&descriptor)?;
    require(
        encoded.len() <= 1024 * 1024,
        "runtime_inventory",
        "Runtime descriptor exceeds bound",
        413,
    )?;
    storage::atomic(&directory.join("descriptor.json"), &encoded, 0o600)?;
    let mut manifest = runtime["manifest"].clone();
    manifest["file_inventory"] = json!({"descriptor":format!("{folder}/descriptor.json"),"sha256":hash(&encoded),"entries":entries,"bytes":bytes,"complete":skipped==0,"skipped_links":skipped});
    runtime::register(
        root,
        config,
        runtime["id"]
            .as_str()
            .ok_or_else(|| Error::input("Runtime ID missing"))?,
        manifest,
        &[instance.to_string()],
    )
}
pub struct Capability {
    root: PathBuf,
    base: PathBuf,
    runtime: Value,
}
pub fn grant(root: &Path, config: &Config, instance: &str, pin: &Value) -> Result<Capability> {
    Ok(Capability {
        root: root.into(),
        base: dependency_root(config, instance)?,
        runtime: runtime::detail(root, instance, pin)?,
    })
}
impl Capability {
    fn descriptor(&self) -> Result<(PathBuf, Value)> {
        let pin = &self.runtime["manifest"]["file_inventory"];
        let reference = pin["descriptor"].as_str().ok_or_else(|| {
            Error::new(
                "inventory_unrecorded",
                "This Runtime revision has no declared file inventory",
                409,
            )
        })?;
        let path = storage::relative(&self.root.join("runtime-files"), reference)?;
        let bytes = file_set::bounded(&path, 1024 * 1024)?;
        require(
            pin["sha256"] == hash(&bytes),
            "inventory_hash",
            "Runtime file descriptor differs",
            409,
        )?;
        let value: Value = serde_json::from_slice(&bytes)?;
        require(
            value["root_identity"] == stamp(&self.base)?,
            "runtime_files_stale",
            "Configured dependency root identity changed",
            409,
        )?;
        Ok((path.parent().unwrap().into(), value))
    }
    fn chunk(&self, folder: &Path, chunk: &Value) -> Result<Vec<Value>> {
        let path = storage::relative(
            folder,
            chunk["name"]
                .as_str()
                .ok_or_else(|| Error::input("Chunk name missing"))?,
        )?;
        let bytes = file_set::bounded(&path, 524288)?;
        require(
            chunk["sha256"] == hash(&bytes),
            "inventory_hash",
            "Runtime source chunk differs",
            409,
        )?;
        bytes
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| Ok(serde_json::from_slice(l)?))
            .collect()
    }
    pub fn call(self, action: &str, args: &Value) -> Result<Value> {
        let (folder, d) = self.descriptor()?;
        let chunks = d["chunks"]
            .as_array()
            .ok_or_else(|| Error::input("Runtime chunks missing"))?;
        if action == "runtime_file_page" {
            let page = args["page"].as_u64().unwrap_or(0) as usize;
            let row = args["row"].as_u64().unwrap_or(0) as usize;
            let limit = args["limit"].as_u64().unwrap_or(200).clamp(1, 200) as usize;
            let mut values = vec![];
            let mut p = page;
            let mut r = row;
            while values.len() < limit && p < chunks.len() {
                let entries = self.chunk(&folder, &chunks[p])?;
                while r < entries.len() && values.len() < limit {
                    let mut e = entries[r].clone();
                    e.as_object_mut().unwrap().remove("proof");
                    if e["dependency"].is_object() {
                        e["dependency_ref"] =
                            loom_primitives::items::dependency_ref(&self.runtime, &e["dependency"]);
                    }
                    e.as_object_mut().unwrap().remove("dependency");
                    e["copies"] = json!([{"type":"runtime","runtime_ref":self.runtime["runtime_ref"],"entry_key":e["entry_key"],"inventory_sha256":self.runtime["manifest"]["file_inventory"]["sha256"]}]);
                    values.push(e);
                    r += 1;
                }
                if r >= entries.len() {
                    p += 1;
                    r = 0;
                }
            }
            return Ok(
                json!({"items":values,"next":if p<chunks.len(){json!({"page":p,"row":r})}else{Value::Null},"complete":d["complete"],"skipped_links":d["skipped_links"],"inventory_sha256":self.runtime["manifest"]["file_inventory"]["sha256"]}),
            );
        }
        require(
            action == "runtime_file_read",
            "action",
            "Unknown runtime file operation",
            400,
        )?;
        let key = args["entry_key"]
            .as_str()
            .ok_or_else(|| Error::input("Exact runtime member required"))?;
        let chunk = chunks
            .iter()
            .find(|c| {
                c["first"].as_str().is_some_and(|s| key >= s)
                    && c["last"].as_str().is_some_and(|s| key <= s)
            })
            .ok_or_else(|| Error::new("runtime_file", "Runtime member not declared", 404))?;
        let entry = self
            .chunk(&folder, chunk)?
            .into_iter()
            .find(|e| e["entry_key"] == key)
            .ok_or_else(|| Error::new("runtime_file", "Runtime member not declared", 404))?;
        let path = storage::relative(&self.base, key)?;
        let proof: storage::VerifiedFile = serde_json::from_value(entry["proof"].clone())?;
        require(
            proof.path == path,
            "runtime_file",
            "Runtime member path differs",
            409,
        )?;
        proof.revalidate()?;
        let mut file = storage::open_registered(&self.base, key)?;
        proof.revalidate_open(&file)?;
        let before = loom_primitives::inventory::file_stamp(&file.metadata()?);
        let offset = args["offset"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .map_err(|_| Error::input("Invalid byte offset"))?
            .min(proof.size);
        let limit = args["limit"].as_u64().unwrap_or(65536).clamp(1, 262144);
        file.seek(SeekFrom::Start(offset))?;
        let mut data = vec![];
        file.by_ref()
            .take(limit.min(proof.size - offset))
            .read_to_end(&mut data)?;
        require(
            before == loom_primitives::inventory::file_stamp(&file.metadata()?),
            "content_changed",
            "Runtime bytes changed during range read",
            409,
        )?;
        use base64::Engine;
        Ok(
            json!({"offset":offset.to_string(),"next_offset":(offset+data.len()as u64).to_string(),"total_bytes":proof.size.to_string(),"data_base64":base64::engine::general_purpose::STANDARD.encode(&data),"sha256":proof.sha256,"eof":offset+data.len()as u64==proof.size,"verified":true}),
        )
    }
}
