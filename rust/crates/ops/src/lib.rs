//! Bounded service composition; SQLite and lifecycle decisions stay on one application lane.
pub mod catalog_maintenance;
pub mod indexer;
pub mod log_collector;
pub mod ownership;
pub mod updates;
pub mod worker;
use loom_core::{
    execution::Work,
    http::{HttpReply, HttpRequest},
    Application, Error, Result,
};
use serde_json::Value;
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::{mpsc, oneshot};
pub enum Message {
    Maintenance(
        String,
        Value,
        Option<String>,
        oneshot::Sender<Result<Value>>,
    ),
    CatalogTick(oneshot::Sender<Result<Value>>),
    Inventory(
        loom_core::inventory::PreparedInventory,
        oneshot::Sender<Result<()>>,
    ),
    Drained(oneshot::Sender<Result<bool>>),
    Runtimes(Value, oneshot::Sender<Result<()>>),
    Media(
        HttpRequest,
        oneshot::Sender<Result<loom_core::media::MediaRead>>,
    ),
    Context(
        HttpRequest,
        oneshot::Sender<Result<loom_core::http::ReadContext>>,
    ),
    HttpVerified(
        HttpRequest,
        loom_core::reconcile::PreparedFiles,
        oneshot::Sender<Result<HttpReply>>,
    ),
    Http(HttpRequest, oneshot::Sender<Result<HttpReply>>),
    Next(BTreeSet<String>, oneshot::Sender<Result<Option<Work>>>),
    Request(Work, oneshot::Sender<Result<Value>>),
    Accepted(Work, Value, oneshot::Sender<Result<()>>),
    Returned(Work, i32, Value, oneshot::Sender<Result<Value>>),
    Deferred(Work, oneshot::Sender<Result<()>>),
    Unknown(Work, oneshot::Sender<Result<()>>),
    Publish(
        Work,
        Result<(Value, loom_core::reconcile::PreparedFiles)>,
        oneshot::Sender<Result<Value>>,
    ),
    Cancelled(Work, oneshot::Sender<Result<bool>>),
}
#[derive(Clone)]
pub struct Service {
    pub state: std::path::PathBuf,
    pub send: mpsc::Sender<Message>,
    readers: Arc<tokio::sync::Semaphore>,
    heavy_readers: Arc<tokio::sync::Semaphore>,
    pending_reads: Arc<tokio::sync::Semaphore>,
    pub stopping: Arc<std::sync::atomic::AtomicBool>,
}
impl Service {
    pub fn start(mut application: Application) -> Self {
        loom_core::onboarding::install(loom_reference::assets());
        let config = application.config.clone();
        let local = application.local.clone();
        let bulk = application.bulk.clone();
        let components = application.components.clone();
        let read_only = application.read_only;
        if !read_only {
            if let Err(error) = application.cache_catalog_sources() {
                eprintln!("Loom source catalog: {}", error.code);
            }
        }
        let (send, mut receive) = mpsc::channel(64);
        let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
        std::thread::spawn(move || {
            while let Some(message) = receive.blocking_recv() {
                match message {
                    Message::Maintenance(id, p, e, s) => {
                        let _ = s.send(application.advance_maintenance(&id, &p, e.as_deref()));
                    }
                    Message::CatalogTick(s) => {
                        let _ = s.send(application.catalog_tick());
                    }
                    Message::Inventory(p, s) => {
                        let _ = s.send(application.commit_inventory(p));
                    }
                    Message::Drained(s) => {
                        let _ = s.send(application.externally_drained());
                    }
                    Message::Runtimes(value, s) => {
                        let _ = s.send(application.observe_runtimes(value));
                    }
                    Message::Media(r, s) => {
                        let _ = s.send(application.media_lookup(r));
                    }
                    Message::Context(r, s) => {
                        let _ = s.send(application.read_context(&r));
                    }
                    Message::HttpVerified(r, proofs, s) => {
                        let path = r.path.split('?').next().unwrap_or("http").to_string();
                        let result = application.http_verified(r, proofs);
                        if let Err(error) = &result {
                            let _ = application.incident(&path, error);
                        }
                        let _ = s.send(result);
                    }
                    Message::Http(r, s) => {
                        let operation = r.path.split('?').next().unwrap_or("http").to_string();
                        let response = application.http(r);
                        if let Err(error) = &response {
                            let _ = application.incident(&operation, error);
                        }
                        let _ = s.send(response);
                    }
                    Message::Next(exclude, s) => {
                        let _ = s.send(application.next_work(&exclude));
                    }
                    Message::Request(w, s) => {
                        let _ = s.send(application.request_for(&w));
                    }
                    Message::Accepted(w, v, s) => {
                        let _ = s.send(application.effect_accepted(&w, v));
                    }
                    Message::Returned(w, code, v, s) => {
                        let _ = s.send(application.stage_returned(&w, code, v));
                    }
                    Message::Deferred(w, s) => {
                        let _ = s.send(application.effect_defer(&w));
                    }
                    Message::Unknown(w, s) => {
                        let _ = s.send(application.effect_unknown(&w));
                    }
                    Message::Publish(w, prepared, s) => {
                        let result = match prepared {
                            Ok((manifest, proofs)) => {
                                application.set_file_sets(proofs.inventories);
                                let result = application.publish_work_verified(
                                    &w,
                                    manifest,
                                    Some(&proofs.files),
                                );
                                application.set_file_sets(Default::default());
                                result
                            }
                            Err(error) => application.publish_work_failed(&w, error),
                        };
                        let _ = s.send(result);
                    }
                    Message::Cancelled(w, s) => {
                        let _ = s.send(application.cancelled(&w));
                    }
                }
            }
        });
        let service = Self {
            state: local.clone(),
            send,
            stopping,
            readers: Arc::new(tokio::sync::Semaphore::new(4)),
            heavy_readers: Arc::new(tokio::sync::Semaphore::new(2)),
            pending_reads: Arc::new(tokio::sync::Semaphore::new(32)),
        };
        if !read_only {
            log_collector::start(
                service.clone(),
                config.clone(),
                local.clone(),
                bulk.clone(),
                components.clone(),
            );
            indexer::start(service.clone(), config, local, bulk, components);
        }
        service
    }
    pub async fn drained(&self) -> Result<bool> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Drained(s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
    pub async fn runtimes(&self, value: Value) -> Result<()> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Runtimes(value, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
    async fn read_permits(
        &self,
        heavy: bool,
    ) -> Result<(
        tokio::sync::OwnedSemaphorePermit,
        Option<tokio::sync::OwnedSemaphorePermit>,
        tokio::sync::OwnedSemaphorePermit,
    )> {
        // Bound both active readers and waiters. Waiting heavy work must not hold
        // a general slot needed by status, telemetry or other lightweight reads.
        let request = self
            .pending_reads
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::new("busy", "Read request queue is full", 503))?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
        let acquire = |semaphore: Arc<tokio::sync::Semaphore>| async move {
            tokio::time::timeout_at(deadline, semaphore.acquire_owned())
                .await
                .map_err(|_| Error::new("busy", "Read request wait expired", 503))?
                .map_err(|_| Error::new("unavailable", "Read workers stopped", 503))
        };
        let heavy = if heavy {
            Some(acquire(self.heavy_readers.clone()).await?)
        } else {
            None
        };
        let permit = acquire(self.readers.clone()).await?;
        Ok((request, heavy, permit))
    }
    pub async fn http(&self, r: HttpRequest) -> Result<HttpReply> {
        if loom_core::http::is_read(&r) || loom_core::http::needs_file_proofs(&r) {
            let (request, heavy, permit) = self
                .read_permits(loom_core::http::is_heavy_read(&r))
                .await?;
            let (send, receive) = oneshot::channel();
            self.send
                .try_send(Message::Context(r.clone(), send))
                .map_err(|_| Error::new("busy", "Application lane is full", 503))?;
            let context = match receive
                .await
                .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
            {
                Ok(context) => context,
                Err(_) if r.path == "/mcp" => {
                    let (send, receive) = oneshot::channel();
                    self.send
                        .send(Message::Http(r, send))
                        .await
                        .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
                    return receive
                        .await
                        .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
                }
                Err(error) => return Err(error),
            };
            if loom_core::http::is_read(&r) {
                return tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    let _heavy = heavy;
                    let _request = request;
                    if let Some((executor, args)) = context.live_file_request(&r)? {
                        let value = loom_host::wire::call(
                            std::path::Path::new(
                                executor["socket"]
                                    .as_str()
                                    .ok_or_else(|| Error::input("Executor socket required"))?,
                            ),
                            std::path::Path::new(
                                executor["credential"]
                                    .as_str()
                                    .ok_or_else(|| Error::input("Executor credential required"))?,
                            ),
                            "runtime_file_read",
                            &args,
                        );
                        return context.http_with_file(r, value);
                    }
                    let source = match context.live_log_request(&r) {
                        Ok(source) => source,
                        Err(_) => return context.http(r),
                    };
                    let logs = if let Some((executor, args)) = source {
                        let result = loom_host::wire::call(
                            std::path::Path::new(
                                executor["socket"]
                                    .as_str()
                                    .ok_or_else(|| Error::input("Executor socket required"))?,
                            ),
                            std::path::Path::new(
                                executor["credential"]
                                    .as_str()
                                    .ok_or_else(|| Error::input("Executor credential required"))?,
                            ),
                            "log_range",
                            &args,
                        );
                        Some(result.map(|mut value| {
                            value["id"] = args["id"].clone();
                            value["attempt"] = args["attempt"].clone();
                            value
                        }))
                    } else {
                        None
                    };
                    context.http_with_logs(r, logs)
                })
                .await
                .map_err(|_| Error::new("query", "Read worker stopped", 503))?;
            }
            let proofs = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _heavy = heavy;
                let _request = request;
                context.prepare_files()
            })
            .await
            .map_err(|_| Error::new("verification", "File verification worker stopped", 503))??;
            let (send, receive) = oneshot::channel();
            self.send
                .send(Message::HttpVerified(r, proofs, send))
                .await
                .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
            return receive.await.map_err(|_| {
                Error::new(
                    "outcome_unknown",
                    "Verified operation reply lost; reconcile the original key",
                    503,
                )
            })?;
        }

        if r.path.starts_with("/api/dashboard/media/") {
            let (request, heavy, permit) = self.read_permits(true).await?;
            let (s, receive) = oneshot::channel();
            self.send
                .try_send(Message::Media(r, s))
                .map_err(|_| Error::new("busy", "Application request lane is full", 503))?;
            let descriptor = receive
                .await
                .map_err(|_| Error::new("unavailable", "Application stopped", 503))??;
            return tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _heavy = heavy;
                let _request = request;
                loom_core::media::read(descriptor)
            })
            .await
            .map_err(|_| Error::new("media", "Registered file reader stopped", 503))?;
        }
        let (send, receive) = oneshot::channel();
        self.send
            .try_send(Message::Http(r, send))
            .map_err(|_| Error::new("busy", "Application request lane is full", 503))?;
        receive.await.map_err(|_| {
            Error::new(
                "outcome_unknown",
                "Application reply lost; reconcile original key",
                503,
            )
        })?
    }
    pub async fn next(&self, exclude: BTreeSet<String>) -> Result<Option<Work>> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Next(exclude, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
    pub async fn request(&self, w: Work) -> Result<Value> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Request(w, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
    pub async fn accepted(&self, w: Work, v: Value) -> Result<()> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Accepted(w, v, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("outcome_unknown", "Acceptance reply lost", 503))?
    }
    pub async fn returned(&self, w: Work, c: i32, v: Value) -> Result<Value> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Returned(w, c, v, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("outcome_unknown", "Return reply lost", 503))?
    }
    pub async fn defer(&self, w: Work) -> Result<()> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Deferred(w, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
    pub async fn unknown(&self, w: Work) -> Result<()> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Unknown(w, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
    pub async fn publish(
        &self,
        w: Work,
        prepared: Result<(Value, loom_core::reconcile::PreparedFiles)>,
    ) -> Result<Value> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Publish(w, prepared, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("outcome_unknown", "Publication reply lost", 503))?
    }
    pub async fn cancelled(&self, w: Work) -> Result<bool> {
        let (s, r) = oneshot::channel();
        self.send
            .send(Message::Cancelled(w, s))
            .await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?;
        r.await
            .map_err(|_| Error::new("unavailable", "Application stopped", 503))?
    }
}

#[cfg(test)]
#[path = "../../../../tests/rust/read_admission.rs"]
mod read_admission_tests;
