use loom_primitives::{
    compat,
    error::require,
    store::{hash, rows},
    Error, Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Instance {
    pub uid: u32,
    pub gid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    pub data: PathBuf,
    #[serde(default)]
    pub components: Option<PathBuf>,
    #[serde(default)]
    pub storage: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub runtimes: Vec<String>,
    #[serde(default)]
    pub gpu: bool,
    #[serde(default)]
    pub declared_runtime: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub uid: u32,
    pub gid: u32,
    pub components: PathBuf,
    pub instances: BTreeMap<String, Instance>,
    #[serde(default)]
    pub runtimes: BTreeMap<String, Value>,
    #[serde(default)]
    pub gpu_devices: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
pub trait Docker: Send + Sync {
    fn call(&self, args: &[String]) -> Result<String>;
    fn log_source(
        &self,
        container: &str,
        path: &Path,
        returned: bool,
    ) -> Result<crate::logs::Source> {
        self.capture_logs(container, path)?;
        Ok(crate::logs::Source {
            path: path.into(),
            generation: uuid::Uuid::new_v4().to_string(),
            closed: returned,
            gap: false,
        })
    }
    fn capture_logs(&self, container: &str, path: &Path) -> Result<()> {
        let text = self.call(&strings(&["logs", container]))?;
        loom_primitives::storage::atomic(path, text.as_bytes(), 0o600)
    }
}
pub struct DockerCli;
impl Docker for DockerCli {
    fn log_source(
        &self,
        container: &str,
        path: &Path,
        returned: bool,
    ) -> Result<crate::logs::Source> {
        crate::logs::follow(container, path, returned)
    }
    fn capture_logs(&self, container: &str, path: &Path) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            let mut child = Command::new("docker")
                .args(["logs", container])
                .stdin(std::process::Stdio::null())
                .stdout(file.try_clone()?)
                .stderr(file.try_clone()?)
                .spawn()?;
            let started = std::time::Instant::now();
            let status = loop {
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                if started.elapsed() > std::time::Duration::from_secs(45) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::new(
                        "logs_timeout",
                        "Log capture timed out; previous log retained",
                        503,
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            };
            require(
                status.success(),
                "logs",
                "Container log capture unavailable",
                503,
            )?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            fs::File::open(path.parent().unwrap())?.sync_all()?;
            Ok(())
        })();
        let _ = fs::remove_file(temporary);
        result
    }
    fn call(&self, args: &[String]) -> Result<String> {
        let mut child = Command::new("docker")
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let mut stdout = child.stdout.take().unwrap();
        let output = std::thread::spawn(move || {
            let mut bytes = vec![];
            stdout
                .by_ref()
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let start = std::time::Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait()? {
                break s;
            }
            if start.elapsed() > std::time::Duration::from_secs(45) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = output.join();
                return Err(Error::new("docker_timeout", "Docker outcome unknown", 503));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let bytes = output
            .join()
            .map_err(|_| Error::new("docker", "Docker output unavailable", 503))??;
        require(
            status.success() && bytes.len() <= 4 * 1024 * 1024,
            "docker",
            "Docker operation unavailable",
            503,
        )?;
        Ok(String::from_utf8_lossy(&bytes).into())
    }
}
pub struct Executor<D: Docker> {
    pub root: PathBuf,
    pub config: Config,
    pub db: Connection,
    pub docker: std::sync::Arc<D>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub ticket: String,
    pub job: String,
    pub stage: String,
    #[serde(rename = "command", alias = "argv")]
    pub argv: Vec<String>,
    #[serde(rename = "host_output", alias = "output")]
    pub output: String,
    #[serde(default)]
    pub output_storage: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default = "cpu")]
    pub lane: String,
    #[serde(default)]
    pub gpu_access: Option<Value>,
    #[serde(default)]
    pub runtime_id: Option<String>,
    #[serde(default)]
    pub runtime_ref: Option<Value>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub resources: Value,
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub tools: BTreeMap<String, String>,
    #[serde(default)]
    pub storage: Vec<String>,
}
fn cpu() -> String {
    "cpu".into()
}
fn identity(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
        "identity",
        "Invalid host identity",
        400,
    )
}
fn mount_text(value: &str) -> Result<()> {
    require(
        !value.contains([',', '\0', '\n', '\r']) && !value.is_empty(),
        "mount",
        "Invalid bind mount path",
        400,
    )
}
fn legacy_data_alias(
    cfg: &Instance,
    approved: &[Value],
    mount: &Value,
    source: &Path,
    target: &Path,
) -> bool {
    let Ok(relative) = target.strip_prefix("/loom/data") else {
        return false;
    };
    let Some(std::path::Component::Normal(namespace)) = relative.components().next() else {
        return false;
    };
    // Numbered stores are reserved for per-Job storage grants. An existing
    // administrator policy may mirror only its exact, read-only legacy bind.
    !namespace.as_encoded_bytes().iter().all(u8::is_ascii_digit)
        && source == cfg.data.join(relative)
        && mount["readonly"] == true
        && approved.contains(mount)
}
fn inside(root: &Path, value: &str) -> Result<PathBuf> {
    mount_text(value)?;
    let root = root.canonicalize()?;
    let path = Path::new(value);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let path = path.canonicalize()?;
    mount_text(
        path.to_str()
            .ok_or_else(|| Error::input("Mount path must be UTF-8"))?,
    )?;
    require(
        path.starts_with(root),
        "grant",
        "Path outside executor grant",
        403,
    )?;
    Ok(path)
}
fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}
impl<D: Docker> Executor<D> {
    pub fn open(root: &Path, config: Config, docker: D) -> Result<Self> {
        require(
            config.instances.values().all(|i| i.uid > 0 && i.gid > 0),
            "instance",
            "Job instances require non-root UID/GID",
            400,
        )?;
        fs::create_dir_all(root)?;
        let db_path = root.join("executor.sqlite3");
        require(
            !db_path
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink()),
            "executor",
            "Executor DB must not be linked",
            409,
        )?;
        let db = Connection::open(&db_path)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600))?;
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS tickets(id TEXT PRIMARY KEY,instance TEXT,container TEXT,gpu INTEGER,state TEXT,request TEXT,exit_code INTEGER,image_id TEXT,logs TEXT);")?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS ticket_launches(ticket TEXT PRIMARY KEY,labels TEXT NOT NULL,argv TEXT NOT NULL);")?;
        let columns = rows(&db, "PRAGMA table_info(tickets)", &[])?;
        for name in ["exit_code", "image_id", "logs", "gpu_devices"] {
            if !columns.iter().any(|c| c["name"] == name) {
                db.execute(
                    &format!(
                        "ALTER TABLE tickets ADD COLUMN {name} {}",
                        if name == "exit_code" {
                            "INTEGER"
                        } else {
                            "TEXT"
                        }
                    ),
                    [],
                )?;
            }
        }
        Ok(Self {
            root: root.into(),
            config,
            db,
            docker: std::sync::Arc::new(docker),
        })
    }
    pub fn authenticate(&self, token: &str, peer_uid: u32) -> Result<String> {
        for (id, instance) in &self.config.instances {
            let supplied = hash(token);
            let expected = instance
                .extra
                .get("token_sha256")
                .and_then(Value::as_str)
                .map(String::from)
                .or_else(|| instance.token.as_ref().map(hash));
            let same = expected.as_ref().is_some_and(|expected| {
                expected.len() == supplied.len()
                    && expected
                        .bytes()
                        .zip(supplied.bytes())
                        .fold(0u8, |a, (x, y)| a | (x ^ y))
                        == 0
            });
            if same {
                require(
                    peer_uid == 0 || peer_uid == instance.uid || peer_uid == self.config.uid,
                    "peer",
                    "Executor peer UID outside grant",
                    403,
                )?;
                return Ok(id.clone());
            }
        }
        Err(Error::new("credential", "Invalid executor credential", 403))
    }
    pub fn handle(&mut self, instance: &str, action: &str, value: Value) -> Result<Value> {
        require(
            self.config.instances.contains_key(instance),
            "instance",
            "Unknown instance",
            403,
        )?;
        match action {
            "cancel_intent" => {
                let request: Start = serde_json::from_value(value.clone())?;
                identity(&request.ticket)?;
                let ticket = format!("{instance}.{}", request.ticket);
                let encoded = compat::dump(&value);
                let old = rows(
                    &self.db,
                    "SELECT request FROM tickets WHERE id=? AND instance=?",
                    &[&ticket, &instance],
                )?
                .into_iter()
                .next();
                if let Some(old) = old {
                    require(
                        old["request"] == encoded,
                        "ticket",
                        "Ticket request conflict",
                        409,
                    )?;
                    self.ticket_action(instance, "cancel", json!({"ticket":ticket}))
                } else {
                    self.db.execute("INSERT INTO tickets(id,instance,container,gpu,state,request,exit_code) VALUES(?,?,?,?, 'returned',?,137)",params![ticket,instance,format!("loom-{}",hash(&ticket)),i64::from(request.lane=="gpu"),encoded])?;
                    Ok(
                        json!({"ticket":ticket,"state":"exited","returned":true,"exit_code":137,"not_started":true}),
                    )
                }
            }
            "status_intent" => {
                let suffix = value["ticket"]
                    .as_str()
                    .ok_or_else(|| Error::input("Ticket required"))?;
                identity(suffix)?;
                let ticket = format!("{instance}.{suffix}");
                let row = rows(
                    &self.db,
                    "SELECT * FROM tickets WHERE id=? AND instance=?",
                    &[&ticket, &instance],
                )?
                .into_iter()
                .next();
                if let Some(row) = row {
                    require(
                        row["request"] == compat::dump(&value),
                        "ticket",
                        "Ticket request conflict",
                        409,
                    )?;
                    let mut state = self.observed(&row)?;
                    state["ticket"] = json!(ticket);
                    state["found"] = json!(true);
                    Ok(state)
                } else {
                    Ok(json!({"found":false}))
                }
            }
            "start" => {
                if value["lane"] == "gpu" {
                    use std::os::fd::AsRawFd;
                    use std::os::unix::fs::OpenOptionsExt;
                    let file = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW)
                        .open(self.root.join("runtime-install.lock"))?;
                    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } != 0
                        || crate::runtime::validation_fenced(&self.root)?
                    {
                        return Ok(
                            json!({"busy":true,"reason":"Runtime validation owner unresolved"}),
                        );
                    }
                    let result = self.start(instance, value);
                    drop(file);
                    result
                } else {
                    self.start(instance, value)
                }
            }
            "status" | "cancel" | "logs" | "log_range" => {
                self.ticket_action(instance, action, value)
            }
            "list_runtimes" => {
                let mut result = crate::runtime::listing(&self.root, &self.config, instance)?;
                let mut runtimes = result["runtimes"].as_array().cloned().unwrap_or_default();
                let aliases = runtimes
                    .iter()
                    .filter(|r| r["legacy"] == true)
                    .cloned()
                    .collect::<Vec<_>>();
                for alias in aliases {
                    let policy = &alias["manifest"]["runtime"];
                    let selected = runtimes
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| {
                            r["legacy"] != true
                                && r["manifest"]["reconciliation"]["runtime_id"] == alias["id"]
                                && r["manifest"]["reconciliation"]["policy_sha256"]
                                    == hash(compat::seal(policy))
                        })
                        .max_by_key(|(_, r)| r["revision"].as_i64().unwrap_or(0))
                        .map(|(i, _)| i);
                    if let Some(i) = selected {
                        let measured = self
                            .docker
                            .call(&strings(&[
                                "image",
                                "inspect",
                                policy["image"].as_str().unwrap_or(""),
                            ]))
                            .ok()
                            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
                        if measured.as_ref().is_some_and(|v| {
                            v[0]["Id"] == runtimes[i]["manifest"]["reconciliation"]["image_id"]
                        }) && crate::runtime::verify_inventory(&runtimes[i]["manifest"]).is_ok()
                        {
                            if !runtimes[i]["aliases"].is_array() {
                                runtimes[i]["aliases"] = json!([]);
                            }
                            runtimes[i]["aliases"]
                                .as_array_mut()
                                .unwrap()
                                .push(alias["id"].clone());
                            runtimes.retain(|r| r["legacy"] != true || r["id"] != alias["id"]);
                        }
                    }
                }
                result["runtimes"] = json!(runtimes);
                let policy = &self.config.instances[instance].declared_runtime;
                result["declared_policy"] = policy.clone();
                if policy["enabled"] == true {
                    let inventory = self.docker.call(&strings(&[
                        "image",
                        "ls",
                        "--no-trunc",
                        "--format",
                        "{{json .}}",
                    ]));
                    if let Ok(lines) = inventory {
                        let mut images = vec![];
                        for line in lines.lines() {
                            if let Ok(row) = serde_json::from_str::<Value>(line) {
                                let id = row["ID"].as_str().unwrap_or("");
                                if id.starts_with("sha256:") && id.len() == 71 {
                                    images.push(json!({"id":id,"tag":format!("{}:{}",row["Repository"].as_str().unwrap_or(""),row["Tag"].as_str().unwrap_or(""))}));
                                }
                            }
                        }
                        result["declared_images"] = json!(images);
                    }
                }
                result["protocol"] = json!({"version":1,"capabilities":["cancel_intent","status_intent","log_range","log_stream_v1","runtime_ref","drain_status","gpu_access"]});
                Ok(result)
            }
            "resolve_runtime" => {
                let item = crate::runtime::detail(&self.root, instance, &value["runtime_ref"])?;
                crate::runtime::verify_inventory(&item["manifest"])?;
                Ok(item)
            }
            "resolve" => {
                let cfg = &self.config.instances[instance];
                let r = self.declared(&value, &cfg.declared_runtime)?;
                Ok(
                    json!({"image":r["image"],"resources":{"cpus":r["cpus"],"memory":r["memory"],"pids":r["pids"]}}),
                )
            }
            "drain_status" => {
                let mut outstanding = vec![];
                for row in rows(
                    &self.db,
                    "SELECT * FROM tickets WHERE instance=? AND state!='returned'",
                    &[&instance],
                )? {
                    let observed = self.observed(&row)?;
                    if observed["returned"] != true {
                        outstanding.push(json!({"ticket":row["id"],"state":observed["state"]}));
                    }
                }
                let runtime_fence = crate::runtime::validation_fenced(&self.root)?;
                Ok(
                    json!({"ready":outstanding.is_empty()&&!runtime_fence,"outstanding":outstanding,"runtime_validation_owner":runtime_fence}),
                )
            }
            "recover" => {
                let job = value["job"]
                    .as_str()
                    .ok_or_else(|| Error::input("Recovery Job required"))?;
                let attempt = value["attempt"]
                    .as_str()
                    .ok_or_else(|| Error::input("Recovery Attempt required"))?;
                identity(job)?;
                identity(attempt)?;
                let prefix = format!("{instance}.{job}.{attempt}.");
                let mut result = vec![];
                for row in rows(
                    &self.db,
                    "SELECT id FROM tickets WHERE instance=? AND state!='returned'",
                    &[&instance],
                )? {
                    let ticket = row["id"].as_str().unwrap_or("");
                    if !ticket.starts_with(&prefix) {
                        continue;
                    }
                    let natural =
                        self.ticket_action(instance, "status", json!({"ticket":ticket}))?;
                    let state = if natural["returned"] == true {
                        natural.clone()
                    } else {
                        self.ticket_action(instance, "cancel", json!({"ticket":ticket}))?
                    };
                    require(
                        state["returned"] == true,
                        "return",
                        "Recovery did not prove return",
                        503,
                    )?;
                    result.push(
                        json!({"ticket":ticket,"natural_return":natural["returned"],"state":state}),
                    );
                }
                Ok(json!(result))
            }
            _ => Err(Error::input("Unknown executor operation")),
        }
    }
    fn declared(&self, data: &Value, policy: &Value) -> Result<Value> {
        require(
            policy["enabled"] == true,
            "runtime",
            "Declared images disabled",
            403,
        )?;
        let image = data["image"]
            .as_str()
            .ok_or_else(|| Error::input("Image required"))?;
        require(
            !image.starts_with('-')
                && image.len() <= 512
                && !image.chars().any(char::is_whitespace),
            "image",
            "Invalid image reference",
            400,
        )?;
        require(
            data["resources"].is_null()
                || data["resources"].as_object().is_some_and(|o| {
                    o.keys()
                        .all(|k| ["cpus", "pids", "memory"].contains(&k.as_str()))
                }),
            "resources",
            "Unknown resource fields; mounts and network are host policy",
            400,
        )?;
        let cpus = data["resources"]["cpus"].as_f64().unwrap_or(1.);
        let pids = data["resources"]["pids"].as_i64().unwrap_or(512);
        let memory = data["resources"]["memory"].as_str().unwrap_or("2g");
        require(
            cpus > 0.
                && cpus <= policy["max_cpus"].as_f64().unwrap_or(4.)
                && pids >= 16
                && pids <= policy["max_pids"].as_i64().unwrap_or(1024),
            "resources",
            "Resources exceed instance grant",
            403,
        )?;
        fn size(v: &str) -> Option<u64> {
            let (n, u) = v.split_at_checked(v.len().checked_sub(1)?)?;
            let factor = match u {
                "k" => 1024,
                "m" => 1024 * 1024,
                "g" => 1024 * 1024 * 1024,
                _ => return None,
            };
            n.parse::<u64>().ok()?.checked_mul(factor)
        }
        require(
            size(memory).is_some_and(|m| {
                m >= 16 * 1024 * 1024
                    && m <= size(policy["max_memory"].as_str().unwrap_or("8g")).unwrap_or(0)
            }),
            "memory",
            "Memory exceeds instance grant",
            403,
        )?;
        let digest = self
            .docker
            .call(&strings(&[
                "image", "inspect", "--format", "{{.Id}}", image,
            ]))?
            .trim()
            .to_string();
        require(
            digest.starts_with("sha256:") && digest.len() == 71,
            "image",
            "Image identity unavailable",
            503,
        )?;
        Ok(
            json!({"image":digest,"cpus":cpus,"memory":memory,"pids":pids,"network":policy["network"].as_str().unwrap_or("none"),"driver_capabilities":policy.get("driver_capabilities").cloned().unwrap_or(json!(["compute","utility"]))}),
        )
    }
    fn select_gpu_devices(&self, request: Option<&Value>) -> Result<Vec<String>> {
        let configured = if self.config.gpu_devices.is_empty() {
            vec!["0".to_owned()]
        } else {
            self.config.gpu_devices.clone()
        };
        let Some(request) = request else {
            return Ok(configured);
        };
        require(
            matches!(request["mode"].as_str(), Some("exclusive" | "shared")),
            "gpu_access",
            "GPU access mode required",
            400,
        )?;
        let devices = if let Some(devices) = request.get("devices") {
            let devices: Vec<String> = serde_json::from_value(devices.clone())?;
            require(
                !devices.is_empty()
                    && devices.iter().all(|id| configured.contains(id))
                    && devices
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == devices.len(),
                "gpu_access",
                "GPU devices must be distinct configured devices",
                409,
            )?;
            devices
        } else {
            vec![configured[0].clone()]
        };
        if request["mode"] == "shared" {
            let budget = &self
                .config
                .extra
                .get("gpu_shared_memory_mib")
                .cloned()
                .unwrap_or_default();
            let requested = request["memory_mib"].as_u64().unwrap_or(0);
            require(
                requested > 0
                    && devices
                        .iter()
                        .all(|id| budget[id].as_u64().is_some_and(|n| n >= requested)),
                "gpu_capacity",
                "Shared GPU access needs a reservation within the configured device budget",
                409,
            )?;
        }
        Ok(devices)
    }

    fn gpu_busy(&self, devices: &[String], request: Option<&Value>) -> Result<bool> {
        let shared = request.is_some_and(|v| v["mode"] == "shared");
        let requested = request.and_then(|v| v["memory_mib"].as_u64()).unwrap_or(0);
        let limits = self
            .config
            .extra
            .get("gpu_shared_memory_mib")
            .cloned()
            .unwrap_or_default();
        let mut reserved = BTreeMap::<String, u64>::new();
        for ticket in rows(
            &self.db,
            "SELECT * FROM tickets WHERE gpu=1 AND state!='returned'",
            &[],
        )? {
            // Unknown status keeps the previous resource claim.
            if self.observed(&ticket).is_ok_and(|v| v["returned"] == true) {
                continue;
            }
            let start: Value = serde_json::from_str(ticket["request"].as_str().unwrap_or("{}"))?;
            let access = start.get("gpu_access").filter(|v| !v.is_null());
            let occupied = if let Some(devices) = ticket["gpu_devices"].as_str() {
                serde_json::from_str::<Vec<String>>(devices)?
            } else {
                self.select_gpu_devices(access)?
            };
            for device in devices.iter().filter(|d| occupied.contains(d)) {
                if !shared || access.is_none_or(|v| v["mode"] != "shared") {
                    return Ok(true);
                }
                let used = reserved.entry(device.clone()).or_default();
                *used =
                    used.saturating_add(access.unwrap()["memory_mib"].as_u64().unwrap_or(u64::MAX));
                if used.saturating_add(requested) > limits[device].as_u64().unwrap_or(0) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn start(&mut self, instance: &str, value: Value) -> Result<Value> {
        let request: Start = serde_json::from_value(value.clone())?;
        identity(&request.ticket)?;
        identity(&request.job)?;
        loom_domain::Stage::parse(&request.stage)?;
        let lane = loom_domain::Lane::parse(&request.lane)?;
        require(
            !request.argv.is_empty()
                && request
                    .argv
                    .iter()
                    .all(|v| !v.is_empty() && !v.contains('\0')),
            "argv",
            "Valid command argv required",
            400,
        )?;
        let cfg = self.config.instances[instance].clone();
        let ticket = format!("{instance}.{}", request.ticket);
        let encoded = compat::dump(&value);
        let old = rows(&self.db, "SELECT * FROM tickets WHERE id=?", &[&ticket])?
            .into_iter()
            .next();
        if let Some(old) = old {
            require(
                old["request"] == encoded,
                "ticket",
                "Ticket request conflict",
                409,
            )?;
            let state = self.observed(&old)?;
            if state["state"] == "created" {
                self.docker.call(&strings(&[
                    "start",
                    old["container"].as_str().unwrap_or(""),
                ]))?;
            }
            return Ok(
                json!({"ticket":ticket,"state":state["state"],"returned":state["returned"],"exit_code":state["exit_code"],"image_id":old["image_id"]}),
            );
        }
        let gpu = lane == loom_domain::Lane::Gpu;
        require(!gpu || cfg.gpu, "gpu", "GPU outside instance grant", 403)?;
        require(
            gpu || request.gpu_access.is_none(),
            "gpu_access",
            "CPU launches cannot claim GPU access",
            400,
        )?;
        let devices = if gpu {
            self.select_gpu_devices(request.gpu_access.as_ref())?
        } else {
            vec![]
        };
        if gpu && self.gpu_busy(&devices, request.gpu_access.as_ref())? {
            return Ok(json!({"busy":true}));
        }
        let gpu_devices = gpu.then(|| serde_json::to_string(&devices)).transpose()?;
        let (runtime, legacy_policy) = if let Some(pin) = &request.runtime_ref {
            require(
                pin["sha256"].is_string(),
                "runtime",
                "Sealed Runtime pin required",
                400,
            )?;
            let item = crate::runtime::detail(&self.root, instance, pin)?;
            crate::runtime::verify_inventory(&item["manifest"])?;
            (
                item["manifest"]["runtime"].clone(),
                item["manifest"]["reconciliation"]["runtime_id"]
                    .as_str()
                    .map(str::to_owned),
            )
        } else if request.image.is_some() {
            let r = self.declared(&value, &cfg.declared_runtime)?;
            require(
                Some(&r["image"]) == value.get("image"),
                "image",
                "Execution requires pinned image",
                409,
            )?;
            (r, None)
        } else {
            let id = request
                .runtime_id
                .as_deref()
                .ok_or_else(|| Error::input("Execution environment required"))?;
            require(
                cfg.runtimes.iter().any(|r| r == id),
                "runtime",
                "Runtime outside instance grant",
                403,
            )?;
            let policy = self
                .config
                .runtimes
                .get(id)
                .cloned()
                .ok_or_else(|| Error::input("Unknown runtime"))?;
            (policy, Some(id.to_owned()))
        };
        require(
            matches!(
                runtime["network"].as_str().unwrap_or("none"),
                "none" | "bridge"
            ),
            "runtime",
            "Invalid network policy",
            400,
        )?;
        let output = inside(&cfg.data, &request.output).or_else(|_| {
            for root in cfg.storage.values() {
                if let Ok(p) = inside(root, &request.output) {
                    return Ok(p);
                }
            }
            Err(Error::new("output", "Output outside storage grant", 403))
        })?;
        let name = format!("loom-{}", hash(&ticket));
        let mut argv = strings(&[
            "create",
            "--log-driver",
            "local",
            "--log-opt",
            "max-size=256m",
            "--log-opt",
            "max-file=2",
            "--name",
            &name,
            "--user",
            &format!("{}:{}", cfg.uid, cfg.gid),
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--network",
            runtime["network"].as_str().unwrap_or("none"),
            "--cpus",
            &runtime["cpus"].as_f64().unwrap_or(1.).to_string(),
            "--memory",
            runtime["memory"].as_str().unwrap_or("2g"),
            "--pids-limit",
            &runtime["pids"].as_i64().unwrap_or(512).to_string(),
            "--tmpfs",
            "/tmp:rw,nosuid,nodev,size=512m",
            "--mount",
            &format!("type=bind,source={},target=/work", output.display()),
        ]);
        for (name, value) in &request.env {
            require(
                !name.contains(['=', '\0']) && !value.contains('\0'),
                "env",
                "Invalid environment",
                400,
            )?;
            argv.extend(strings(&["--env", &format!("{name}={value}")]));
        }
        for (name, reference) in &request.inputs {
            identity(name)?;
            let (root, value) = if reference.is_object() {
                let storage = reference["storage"]
                    .as_str()
                    .ok_or_else(|| Error::input("Input storage required"))?;
                (
                    cfg.storage
                        .get(storage)
                        .ok_or_else(|| Error::new("input", "Input storage outside grant", 403))?,
                    reference["ref"]
                        .as_str()
                        .ok_or_else(|| Error::input("Input ref required"))?,
                )
            } else {
                (
                    &cfg.data,
                    reference
                        .as_str()
                        .ok_or_else(|| Error::input("Input path required"))?,
                )
            };
            let path = inside(root, value)?;
            require(path.is_file(), "input", "Input file unavailable", 409)?;
            argv.extend(strings(&[
                "--mount",
                &format!(
                    "type=bind,source={},target=/inputs/{name},readonly",
                    path.display()
                ),
            ]));
        }
        if let Some(component) = &request.component {
            let path = inside(
                cfg.components.as_ref().unwrap_or(&self.config.components),
                component,
            )?;
            argv.extend(strings(&[
                "--mount",
                &format!(
                    "type=bind,source={},target=/component,readonly",
                    path.display()
                ),
            ]));
        }
        for (alias, path) in &request.tools {
            identity(alias)?;
            let path = inside(
                &cfg.components
                    .as_ref()
                    .unwrap_or(&self.config.components)
                    .join(".tools"),
                path,
            )?;
            argv.extend(strings(&[
                "--mount",
                &format!(
                    "type=bind,source={},target=/tools/{alias},readonly",
                    path.display()
                ),
            ]));
        }
        for selector in &request.storage {
            require(
                selector
                    .strip_prefix("data/")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())),
                "storage",
                "Invalid storage selector",
                400,
            )?;
            let root = cfg
                .storage
                .get(selector)
                .ok_or_else(|| Error::new("storage", "Storage outside instance grant", 403))?;
            require(
                root.canonicalize()? == *root,
                "storage",
                "Storage redirected",
                409,
            )?;
            mount_text(
                root.to_str()
                    .ok_or_else(|| Error::input("Storage path must be UTF-8"))?,
            )?;
            argv.extend(strings(&[
                "--mount",
                &format!(
                    "type=bind,source={},target=/loom/{selector}",
                    root.display()
                ),
            ]));
        }
        let mut destinations = std::collections::BTreeSet::new();
        let approved_legacy_mounts = legacy_policy
            .filter(|id| cfg.runtimes.contains(id))
            .and_then(|id| self.config.runtimes.get(&id))
            .and_then(|policy| policy["mounts"].as_array())
            .map(Vec::as_slice)
            .unwrap_or_default();
        for mount in runtime["mounts"].as_array().into_iter().flatten() {
            let source = mount["source"]
                .as_str()
                .ok_or_else(|| Error::input("Runtime mount source required"))?;
            let target = mount["target"]
                .as_str()
                .ok_or_else(|| Error::input("Runtime mount target required"))?;
            mount_text(source)?;
            mount_text(target)?;
            let p = Path::new(source);
            let t = Path::new(target);
            require(
                p.is_absolute()
                    && p.canonicalize()? == p
                    && t.is_absolute()
                    && t.components()
                        .skip(1)
                        .all(|c| matches!(c, std::path::Component::Normal(_)))
                    && (![
                        "/work",
                        "/inputs",
                        "/component",
                        "/tools",
                        "/loom",
                        "/var/run/docker.sock",
                    ]
                    .iter()
                    .any(|prefix| t.starts_with(prefix))
                        || legacy_data_alias(&cfg, approved_legacy_mounts, mount, p, t))
                    && destinations.insert(target),
                "mount",
                "Runtime mount redirected or conflicts with execution mounts",
                409,
            )?;
            argv.extend(strings(&[
                "--mount",
                &format!(
                    "type=bind,source={source},target={target}{}",
                    if mount["readonly"] == false {
                        ""
                    } else {
                        ",readonly"
                    }
                ),
            ]));
        }
        if gpu {
            let selection = format!("device={}", devices.join(","));
            // Docker parses --gpus as CSV, including when invoked without a shell.
            let selection = if devices.len() > 1 {
                format!("\"{selection}\"")
            } else {
                selection
            };
            let capabilities = runtime["driver_capabilities"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or("compute,utility".into());
            require(
                capabilities.split(',').all(|v| {
                    matches!(
                        v,
                        "compute"
                            | "utility"
                            | "graphics"
                            | "display"
                            | "video"
                            | "compat32"
                            | "ngx"
                    )
                }),
                "driver",
                "Invalid driver capabilities",
                400,
            )?;
            argv.extend(strings(&[
                "--gpus",
                &selection,
                "--env",
                &format!("NVIDIA_DRIVER_CAPABILITIES={capabilities}"),
            ]));
        }
        argv.extend(strings(&[
            "--entrypoint",
            &request.argv[0],
            runtime["image"]
                .as_str()
                .ok_or_else(|| Error::input("Runtime image required"))?,
        ]));
        argv.extend(request.argv.iter().skip(1).cloned());
        let launch = hash(compat::json(&json!(argv), false, true));
        let labels = BTreeMap::from([
            ("loom.instance", instance.to_string()),
            ("loom.job", request.job.clone()),
            ("loom.stage", request.stage.clone()),
            ("loom.ticket", ticket.clone()),
            ("loom.request-sha256", hash(&encoded)),
            ("loom.launch-sha256", launch),
        ]);
        let labelargs: Vec<String> = labels
            .iter()
            .flat_map(|(k, v)| ["--label".to_string(), format!("{k}={v}")])
            .collect();
        argv.splice(1..1, labelargs);
        // Intent precedes create/start. A lost reply never creates a second identity.
        let tx = self.db.unchecked_transaction()?;
        tx.execute("INSERT INTO tickets(id,instance,container,gpu,state,request,gpu_devices) VALUES(?,?,?,?,'creating',?,?)",params![ticket,instance,name,i64::from(gpu),encoded,gpu_devices])?;
        tx.execute(
            "INSERT INTO ticket_launches VALUES(?,?,?)",
            params![
                ticket,
                serde_json::to_string(&labels)?,
                serde_json::to_string(&argv)?
            ],
        )?;
        tx.commit()?;
        let created = self.docker.call(&argv);
        if created.is_err() {
            let inspect = self.docker.call(&strings(&["inspect", &name]))?;
            let container: Value = serde_json::from_str(&inspect)?;
            let container = &container[0];
            require(
                labels
                    .iter()
                    .all(|(k, v)| container["Config"]["Labels"][k] == *v)
                    && container["State"]["Status"] == "created",
                "container_identity",
                "Unconfirmed container identity; owner retained",
                409,
            )?;
        }
        let inspected: Value =
            serde_json::from_str(&self.docker.call(&strings(&["inspect", &name]))?)?;
        let image = inspected[0]["Image"].as_str().unwrap_or("");
        self.db.execute(
            "UPDATE tickets SET state='created',image_id=? WHERE id=?",
            params![image, ticket],
        )?;
        self.docker.call(&strings(&["start", &name]))?;
        let row = rows(&self.db, "SELECT * FROM tickets WHERE id=?", &[&ticket])?.remove(0);
        let state = self.observed(&row)?;
        Ok(
            json!({"ticket":ticket,"state":state["state"],"returned":state["returned"],"exit_code":state["exit_code"],"image_id":image}),
        )
    }
    fn observed(&self, row: &Value) -> Result<Value> {
        if row["state"] == "returned" && !row["exit_code"].is_null() {
            return Ok(
                json!({"state":"exited","returned":true,"exit_code":row["exit_code"],"image_id":row["image_id"]}),
            );
        }
        let inspect = match self.docker.call(&strings(&[
            "inspect",
            row["container"].as_str().unwrap_or(""),
        ])) {
            Ok(v) => v,
            Err(_) => return Ok(json!({"state":"unknown","returned":false})),
        };
        let data: Value = serde_json::from_str(&inspect)?;
        let labels: Option<String> = self
            .db
            .query_row(
                "SELECT labels FROM ticket_launches WHERE ticket=?",
                [row["id"].as_str()],
                |r| r.get(0),
            )
            .optional()?;
        let expected: BTreeMap<String, String> = if let Some(labels) = labels {
            serde_json::from_str(&labels)?
        } else {
            let request: Value = serde_json::from_str(row["request"].as_str().unwrap_or("{}"))?;
            BTreeMap::from([
                (
                    "loom.instance".into(),
                    row["instance"].as_str().unwrap_or("").into(),
                ),
                (
                    "loom.ticket".into(),
                    row["id"].as_str().unwrap_or("").into(),
                ),
                (
                    "loom.job".into(),
                    request["job"].as_str().unwrap_or("").into(),
                ),
                (
                    "loom.stage".into(),
                    request["stage"].as_str().unwrap_or("").into(),
                ),
                ("loom.request-sha256".into(), hash(compat::dump(&request))),
            ])
        };
        require(
            expected
                .iter()
                .all(|(k, v)| data[0]["Config"]["Labels"][k] == *v),
            "container_identity",
            "Container identity changed; resource owner retained",
            409,
        )?;
        let state = &data[0]["State"];
        let returned = matches!(state["Status"].as_str(), Some("exited" | "dead"));
        if returned {
            self.db.execute(
                "UPDATE tickets SET state='returned',exit_code=? WHERE id=?",
                params![state["ExitCode"].as_i64(), row["id"].as_str()],
            )?;
        }
        Ok(
            json!({"state":state["Status"],"returned":returned,"exit_code":if returned{state["ExitCode"].clone()}else{Value::Null},"image_id":row["image_id"]}),
        )
    }
    pub fn log_read(&mut self, instance: &str, args: &Value) -> Result<crate::logs::LogRead<D>> {
        let ticket = args["ticket"]
            .as_str()
            .ok_or_else(|| Error::input("Ticket required"))?;
        let row = rows(
            &self.db,
            "SELECT * FROM tickets WHERE id=? AND instance=?",
            &[&ticket, &instance],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("ticket", "Ticket outside instance grant", 404))?;
        let state = self.observed(&row)?;
        require(
            state["state"] != "unknown",
            "container_identity",
            "Cannot read unverified container logs",
            503,
        )?;
        Ok(crate::logs::LogRead {
            docker: self.docker.clone(),
            root: self.root.clone(),
            container: row["container"].as_str().unwrap_or("").into(),
            ticket: ticket.into(),
            returned: state["returned"] == true,
        })
    }
    fn ticket_action(&mut self, instance: &str, action: &str, args: Value) -> Result<Value> {
        let ticket = args["ticket"]
            .as_str()
            .ok_or_else(|| Error::input("Ticket required"))?;
        let row = rows(
            &self.db,
            "SELECT * FROM tickets WHERE id=? AND instance=?",
            &[&ticket, &instance],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("ticket", "Ticket outside instance grant", 404))?;
        if action == "cancel" {
            let state = self.observed(&row)?;
            require(
                state["state"] != "unknown",
                "container_identity",
                "Cannot cancel an unverified container identity",
                503,
            )?;
            if state["state"] == "created" {
                self.docker
                    .call(&strings(&["rm", row["container"].as_str().unwrap_or("")]))?;
                self.db.execute(
                    "UPDATE tickets SET state='returned',exit_code=137 WHERE id=?",
                    [&ticket],
                )?;
                return Ok(
                    json!({"state":"exited","returned":true,"exit_code":137,"not_started":true,"image_id":row["image_id"]}),
                );
            }
            if state["returned"] != true {
                self.docker.call(&strings(&[
                    "stop",
                    "--time",
                    "10",
                    row["container"].as_str().unwrap_or(""),
                ]))?;
            }
            return self.observed(&row);
        }
        if action == "status" {
            return self.observed(&row);
        }
        use std::io::{Read, Seek, SeekFrom};
        use std::os::unix::fs::DirBuilderExt;
        let directory = self.root.join("logs");
        if !directory.exists() {
            fs::DirBuilder::new().mode(0o700).create(&directory)?;
        }
        require(
            !directory.symlink_metadata()?.file_type().is_symlink(),
            "logs",
            "Log directory redirected",
            409,
        )?;
        let path = directory.join(format!("{}.log", hash(ticket)));
        let sealed = path.with_extension("complete");
        if row["state"] != "returned" || !sealed.is_file() {
            self.docker
                .capture_logs(row["container"].as_str().unwrap_or(""), &path)?;
            if row["state"] == "returned" {
                loom_primitives::storage::atomic(&sealed, b"returned", 0o600)?;
            }
        }
        let mut file = loom_primitives::storage::open_regular(&path)?;
        let total = file.metadata()?.len();
        let limit = args["limit"]
            .as_u64()
            .unwrap_or(65536)
            .clamp(1, 1024 * 1024);
        let offset = if action == "logs" {
            total.saturating_sub(1024 * 1024)
        } else {
            args["offset"].as_u64().unwrap_or(0).min(total)
        };
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![];
        file.take(if action == "logs" { 1024 * 1024 } else { limit })
            .read_to_end(&mut bytes)?;
        let end = offset + bytes.len() as u64;
        Ok(
            json!({"offset":offset,"next_offset":end,"text":String::from_utf8_lossy(&bytes),"data_base64":({use base64::Engine;base64::engine::general_purpose::STANDARD.encode(&bytes)}),"eof":end==total,"total_bytes":total,"truncated":action=="logs"&&offset>0}),
        )
    }
}
