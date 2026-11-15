//! Granted log reads run independently of the Executor authority mutex.
use crate::executor::Docker;
use loom_core::{error::require, storage, store::hash, Error, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, Once, OnceLock},
    time::{Duration, Instant},
};
pub struct LogRead<D: Docker> {
    pub docker: Arc<D>,
    pub root: PathBuf,
    pub container: String,
    pub ticket: String,
    pub returned: bool,
}
#[derive(Clone)]
pub struct Source {
    pub path: PathBuf,
    pub generation: String,
    pub closed: bool,
    pub gap: bool,
}
struct Collector {
    child: Child,
    path: PathBuf,
    generation: String,
    gap: bool,
}
type Collectors = BTreeMap<PathBuf, Arc<Mutex<Collector>>>;
const MAX_CAPTURE_BYTES: u64 = 256 * 1024 * 1024;
static REAPER: Once = Once::new();
static COLLECTORS: OnceLock<Mutex<Collectors>> = OnceLock::new();
pub fn follow(container: &str, path: &Path, returned: bool) -> Result<Source> {
    use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};
    let map = COLLECTORS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let pointer = path.with_extension("source.json");
    start_reaper();
    if pointer.is_file() {
        let saved: Value = serde_json::from_slice(&fs::read(&pointer)?)?;
        if saved["closed"] == true {
            let generation = saved["generation"]
                .as_str()
                .ok_or_else(|| Error::input("Log generation missing"))?
                .to_string();
            let id = uuid::Uuid::parse_str(&generation)
                .map_err(|_| Error::input("Invalid saved generation"))?;
            return Ok(Source {
                path: path.with_extension(format!("{id}.log")),
                generation,
                closed: true,
                gap: saved["gap"] == true,
            });
        }
    }
    let collector = {
        let mut map = map
            .lock()
            .map_err(|_| Error::new("logs", "Collector unavailable", 503))?;
        if let Some(c) = map.get(path) {
            c.clone()
        } else {
            let pointer = path.with_extension("source.json");
            let previous = pointer.exists();
            let generation = uuid::Uuid::now_v7().to_string();
            let raw = path.with_extension(format!("{generation}.log"));
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&raw)?;
            let mut command = Command::new("docker");
            command
                .args(["logs", "--follow", container])
                .stdin(Stdio::null())
                .stdout(file.try_clone()?)
                .stderr(file);
            let parent = std::process::id() as i32;
            unsafe {
                command.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: MAX_CAPTURE_BYTES,
                        rlim_max: MAX_CAPTURE_BYTES,
                    };
                    if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        libc::_exit(137)
                    }
                    Ok(())
                });
            }
            let child = command.spawn()?;
            storage::atomic(
                &pointer,
                &serde_json::to_vec(
                    &json!({"generation":generation,"gap":previous,"pid":child.id()}),
                )?,
                0o600,
            )?;
            let c = Arc::new(Mutex::new(Collector {
                child,
                path: raw,
                generation,
                gap: previous,
            }));
            map.insert(path.into(), c.clone());
            c
        }
    };
    let mut c = collector
        .lock()
        .map_err(|_| Error::new("logs", "Collector unavailable", 503))?;
    let start = Instant::now();
    let closed = loop {
        if let Some(status) = c.child.try_wait()? {
            let exhausted = fs::metadata(&c.path).is_ok_and(|m| m.len() >= MAX_CAPTURE_BYTES);
            c.gap |= exhausted && !status.success();
            require(
                status.success() || exhausted,
                "logs_unavailable",
                "Container log source unavailable",
                503,
            )?;
            break true;
        }
        if !returned {
            break false;
        }
        require(
            start.elapsed() < Duration::from_secs(45),
            "logs_timeout",
            "Container logs have not sealed",
            503,
        )?;
        std::thread::sleep(Duration::from_millis(10));
    };
    let source = Source {
        path: c.path.clone(),
        generation: c.generation.clone(),
        closed,
        gap: c.gap,
    };
    if closed {
        let mut registry = map
            .lock()
            .map_err(|_| Error::input("Collector unavailable"))?;
        if registry
            .get(path)
            .is_some_and(|current| Arc::ptr_eq(current, &collector))
        {
            let state = json!({"generation":source.generation,"gap":source.gap,"closed":true,"available":true});
            storage::atomic(&pointer, &serde_json::to_vec(&state)?, 0o600)?;
            storage::atomic(
                &source.path.with_extension("json"),
                &serde_json::to_vec(&state)?,
                0o600,
            )?;
            registry.remove(path);
        }
    }
    Ok(source)
}
impl<D: Docker> LogRead<D> {
    pub fn read(self, args: &Value) -> Result<Value> {
        let directory = self.root.join("logs");
        loom_core::index_store::secure_dir(&directory)?;
        let path = directory.join(format!("{}.log", hash(&self.ticket)));
        let current: Value = fs::read(path.with_extension("source.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let source = if let Some(generation) = args["source_generation"]
            .as_str()
            .filter(|g| current["generation"].as_str() != Some(*g))
        {
            let id = uuid::Uuid::parse_str(generation)
                .map_err(|_| Error::input("Invalid log generation"))?;
            let raw = path.with_extension(format!("{}.log", id));
            let metadata: Value = fs::read(raw.with_extension("json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or(Value::Null);
            require(
                metadata["available"] != false,
                "logs_unavailable",
                "Log source is unavailable",
                503,
            )?;
            if raw.is_file() {
                Source {
                    path: raw,
                    generation: generation.into(),
                    closed: true,
                    gap: true,
                }
            } else {
                return Err(Error::new(
                    "source_generation_changed",
                    "Requested log generation is unavailable",
                    409,
                ));
            }
        } else {
            self.docker
                .log_source(&self.container, &path, self.returned)?
        };
        if let Some(generation) = args["source_generation"].as_str() {
            require(
                generation == source.generation,
                "source_generation_changed",
                "Log generation changed; restart at byte zero",
                409,
            )?;
        }
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&source.path)?;
        let total = f.metadata()?.len();
        let limit = args["limit"]
            .as_u64()
            .unwrap_or(65536)
            .clamp(1, 1024 * 1024);
        let offset = if args["tail"] == true {
            total.saturating_sub(limit)
        } else {
            args["offset"].as_u64().unwrap_or(0).min(total)
        };
        f.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![];
        f.take(limit).read_to_end(&mut bytes)?;
        use base64::Engine;
        Ok(
            json!({"offset":offset,"next_offset":offset+bytes.len() as u64,"total_bytes":total,"text":String::from_utf8_lossy(&bytes),"encoding_errors":std::str::from_utf8(&bytes).is_err(),"data_base64":base64::engine::general_purpose::STANDARD.encode(&bytes),"eof":offset+bytes.len() as u64==total,"source_generation":source.generation,"closed":source.closed,"stream_returned":self.returned,"gap":source.gap,"truncated":args["tail"]==true&&offset>0}),
        )
    }
}

fn start_reaper() {
    REAPER.call_once(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_millis(500));
            let Some(map) = COLLECTORS.get() else { continue };
            let candidates = match map.lock() {
                Ok(registry) => registry.iter().map(|(p,c)| (p.clone(),c.clone())).collect::<Vec<_>>(),
                Err(_) => continue,
            };
            for (path, collector) in candidates {
                let state = if let Ok(mut c) = collector.try_lock() {
                    match c.child.try_wait() {
                        Ok(Some(status)) => {
                            let exhausted = fs::metadata(&c.path).is_ok_and(|m| m.len() >= MAX_CAPTURE_BYTES);
                            let available = status.success() || exhausted;
                            Some((c.path.clone(), json!({
                                "generation":c.generation,"gap":c.gap || !status.success(),
                                "closed":available,"available":available,
                                "retention_reason":if exhausted && !status.success(){Some("capture_budget_exhausted")}else{None}
                            })))
                        }
                        _ => None,
                    }
                } else { None };
                // Drop the collector guard before the registry lock. A delayed
                // old generation must never replace the current source pointer.
                if let Some((raw, state)) = state {
                    if let Ok(mut registry) = map.lock() {
                        if registry.get(&path).is_some_and(|c| Arc::ptr_eq(c, &collector)) {
                            if let Ok(bytes) = serde_json::to_vec(&state) {
                                let sealed = storage::atomic(&raw.with_extension("json"), &bytes, 0o600)
                                    .and_then(|_| storage::atomic(&path.with_extension("source.json"), &bytes, 0o600));
                                if sealed.is_ok() { registry.remove(&path); }
                            }
                        }
                    }
                }
            }
        });
    });
}
