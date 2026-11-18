//! Rebuildable search projection. Its writer never owns domain transitions.
use crate::{auth::Actor, error::require, items, store::*, Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use sha2::Sha256;
use std::{
    fs,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
pub const SCHEMA:&str="CREATE TABLE IF NOT EXISTS meta(id INTEGER PRIMARY KEY CHECK(id=1),generation TEXT NOT NULL,origin BLOB NOT NULL,checkpoint INTEGER NOT NULL DEFAULT 0,schema INTEGER NOT NULL DEFAULT 1,version INTEGER NOT NULL DEFAULT 0,chunk INTEGER NOT NULL DEFAULT 0,verification_after BLOB); CREATE TABLE IF NOT EXISTS docs(rowid INTEGER PRIMARY KEY,id BLOB NOT NULL,revision BLOB,kind TEXT NOT NULL,parent BLOB,label TEXT NOT NULL,text TEXT NOT NULL,recorded REAL NOT NULL,scope TEXT NOT NULL,role TEXT NOT NULL,valid_from INTEGER NOT NULL,valid_to INTEGER,body TEXT NOT NULL,UNIQUE(id,valid_from)); CREATE INDEX IF NOT EXISTS docs_identity ON docs(id,valid_from,valid_to); CREATE INDEX IF NOT EXISTS docs_page ON docs(kind,scope,role,recorded,id); CREATE INDEX IF NOT EXISTS docs_parent ON docs(parent,valid_from,valid_to); CREATE VIRTUAL TABLE IF NOT EXISTS terms USING fts5(text,content='docs',content_rowid='rowid',tokenize='unicode61'); CREATE VIRTUAL TABLE IF NOT EXISTS grams USING fts5(text,content='docs',content_rowid='rowid',tokenize='trigram'); CREATE TRIGGER IF NOT EXISTS docs_terms AFTER INSERT ON docs BEGIN INSERT INTO terms(rowid,text) VALUES(new.rowid,new.text); INSERT INTO grams(rowid,text) VALUES(new.rowid,new.text); END; CREATE TRIGGER IF NOT EXISTS docs_delete AFTER DELETE ON docs BEGIN INSERT INTO terms(terms,rowid,text) VALUES('delete',old.rowid,old.text); INSERT INTO grams(grams,rowid,text) VALUES('delete',old.rowid,old.text); END; CREATE TABLE IF NOT EXISTS projection_versions(version INTEGER PRIMARY KEY,created REAL NOT NULL); CREATE TABLE IF NOT EXISTS projection_retention(id INTEGER PRIMARY KEY,min_version INTEGER NOT NULL); INSERT OR IGNORE INTO projection_retention VALUES(1,0); CREATE TABLE IF NOT EXISTS log_sources(stream BLOB,generation TEXT,offset INTEGER NOT NULL,stamp TEXT NOT NULL,PRIMARY KEY(stream,generation));";
pub fn directory(local: &Path, config: &Config) -> PathBuf {
    config.extra.get("catalog").unwrap_or(&Value::Null)["index_directory"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| local.join("search"))
}
pub fn active_directory(app: &Application) -> Result<PathBuf> {
    let base = directory(&app.local, &app.config);
    let pointer = base.join("active.json");
    if !pointer.exists() {
        return Ok(base);
    }
    let value: Value = serde_json::from_slice(&crate::file_set::bounded(&pointer, 8192)?)?;
    let relative = value["directory"]
        .as_str()
        .ok_or_else(|| Error::input("Search generation pointer missing"))?;
    require(
        relative.starts_with("shadows/") && relative.split('/').count() == 2,
        "index_generation",
        "Invalid generation pointer",
        409,
    )?;
    crate::storage::relative(&base, relative)
}
/// Only a caught-up, source-qualified shadow may become active. Existing readers keep their DB handles.
pub fn activate(app: &Application, task: &str) -> Result<bool> {
    require(
        task.bytes().all(|b| b.is_ascii_digit()),
        "task",
        "Invalid rebuild identity",
        400,
    )?;
    let base = directory(&app.local, &app.config);
    let relative = format!("shadows/{task}");
    let directory = base.join(&relative);
    secure_dir(&directory)?;
    let db = Connection::open_with_flags(
        directory.join("catalog.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let (origin, checkpoint, generation): (Vec<u8>, i64, String) = db.query_row(
        "SELECT origin,checkpoint,generation FROM meta WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let expected: Vec<u8> =
        app.db
            .query_row("SELECT origin FROM catalog_meta WHERE id=1", [], |r| {
                r.get(0)
            })?;
    require(
        origin == expected,
        "index_origin",
        "Shadow belongs to another authority",
        409,
    )?;
    let head: i64 = app.db.query_row(
        "SELECT COALESCE(max(seq),0) FROM catalog_changes",
        [],
        |r| r.get(0),
    )?;
    if checkpoint != head {
        return Ok(false);
    }
    crate::storage::atomic(
        &base.join("active.json"),
        &serde_json::to_vec(
            &json!({"directory":relative,"generation":generation,"activated_at":now(),"through":checkpoint}),
        )?,
        0o600,
    )?;
    Ok(true)
}
/// A disposable projection must never contain authority or share its tree.
/// Validate before an offline rebuild renames anything, without touching sources.
pub fn validate_layout(app: &Application) -> Result<()> {
    let index = directory(&app.local, &app.config);
    let manifest = crate::inventory::root(app);
    for path in [&index, &manifest] {
        require(
            path.is_absolute()
                && !path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)),
            "catalog_path",
            "Catalog roots must be absolute and normalized",
            400,
        )?;
    }
    // Reject linked ancestors before a rebuild can rename a directory reached
    // through another spelling of an authoritative root.
    for path in [&index, &manifest] {
        let mut current = PathBuf::from("/");
        for component in path.components() {
            current.push(component);
            if let Ok(metadata) = current.symlink_metadata() {
                require(
                    !metadata.file_type().is_symlink(),
                    "catalog_path",
                    "Catalog directory redirected",
                    409,
                )?;
            }
        }
    }
    let overlaps = |a: &Path, b: &Path| a.starts_with(b) || b.starts_with(a);
    let mut protected = vec![
        app.bulk.clone(),
        app.components.clone(),
        app.local.join("log-spool"),
        app.local.join("file-sources"),
    ];
    protected.extend(
        app.config
            .data_stores
            .values()
            .filter_map(|store| store["path"].as_str())
            .map(PathBuf::from),
    );
    if let Some(path) = app
        .config
        .extra
        .get("dependency_directory")
        .and_then(Value::as_str)
    {
        protected.push(PathBuf::from(path));
    }
    require(
        !overlaps(&index, &manifest)
            && [&index,&manifest].iter().all(|path|
                !app.local.starts_with(path) && protected.iter().all(|root| !overlaps(path,root))),
        "catalog_layout",
        "Derived catalog roots must be separate from authoritative state, logs, registered stores, software and dependencies",
        409,
    )
}
pub use loom_primitives::index_store::secure_dir;
pub struct IndexWriter {
    pub db: Connection,
    pub path: PathBuf,
}
impl IndexWriter {
    pub fn open(app: &Application) -> Result<Self> {
        validate_layout(app)?;
        let directory = active_directory(app)?;
        secure_dir(&directory)?;
        let path = directory.join("catalog.sqlite3");
        require(
            !path
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink()),
            "index_path",
            "Index DB redirected",
            409,
        )?;
        let _ = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)?;
        let db = Connection::open(&path)?;
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA cache_size=-16384;",
        )?;
        db.execute_batch(SCHEMA)?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS relation_docs(source BLOB NOT NULL,source_revision BLOB,target_kind TEXT NOT NULL,target_identity TEXT NOT NULL,relation TEXT NOT NULL,attributes TEXT NOT NULL,valid_from INTEGER NOT NULL,valid_to INTEGER);CREATE INDEX IF NOT EXISTS relation_source ON relation_docs(source,valid_from,valid_to);CREATE INDEX IF NOT EXISTS relation_target ON relation_docs(target_kind,target_identity,relation,valid_from,valid_to);")?;
        db.execute_batch("CREATE INDEX IF NOT EXISTS docs_historical_latest ON docs(json_extract(body,'$.native_ref.kind'),json_extract(body,'$.native_ref.identity.source'),json_extract(body,'$.native_ref.identity.kind'),json_extract(body,'$.native_ref.identity.id'),json_extract(body,'$.source_order'),valid_from);")?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS authority_anchor(id INTEGER PRIMARY KEY CHECK(id=1),seq INTEGER NOT NULL,digest TEXT NOT NULL);")?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS file_receipts(id BLOB NOT NULL,stamp TEXT NOT NULL,sha256 TEXT NOT NULL,observed REAL NOT NULL,PRIMARY KEY(id,stamp));")?;
        let origin: Vec<u8> =
            app.db
                .query_row("SELECT origin FROM catalog_meta WHERE id=1", [], |r| {
                    r.get(0)
                })?;
        db.execute(
            "INSERT OR IGNORE INTO meta(id,generation,origin) VALUES(1,?,?)",
            params![uuid::Uuid::now_v7().to_string(), origin],
        )?;
        let same: Vec<u8> = db.query_row("SELECT origin FROM meta WHERE id=1", [], |r| r.get(0))?;
        require(
            same == origin,
            "index_origin",
            "Index belongs to another authority",
            409,
        )?;
        verify_anchor(app, &db)?;
        Ok(Self { db, path })
    }
    pub fn upsert(
        db: &Connection,
        seq: i64,
        id: &[u8],
        revision: Option<&[u8]>,
        body: &Value,
    ) -> Result<()> {
        let old: Option<(i64, String)> = db
            .query_row(
                "SELECT valid_from,body FROM docs WHERE id=? AND valid_to IS NULL",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let encoded = encode(body)?;
        if old.as_ref().is_some_and(|(_, b)| b == &encoded) {
            return Ok(());
        }
        if let Some((start, _)) = old {
            if start == seq {
                db.execute(
                    "DELETE FROM docs WHERE id=? AND valid_from=?",
                    params![id, seq],
                )?;
            } else {
                db.execute(
                    "UPDATE docs SET valid_to=? WHERE id=? AND valid_to IS NULL",
                    params![seq, id],
                )?;
            }
        }
        let text = [
            body["item_id"].as_str().unwrap_or(""),
            body["label"].as_str().unwrap_or(""),
            body["summary"].as_str().unwrap_or(""),
            body["path"].as_str().unwrap_or(""),
            body["native_ref"]["identity"]["id"].as_str().unwrap_or(""),
        ]
        .join(" ");
        let parent = body["parent_item_id"]
            .as_str()
            .map(|s| items::parse(&json!(s)))
            .transpose()?;
        db.execute("INSERT INTO docs(id,revision,kind,parent,label,text,recorded,scope,role,valid_from,body) VALUES(?,?,?,?,?,?,?,?,?,?,?)",params![id,revision,body["kind"].as_str().unwrap_or("artifact"),parent,body["label"].as_str().unwrap_or(""),text,body["recorded_at"].as_f64().unwrap_or(0.),body["scope"].as_str().unwrap_or("research"),body["role"].as_str().unwrap_or("main"),seq,encoded])?;
        Ok(())
    }
    pub fn sync(&mut self, app: &Application, limit: usize) -> Result<usize> {
        let last: i64 = self
            .db
            .query_row("SELECT checkpoint FROM meta WHERE id=1", [], |r| r.get(0))?;
        let mut q=app.db.prepare("SELECT seq,item,revision,operation,body FROM catalog_changes WHERE seq>? ORDER BY seq LIMIT ?")?;
        let events = q
            .query_map(params![last, limit.min(128) as i64], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (seq, item, revision, operation, body) in &events {
            let value: Value = serde_json::from_str(body)?;
            let relations = if operation == "upsert" {
                match crate::graph::index_relations(app, &value) {
                    Ok(v) => v,
                    Err(e) if e.status == 404 || e.code == "input" => vec![],
                    Err(e) => return Err(e),
                }
            } else {
                vec![]
            };
            let tx = self.db.transaction()?;
            let version: i64 =
                tx.query_row("SELECT version+1 FROM meta WHERE id=1", [], |r| r.get(0))?;
            if operation == "upsert" {
                Self::upsert(&tx, version, item, revision.as_deref(), &value)?;
                tx.execute(
                    "UPDATE relation_docs SET valid_to=? WHERE source=? AND valid_to IS NULL",
                    params![version, item],
                )?;
                for e in relations {
                    if e["native_ref"].is_object() {
                        tx.execute(
                            "INSERT INTO relation_docs VALUES(?,?,?,?,?,?,?,NULL)",
                            params![
                                item,
                                revision,
                                e["native_ref"]["kind"].as_str(),
                                encode(&e["native_ref"]["identity"])?,
                                e["relation"].as_str(),
                                encode(&e)?,
                                version
                            ],
                        )?;
                    }
                }
            }
            if operation == "inventory" {
                let chunk: i64 =
                    tx.query_row("SELECT chunk FROM meta WHERE id=1", [], |r| r.get(0))?;
                let finished =
                    crate::inventory::project(&tx, app, version, &value, chunk as usize)?;
                if !finished {
                    tx.execute(
                        "UPDATE meta SET chunk=chunk+1,version=? WHERE id=1",
                        [version],
                    )?;
                    tx.commit()?;
                    return Ok(1);
                }
                tx.execute("UPDATE meta SET chunk=0 WHERE id=1", [])?;
            }
            if operation == "tombstone" {
                tx.execute(
                    "UPDATE relation_docs SET valid_to=? WHERE source=? AND valid_to IS NULL",
                    params![version, item],
                )?;
                tx.execute(
                    "UPDATE docs SET valid_to=? WHERE id=? AND valid_to IS NULL",
                    params![version, item],
                )?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO projection_versions VALUES(?,?)",
                params![version, now()],
            )?;
            tx.execute("INSERT INTO authority_anchor VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET seq=excluded.seq,digest=excluded.digest",params![seq,anchor_digest(&app.db,*seq)?])?;
            tx.execute(
                "UPDATE meta SET checkpoint=?,version=? WHERE id=1",
                params![seq, version],
            )?;
            tx.commit()?;
        }
        Ok(events.len())
    }
}
fn app_schema(app: &Application) -> Result<i64> {
    Ok(app.db.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}
fn anchor_digest(db: &Connection, seq: i64) -> Result<String> {
    let value: (Vec<u8>, Option<Vec<u8>>, String, String) = db.query_row(
        "SELECT item,revision,operation,body FROM catalog_changes WHERE seq=?",
        [seq],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    Ok(hash(serde_json::to_vec(&value)?))
}
fn verify_anchor(app: &Application, index: &Connection) -> Result<()> {
    let exists: bool = index.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='authority_anchor')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(());
    }
    let anchor: Option<(i64, String)> = index
        .query_row(
            "SELECT seq,digest FROM authority_anchor WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((seq, digest)) = anchor {
        let authority = Connection::open_with_flags(
            app.local.join("ops.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        require(anchor_digest(&authority,seq).is_ok_and(|v|v==digest),"index_authority_changed","Search is ahead of or conflicts with restored authority; rebuild the derived generation",409)?;
    }
    Ok(())
}
pub(crate) fn read(app: &Application) -> Result<Connection> {
    let path = active_directory(app)?.join("catalog.sqlite3");
    require(
        path.is_file() && !path.symlink_metadata()?.file_type().is_symlink(),
        "index_unavailable",
        "Search index is unavailable",
        503,
    )?;
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(std::time::Duration::from_millis(200))?;
    let origin: Vec<u8> =
        app.db
            .query_row("SELECT origin FROM catalog_meta WHERE id=1", [], |r| {
                r.get(0)
            })?;
    let header_exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='meta')",
        [],
        |r| r.get(0),
    )?;
    require(
        header_exists,
        "index_unavailable",
        "Search index initialization is pending",
        503,
    )?;
    let actual: Option<Vec<u8>> = db
        .query_row("SELECT origin FROM meta WHERE id=1", [], |r| r.get(0))
        .optional()?;
    let actual = actual.ok_or_else(|| {
        Error::new(
            "index_unavailable",
            "Search index initialization is pending",
            503,
        )
    })?;
    require(
        origin == actual,
        "index_origin",
        "Search authority mismatch",
        409,
    )?;
    verify_anchor(app, &db)?;
    use std::os::unix::ffi::OsStrExt;
    let path = app.local.join("ops.sqlite3");
    let uri = path
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/._-".contains(b) {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect::<String>();
    db.execute(
        "ATTACH DATABASE ? AS authority",
        [format!("file:{uri}?mode=ro")],
    )?;
    Ok(db)
}
pub(crate) fn context(app: &Application, actor: &Actor) -> Result<(Vec<u8>, String)> {
    let (key, epoch): (Vec<u8>, i64) = app.db.query_row(
        "SELECT cursor_key,access_epoch FROM catalog_meta WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((
        key,
        hash(encode(&json!([
            actor.id,
            actor.scopes,
            actor.credential_id,
            epoch,
            app.config.machine_id,
            app.config.storage_id,
            app.config.data_stores,
            app.bulk,
            app.components
        ]))?),
    ))
}
pub(crate) fn sign(value: &Value, key: &[u8]) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    let mut h =
        Hmac::<Sha256>::new_from_slice(key).map_err(|_| Error::input("Invalid cursor key"))?;
    h.update(&bytes);
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&bytes),
        URL_SAFE_NO_PAD.encode(h.finalize().into_bytes())
    ))
}
pub(crate) fn cursor(text: &str, key: &[u8]) -> Result<Value> {
    require(
        text.len() <= 4096,
        "cursor_expired",
        "Invalid search cursor",
        400,
    )?;
    let (body, signature) = text
        .split_once('.')
        .ok_or_else(|| Error::input("Invalid cursor"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| Error::input("Invalid cursor"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| Error::input("Invalid cursor"))?;
    let mut h = Hmac::<Sha256>::new_from_slice(key).map_err(|_| Error::input("Invalid cursor"))?;
    h.update(&bytes);
    require(
        h.verify_slice(&signature).is_ok(),
        "cursor_expired",
        "Invalid search cursor",
        409,
    )?;
    Ok(serde_json::from_slice(&bytes)?)
}
impl Application {
    pub fn search_status(&self) -> Result<Value> {
        let head: i64 = self.db.query_row(
            "SELECT COALESCE(max(seq),0) FROM catalog_changes",
            [],
            |r| r.get(0),
        )?;
        let pending: i64 = self.db.query_row(
            "SELECT count(*) FROM catalog_backfill WHERE complete=0",
            [],
            |r| r.get(0),
        )?;
        let dirty: i64 = self
            .db
            .query_row("SELECT count(*) FROM catalog_dirty", [], |r| r.get(0))?;
        let inventories:i64=self.db.query_row("SELECT count(*) FROM catalog_revisions r JOIN catalog_items i ON i.id=r.item WHERE (i.native_kind IN('entity','historical','component','tool') OR (i.native_kind='runtime' AND json_type(r.body,'$.file_inventory')='object')) AND NOT EXISTS(SELECT 1 FROM catalog_manifests m WHERE m.revision=r.id)",[],|r|r.get(0))?;
        match read(self) {
            Ok(db) => {
                let (generation, through): (String, i64) = db.query_row(
                    "SELECT generation,checkpoint FROM meta WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                Ok(
                    json!({"status":if through<head||pending>0||dirty>0||inventories>0{"index_behind"}else{"ready"},"generation":generation,"catalog_head":head.to_string(),"indexed_through":through.to_string(),"backfill_scopes_pending":pending,"adoptions_pending":dirty,"inventories_pending":inventories,"capabilities":{"items":true,"files":true,"logs":true},"observed_at":now()}),
                )
            }
            Err(_) => Ok(
                json!({"status":"index_unavailable","catalog_head":head.to_string(),"backfill_scopes_pending":pending,"adoptions_pending":dirty,"inventories_pending":inventories}),
            ),
        }
    }
    pub fn search_items(&self, actor: &Actor, args: &Value) -> Result<Value> {
        let db = read(self)?;
        let (generation, mut checkpoint): (String, i64) = db.query_row(
            "SELECT generation,checkpoint FROM meta WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let minimum = args["at_least"]
            .as_str()
            .map(|s| {
                s.parse::<i64>()
                    .map_err(|_| Error::input("Invalid consistency watermark"))
            })
            .transpose()?;
        let wait = std::time::Instant::now();
        let timeout =
            std::time::Duration::from_millis(args["timeout_ms"].as_u64().unwrap_or(0).min(1000));
        while minimum.is_some_and(|m| checkpoint < m) && wait.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
            checkpoint =
                db.query_row("SELECT checkpoint FROM meta WHERE id=1", [], |r| r.get(0))?;
        }
        let ownership_head = if args["include_system"] == false {
            hash(encode(&json!(crate::store::rows(
                &self.db,
                "SELECT kind,id,owner FROM item_owners ORDER BY kind,id",
                &[]
            )?))?)
        } else {
            String::new()
        };
        let lifecycle_head = if args["include_archived"] == false
            || args["lifecycle_state"].is_string()
        {
            hash(encode(&json!(crate::store::rows(&self.db,"SELECT kind,id,revision,json_extract(value,'$.state') state FROM runtime_curation ORDER BY kind,id,revision",&[])?))?)
        } else {
            String::new()
        };
        let task_head = if args["scenario_ref"].is_object() || args["task_id"].is_string() {
            self.catalog_head()?
        } else {
            0
        };
        let mut query = args.clone();
        query.as_object_mut().unwrap().remove("cursor");
        let query_hash = hash(encode(&query)?);
        let (key, access) = context(self, actor)?;
        let c = args["cursor"]
            .as_str()
            .map(|s| cursor(s, &key))
            .transpose()?;
        if let Some(c) = &c {
            require(
                c["generation"] == generation
                    && c["query_version"] == 2
                    && c["ownership_head"].as_str().unwrap_or("") == ownership_head
                    && c["lifecycle_head"].as_str().unwrap_or("") == lifecycle_head
                    && c["task_head"].as_i64().unwrap_or(0) == task_head
                    && c["query"] == query_hash
                    && c["access"] == access
                    && c["expires"].as_f64().is_some_and(|t| t > now()),
                "cursor_expired",
                "Search snapshot or access changed",
                409,
            )?;
        }
        let current_version: i64 =
            db.query_row("SELECT version FROM meta WHERE id=1", [], |r| r.get(0))?;
        let watermark = c
            .as_ref()
            .and_then(|c| c["watermark"].as_i64())
            .unwrap_or(current_version);
        let min_version: i64 = db.query_row(
            "SELECT min_version FROM projection_retention WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        require(
            watermark >= min_version,
            "cursor_expired",
            "Search history expired",
            409,
        )?;
        let text = args["text"].as_str().unwrap_or("").trim();
        require(
            text.len() <= 1024 && text.split_whitespace().count() <= 16,
            "query_too_broad",
            "Search text exceeds limits",
            400,
        )?;
        let pattern = format!(
            "\"{}\"{}",
            text.replace('"', "\"\""),
            if text.chars().count() < 3 { "*" } else { "" }
        );
        let fts = if text.chars().count() >= 3 {
            "grams"
        } else {
            "terms"
        };
        let kinds = args.get("kinds").cloned().unwrap_or(json!([]));
        let parent = args["parent_item_id"]
            .as_str()
            .map(|s| items::parse(&json!(s)))
            .transpose()?;
        let view = args["view"].as_str().unwrap_or("research");
        let file_scope = kinds
            .as_array()
            .is_none_or(|a| a.is_empty() || a.iter().any(|k| k == "file"));
        let role = args["role"].as_str().unwrap_or("main");
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as i64;
        let last_time = c.as_ref().and_then(|c| c["last_time"].as_f64());
        let last_tier = c
            .as_ref()
            .and_then(|c| c["last_tier"].as_i64())
            .unwrap_or(0);
        let last_revision = c
            .as_ref()
            .and_then(|c| c["last_revision"].as_str())
            .unwrap_or("");
        let all_history = args["history"] == "all";
        let last_id = c
            .as_ref()
            .and_then(|c| c["last_id"].as_str())
            .map(|s| items::parse(&json!(s)))
            .transpose()?;
        require(
            !(args["task_id"].is_string() && args["scenario_ref"].is_object()),
            "query",
            "Choose task_id or an exact scenario_ref",
            400,
        )?;
        let scenario = args["task_id"]
            .as_str()
            .or_else(|| args["scenario_ref"]["id"].as_str())
            .unwrap_or("");
        let scenario_revision = args["scenario_ref"]["revision"].as_i64();
        let scope="CASE WHEN kind='file' THEN COALESCE((SELECT scope FROM docs owner WHERE owner.id=docs.parent AND owner.valid_from<=?1 AND (owner.valid_to IS NULL OR owner.valid_to>?1) ORDER BY owner.valid_from DESC LIMIT 1),scope) ELSE scope END";
        let role_field="CASE WHEN kind='file' THEN COALESCE((SELECT role FROM docs owner WHERE owner.id=docs.parent AND owner.valid_from<=?1 AND (owner.valid_to IS NULL OR owner.valid_to>?1) ORDER BY owner.valid_from DESC LIMIT 1),role) ELSE role END";
        let tier="CASE WHEN ?2='' THEN 0 WHEN json_extract(body,'$.item_id')=?2 OR json_extract(body,'$.native_ref.identity.id')=?2 OR json_extract(body,'$.path')=?2 OR label=?2 THEN 0 WHEN substr(label,1,length(?2))=?2 COLLATE NOCASE OR substr(json_extract(body,'$.path'),1,length(?2))=?2 THEN 1 ELSE 2 END";
        // Literal optional clauses let SQLite drive a selective FTS rowid lookup.
        // A parameterized OR(text='', MATCH...) forces a scan of the whole catalog.
        let text_clause = if text.is_empty() {
            "1".to_string()
        } else {
            format!("rowid IN(SELECT rowid FROM {fts} WHERE {fts} MATCH ?3)")
        };
        let kind_clause = if kinds.as_array().is_none_or(Vec::is_empty) {
            "1"
        } else {
            "kind IN(SELECT value FROM json_each(?4))"
        };
        let view_clause = if view == "all" {
            "1".to_string()
        } else {
            format!("{scope}=?5")
        };
        let role_clause = if role == "all" {
            "1".to_string()
        } else {
            format!("{role_field}=?6")
        };
        let entities = if app_schema(self)? >= 7 {
            "authority.all_entity_revisions"
        } else {
            "authority.entity_revisions"
        };
        let native_current=format!("(?18=1 OR ?20='historical' OR COALESCE(json_extract(body,'$.native_ref.kind'),'')!='historical' OR NOT ((json_extract(body,'$.native_ref.identity.kind')='job_view' AND (EXISTS(SELECT 1 FROM authority.jobs j WHERE j.id=json_extract(docs.body,'$.native_ref.identity.id')) OR EXISTS(SELECT 1 FROM authority.job_workflows w WHERE w.id=json_extract(docs.body,'$.native_ref.identity.id')))) OR (json_extract(body,'$.native_ref.identity.kind') IN('dataset','model','experiment','run','artifact') AND EXISTS(SELECT 1 FROM {entities} e WHERE e.id=json_extract(docs.body,'$.native_ref.identity.id')))))");
        let system_clause = if args["include_system"] == false {
            let source="json_extract(CASE WHEN kind='file' THEN COALESCE((SELECT body FROM docs owner WHERE owner.id=docs.parent AND owner.valid_from<=?1 AND (owner.valid_to IS NULL OR owner.valid_to>?1) ORDER BY owner.valid_from DESC LIMIT 1),body) ELSE body END,'$.native_ref')";
            format!(
                "NOT COALESCE(({}),0)",
                crate::system_ownership::native_sql(source, "authority.")
            )
        } else {
            "1".to_string()
        };
        let lifecycle_clause = crate::registry_lifecycle::listing_clause(
            args,
            &crate::registry_lifecycle::native_state_sql(
                "json_extract(CASE WHEN kind='file' THEN COALESCE((SELECT body FROM docs owner WHERE owner.id=docs.parent AND owner.valid_from<=?1 AND (owner.valid_to IS NULL OR owner.valid_to>?1) ORDER BY owner.valid_from DESC LIMIT 1),body) ELSE body END,'$.native_ref')",
                "authority.",
            ),
        );
        let sql=format!("SELECT id,body,recorded,{scope} AS effective_scope,{role_field} AS effective_role,{tier} AS match_tier,COALESCE(hex(revision),'') AS revision_key FROM docs WHERE (?8!='' OR {native_current}) AND valid_from<=?1 AND ((?18=0 AND (valid_to IS NULL OR valid_to>?1)) OR (?18=1 AND NOT EXISTS(SELECT 1 FROM docs newer WHERE newer.id=docs.id AND newer.revision=docs.revision AND newer.valid_from>docs.valid_from AND newer.valid_from<=?1))) AND ({system_clause}) AND ({lifecycle_clause}) AND ({text_clause}) AND ({kind_clause}) AND ({view_clause}) AND ({role_clause}) AND (?5='all' OR COALESCE(json_extract(body,'$.archived'),0)=0) AND (?7 IS NULL OR parent=?7) AND (?8='' OR (json_extract(body,'$.scenario_ref.id')=?8 AND (?9 IS NULL OR json_extract(body,'$.scenario_ref.revision')=?9)) OR EXISTS(SELECT 1 FROM authority.scenario_links sl WHERE sl.scenario_id=?8 AND (?9 IS NULL OR sl.revision=?9) AND sl.superseded_by IS NULL AND (CASE WHEN sl.target_kind='protocol_artifact' THEN 'entity' ELSE sl.target_kind END)=json_extract(docs.body,'$.native_ref.kind') AND NOT EXISTS(SELECT 1 FROM json_each(sl.target) field WHERE json_extract(docs.body,'$.native_ref.identity.'||field.key) IS NOT field.value))) AND (?13='' OR json_extract(body,'$.owner_revision_id')=?13 OR json_extract(body,'$.revision_id')=?13) AND (kind!='file' OR ?18=1 OR ?13!='' OR EXISTS(SELECT 1 FROM docs owner WHERE owner.id=docs.parent AND owner.valid_from<=?1 AND (owner.valid_to IS NULL OR owner.valid_to>?1) AND json_extract(owner.body,'$.revision_id')=json_extract(docs.body,'$.owner_revision_id'))) AND (?14='' OR json_extract(body,'$.category')=?14) AND ((?15 IS NULL AND ?16 IS NULL) OR json_type(body,'$.recorded_at') IN('integer','real')) AND (?15 IS NULL OR recorded>=?15) AND (?16 IS NULL OR recorded<=?16) AND (?17='' OR json_extract(body,'$.availability')=?17) AND (?8!='' OR ?18=1 OR COALESCE(json_extract(body,'$.native_ref.kind'),'')!='historical' OR NOT EXISTS(SELECT 1 FROM docs h WHERE json_extract(h.body,'$.native_ref.kind')='historical' AND json_extract(h.body,'$.native_ref.identity.source')=json_extract(docs.body,'$.native_ref.identity.source') AND json_extract(h.body,'$.native_ref.identity.kind')=json_extract(docs.body,'$.native_ref.identity.kind') AND json_extract(h.body,'$.native_ref.identity.id')=json_extract(docs.body,'$.native_ref.identity.id') AND json_extract(h.body,'$.source_order')>json_extract(docs.body,'$.source_order') AND h.valid_from<=?1 AND (h.valid_to IS NULL OR h.valid_to>?1))) AND (?19='' OR json_extract(body,'$.origin_type')=?19) AND (?20='' OR json_extract(body,'$.native_ref.kind')=?20) AND (?23='' OR json_extract(body,'$.dependency_ref')=?23) AND (?24 IS NULL OR id IN (SELECT e.source FROM relation_docs e JOIN authority.catalog_aliases a ON a.kind=e.target_kind AND a.identity=e.target_identity WHERE a.item=?24 AND e.valid_from<=?1 AND (?18=1 OR e.valid_to IS NULL OR e.valid_to>?1) AND (?25='' OR e.relation=?25)) OR id IN(SELECT a.item FROM relation_docs e JOIN authority.catalog_aliases a ON a.kind=e.target_kind AND a.identity=e.target_identity WHERE e.source=?24 AND e.valid_from<=?1 AND (?18=1 OR e.valid_to IS NULL OR e.valid_to>?1) AND (?25='' OR e.relation=?25))) AND (?10 IS NULL OR match_tier>?21 OR (match_tier=?21 AND (recorded<?10 OR (recorded=?10 AND (id>?11 OR (id=?11 AND revision_key>?22)))))) ORDER BY match_tier,recorded DESC,id,revision_key LIMIT ?12");
        let mut q = db.prepare(&sql)?;
        let rows = q
            .query_map(
                params![
                    watermark,
                    text,
                    pattern,
                    encode(&kinds)?,
                    view,
                    role,
                    parent,
                    scenario,
                    scenario_revision,
                    last_time,
                    last_id,
                    if file_scope { 256 } else { limit + 1 },
                    args["revision_id"].as_str().unwrap_or(""),
                    args["category"].as_str().unwrap_or(""),
                    args["recorded_from"].as_f64(),
                    args["recorded_to"].as_f64(),
                    args["availability"].as_str().unwrap_or(""),
                    all_history,
                    args["origin_type"].as_str().unwrap_or(""),
                    args["native_kind"].as_str().unwrap_or(""),
                    last_tier,
                    last_revision,
                    args.get("dependency_ref")
                        .map(encode)
                        .transpose()?
                        .unwrap_or_default(),
                    args["related_to"]["item_id"]
                        .as_str()
                        .map(|v| items::parse(&json!(v)))
                        .transpose()?,
                    args["related_to"]["relation"].as_str().unwrap_or("")
                ],
                |r| {
                    Ok((
                        r.get::<_, Vec<u8>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, f64>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| Error::input("Unsupported search expression"))?;
        let mut more = if file_scope {
            rows.len() == 256
        } else {
            rows.len() > limit as usize
        };
        let mut values = vec![];
        let mut bytes = 1024;
        let mut last = None;

        for (id, body, time, scope, role, tier, revision_key) in rows {
            let mut value: Value = serde_json::from_str(&body)?;
            value["scope"] = json!(scope);
            value["role"] = json!(role);
            let allowed:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM catalog_items WHERE id=? AND deleted=0) AND (? IS NULL OR EXISTS(SELECT 1 FROM catalog_manifests WHERE revision=?))",params![if value["kind"]=="file"{items::parse(&value["parent_item_id"])?}else{id.clone()},value["owner_revision_id"].as_str(),value["owner_revision_id"].as_str().map(|s|items::parse(&json!(s))).transpose()?.unwrap_or_default()],|r|r.get(0))?;
            require(
                allowed,
                "cursor_expired",
                "Source availability changed; retry search",
                409,
            )?;
            if value["kind"] == "file" {
                // Names/digests are shared recorded metadata. Content matches and
                // snippets additionally require a currently readable registered copy.
                let metadata = [
                    value["item_id"].as_str().unwrap_or(""),
                    value["label"].as_str().unwrap_or(""),
                    value["path"].as_str().unwrap_or(""),
                ]
                .join(" ");
                let metadata_match = text.is_empty()
                    || crate::auth::casefold(&metadata).contains(&crate::auth::casefold(text));
                if !metadata_match
                    && self
                        .canonical_file(&value)
                        .and_then(|entry| {
                            let readable =
                                crate::inventory::open_copies(self, &entry).any(|candidate| {
                                    candidate
                                        .and_then(|(file, _)| {
                                            file_receipt(
                                                self,
                                                &entry["item_id"],
                                                &crate::inventory::file_stamp(&file.metadata()?),
                                            )
                                        })
                                        .unwrap_or(false)
                                });
                            require(
                                readable,
                                "content_changed",
                                "Content search requires a current verified file receipt",
                                409,
                            )
                        })
                        .is_err()
                {
                    // Continue from the last inspected candidate even when this
                    // bounded page contains only unavailable content matches.
                    last = Some((time, items::decimal(&id)?, tier, revision_key));
                    continue;
                }
                value.as_object_mut().unwrap().remove("summary");
                value.as_object_mut().unwrap().remove("extraction");
            }
            if values.len() == limit as usize {
                more = true;
                break;
            }
            if bytes + body.len() > 900 * 1024 {
                more = true;
                break;
            }
            bytes += body.len();
            last = Some((time, items::decimal(&id)?, tier, revision_key));
            let mut compact = value;
            for field in ["inventory", "copies", "dependencies", "source_sha256"] {
                compact.as_object_mut().unwrap().remove(field);
            }
            compact["match_basis"] = json!(if text.is_empty() { "filter" } else { "lexical" });
            values.push(compact);
        }
        let next = if more {
            last.map(|(time,id,tier,revision)|sign(&json!({"query_version":2,"task_head":task_head,"ownership_head":ownership_head,"lifecycle_head":lifecycle_head,"last_tier":tier,"last_revision":revision,"generation":generation,"watermark":watermark,"query":query_hash,"access":access,"expires":c.as_ref().and_then(|c|c["expires"].as_f64()).unwrap_or(now()+300.),"last_time":time,"last_id":id}),&key)).transpose()?
        } else {
            None
        };
        Ok(
            json!({"items":values,"next_cursor":next,"partial":self.search_status()?["status"]!="ready"||minimum.is_some_and(|m|checkpoint<m)||(file_scope&&!text.is_empty()),"consistency":{"generation":generation,"indexed_through":checkpoint.to_string(),"snapshot":watermark.to_string(),"at_least_satisfied":minimum.is_none_or(|m|checkpoint>=m)},"warnings":if file_scope&&!text.is_empty(){json!(["file_content_coverage_bounded"])}else{json!([])}}),
        )
    }
    pub fn indexed_document(&self, id: &Value) -> Result<Value> {
        let id = items::parse(id)?;
        let db = read(self)?;
        let text:Option<String>=db.query_row("SELECT body FROM docs WHERE id=? AND valid_to IS NULL ORDER BY valid_from DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
        serde_json::from_str(
            &text.ok_or_else(|| Error::new("unknown_item", "Indexed Item not found", 404))?,
        )
        .map_err(Error::from)
    }
}

pub fn file_receipt(app: &Application, id: &Value, stamp: &str) -> Result<bool> {
    let db = read(app)?;
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM file_receipts WHERE id=? AND stamp=?)",
        params![items::parse(id)?, stamp],
        |r| r.get(0),
    )?)
}
impl IndexWriter {
    /// Bounded derived verification/extraction. Never runs on the authority or request lane.
    pub fn observe_files(&mut self, app: &Application) -> Result<usize> {
        use sha2::Digest;
        use std::io::{Read, Seek, SeekFrom};
        let after: Option<Vec<u8>> =
            self.db
                .query_row("SELECT verification_after FROM meta WHERE id=1", [], |r| {
                    r.get(0)
                })?;
        let mut q=self.db.prepare("SELECT id,body FROM docs WHERE kind='file' AND valid_to IS NULL AND (? IS NULL OR id>?) ORDER BY id LIMIT 8")?;
        let batch = q
            .query_map(params![after, after], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(q);
        if batch.is_empty() {
            self.db
                .execute("UPDATE meta SET verification_after=NULL WHERE id=1", [])?;
            return Ok(0);
        }
        let mut count = 0;
        'files: for (id, body) in batch {
            self.db
                .execute("UPDATE meta SET verification_after=? WHERE id=1", [&id])?;
            let hit: Value = serde_json::from_str(&body)?;
            let Ok(entry) = app.canonical_file(&hit) else {
                continue;
            };
            let mut chosen = None;
            let mut budget = 64 * 1024 * 1024u64;
            for candidate in crate::inventory::open_copies(app, &entry) {
                let Ok((mut f, _)) = candidate else { continue };
                let Ok(metadata) = f.metadata() else { continue };
                let stamp = crate::inventory::file_stamp(&metadata);
                let exists: bool = self.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM file_receipts WHERE id=? AND stamp=?)",
                    params![id, stamp],
                    |r| r.get(0),
                )?;
                if exists {
                    continue 'files;
                }
                // Keep the original per-File hashing budget across all copies.
                if metadata.len() > budget {
                    continue;
                }
                budget -= metadata.len();
                let verification = (|| -> Result<String> {
                    let mut h = sha2::Sha256::new();
                    let mut buf = [0u8; 65536];
                    let mut bounded = f.by_ref().take(metadata.len() + 1);
                    loop {
                        let n = bounded.read(&mut buf)?;
                        if n == 0 {
                            break;
                        }
                        h.update(&buf[..n]);
                    }
                    Ok(format!("{:x}", h.finalize()))
                })();
                let Ok(digest) = verification else { continue };
                if entry["sha256"] != digest
                    || stamp != crate::inventory::file_stamp(&f.metadata()?)
                {
                    continue;
                }
                chosen = Some((f, metadata, stamp, digest));
                break;
            }
            let Some((mut f, metadata, stamp, digest)) = chosen else {
                continue;
            };
            let mut projected = hit.clone();
            projected["copies"] = entry["copies"].clone();
            projected["availability"] = json!("verified");
            projected["observed_at"] = json!(now());
            let ext = std::path::Path::new(entry["path"].as_str().unwrap_or(""))
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if [
                "txt", "md", "json", "jsonl", "toml", "yaml", "yml", "csv", "rs", "py", "log",
            ]
            .contains(&ext)
                && metadata.len() <= 262144
            {
                f.seek(SeekFrom::Start(0))?;
                let mut b = vec![];
                f.take(262145).read_to_end(&mut b)?;
                if let Ok(text) = String::from_utf8(b) {
                    projected["summary"] = json!(text.chars().take(8192).collect::<String>());
                    projected["extraction"] = json!("utf8-text/v1");
                }
            }
            let tx = self.db.transaction()?;
            let version: i64 =
                tx.query_row("SELECT version+1 FROM meta WHERE id=1", [], |r| r.get(0))?;
            Self::upsert(
                &tx,
                version,
                &id,
                Some(&items::parse(&hit["revision_id"])?),
                &projected,
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO file_receipts VALUES(?,?,?,?)",
                params![id, stamp, digest, now()],
            )?;
            tx.execute("UPDATE meta SET version=? WHERE id=1", [version])?;
            tx.commit()?;
            count += 1;
        }
        Ok(count)
    }
}
