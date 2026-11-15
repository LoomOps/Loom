//! Producer-side request/progress/output contract. No scheduler or credentials.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
static CANCELLED: AtomicBool = AtomicBool::new(false);
extern "C" fn cancelled(_: i32) {
    CANCELLED.store(true, Ordering::Relaxed);
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub job: String,
    #[serde(default)]
    pub attempt: String,
    #[serde(default)]
    pub stage: String,
    pub output_dir: PathBuf,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub profile: Value,
    #[serde(default)]
    pub inputs: Value,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}
impl Request {
    pub fn read(path: &Path) -> Result<Self> {
        let meta = fs::metadata(path)?;
        if meta.len() > 8 * 1024 * 1024 {
            return Err("Request exceeds bound".into());
        }
        let r: Self = serde_json::from_slice(&fs::read(path)?)?;
        if !r.output_dir.is_absolute() {
            return Err("Output directory must be absolute".into());
        }
        Ok(r)
    }
    pub fn tool_binary(&self, alias: &str) -> Option<&str> {
        self.extra.get("tool_binaries")?.get(alias)?.as_str()
    }
    pub fn from_args() -> Result<Self> {
        let path = std::env::args()
            .next_back()
            .filter(|_| std::env::args().len() > 1)
            .ok_or("Request file argument required")?;
        Self::read(Path::new(&path))
    }
}
fn inside(root: &Path, name: &str) -> Result<PathBuf> {
    let p = Path::new(name);
    if name.is_empty()
        || p.is_absolute()
        || !p.components().all(|p| matches!(p, Component::Normal(_)))
    {
        return Err("Invalid output locator".into());
    }
    let root = root.canonicalize()?;
    let mut out = root.clone();
    for part in p.components() {
        out.push(part);
        if out
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err("Output symlink denied".into());
        }
    }
    Ok(out)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("Missing parent")?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".producer-{}", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if tmp.exists() {
        let _ = fs::remove_file(tmp);
    }
    result
}
pub struct Producer {
    pub request: Request,
    sequence: u64,
    entities: Vec<Value>,
    metrics: Vec<Value>,
}
impl Producer {
    pub fn new(request: Request) -> Result<Self> {
        fs::create_dir_all(&request.output_dir)?;
        unsafe {
            libc::signal(libc::SIGTERM, cancelled as *const () as libc::sighandler_t);
            libc::signal(libc::SIGINT, cancelled as *const () as libc::sighandler_t);
        }
        Ok(Self {
            request,
            sequence: 0,
            entities: vec![],
            metrics: vec![],
        })
    }
    pub fn check_cancelled(&self) -> Result<()> {
        if CANCELLED.load(Ordering::Relaxed) {
            Err("Producer cancellation requested".into())
        } else {
            Ok(())
        }
    }
    pub fn progress(&mut self, step: &str, state: &str) -> Result<()> {
        self.check_cancelled()?;
        if !matches!(state, "pending" | "running" | "completed" | "failed") {
            return Err("Invalid progress state".into());
        }
        self.sequence += 1;
        if let Some(plan) = self
            .request
            .extra
            .get("steps")
            .and_then(|v| v.as_array().or_else(|| v["plan"].as_array()))
        {
            if !plan.iter().any(|s| s["id"] == step) {
                return Err("Progress Step is outside the accepted plan".into());
            }
            let file = inside(&self.request.output_dir, "steps.json")?;
            let mut report = if file.exists() {
                let bytes = fs::read(&file)?;
                if bytes.len() > 262144 {
                    return Err("Progress report exceeds bound".into());
                }
                serde_json::from_slice::<Value>(&bytes)?
            } else {
                json!({"schema":"loom-job-steps/v1","profile_sha256":self.request.profile["sha256"],"steps":[]})
            };
            if report["profile_sha256"] != self.request.profile["sha256"] {
                return Err("Progress belongs to another Profile".into());
            }
            let rows = report["steps"]
                .as_array_mut()
                .ok_or("Invalid progress rows")?;
            if let Some(row) = rows.iter_mut().find(|s| s["id"] == step) {
                if ["completed", "failed", "skipped", "reused"]
                    .iter()
                    .any(|state| row["state"] == *state)
                    && row["state"] != state
                {
                    return Err("Cannot regress a completed Step".into());
                }
                row["state"] = json!(state);
            } else {
                rows.push(json!({"id":step,"state":state}));
            }
            atomic(&file, &serde_json::to_vec(&report)?)?;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs_f64();
        let v = json!({"schema":"loom-progress/v1","job":self.request.job,"attempt":self.request.attempt,"stage":self.request.stage,"step":step,"sequence":self.sequence,"state":state,"recorded":now});
        atomic(
            &inside(&self.request.output_dir, "progress.json")?,
            &serde_json::to_vec(&v)?,
        )
    }
    pub fn write_artifact(&mut self, id: &str, name: &str, bytes: &[u8]) -> Result<()> {
        self.check_cancelled()?;
        if id.is_empty() || self.entities.iter().any(|v| v["id"] == id) {
            return Err("Unique artifact ID required".into());
        }
        let path = inside(&self.request.output_dir, name)?;
        atomic(&path, bytes)?;
        self.entities.push(json!({"id":id,"kind":"artifact","path":name,"provenance":{"profile":self.request.profile["revision"],"producer_sha256":self.request.profile["sha256"]},"relations":[]}));
        Ok(())
    }
    pub fn register_entity(&mut self, entity: Value) -> Result<()> {
        let path = entity["path"].as_str().ok_or("Entity path required")?;
        let p = inside(&self.request.output_dir, path)?;
        if !p.is_file() {
            return Err("Entity output unavailable".into());
        }
        if self.entities.iter().any(|e| e["id"] == entity["id"]) {
            return Err("Duplicate entity ID".into());
        }
        self.entities.push(entity);
        Ok(())
    }
    pub fn metric(&mut self, metric: Value) -> Result<()> {
        if !metric.is_object() {
            return Err("Metric object required".into());
        }
        self.metrics.push(metric);
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        self.check_cancelled()?;
        let manifest = json!({"entities":self.entities,"metrics":self.metrics});
        atomic(
            &inside(&self.request.output_dir, "result.json")?,
            &serde_json::to_vec(&manifest)?,
        )?;
        if self
            .request
            .extra
            .get("steps")
            .and_then(|v| v.as_array().or_else(|| v["plan"].as_array()))
            .is_none_or(|p| p.iter().any(|s| s["id"] == "run"))
        {
            self.progress("run", "completed")
        } else {
            Ok(())
        }
    }
    pub fn run_worker(&mut self, program: &str, args: &[&str]) -> Result<()> {
        self.check_cancelled()?;
        use std::os::unix::process::CommandExt;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .process_group(0)
            .spawn()?;
        let group = child.id() as i32;
        loop {
            if let Some(exit) = child.try_wait()? {
                let descendants = unsafe { libc::kill(-group, libc::SIGKILL) } == 0;
                if descendants {
                    return Err("External SDK worker left unreturned descendants".into());
                }
                if !exit.success() {
                    return Err("External SDK worker failed".into());
                }
                return Ok(());
            }
            if self.check_cancelled().is_err() {
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err("External SDK worker cancelled".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    pub fn source_hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
}
#[cfg(test)]
#[path = "../../../../tests/rust/sdk.rs"]
mod tests;
