//! SQLite is an internal repository; no raw SQL crosses an application interface.
use crate::{auth::Actor, error::require, Error, Result};
use rusqlite::{params, Connection};
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
    pub(crate) live_file: Option<Result<Value>>,
    pub(crate) live_logs: Option<Result<Value>>,
    pub(crate) file_sets: std::collections::BTreeMap<String, Value>,
    pub(crate) file_proofs: std::collections::BTreeMap<PathBuf, crate::storage::VerifiedFile>,
    pub(crate) catalog_cache: std::sync::Arc<std::sync::Mutex<Option<(String, Value)>>>,
}
fn compatibility_views(db: &Connection, version: i64) -> Result<()> {
    if version < 7 {
        db.execute_batch("CREATE TEMP VIEW IF NOT EXISTS all_entity_revisions AS SELECT * FROM main.entity_revisions; CREATE TEMP VIEW IF NOT EXISTS all_entity_locations AS SELECT * FROM main.entity_locations;")?;
    }
    Ok(())
}
impl Application {
    pub fn set_file_sets(&mut self, sets: std::collections::BTreeMap<String, Value>) {
        self.file_sets = sets;
    }
    pub fn open(local: &Path, bulk: &Path, components: &Path, config: Config) -> Result<Self> {
        let path = local.join("ops.sqlite3");
        if path.is_file() {
            let db =
                Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            require(
                !matches!(version, 4..=6),
                "schema_migration_required",
                "Run the explicit drained catalog migration before starting this native writer",
                409,
            )?;
        }
        Self::open_schema(local, bulk, components, config, 7)
    }
    pub fn open_schema(
        local: &Path,
        bulk: &Path,
        components: &Path,
        config: Config,
        target: i64,
    ) -> Result<Self> {
        require(
            matches!(target, 5..=7),
            "schema",
            "Unsupported target schema",
            409,
        )?;
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
            matches!(version, 0 | 4..=7) && (version <= target || version == 0),
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
        if matches!(version, 5 | 6) && target > version {
            require(!db.query_row("SELECT EXISTS(SELECT 1 FROM rust_effects WHERE state IN('pending','accepted','unknown') AND stage!='publish')",[],|r|r.get::<_,bool>(0))?,"migration_drain","Return all execution owners before the catalog schema boundary",409)?;
        }
        if version >= 6 {
            let tables:i64=db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN('catalog_meta','catalog_items','catalog_revisions','catalog_aliases','catalog_changes','catalog_dirty','catalog_backfill','catalog_sources','catalog_manifests')",[],|r|r.get(0))?;
            require(tables==9,"catalog_authority_missing","Restore authoritative Item metadata before starting; an index rebuild cannot replace it",409)?;
            let meta:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM catalog_meta WHERE id=1 AND length(origin)=16 AND length(cursor_key)=32)",[],|r|r.get(0))?;
            let broken:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM catalog_revisions r LEFT JOIN catalog_items i ON i.id=r.item WHERE i.id IS NULL) OR EXISTS(SELECT 1 FROM catalog_aliases a LEFT JOIN catalog_items i ON i.id=a.item LEFT JOIN catalog_revisions r ON r.id=a.revision WHERE i.id IS NULL OR r.id IS NULL OR r.item!=a.item) OR EXISTS(SELECT 1 FROM catalog_items i LEFT JOIN catalog_revisions r ON r.id=i.current_revision WHERE i.current_revision IS NOT NULL AND (r.id IS NULL OR r.item!=i.id)) OR EXISTS(SELECT 1 FROM catalog_manifests m LEFT JOIN catalog_revisions r ON r.id=m.revision WHERE r.id IS NULL OR r.item!=m.item) OR EXISTS(SELECT 1 FROM catalog_changes c LEFT JOIN catalog_items i ON i.id=c.item WHERE i.id IS NULL)",[],|r|r.get(0))?;
            require(meta&&!broken,"catalog_authority_missing","Authoritative Item identity metadata is incomplete; preserve evidence and restore it",409)?;
        }
        if version == 7 {
            let count:i64=db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN('reconcile_plans','asset_imports','file_bindings','log_retention','catalog_tasks','asset_revisions','asset_locations')",[],|r|r.get(0))?;
            require(
                count == 7,
                "reconciliation_authority_missing",
                "Restore reconciliation authority; never recreate lost plans or bindings silently",
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
        if target >= 7 {
            tx.execute_batch(include_str!("../../../contracts/reconcile-schema.sql"))?;
        }
        if target >= 6 {
            crate::items::initialize(&tx)?;
        }
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
        compatibility_views(&db, target)?;
        Ok(Self {
            db,
            config,
            local: local.into(),
            bulk: bulk.into(),
            components: components.into(),
            read_only: false,
            query_snapshot: false,
            live_logs: None,
            live_file: None,
            file_proofs: Default::default(),
            file_sets: Default::default(),
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
            matches!(
                db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))?,
                5..=7
            ),
            "schema",
            "Read-only serving requires a previously migrated Rust database",
            409,
        )?;
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        let version = db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))?;
        compatibility_views(&db, version)?;
        Ok(Self {
            db,
            config,
            local: local.into(),
            bulk: bulk.into(),
            components: components.into(),
            read_only: true,
            query_snapshot: false,
            live_logs: None,
            live_file: None,
            file_proofs: Default::default(),
            file_sets: Default::default(),
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
            (
                "system",
                "rebuild_search" | "catalog_task" | "prune_search" | "retry_catalog_task",
            )
            | ("jobs", "prune_logs") => self.maintenance(actor, action, &args),
            ("results", a) if a.starts_with("reconcile_") => self.reconciliation(actor, a, &args),
            ("jobs", "log_streams") => self.log_streams(&args),
            ("jobs", "read_log") => self.read_log(&args),
            ("jobs", "search_logs") => self.search_logs(actor, &args),
            ("results", "item_relations") => self.item_graph(actor, &args),
            ("system", "dashboard_summary") => self.dashboard_summary(),
            ("system", "dashboard_activity") => {
                self.dashboard_activity(args["view"].as_str().unwrap_or("research"))
            }
            ("system", "dashboard_entity") => self.dashboard_entity(&args),
            ("system", "dashboard_page") => self.dashboard_page(actor, &args),
            ("results", "search_items") => self.search_items(actor, &args),
            ("results", "item_files") => {
                let mut query = args.clone();
                let parent = self.get_item(
                    &json!({"item_id":args["item_id"],"revision_id":args["revision_id"]}),
                )?;
                if parent["native_ref"]["kind"] == "dependency" {
                    query["dependency_ref"] = parent["native_ref"]["identity"].clone();
                    query.as_object_mut().unwrap().remove("revision_id");
                } else {
                    query["parent_item_id"] = args["item_id"].clone();
                }
                if parent["native_ref"]["kind"] != "dependency" && !args["revision_id"].is_string()
                {
                    query["revision_id"] =
                        self.get_item(&json!({"item_id":args["item_id"]}))?["revision_id"].clone();
                }
                query.as_object_mut().unwrap().remove("item_id");
                query["kinds"] = json!(["file"]);
                query["view"] = json!("all");
                query["role"] = json!("all");
                self.search_items(actor, &query)
            }
            ("results", "get_file") => self.get_file(&args),
            ("results", "read_file") => self.read_file(&args),
            ("system", "search_status") => self.search_status(),
            ("results", "get_item") => self.get_item(&args),
            ("results", "resolve_item") => self.resolve_item(&args),
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
    pub fn transaction(&mut self) -> Result<UnitOfWork<'_>> {
        UnitOfWork::begin(&self.db)
    }
    pub fn externally_drained(&self) -> Result<bool> {
        Ok(!self.db.query_row("SELECT EXISTS(SELECT 1 FROM rust_effects WHERE state IN ('pending','accepted','unknown') AND stage!='publish')",[],|r|r.get::<_,bool>(0))?)
    }
    pub fn status(&self) -> Result<Value> {
        let mode: String =
            self.db
                .query_row("SELECT mode FROM node_control WHERE id=1", [], |r| r.get(0))?;
        Ok(
            json!({"service":"Loom Ops","build":{"build_id":env!("LOOM_BUILD_ID"),"image_build_id":env!("LOOM_BUILD_ID"),"source_modified":false,"source_kind":"immutable_rust_binary","git_commit":env!("LOOM_GIT_COMMIT")},"storage_id":self.config.storage_id,"inventory":self.config.extra.get("inventory").cloned().unwrap_or(serde_json::json!({})),"engine":"rust","build_id":env!("LOOM_BUILD_ID"),"image_build_id":env!("LOOM_BUILD_ID"),"git_commit":env!("LOOM_GIT_COMMIT"),"source_modified":false,"source_kind":"immutable_rust_binary","api_revision":crate::onboarding::identity()["api_revision"],"machine_id":self.config.machine_id,"admission":{"mode":mode},"source":env!("LOOM_RUST_SOURCE"),"version":env!("CARGO_PKG_VERSION"),"authoring":{"default_language":"rust","python":"external SDK integration","sdk":"loom-producer/v1"}}),
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

/// All authoritative native mutations and their Item changes share one transaction.
/// The savepoint branch allows a caller to compose commands without a second writer.
pub struct UnitOfWork<'a> {
    db: &'a Connection,
    savepoint: Option<String>,
    done: bool,
    since: i64,
    catalog: bool,
}
impl<'a> UnitOfWork<'a> {
    fn begin(db: &'a Connection) -> Result<Self> {
        let savepoint = if db.is_autocommit() {
            db.execute_batch("BEGIN IMMEDIATE")?;
            None
        } else {
            let name = format!("unit_{}", id());
            db.execute_batch(&format!("SAVEPOINT {name}"))?;
            Some(name)
        };
        // Construct the guard before any fallible query so BEGIN/SAVEPOINT is
        // rolled back even if catalog metadata cannot be read.
        let mut unit = Self {
            db,
            savepoint,
            done: false,
            since: 0,
            catalog: false,
        };
        unit.catalog = crate::items::installed(db)?;
        if unit.catalog {
            unit.since =
                db.query_row("SELECT COALESCE(max(seq),0) FROM catalog_dirty", [], |r| {
                    r.get(0)
                })?;
        }
        Ok(unit)
    }
    pub fn commit(mut self) -> Result<()> {
        if self.catalog {
            crate::items::flush(self.db, self.since, 4096)?;
        }
        self.db.execute_batch(
            &self
                .savepoint
                .as_ref()
                .map(|s| format!("RELEASE {s}"))
                .unwrap_or("COMMIT".into()),
        )?;
        self.done = true;
        Ok(())
    }
}
impl std::ops::Deref for UnitOfWork<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.db
    }
}
impl Drop for UnitOfWork<'_> {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.db.execute_batch(
                &self
                    .savepoint
                    .as_ref()
                    .map(|s| format!("ROLLBACK TO {s}; RELEASE {s}"))
                    .unwrap_or("ROLLBACK".into()),
            );
        }
    }
}
