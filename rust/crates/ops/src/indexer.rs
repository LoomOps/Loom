//! Independent bounded projection worker. The application lane only commits typed metadata.
use crate::{Message, Service};
use loom_core::{inventory, Application, Config};
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};
pub fn start(service: Service, config: Config, local: PathBuf, bulk: PathBuf, components: PathBuf) {
    std::thread::spawn(move || {
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
                    let prepared = match inventory::prepare(&app, &item, &revision) {
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
                let mut index = loom_core::index_store::IndexWriter::open(&app)?;
                index.sync(&app, 128)?;
                loom_core::log_index::project(&mut index, &app)?;
                index.observe_files(&app)?;
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("Loom search projection: {}", error.code);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}
