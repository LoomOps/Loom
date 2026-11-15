//! SQLite is an internal repository; no raw SQL crosses an application interface.
use crate::{auth::Actor, error::require, Error, Result};
use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub machine_id: String,
    pub storage_id: String,
    #[serde(default)]
    pub actors: Vec<Actor>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Value>,
    #[serde(default)]
    pub data_stores: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Config {
    pub fn read(path: &Path) -> Result<Self> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
}
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub fn id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
pub fn encode(v: &Value) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}
pub struct Application {
    pub(crate) db: Connection,
    pub config: Config,
    pub local: PathBuf,
    pub bulk: PathBuf,
    pub components: PathBuf,
    pub read_only: bool,
    pub(crate) query_snapshot: bool,
    pub(crate) live_logs: Option<Result<Value>>,
    pub(crate) file_proofs: std::collections::BTreeMap<PathBuf, crate::storage::VerifiedFile>,
    pub(crate) catalog_cache: std::sync::Arc<std::sync::Mutex<Option<(String, Value)>>>,
}
impl Application {
    pub fn open(local: &Path, bulk: &Path, components: &Path, config: Config) -> Result<Self> {
        if !local.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(local)?;
        }
        let path = local.join("ops.sqlite3");
        let _ = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)?;
        let db = Connection::open(&path)?;
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        require(
            matches!(version, 0 | 4 | 5),
            "schema",
            "Unsupported Ops schema",
            409,
        )?;
        if version == 0 {
            let existing:i64=db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0))?;
            require(
                existing == 0,
                "schema",
                "Unversioned existing database requires explicit import review",
                409,
            )?;
        }
        if version == 4 {
            let running:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE state IN ('preparing','running','processing'))",[],|r|r.get(0))?;
            require(
                !running,
                "migration_drain",
                "Legacy execution must drain before Rust authority migration",
                409,
            )?;
        }
        let tx = db.unchecked_transaction()?;
        tx.execute_batch(include_str!("../../../contracts/legacy-schema.sql"))?;
        tx.execute_batch(include_str!("../../../contracts/redesign-schema.sql"))?;
        tx.execute(
            "INSERT OR IGNORE INTO node_control(id,mode) VALUES(1,'active')",
            [],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO dispatch_control VALUES(1,0,NULL,NULL,NULL)",
            [],
        )?;
        tx.execute("INSERT OR IGNORE INTO rust_authority VALUES(1,1,1)", [])?;
        tx.execute("INSERT OR IGNORE INTO rust_job_fences SELECT j.id,COALESCE((SELECT max(revision) FROM job_revisions WHERE job=j.id),0),(SELECT epoch FROM rust_authority WHERE id=1) FROM jobs j",[])?;
        tx.pragma_update(None, "user_version", 5)?;
        tx.commit()?;
        Ok(Self {
            db,
            config,
            local: local.into(),
            bulk: bulk.into(),
            components: components.into(),
            read_only: false,
            query_snapshot: false,
            live_logs: None,
            file_proofs: Default::default(),
            catalog_cache: Default::default(),
        })
    }
    pub fn open_read_only(
        local: &Path,
        bulk: &Path,
        components: &Path,
        config: Config,
    ) -> Result<Self> {
        let db = Connection::open_with_flags(
            local.join("ops.sqlite3"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        require(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? == 5,
            "schema",
            "Read-only serving requires a previously migrated Rust database",
            409,
        )?;
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        Ok(Self {
            db,
            config,
            local: local.into(),
            bulk: bulk.into(),
            components: components.into(),
            read_only: true,
            query_snapshot: false,
            live_logs: None,
            file_proofs: Default::default(),
            catalog_cache: Default::default(),
        })
    }
    pub fn execute(
        &mut self,
        actor: &Actor,
        group: &str,
        action: &str,
        args: Value,
    ) -> Result<Value> {
        let contract = crate::contracts::action(group, action)?;
        require(
            !self.query_snapshot || contract.scope == "read" && action != "check_updates",
            "query_snapshot",
            "Read snapshots cannot apply commands",
            409,
        )?;
        actor.require(&contract.scope)?;
        require(
            !self.read_only || contract.scope == "read",
            "read_only",
            "This instance is read-only",
            409,
        )?;
        crate::contracts::validate(&contract.schema, &args)?;
        // Token authorization and operation are evaluated on the same serialized lane.
        crate::auth::validate_current(&self.db, actor)?;
        let mut value = match (group, action) {
            ("results", "register_scenario") => {
                self.create_scenario(actor, crate::contracts::decode(args)?)
            }
            ("results", "update_scenario") => {
                self.update_scenario(actor, crate::contracts::decode(args)?)
            }
            ("results", "archive_scenario") => self.archive_scenario(actor, args),
            ("results", "link_scenario") => {
                self.link_scenario(actor, crate::contracts::decode(args)?)
            }
            ("results", "supersede_scenario_link") => self.supersede_scenario_link(actor, args),
            ("results", "scenarios" | "get_scenario" | "scenario_graph") => {
                self.scenario_query(action, args)
            }
            ("components", "scaffold") => crate::software::scaffold(&args),
            ("components", "validate_package") => crate::software::validate_package(&args),
            ("components", _) => self.software_action(actor, action, args),
            ("jobs", _) => self.job_action(actor, action, args),
            ("recovery", _) => self.recover(actor, action, args),
            ("events", _) => self.event_action(actor, action, args),
            ("results", _) => self.result_action(actor, action, args),
            ("system", _) => self.system_action(actor, action, args),
            _ => Err(Error::input("Unknown operation")),
        }?;
        if group == "jobs" && action == "queue" {
            crate::query::queue_permissions(
                &mut value,
                !self.read_only && actor.scopes.iter().any(|s| s == "submit"),
            );
        }
        Ok(value)
    }
    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        Ok(self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }
    pub fn externally_drained(&self) -> Result<bool> {
        Ok(!self.db.query_row("SELECT EXISTS(SELECT 1 FROM rust_effects WHERE state IN ('pending','accepted','unknown') AND stage!='publish')",[],|r|r.get::<_,bool>(0))?)
    }
    pub fn status(&self) -> Result<Value> {
        let mode: String =
            self.db
                .query_row("SELECT mode FROM node_control WHERE id=1", [], |r| r.get(0))?;
        Ok(
            json!({"service":"Loom Ops","build":{"build_id":env!("LOOM_BUILD_ID"),"image_build_id":env!("LOOM_BUILD_ID"),"source_modified":false,"source_kind":"immutable_rust_binary","git_commit":env!("LOOM_GIT_COMMIT")},"storage_id":self.config.storage_id,"inventory":self.config.extra.get("inventory").cloned().unwrap_or(serde_json::json!({})),"engine":"rust","build_id":env!("LOOM_BUILD_ID"),"image_build_id":env!("LOOM_BUILD_ID"),"git_commit":env!("LOOM_GIT_COMMIT"),"source_modified":false,"source_kind":"immutable_rust_binary","api_revision":"loom-ops/v47","machine_id":self.config.machine_id,"admission":{"mode":mode},"source":env!("LOOM_RUST_SOURCE"),"version":env!("CARGO_PKG_VERSION"),"authoring":{"default_language":"rust","python":"external SDK integration","sdk":"loom-producer/v1"}}),
        )
    }
}
pub fn active(db: &Connection) -> Result<()> {
    let mode: String =
        db.query_row("SELECT mode FROM node_control WHERE id=1", [], |r| r.get(0))?;
    require(
        mode == "active",
        "admission_fenced",
        "Job admission fenced",
        409,
    )
}
pub fn return_allowed(db: &Connection) -> Result<()> {
    let mode: String =
        db.query_row("SELECT mode FROM node_control WHERE id=1", [], |r| r.get(0))?;
    require(
        matches!(mode.as_str(), "active" | "draining"),
        "admission_fenced",
        "Execution return authority fenced",
        409,
    )
}
pub fn replay(
    db: &Connection,
    actor: &Actor,
    key: &str,
    operation: &str,
    args: &Value,
) -> Result<Option<Value>> {
    require(
        !key.is_empty() && key.len() <= 2048,
        "key",
        "Invalid operation key",
        400,
    )?;
    let mut s = db.prepare("SELECT request,result FROM rust_receipts WHERE actor=? AND key=?")?;
    let mut rows = s.query(params![actor.id, key])?;
    if let Some(r) = rows.next()? {
        let request: String = r.get(0)?;
        require(
            request == encode(&json!([operation, args]))?,
            "idempotency_conflict",
            "Key reused for different input",
            409,
        )?;
        return Ok(Some(serde_json::from_str(&r.get::<_, String>(1)?)?));
    }
    Ok(None)
}
pub fn receipt(
    db: &Connection,
    actor: &Actor,
    key: &str,
    operation: &str,
    args: &Value,
    result: &Value,
) -> Result<()> {
    db.execute(
        "INSERT INTO rust_receipts VALUES(?,?,?,?,?,?)",
        params![
            actor.id,
            key,
            operation,
            encode(&json!([operation, args]))?,
            encode(result)?,
            now()
        ],
    )?;
    Ok(())
}
pub fn json_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut result = serde_json::Map::new();
    for i in 0..row.as_ref().column_count() {
        let key = row.as_ref().column_name(i)?;
        let v = match row.get_ref(i)? {
            rusqlite::types::ValueRef::Null => Value::Null,
            rusqlite::types::ValueRef::Integer(n) => json!(n),
            rusqlite::types::ValueRef::Real(n) => json!(n),
            rusqlite::types::ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into()),
            _ => Value::Null,
        };
        result.insert(key.into(), v);
    }
    Ok(Value::Object(result))
}
pub fn rows(db: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<Value>> {
    let mut stmt = db.prepare(sql)?;
    let items = stmt
        .query_map(args, json_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Cross-language lock shared with the legacy worker, migrator and source activation.
pub struct WriterLock(std::fs::File);
impl WriterLock {
    pub fn acquire(root: &Path) -> Result<Self> {
        use std::os::fd::AsRawFd;
        fs::create_dir_all(root)?;
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(root.join("worker.lock"))?;
        require(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "writer_owned",
            "Another worker or migrator owns this instance",
            409,
        )?;
        Ok(Self(file))
    }
}
impl Drop for WriterLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
