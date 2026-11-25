//! Independent bounded projection worker. The application lane only commits typed metadata.
use crate::{Message, Service};
use loom_core::{inventory, Application, Config};
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};
pub fn start(service: Service, config: Config, local: PathBuf, bulk: PathBuf, components: PathBuf) {
    std::thread::spawn(move || {
        let mut browse_retry = std::collections::BTreeMap::new();
        while !service.stopping.load(Ordering::Acquire) {
            let result = (|| -> loom_core::Result<()> {
                let (send, receive) = tokio::sync::oneshot::channel();
                if service
                    .send
                    .blocking_send(Message::CatalogTick(send))
                    .is_err()
                {
                    return Ok(());
                }
                let _ = receive.blocking_recv();
                let app = Application::open_read_only(&local, &bulk, &components, config.clone())?;
                for (item, revision) in app.pending_inventories()? {
                    if service.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let prepared = match prepare_inventory(&app, &item, &revision) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Loom inventory projection: {}", e.code);
                            continue;
                        }
                    };
                    let (send, receive) = tokio::sync::oneshot::channel();
                    if service
                        .send
                        .blocking_send(Message::Inventory(prepared, send))
                        .is_err()
                    {
                        return Ok(());
                    }
                    let _ = receive.blocking_recv();
                }
                let clock = std::time::Instant::now();
                browse_retry.retain(|_, retry| *retry > clock);
                for (revision, target) in app.pending_browse_summaries(browse_retry.keys())? {
                    match app.prepare_browse_summary(revision.clone(), target) {
                        Ok(prepared) => {
                            let (send, receive) = tokio::sync::oneshot::channel();
                            if service
                                .send
                                .blocking_send(Message::BrowseSummary(prepared, send))
                                .is_err()
                            {
                                return Ok(());
                            }
                            if !matches!(receive.blocking_recv(), Ok(Ok(()))) {
                                browse_retry.insert(revision, clock + Duration::from_secs(30));
                            }
                            break;
                        }
                        Err(error) => {
                            browse_retry.insert(revision, clock + Duration::from_secs(30));
                            eprintln!("Loom browse projection: {}", error.code);
                        }
                    }
                }
                if let Some(task) = app.pending_maintenance()? {
                    let budget = if task["kind"] == "rebuild" {
                        crate::catalog_maintenance::check_index_budget(&app)
                    } else {
                        Ok(())
                    };
                    let result = budget.and_then(|_| crate::catalog_maintenance::step(&app, &task));
                    let (progress, error) = match result {
                        Ok(p) => (p, None),
                        Err(e) => (serde_json::json!({"complete":false}), Some(e.code)),
                    };
                    let (send, receive) = tokio::sync::oneshot::channel();
                    service
                        .send
                        .blocking_send(Message::Maintenance(
                            task["id"].as_str().unwrap_or("").to_string(),
                            progress,
                            error,
                            send,
                        ))
                        .map_err(|_| loom_core::Error::conflict("Maintenance lane stopped"))?;
                    receive
                        .blocking_recv()
                        .map_err(|_| loom_core::Error::conflict("Maintenance reply lost"))??;
                }
                crate::catalog_maintenance::check_index_budget(&app)?;
                let mut index = loom_core::index_store::IndexWriter::open(&app)?;
                index.sync(&app, 128)?;
                loom_core::log_index::project(&mut index, &app)?;
                if config
                    .extra
                    .get("catalog")
                    .is_some_and(|c| c["verify_registered_files"] == true)
                {
                    index.observe_files(&app)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("Loom search projection: {}", error.code);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

struct RuntimeMembers {
    executor: serde_json::Value,
    pin: serde_json::Value,
    expected: serde_json::Value,
    cursor: serde_json::Value,
    values: std::vec::IntoIter<serde_json::Value>,
    finished: bool,
}
impl Iterator for RuntimeMembers {
    type Item = loom_core::Result<serde_json::Value>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(value) = self.values.next() {
                return Some(Ok(value));
            }
            if self.finished {
                return None;
            }
            let result = (|| -> loom_core::Result<serde_json::Value> {
                loom_host::wire::call(
                    std::path::Path::new(
                        self.executor["socket"]
                            .as_str()
                            .ok_or_else(|| loom_core::Error::input("Executor socket required"))?,
                    ),
                    std::path::Path::new(
                        self.executor["credential"].as_str().ok_or_else(|| {
                            loom_core::Error::input("Executor credential required")
                        })?,
                    ),
                    "runtime_file_page",
                    &serde_json::json!({"runtime_ref":self.pin,"page":self.cursor["page"].as_u64().unwrap_or(0),"row":self.cursor["row"].as_u64().unwrap_or(0),"limit":200}),
                )
            })();
            match result {
                Ok(page) => {
                    if page["inventory_sha256"] != self.expected {
                        self.finished = true;
                        return Some(Err(loom_core::Error::conflict(
                            "Runtime inventory generation changed",
                        )));
                    }
                    self.cursor = page["next"].clone();
                    self.finished = self.cursor.is_null();
                    self.values = page["items"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter();
                }
                Err(e) => {
                    self.finished = true;
                    return Some(Err(e));
                }
            }
        }
    }
}
fn prepare_inventory(
    app: &Application,
    item: &[u8],
    revision: &[u8],
) -> loom_core::Result<inventory::PreparedInventory> {
    let detail=app.get_item(&serde_json::json!({"item_id":loom_core::items::decimal(item)?,"revision_id":loom_core::items::decimal(revision)?}))?;
    if detail["native_ref"]["kind"] == "runtime" {
        let executor =
            app.config.extra.get("executor").cloned().ok_or_else(|| {
                loom_core::Error::input("Protected runtime file provider required")
            })?;
        let values = RuntimeMembers {
            executor,
            pin: detail["native_ref"]["identity"].clone(),
            expected: detail["file_inventory"]["sha256"].clone(),
            cursor: serde_json::Value::Null,
            values: vec![].into_iter(),
            finished: false,
        };
        inventory::prepare_entries(app, item, revision, values)
    } else {
        inventory::prepare(app, item, revision)
    }
}
