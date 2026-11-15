//! Bounded resumable copies of granted Executor streams. Independent of Job return/publication.
use crate::Service;
use loom_core::{execution::Work, log_index, storage, store::Config, Application, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Duration,
};
pub fn start(service: Service, config: Config, local: PathBuf, bulk: PathBuf, components: PathBuf) {
    std::thread::spawn(move || {
        let mut after = String::new();
        while !service.stopping.load(Ordering::Acquire) {
            let result = (|| -> Result<()> {
                let Some(executor) = config.extra.get("executor") else {
                    return Ok(());
                };
                let app = Application::open_read_only(&local, &bulk, &components, config.clone())?;
                let batch = app.log_collection_candidates(&after)?;
                if batch.is_empty() {
                    after.clear();
                }
                for row in batch {
                    after = row["id"].as_str().unwrap_or("").to_string();
                    if service.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let work: Work = serde_json::from_str(row["request"].as_str().unwrap_or("{}"))?;
                    let evidence: Value =
                        serde_json::from_str(row["evidence"].as_str().unwrap_or("{}"))?;
                    let Some(ticket) = evidence["ticket"].as_str() else {
                        continue;
                    };
                    let state = log_index::read_state(&app, &work)?;
                    if state["closed"] == true {
                        continue;
                    }
                    let path = log_index::path(&app.local, &work);
                    loom_core::index_store::secure_dir(path.parent().unwrap())?;
                    let offset = state["offset"].as_u64().unwrap_or(0);
                    let total_budget = config
                        .extra
                        .get("catalog")
                        .and_then(|c| c["log_total_max_bytes"].as_u64())
                        .unwrap_or(2 * 1024 * 1024 * 1024);
                    let used = fs::read_dir(path.parent().unwrap())?.try_fold(
                        0u64,
                        |sum, e| -> Result<u64> {
                            let p = e?.path();
                            Ok(
                                sum.saturating_add(if p.extension().is_some_and(|s| s == "log") {
                                    p.symlink_metadata()?.len()
                                } else {
                                    0
                                }),
                            )
                        },
                    )?;
                    if used >= total_budget {
                        let mut stopped = state.clone();
                        stopped["offset"] = json!(offset);
                        stopped["closed"] = json!(true);
                        stopped["gap"] = json!(true);
                        stopped["retention_reason"] = json!("global_capture_budget_exhausted");
                        storage::atomic(
                            &log_index::state_path(&app.local, &work),
                            &serde_json::to_vec(&stopped)?,
                            0o600,
                        )?;
                        continue;
                    }
                    let budget = config
                        .extra
                        .get("catalog")
                        .and_then(|c| c["log_stream_max_bytes"].as_u64())
                        .unwrap_or(256 * 1024 * 1024);
                    if offset >= budget {
                        let mut stopped = state.clone();
                        stopped["closed"] = json!(true);
                        stopped["gap"] = json!(true);
                        stopped["retention_reason"] = json!("capture_budget_exhausted");
                        storage::atomic(
                            &log_index::state_path(&app.local, &work),
                            &serde_json::to_vec(&stopped)?,
                            0o600,
                        )?;
                        continue;
                    }
                    let result = loom_host::wire::call(
                        Path::new(executor["socket"].as_str().unwrap_or("")),
                        Path::new(executor["credential"].as_str().unwrap_or("")),
                        "log_range",
                        &json!({"ticket":ticket,"offset":offset,"limit":(budget-offset).min(total_budget-used).min(262144)}),
                    );
                    let Ok(value) = result else { continue };
                    let Some(generation) = value["source_generation"].as_str() else {
                        continue;
                    };
                    // A restarted host collector creates a new source generation. Retain
                    // the old observed prefix and never append replayed bytes to its offset.
                    if offset > 0
                        && state["generation"]
                            .as_str()
                            .is_some_and(|g| g != generation)
                    {
                        let previous = state["generation"].as_str().unwrap();
                        let generation_id = uuid::Uuid::parse_str(previous).map_err(|_| {
                            loom_core::Error::input("Invalid retained log generation")
                        })?;
                        let archive = path.with_extension(format!("{generation_id}.log"));
                        if path.exists() && !archive.exists() {
                            fs::rename(&path, &archive)?;
                        }
                        let mut old = state.clone();
                        old["closed"] = json!(true);
                        old["gap"] = json!(true);
                        storage::atomic(
                            &archive.with_extension("json"),
                            &serde_json::to_vec(&old)?,
                            0o600,
                        )?;
                        storage::atomic(
                            &log_index::state_path(&app.local, &work),
                            &serde_json::to_vec(
                                &json!({"generation":generation,"offset":0,"closed":false,"gap":true}),
                            )?,
                            0o600,
                        )?;
                        continue;
                    }
                    use base64::Engine;
                    use std::os::unix::fs::OpenOptionsExt;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(value["data_base64"].as_str().unwrap_or(""))
                        .map_err(|_| loom_core::Error::input("Invalid log bytes"))?;
                    loom_core::error::require(
                        value["offset"].as_u64() == Some(offset)
                            && value["next_offset"].as_u64() == Some(offset + bytes.len() as u64),
                        "logs",
                        "Log offset changed",
                        409,
                    )?;
                    let mut f = fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW)
                        .open(&path)?;
                    // Recover a crash between append fsync and pointer fsync by truncating to the committed offset.
                    loom_core::error::require(
                        f.metadata()?.len() >= offset,
                        "retention_gap",
                        "Local log bytes missing",
                        409,
                    )?;
                    f.set_len(offset)?;
                    f.seek(SeekFrom::Start(offset))?;
                    f.write_all(&bytes)?;
                    f.sync_all()?;
                    let closed = value["closed"] == true && value["eof"] == true;
                    storage::atomic(
                        &log_index::state_path(&app.local, &work),
                        &serde_json::to_vec(
                            &json!({"generation":generation,"offset":offset+bytes.len() as u64,"closed":closed,"gap":value["gap"]==true}),
                        )?,
                        0o600,
                    )?;
                    if closed && work.stage == "run" {
                        let mut f = fs::File::open(&path)?;
                        let mut bytes = vec![];
                        if f.metadata()?.len() <= 16 * 1024 * 1024 {
                            f.read_to_end(&mut bytes)?;
                            storage::atomic(&work.folder.join("worker.log"), &bytes, 0o600)?;
                        }
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("Loom log collector: {}", error.code)
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}
