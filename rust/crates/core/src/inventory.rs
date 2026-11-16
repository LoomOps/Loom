//! Stable File identities live in sealed, bounded manifests; the search DB can be discarded.
use crate::{
    error::require,
    index_store::{secure_dir, IndexWriter},
    items, storage,
    store::*,
    Error, Result,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
pub const CHUNK_BYTES: usize = 4 * 1024 * 1024;
pub fn root(app: &Application) -> PathBuf {
    app.config.extra.get("catalog").unwrap_or(&Value::Null)["manifest_directory"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| app.local.join("catalog-manifests"))
}
#[derive(Clone, Debug)]
pub struct PreparedInventory {
    pub item: Vec<u8>,
    pub revision: Vec<u8>,
    pub descriptor: Value,
    pub sha256: String,
}
fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut file = storage::open_regular(path)?;
    let mut bytes = vec![];
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= limit,
        "inventory",
        "Inventory metadata exceeds bound",
        413,
    )?;
    Ok(bytes)
}
fn source_entries(app: &Application, native: &Value) -> Result<Vec<Value>> {
    let v = &native["identity"];
    let mut result = vec![];
    match native["kind"].as_str().unwrap_or("") {
        "entity" => {
            let text: String = app.db.query_row(
                "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
                params![v["id"].as_str(), v["revision"].as_i64()],
                |r| r.get(0),
            )?;
            let m: Value = serde_json::from_str(&text)?;
            for c in m["components"].as_array().into_iter().flatten() {
                let locations=rows(&app.db,"SELECT machine_id,storage_id,ref,status FROM all_entity_locations WHERE id=? AND revision=? AND component=?",&[&v["id"].as_str(),&v["revision"].as_i64(),&c["ref"].as_str()])?;
                result.push(json!({"entry_key":c["ref"],"path":c["ref"],"role":if c["ref"]==m["path"]{"primary"}else{"member"},"size":c["size"],"sha256":c["sha256"],"copies":locations.into_iter().map(|mut l|{l["type"]=json!("storage");l}).collect::<Vec<_>>()}));
            }
        }
        "historical" => {
            let (m, locations) = app.historical_files(v)?;
            for c in m["components"].as_array().into_iter().flatten() {
                result.push(json!({"entry_key":c["ref"],"path":c["ref"],"role":"preserved","size":c["size"],"sha256":c["sha256"],"copies":locations.iter().filter(|l|l["component"]==c["ref"]).map(|l|{let mut l=l.clone();l["type"]=json!("storage");l}).collect::<Vec<_>>()}));
            }
        }
        "component" | "tool" => {
            let table = if native["kind"] == "tool" {
                "tools"
            } else {
                "components"
            };
            let text: String = app.db.query_row(
                &format!("SELECT manifest FROM {table} WHERE owner=? AND id=? AND revision=?"),
                params![
                    v["owner"].as_str(),
                    v["id"].as_str(),
                    v["revision"].as_i64()
                ],
                |r| r.get(0),
            )?;
            let m: Value = serde_json::from_str(&text)?;
            let prefix = format!(
                "{}{}/{}/revisions/{}",
                if table == "tools" { ".tools/" } else { "" },
                v["owner"].as_str().unwrap_or(""),
                v["id"].as_str().unwrap_or(""),
                v["revision"]
            );
            for (name, sha) in m["source_pins"].as_object().into_iter().flatten() {
                result.push(json!({"entry_key":name,"path":name,"role":"source","sha256":sha,"size":null,"copies":[{"type":"components","ref":format!("{prefix}/{name}")}]}));
            }
            if let Some(binary) = m["build_receipt"].get("binary") {
                let mut copy = binary.clone();
                copy["type"] = json!("storage");
                result.push(json!({"entry_key":"@build","path":"build artifact","role":"build","size":binary["size"],"sha256":binary["sha256"],"copies":[copy]}));
            }
        }
        _ => {}
    }
    Ok(result)
}
pub fn prepare(app: &Application, item: &[u8], revision: &[u8]) -> Result<PreparedInventory> {
    let native: String = app.db.query_row(
        "SELECT native_ref FROM catalog_revisions WHERE id=? AND item=?",
        params![revision, item],
        |r| r.get(0),
    )?;
    let native: Value = serde_json::from_str(&native)?;
    if native["kind"] == "runtime" {
        return Err(Error::new(
            "runtime_provider_required",
            "Use the granted host inventory provider",
            409,
        ));
    }
    if native["kind"] == "entity" {
        let text: String = app.db.query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![
                native["identity"]["id"].as_str(),
                native["identity"]["revision"].as_i64()
            ],
            |r| r.get(0),
        )?;
        let value: Value = serde_json::from_str(&text)?;
        if value["file_set"].is_object() {
            let primary = value["path"]
                .as_str()
                .or_else(|| value["components"][0]["ref"].as_str())
                .unwrap_or("")
                .to_string();
            let members =
                crate::file_set::Members::open(app, &value["file_set"], None)?.map(move |entry| {
                    let mut entry = entry?;
                    if entry["entry_key"] == primary {
                        entry["role"] = json!("primary");
                    }
                    Ok(entry)
                });
            return prepare_entries(app, item, revision, members);
        }
    }
    let mut entries = source_entries(app, &native)?;
    entries.sort_by(|a, b| a["entry_key"].as_str().cmp(&b["entry_key"].as_str()));
    prepare_entries(app, item, revision, entries.into_iter().map(Ok))
}
pub fn prepare_entries(
    app: &Application,
    item: &[u8],
    revision: &[u8],
    entries: impl Iterator<Item = Result<Value>>,
) -> Result<PreparedInventory> {
    crate::index_store::validate_layout(app)?;
    let root = root(app);
    secure_dir(&root)?;
    let name = items::decimal(revision)?;
    let folder = root.join(&name);
    secure_dir(&folder)?;
    let descriptor_path = folder.join("descriptor.json");
    if descriptor_path.is_file() {
        let bytes = bounded(&descriptor_path, 1024 * 1024)?;
        let descriptor: Value = serde_json::from_slice(&bytes)?;
        require(
            descriptor["item_id"] == items::decimal(item)?
                && descriptor["owner_revision_id"] == name,
            "inventory",
            "Inventory owner mismatch",
            409,
        )?;
        validate_descriptor(&root, &descriptor)?;
        return Ok(PreparedInventory {
            item: item.to_vec(),
            revision: revision.to_vec(),
            descriptor,
            sha256: hash(bytes),
        });
    }
    // A resumable intent retains assigned IDs/chunks. Never overwrite an accepted descriptor.
    let intent = folder.join("intent.json");
    let mut chunks: Vec<Value> = if intent.exists() {
        serde_json::from_slice::<Value>(&bounded(&intent, 1024 * 1024)?)?["chunks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        vec![]
    };
    let mut skip = chunks
        .iter()
        .filter_map(|c| c["entries"].as_u64())
        .sum::<u64>();
    let mut retained_entries = RetainedEntries {
        folder: folder.clone(),
        chunks: chunks.clone().into_iter(),
        lines: vec![].into_iter(),
    };
    let parent = app.get_item(&json!({"item_id":items::decimal(item)?,"revision_id":name}))?;
    let mut total = skip;
    let mut buffer = vec![];
    let mut count = 0;
    let mut previous = String::new();
    let flush = |buffer: &mut Vec<u8>, count: &mut u64, chunks: &mut Vec<Value>| -> Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        let file = format!("chunk-{:08}.jsonl", chunks.len());
        let digest = hash(&buffer);
        storage::reserve_space(
            &folder,
            buffer.len() as u64,
            app.config
                .extra
                .get("catalog")
                .and_then(|c| c["minimum_free_bytes"].as_u64())
                .unwrap_or(64 * 1024 * 1024),
        )?;
        storage::atomic(&folder.join(&file), buffer, 0o600)?;
        chunks.push(json!({"name":file,"sha256":digest,"entries":count}));
        storage::atomic(
            &intent,
            &serde_json::to_vec(&json!({"schema":"loom-file-intent/v1","chunks":chunks}))?,
            0o600,
        )?;
        buffer.clear();
        *count = 0;
        Ok(())
    };
    // Legacy callers supply sorted entries; new producer inventories must stream canonical order.
    for entry in entries {
        let mut entry = entry?;
        let key = entry["entry_key"]
            .as_str()
            .ok_or_else(|| Error::input("File entry key required"))?
            .to_string();
        require(
            !key.is_empty() && key.len() <= 2048 && key > previous,
            "inventory",
            "File keys must be unique and ordered",
            409,
        )?;
        previous = key;
        let source_digest = hash(encode(
            &json!({"entry_key":entry["entry_key"],"path":entry["path"],"role":entry["role"],"size":entry["size"],"sha256":entry["sha256"],"media_type":entry["media_type"]}),
        )?);
        if skip > 0 {
            let retained: Value = retained_entries
                .next()?
                .ok_or_else(|| Error::conflict("Resumable source truncated"))?;
            require(
                retained["source_sha256"] == source_digest,
                "inventory",
                "Resumed source differs from retained inventory",
                409,
            )?;
            skip -= 1;
            continue;
        }
        entry["source_sha256"] = json!(source_digest);
        entry["scenario_ref"] = parent["scenario_ref"].clone();
        let file_id = items::decimal(&items::allocate())?;
        entry["item_id"] = json!(file_id);
        entry["kind"] = json!("file");
        entry["revision_id"] = json!(items::decimal(&items::allocate())?);
        entry["owner_revision_id"] = json!(name);
        entry["parent_item_id"] = json!(items::decimal(item)?);
        entry["label"] = entry["path"].clone();
        entry["recorded_at"] = parent["recorded_at"].clone();
        entry["scope"] = parent["scope"].clone();
        entry["origin_type"] = parent["origin_type"].clone();
        entry["file_role"] = entry["role"].clone();
        entry["role"] = json!("main");
        let mut line = serde_json::to_vec(&entry)?;
        require(
            line.len() <= 65536,
            "inventory",
            "File descriptor exceeds bound",
            413,
        )?;
        line.push(b'\n');
        if buffer.len() + line.len() > CHUNK_BYTES {
            flush(&mut buffer, &mut count, &mut chunks)?;
        }
        buffer.extend(line);
        count += 1;
        total += 1;
    }
    require(
        skip == 0,
        "inventory",
        "Resumed inventory source changed",
        409,
    )?;
    flush(&mut buffer, &mut count, &mut chunks)?;
    let descriptor = json!({"schema":"loom-file-set/v1","item_id":items::decimal(item)?,"owner_revision_id":name,"entries":total,"chunks":chunks});
    let bytes = serde_json::to_vec(&descriptor)?;
    require(
        bytes.len() <= 1024 * 1024,
        "inventory",
        "Inventory descriptor exceeds bound",
        413,
    )?;
    storage::atomic(&descriptor_path, &bytes, 0o600)?;
    Ok(PreparedInventory {
        item: item.to_vec(),
        revision: revision.to_vec(),
        descriptor,
        sha256: hash(bytes),
    })
}
pub fn validate_descriptor(root: &Path, d: &Value) -> Result<()> {
    require(
        d["schema"] == "loom-file-set/v1",
        "inventory",
        "Unsupported file-set schema",
        409,
    )?;
    let folder = root.join(items::decimal(&items::parse(&d["owner_revision_id"])?)?);
    let mut total = 0;
    for c in d["chunks"]
        .as_array()
        .ok_or_else(|| Error::input("Inventory chunks required"))?
    {
        let p = storage::relative(
            &folder,
            c["name"]
                .as_str()
                .ok_or_else(|| Error::input("Chunk name required"))?,
        )?;
        let bytes = bounded(&p, CHUNK_BYTES)?;
        require(
            c["sha256"] == hash(&bytes),
            "inventory",
            "Inventory chunk changed",
            409,
        )?;
        let n = bytes
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .count() as u64;
        require(
            c["entries"].as_u64() == Some(n),
            "inventory",
            "Inventory chunk count changed",
            409,
        )?;
        total += n;
    }
    require(
        d["entries"].as_u64() == Some(total),
        "inventory",
        "Inventory count changed",
        409,
    )
}
pub fn project(
    db: &Connection,
    app: &Application,
    seq: i64,
    d: &Value,
    page: usize,
) -> Result<bool> {
    let parent = items::parse(&d["item_id"])?;
    let revision = items::parse(&d["owner_revision_id"])?;
    let (path, digest): (String, String) = app.db.query_row(
        "SELECT descriptor,sha256 FROM catalog_manifests WHERE revision=? AND item=?",
        params![revision, parent],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let root = root(app);
    let bytes = bounded(&storage::relative(&root, &path)?, 1024 * 1024)?;
    require(
        hash(bytes) == digest,
        "inventory",
        "Inventory descriptor changed",
        409,
    )?;
    let folder = root.join(items::decimal(&revision)?);
    for c in d["chunks"].as_array().unwrap().iter().skip(page).take(1) {
        let p = storage::relative(&folder, c["name"].as_str().unwrap())?;
        let bytes = bounded(&p, CHUNK_BYTES)?;
        require(
            hash(&bytes) == c["sha256"],
            "inventory",
            "Inventory chunk changed",
            409,
        )?;
        for line in BufReader::new(bytes.as_slice()).lines() {
            let mut entry: Value = serde_json::from_str(&line?)?;
            entry["inventory"] = json!({"descriptor":path,"sha256":digest,"chunk":c});
            let id = items::parse(&entry["item_id"])?;
            let revision = items::parse(&entry["revision_id"])?;
            IndexWriter::upsert(db, seq, &id, Some(&revision), &entry)?;
        }
    }
    Ok(page + 1 >= d["chunks"].as_array().unwrap().len())
}
impl Application {
    pub fn pending_inventories(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut q=self.db.prepare("SELECT r.item,r.id FROM catalog_revisions r JOIN catalog_items i ON r.item=i.id WHERE (i.native_kind IN ('entity','historical','component','tool') OR (i.native_kind='runtime' AND json_type(r.body,'$.file_inventory')='object')) AND NOT EXISTS(SELECT 1 FROM catalog_manifests m WHERE m.revision=r.id) ORDER BY r.rowid LIMIT 8")?;
        let values = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(values)
    }
    pub fn commit_inventory(&mut self, p: PreparedInventory) -> Result<()> {
        require(
            !self.read_only,
            "read_only",
            "Cannot register inventory from a read snapshot",
            409,
        )?;
        let path = format!("{}/descriptor.json", items::decimal(&p.revision)?);
        let bytes = bounded(&storage::relative(&root(self), &path)?, 1024 * 1024)?;
        require(
            hash(&bytes) == p.sha256 && serde_json::from_slice::<Value>(&bytes)? == p.descriptor,
            "inventory",
            "Staged inventory changed",
            409,
        )?;
        let tx = self.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalog_revisions WHERE id=? AND item=?)",
            params![p.revision, p.item],
            |r| r.get(0),
        )?;
        require(exists, "inventory", "Inventory owner not committed", 409)?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO catalog_manifests VALUES(?,?,?,?,?,?)",
            params![
                p.revision,
                p.item,
                path,
                p.sha256,
                p.descriptor["entries"].as_i64(),
                now()
            ],
        )?;
        if inserted == 1 {
            tx.execute("INSERT INTO catalog_changes(item,revision,operation,body,created) VALUES(?,?,'inventory',?,?)",params![p.item,p.revision,encode(&p.descriptor)?,now()])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn get_file(&self, args: &Value) -> Result<Value> {
        let entry = self.indexed_document(&args["file_item_id"])?;
        self.canonical_file(&entry)
    }
    pub fn canonical_file(&self, hit: &Value) -> Result<Value> {
        let revision = items::parse(&hit["owner_revision_id"])?;
        let (path, digest): (String, String) = self.db.query_row(
            "SELECT descriptor,sha256 FROM catalog_manifests WHERE revision=?",
            [revision],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let root = root(self);
        let dbytes = bounded(&storage::relative(&root, &path)?, 1024 * 1024)?;
        require(
            hash(&dbytes) == digest && hit["inventory"]["sha256"] == digest,
            "inventory",
            "Canonical inventory mismatch",
            409,
        )?;
        let d: Value = serde_json::from_slice(&dbytes)?;
        let name = hit["inventory"]["chunk"]["name"]
            .as_str()
            .ok_or_else(|| Error::input("Chunk reference required"))?;
        let chunk = d["chunks"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["name"] == name))
            .ok_or_else(|| Error::new("inventory", "Unknown inventory page", 404))?;
        let folder = root.join(
            d["owner_revision_id"]
                .as_str()
                .ok_or_else(|| Error::input("Inventory owner required"))?,
        );
        let bytes = bounded(&storage::relative(&folder, name)?, CHUNK_BYTES)?;
        require(
            hash(&bytes) == chunk["sha256"],
            "inventory",
            "File inventory changed",
            409,
        )?;
        for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let mut entry: Value = serde_json::from_slice(line)?;
            if entry["item_id"] == hit["item_id"] {
                let owner=self.get_item(&json!({"item_id":entry["parent_item_id"],"revision_id":entry["owner_revision_id"]}))?;
                let native = &owner["native_ref"];
                if native["kind"] == "entity" {
                    let v = &native["identity"];
                    let current=rows(&self.db,"SELECT 'storage' AS type,machine_id,storage_id,ref,status FROM all_entity_locations WHERE id=? AND revision=? AND component=? ORDER BY storage_id,ref LIMIT 200",&[&v["id"].as_str(),&v["revision"].as_i64(),&entry["entry_key"].as_str()])?;
                    if !current.is_empty() {
                        entry["copies"] = json!(current);
                    }
                } else if native["kind"] == "historical" {
                    let (_, locations) = self.historical_files(&native["identity"])?;
                    entry["copies"] = json!(locations
                        .into_iter()
                        .filter(|c| c["component"] == entry["entry_key"])
                        .take(200)
                        .map(|mut c| {
                            c["type"] = json!("storage");
                            c
                        })
                        .collect::<Vec<_>>());
                }
                if let Ok(mut query) = self.db.prepare(
                    "SELECT locator FROM file_bindings WHERE file=? ORDER BY locator LIMIT 200",
                ) {
                    let file_id = items::parse(&entry["item_id"])?;
                    let extra = query
                        .query_map([file_id], |r| r.get::<_, String>(0))?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for text in extra {
                        let mut copy: Value = serde_json::from_str(&text)?;
                        copy["type"] = json!("storage");
                        if !entry["copies"].is_array() {
                            entry["copies"] = json!([]);
                        }
                        if !entry["copies"].as_array().unwrap().contains(&copy) {
                            entry["copies"].as_array_mut().unwrap().push(copy);
                        }
                    }
                }
                return Ok(entry);
            }
        }
        Err(Error::new(
            "unknown_item",
            "File not in canonical inventory",
            404,
        ))
    }
    fn authoritative_verification(&self, entry: &Value, file: &fs::File) -> Result<bool> {
        let id = items::parse(&entry["item_id"])?;
        if let Ok(mut q) = self
            .db
            .prepare("SELECT evidence FROM file_bindings WHERE file=? LIMIT 200")
        {
            for text in q.query_map([id], |r| r.get::<_, String>(0))? {
                let evidence: Value = serde_json::from_str(&text?)?;
                if let Ok(proof) =
                    serde_json::from_value::<storage::VerifiedFile>(evidence["proof"].clone())
                {
                    if entry["sha256"] == proof.sha256
                        && entry["size"].as_u64() == Some(proof.size)
                        && proof.revalidate_open(file).is_ok()
                    {
                        return Ok(true);
                    }
                }
            }
        }
        let owner = self.get_item(
            &json!({"item_id":entry["parent_item_id"],"revision_id":entry["owner_revision_id"]}),
        )?;
        if owner["native_ref"]["kind"] != "entity" {
            return Ok(false);
        }
        let native = &owner["native_ref"]["identity"];
        let text: String = self.db.query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![native["id"].as_str(), native["revision"].as_i64()],
            |r| r.get(0),
        )?;
        let manifest: Value = serde_json::from_str(&text)?;
        if manifest["file_set"].is_object()
            && crate::file_set::verified_member(
                self,
                &manifest["file_set"],
                entry["entry_key"].as_str().unwrap_or(""),
                entry,
                file,
            )?
        {
            return Ok(true);
        }
        if let Some(plan) = manifest["origin"]["plan_id"].as_str() {
            let imported: bool = self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM asset_imports WHERE id=? AND revision=?)",
                params![native["id"].as_str(), native["revision"].as_i64()],
                |r| r.get(0),
            )?;
            if imported {
                let text: String = self.db.query_row(
                    "SELECT validation FROM reconcile_plans WHERE id=? AND state='applied'",
                    [plan],
                    |r| r.get(0),
                )?;
                let validation: Value = serde_json::from_str(&text)?;
                for value in validation["proofs"].as_array().into_iter().flatten() {
                    let proof: storage::VerifiedFile = serde_json::from_value(value.clone())?;
                    if entry["sha256"] == proof.sha256
                        && entry["size"].as_u64() == Some(proof.size)
                        && proof.revalidate_open(file).is_ok()
                    {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
    fn verified_open_copy(&self, entry: &Value, f: &mut fs::File) -> Result<(String, u64)> {
        let metadata = f.metadata()?;
        let stamp = file_stamp(&metadata);
        let mut cached = crate::index_store::file_receipt(self, &entry["item_id"], &stamp)?;
        if !cached {
            cached = self.authoritative_verification(entry, f)?;
        }
        if !cached {
            require(
                metadata.len() <= 4 * 1024 * 1024,
                "verification_pending",
                "Verify this exact File with the authorized verifier or enable configured background verification",
                409,
            )?;
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            require(
                entry["sha256"] == format!("{:x}", h.finalize()),
                "content_changed",
                "File differs from recorded bytes",
                409,
            )?;
        }
        let size = metadata.len();
        require(
            entry["size"].as_u64().is_none_or(|n| n == size),
            "content_changed",
            "File size changed",
            409,
        )?;
        Ok((stamp, size))
    }
    pub fn read_file(&self, args: &Value) -> Result<Value> {
        let entry = self.get_file(args)?;
        if let Some(value) = &self.live_file {
            let mut value = value.clone()?;
            require(
                value["sha256"] == entry["sha256"],
                "content_changed",
                "Protected source digest differs from the registered File",
                409,
            )?;
            value["file_item_id"] = args["file_item_id"].clone();
            return Ok(value);
        }
        let mut chosen = None;
        let mut first_error = None;
        for candidate in open_copies(self, &entry) {
            let result = candidate.and_then(|(mut file, _)| {
                self.verified_open_copy(&entry, &mut file)
                    .map(|(stamp, size)| (file, stamp, size))
            });
            match result {
                Ok(value) => {
                    chosen = Some(value);
                    break;
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error)
                    }
                }
            }
        }
        let (mut f, stamp, size) = chosen.ok_or_else(|| {
            first_error.unwrap_or_else(|| {
                Error::new(
                    "source_unavailable",
                    "No granted verified copy is connected",
                    404,
                )
            })
        })?;
        let offset = args["offset"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .map_err(|_| Error::input("Invalid byte offset"))?
            .min(size);
        let limit = args["limit"].as_u64().unwrap_or(65536).min(256 * 1024);
        f.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![];
        f.by_ref().take(limit).read_to_end(&mut bytes)?;
        require(
            stamp == file_stamp(&f.metadata()?),
            "content_changed",
            "File changed during range read",
            409,
        )?;
        use base64::Engine;
        Ok(
            json!({"file_item_id":entry["item_id"],"offset":offset.to_string(),"next_offset":(offset+bytes.len() as u64).to_string(),"total_bytes":size.to_string(),"data_base64":base64::engine::general_purpose::STANDARD.encode(&bytes),"eof":offset+bytes.len() as u64==size,"verification":"verified"}),
        )
    }
}

pub fn file_stamp(m: &fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec()
    )
}
pub(crate) fn open_copies<'a>(
    app: &'a Application,
    entry: &'a Value,
) -> impl Iterator<Item = Result<(fs::File, Value)>> + 'a {
    let maximum = app
        .config
        .extra
        .get("catalog")
        .and_then(|c| c["read_copy_candidates"].as_u64())
        .unwrap_or(8)
        .clamp(1, 32) as usize;
    entry["copies"]
        .as_array()
        .into_iter()
        .flatten()
        .take(maximum)
        .map(move |copy| {
            let mut candidate = entry.clone();
            candidate["copies"] = json!([copy]);
            open_copy(app, &candidate)
        })
}
pub fn open_copy(app: &Application, entry: &Value) -> Result<(fs::File, Value)> {
    for c in entry["copies"].as_array().into_iter().flatten() {
        let path = match c["type"].as_str() {
            Some("components") => {
                storage::relative(&app.components, c["ref"].as_str().unwrap_or(""))
            }
            Some("storage") => storage::locator(&app.config, &app.bulk, c),
            _ => continue,
        };
        let Ok(path) = path else { continue };
        let root = if c["type"] == "components" {
            app.components.clone()
        } else {
            let selector = if c["storage_id"] == app.config.storage_id {
                "data/0"
            } else {
                app.config
                    .data_stores
                    .iter()
                    .find(|(_, s)| s["storage_id"] == c["storage_id"])
                    .map(|(k, _)| k.as_str())
                    .unwrap_or("")
            };
            let Ok((root, _)) =
                storage::store(&app.config, &app.bulk, selector, c["storage_id"].as_str())
            else {
                continue;
            };
            root
        };
        if let Some(reference) = path.strip_prefix(&root).ok().and_then(|p| p.to_str()) {
            if let Ok(f) = storage::open_registered(&root, reference) {
                return Ok((f, c.clone()));
            }
        }
    }
    Err(Error::new(
        "source_unavailable",
        "No granted file copy is connected",
        404,
    ))
}

struct RetainedEntries {
    folder: PathBuf,
    chunks: std::vec::IntoIter<Value>,
    lines: std::vec::IntoIter<Vec<u8>>,
}
impl RetainedEntries {
    fn next(&mut self) -> Result<Option<Value>> {
        loop {
            if let Some(line) = self.lines.next() {
                return Ok(Some(serde_json::from_slice(&line)?));
            }
            let Some(c) = self.chunks.next() else {
                return Ok(None);
            };
            let bytes = bounded(
                &storage::relative(&self.folder, c["name"].as_str().unwrap_or(""))?,
                CHUNK_BYTES,
            )?;
            require(
                hash(&bytes) == c["sha256"],
                "inventory",
                "Resumable chunk changed",
                409,
            )?;
            self.lines = bytes
                .split(|b| *b == b'\n')
                .filter(|b| !b.is_empty())
                .map(Vec::from)
                .collect::<Vec<_>>()
                .into_iter();
        }
    }
}
