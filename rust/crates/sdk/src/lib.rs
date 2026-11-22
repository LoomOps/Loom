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
    execution_outcome: Option<Value>,
    work_sample: Option<WorkSample>,
}

struct WorkSample {
    phase: String,
    unit: String,
    total: u64,
    completed: u64,
    recorded: f64,
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
            execution_outcome: None,
            work_sample: None,
        })
    }
    pub fn check_cancelled(&self) -> Result<()> {
        if CANCELLED.load(Ordering::Relaxed) {
            Err("Producer cancellation requested".into())
        } else {
            Ok(())
        }
    }
    /// Report a cumulative counter for the current run. Use one reporter.
    /// Samples are emitted at most once every 30 seconds until completion.
    pub fn report_work_progress(
        &mut self,
        phase: &str,
        completed: u64,
        total: u64,
        unit: &str,
    ) -> Result<()> {
        self.check_cancelled()?;
        if self.request.stage != "run"
            || self.request.job.is_empty()
            || self.request.attempt.is_empty()
            || !matches!(phase, "training" | "execution")
            || total == 0
            || completed > total
            || unit.trim().is_empty()
            || unit.chars().count() > 32
            || unit.chars().any(char::is_control)
        {
            return Err("Invalid work progress counter".into());
        }
        let recorded = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs_f64();
        let previous = self.work_sample.as_ref().filter(|sample| {
            sample.phase == phase
                && sample.unit == unit
                && sample.total == total
                && sample.completed <= completed
                && sample.recorded < recorded
        });
        if completed < total && previous.is_some_and(|v| recorded - v.recorded < 30.0) {
            return Ok(());
        }
        let report = json!({"schema":"loom-work-progress/v1","job":self.request.job,
            "attempt":self.request.attempt,"stage":self.request.stage,"phase":phase,"unit":unit,"total":total,
            "sample":{"completed":completed,"recorded":recorded},
            "previous":previous.map(|v|json!({"completed":v.completed,"recorded":v.recorded}))});
        atomic(
            &inside(&self.request.output_dir, "work-progress.json")?,
            &serde_json::to_vec(&report)?,
        )?;
        self.work_sample = Some(WorkSample {
            phase: phase.into(),
            unit: unit.into(),
            total,
            completed,
            recorded,
        });
        Ok(())
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
    /// Label a pending output without changing its ID, file or accepted provenance.
    pub fn set_entity_label(&mut self, id: &str, label: &str) -> Result<()> {
        let label = label.trim();
        if label.is_empty() || label.chars().count() > 200 || label.chars().any(char::is_control) {
            return Err("Output label must be 1–200 printable characters".into());
        }
        let entity = self
            .entities
            .iter_mut()
            .find(|e| e["id"] == id)
            .ok_or("Pending output not found")?;
        entity["label"] = json!(label);
        Ok(())
    }
    /// Write a hash-bound analysis report for this output's frozen coverage contract.
    pub fn report(&mut self, id: &str, requirement_id: &str, mut data: Value) -> Result<()> {
        let entity = self
            .entities
            .iter_mut()
            .find(|e| e["id"] == id)
            .ok_or("Pending output not found")?;
        let pin = self.request.profile["reporting_pins"]
            .get(format!("output:{id}").as_str())
            .or_else(|| {
                self.request.profile["reporting_pins"].get(entity["kind"].as_str().unwrap_or(""))
            })
            .ok_or("No frozen reporting contract")?;
        let requirements = pin["definition"]["effective_requirements"]
            .as_array()
            .or_else(|| pin["definition"]["requirements"].as_array())
            .ok_or("No resolved coverage")?;
        if !requirements.iter().any(|r| r["id"] == requirement_id) {
            return Err("Unknown or excluded report requirement".into());
        }
        if !requirement_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err("Invalid report ID".into());
        }
        let contract_ref =
            json!({"id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"]});
        if data.get("inputs").is_some() {
            data["inputs"] = self
                .request
                .extra
                .get("report_inputs")
                .cloned()
                .unwrap_or(json!([]));
        }
        let report = json!({"schema":"loom-report/v1","contract_ref":contract_ref,"requirement_id":requirement_id,"producer":{"job":self.request.job,"attempt":self.request.attempt,"profile_sha256":self.request.profile["sha256"]},"data":data});
        let relative = Path::new(entity["path"].as_str().ok_or("Output path missing")?)
            .parent()
            .unwrap_or(Path::new(""));
        let name = format!("report-{id}-{requirement_id}.json");
        let path = inside(
            &self.request.output_dir,
            relative.join(&name).to_str().ok_or("Invalid report path")?,
        )?;
        let bytes = serde_json::to_vec_pretty(&report)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("Report exceeds 8 MiB".into());
        }
        if entity.get("reporting").is_none() {
            entity["reporting"] = json!({"contract_ref":contract_ref,"reports":[]});
        }
        let reports = entity["reporting"]["reports"]
            .as_array_mut()
            .ok_or("Invalid report bindings")?;
        if reports
            .iter()
            .any(|r| r["requirement_id"] == requirement_id)
        {
            return Err("Report requirement already bound".into());
        }
        atomic(&path, &bytes)?;
        reports.push(json!({"requirement_id":requirement_id,"component":name,"sha256":format!("{:x}",Sha256::digest(&bytes))}));
        if entity.get("components").is_none() {
            entity["components"] = json!([]);
        }
        entity["components"]
            .as_array_mut()
            .ok_or("Invalid output members")?
            .push(json!(name));
        Ok(())
    }
    /// Set a template-backed research object; the server validates the frozen Profile.
    pub fn set_entity_metadata(
        &mut self,
        id: &str,
        template_ref: Value,
        research: Value,
    ) -> Result<()> {
        if !research.is_object()
            || serde_json::to_vec(&research)?.len() > 128 * 1024
            || template_ref["id"].as_str().is_none_or(str::is_empty)
            || template_ref["revision"].as_u64().is_none_or(|r| r == 0)
            || !template_ref["sha256"]
                .as_str()
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(
                "An exact metadata template pin and bounded research object are required".into(),
            );
        }
        let entity = self
            .entities
            .iter_mut()
            .find(|e| e["id"] == id)
            .ok_or("Pending output not found")?;
        if !entity["metadata"].is_object() {
            entity["metadata"] = json!({});
        }
        entity["metadata"]["template_ref"] = template_ref;
        entity["metadata"]["research"] = research;
        Ok(())
    }
    /// Declare exactly which accepted Dataset input contributes data to an output.
    /// Other consumed artifacts stay execution provenance, not Dataset parents.
    pub fn derive_dataset(&mut self, id: &str, input: &str, transformation: Value) -> Result<()> {
        let entity = self
            .entities
            .iter_mut()
            .find(|e| e["id"] == id)
            .ok_or("Pending Dataset not found")?;
        if entity["kind"] != "dataset" || input.is_empty() || !transformation.is_object() {
            return Err("Dataset input and typed transformation required".into());
        }
        if entity["lineage"].is_null() {
            entity["lineage"] = json!([]);
        }
        let parents = entity["lineage"]
            .as_array_mut()
            .ok_or("Invalid Dataset lineage")?;
        if parents.iter().any(|p| p["input"] == input) {
            return Err("Dataset input already declared".into());
        }
        parents.push(json!({"input":input,"transformation":transformation}));
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
    /// Members must be sorted unique owner-relative paths, including the primary.
    /// Chunked output is independent of core Item/File IDs and does not load a tree.
    pub fn register_inventory(
        &mut self,
        id: &str,
        kind: &str,
        primary: &str,
        members: impl Iterator<Item = Result<String>>,
        metadata: Value,
    ) -> Result<()> {
        use std::io::Read;
        self.check_cancelled()?;
        if !matches!(kind, "dataset" | "model" | "experiment" | "artifact")
            || id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err("Invalid output identity/kind".into());
        }
        let primary_file = inside(&self.request.output_dir, primary)?;
        let base = primary_file.parent().ok_or("Primary directory missing")?;
        let primary_name = primary_file
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid primary")?;
        let folder = format!(".loom-inventory-{id}");
        let mut chunk = vec![];
        let mut chunks = vec![];
        let mut count = 0u64;
        let mut chunk_count = 0u64;
        let mut previous = String::new();
        let mut saw_primary = false;
        let flush =
            |chunk: &mut Vec<u8>, chunk_count: &mut u64, chunks: &mut Vec<Value>| -> Result<()> {
                if chunk.is_empty() {
                    return Ok(());
                }
                let reference = format!("{folder}/{:06}.jsonl", chunks.len());
                atomic(&inside(base, &reference)?, chunk)?;
                chunks.push(
                json!({"ref":reference,"sha256":Self::source_hash(chunk),"entries":*chunk_count}),
            );
                chunk.clear();
                *chunk_count = 0;
                Ok(())
            };
        for member in members {
            self.check_cancelled()?;
            let member = member?;
            if member <= previous {
                return Err("Inventory paths must be sorted and unique".into());
            }
            previous = member.clone();
            let path = inside(base, &member)?;
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
            let mut file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&path)?;
            let before = file.metadata()?;
            if !before.is_file() {
                return Err("Inventory member must be a regular file".into());
            }
            let mut sha = Sha256::new();
            let mut buffer = [0u8; 65536];
            let mut size = 0u64;
            loop {
                self.check_cancelled()?;
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                sha.update(&buffer[..n]);
                size += n as u64;
            }
            let after = path.symlink_metadata()?;
            if before.len() != size
                || before.dev() != after.dev()
                || before.ino() != after.ino()
                || before.mtime() != after.mtime()
                || before.mtime_nsec() != after.mtime_nsec()
                || before.ctime() != after.ctime()
                || before.ctime_nsec() != after.ctime_nsec()
            {
                return Err("Inventory member changed while hashing".into());
            }
            let bytes = serde_json::to_vec(
                &json!({"key":member,"path":member,"size":size,"sha256":format!("{:x}",sha.finalize()),"role":if member==primary_name{"primary"}else{"member"}}),
            )?;
            if bytes.len() > 65536 {
                return Err("Inventory entry exceeds bound".into());
            }
            if chunk.len() + bytes.len() + 1 > 524288 {
                flush(&mut chunk, &mut chunk_count, &mut chunks)?;
            }
            chunk.extend(bytes);
            chunk.push(b'\n');
            count += 1;
            chunk_count += 1;
            saw_primary |= member == primary_name;
        }
        flush(&mut chunk, &mut chunk_count, &mut chunks)?;
        if !saw_primary {
            return Err("Inventory must include its exact primary filename".into());
        }
        self.register_entity(json!({"id":id,"kind":kind,"path":primary,"metadata":metadata,"provenance":{"profile":self.request.profile["revision"],"producer_sha256":self.request.profile["sha256"]},"relations":[],"inventory":{"schema":"loom-output-inventory/v1","entries":count,"chunks":chunks}}))
    }
    pub fn metric(&mut self, metric: Value) -> Result<()> {
        if !metric.is_object() {
            return Err("Metric object required".into());
        }
        self.metrics.push(metric);
        Ok(())
    }
    /// Report a technical failure while retaining the registered diagnostic artifacts.
    pub fn fail_with_diagnostics(
        &mut self,
        code: &str,
        planned: u64,
        started: u64,
        valid: u64,
    ) -> Result<()> {
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            || planned == 0
            || started > planned
            || valid > started
        {
            return Err("Invalid technical failure code or case counts".into());
        }
        self.execution_outcome = Some(json!({
            "status":"failed","code":code,"planned":planned,"started":started,"valid":valid,
        }));
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        self.check_cancelled()?;
        let mut manifest = json!({"entities":self.entities,"metrics":self.metrics});
        if let Some(outcome) = &self.execution_outcome {
            manifest["execution_outcome"] = outcome.clone();
        }
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
            let state = if self.execution_outcome.is_some() {
                "failed"
            } else {
                "completed"
            };
            self.progress("run", state)
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
