//! One bounded background step per task. A rebuild never stops the active generation.
use loom_core::{index_store, log_index, storage, store::Config, Application, Error, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};

pub fn step(app: &Application, task: &Value) -> Result<Value> {
    let id = task["id"]
        .as_str()
        .ok_or_else(|| Error::input("Task identity missing"))?;
    let request: Value = serde_json::from_str(task["request"].as_str().unwrap_or("{}"))?;
    let progress: Value = serde_json::from_str(task["progress"].as_str().unwrap_or("{}"))?;
    if task["kind"] == "rebuild" {
        let batches = progress["batches"].as_u64().unwrap_or(0) + 1;
        loom_core::error::require(
            batches <= request["max_batches"].as_u64().unwrap_or(100000),
            "maintenance_budget",
            "Rebuild batch budget exhausted; active search remains available",
            409,
        )?;
        let mut config: Config = app.config.clone();
        let root = index_store::directory(&app.local, &config);
        let shadow = root.join("shadows").join(id);
        index_store::secure_dir(&shadow)?;
        config.extra.entry("catalog".into()).or_insert(json!({}))["index_directory"] =
            json!(shadow);
        let source = Application::open_read_only(&app.local, &app.bulk, &app.components, config)?;
        let mut index = index_store::IndexWriter::open(&source)?;
        index.sync(&source, 128)?;
        log_index::project(&mut index, &source)?;
        let head: i64 = source.catalog_head()?;
        let checkpoint: i64 =
            index
                .db
                .query_row("SELECT checkpoint FROM meta WHERE id=1", [], |r| r.get(0))?;
        Ok(
            json!({"batches":batches,"through":checkpoint.to_string(),"head":head.to_string(),"complete":checkpoint==head,"logs":"incremental catch-up; ranges remain readable"}),
        )
    } else if task["kind"] == "cleanup" {
        let root = index_store::directory(&app.local, &app.config);
        let active = index_store::active_directory(app)?;
        let folder = root.join("shadows");
        let mut removed = vec![];
        let maximum = request["max_generations"].as_u64().unwrap_or(4).min(8) as usize;
        if folder.exists() {
            for entry in fs::read_dir(&folder)?.take(10000) {
                let entry = entry?;
                let path = entry.path();
                let metadata = path.symlink_metadata()?;
                if path == active || !metadata.is_dir() || metadata.file_type().is_symlink() {
                    continue;
                }
                if !entry
                    .file_name()
                    .to_str()
                    .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
                {
                    continue;
                }
                if metadata
                    .modified()?
                    .elapsed()
                    .map(|d| d.as_secs() < 600)
                    .unwrap_or(true)
                {
                    continue;
                }
                let db = rusqlite::Connection::open_with_flags(
                    path.join("catalog.sqlite3"),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )?;
                let origin: Vec<u8> =
                    db.query_row("SELECT origin FROM meta WHERE id=1", [], |r| r.get(0))?;
                let expected = index_store::IndexWriter::open(app)?.db.query_row(
                    "SELECT origin FROM meta WHERE id=1",
                    [],
                    |r| r.get::<_, Vec<u8>>(0),
                )?;
                loom_core::error::require(
                    origin == expected,
                    "index_origin",
                    "Retired generation belongs to another authority",
                    409,
                )?;
                drop(db);
                fs::remove_dir_all(&path)?;
                removed.push(entry.file_name().to_string_lossy().to_string());
                if removed.len() >= maximum {
                    break;
                }
            }
        }
        Ok(
            json!({"complete":true,"removed_generations":removed,"minimum_age_seconds":600,"active_generation_preserved":true}),
        )
    } else if task["kind"] == "retention" {
        let after = progress["after"].as_u64().unwrap_or(0) as usize;
        let streams = request["streams"]
            .as_array()
            .ok_or_else(|| Error::input("Stream selections missing"))?;
        for v in streams.iter().skip(after).take(8) {
            let (_, work) = log_index::source(app, &v["stream_id"])?;
            let state = log_index::read_state(app, &work)?;
            let evidence = app.log_source_evidence(&work.effect)?;
            if let Some(ticket) = evidence["ticket"].as_str() {
                let provider = app.config.extra.get("executor").unwrap_or(&Value::Null);
                if let (Some(socket), Some(credential)) =
                    (provider["socket"].as_str(), provider["credential"].as_str())
                {
                    loom_host::wire::call(
                        Path::new(socket),
                        Path::new(credential),
                        "log_expire",
                        &json!({"ticket":ticket,"source_generation":v["source_generation"]}),
                    )?;
                }
            }
            let current = state["generation"] == v["source_generation"];
            let path = if current {
                log_index::path(&app.local, &work)
            } else {
                let generation =
                    uuid::Uuid::parse_str(v["source_generation"].as_str().unwrap_or(""))
                        .map_err(|_| Error::input("Invalid log generation"))?;
                log_index::path(&app.local, &work).with_extension(format!("{generation}.log"))
            };
            let header = json!({"generation":v["source_generation"],"closed":true,"available":false,"gap":true,"offset":0,"retention_reason":request["reason"],"expired":true});
            storage::atomic(
                &path.with_extension("json"),
                &serde_json::to_vec(&header)?,
                0o600,
            )?;
            if path.exists() {
                loom_core::error::require(
                    !path.symlink_metadata()?.file_type().is_symlink(),
                    "retention",
                    "Linked spool file refused",
                    409,
                )?;
                fs::remove_file(&path)?;
            }
            if current && work.stage == "run" {
                let old = work.folder.join("worker.log");
                if old.is_file() && !old.symlink_metadata()?.file_type().is_symlink() {
                    fs::remove_file(old)?;
                }
            }
        }
        let through = (after + 8).min(streams.len());
        Ok(
            json!({"after":through,"complete":through==streams.len(),"raw_retention":"expired","selected_generations":streams.len()}),
        )
    } else {
        Err(Error::input("Unknown maintenance task kind"))
    }
}

// Budget accounting is metadata-only and limited to this private index tree.
pub fn check_index_budget(app: &Application) -> Result<()> {
    let root = index_store::directory(&app.local, &app.config);
    if root.exists() {
        storage::reserve_space(
            &root,
            16 * 1024 * 1024,
            app.config
                .extra
                .get("catalog")
                .and_then(|c| c["minimum_free_bytes"].as_u64())
                .unwrap_or(64 * 1024 * 1024),
        )?;
    }
    let max = app
        .config
        .extra
        .get("catalog")
        .and_then(|c| c["index_max_bytes"].as_u64())
        .unwrap_or(4 * 1024 * 1024 * 1024);
    fn size(path: &Path, left: &mut usize) -> Result<u64> {
        if !path.exists() {
            return Ok(0);
        }
        let mut total = 0u64;
        for e in fs::read_dir(path)? {
            let e = e?;
            *left = left.checked_sub(1).ok_or_else(|| {
                Error::new(
                    "index_budget",
                    "Private metadata entry budget exhausted",
                    409,
                )
            })?;
            let m = e.path().symlink_metadata()?;
            if m.file_type().is_symlink() {
                continue;
            }
            total = total.saturating_add(if m.is_dir() {
                size(&e.path(), left)?
            } else {
                m.len()
            });
        }
        Ok(total)
    }
    loom_core::error::require(size(&root,&mut 100000)?<max,"index_budget","Search disk budget exhausted; expire retired generations or increase the configured budget",409)?;
    Ok(())
}
