//! Producer reports are presentation evidence, never scheduling or return authority.
use crate::{contracts, error::require, storage, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Read, path::Path};

fn read_report(folder: &Path, name: &str, limit: u64) -> Result<Value> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = storage::relative(folder, name)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    require(
        metadata.is_file() && metadata.len() <= limit,
        "progress",
        "Invalid producer progress file",
        400,
    )?;
    let mut bytes = vec![];
    file.take(limit + 1).read_to_end(&mut bytes)?;
    require(
        bytes.len() as u64 <= limit,
        "progress",
        "Progress report exceeds limit",
        400,
    )?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub(crate) fn forecast(folder: &Path, job: &str, attempt: &str, now: f64) -> Option<Value> {
    let report = read_report(folder, "work-progress.json", 4096).ok()?;
    if report["schema"] != "loom-work-progress/v1"
        || report["job"] != job
        || report["attempt"] != attempt
        || report["stage"] != "run"
    {
        return None;
    }
    let phase = report["phase"].as_str()?;
    let unit = report["unit"].as_str()?;
    let total = report["total"].as_u64()?;
    let completed = report["sample"]["completed"].as_u64()?;
    let recorded = report["sample"]["recorded"].as_f64()?;
    if !matches!(phase, "training" | "execution")
        || unit.trim().is_empty()
        || unit.chars().count() > 32
        || unit.chars().any(char::is_control)
        || completed >= total
        || !(now - 900.0..=now).contains(&recorded)
    {
        return None;
    }
    let mut estimate = json!({"status":"measuring","phase":phase,"progress":completed,
        "total":total,"progress_unit":unit,"as_of":recorded});
    let previous = &report["previous"];
    if previous.is_null() {
        return Some(estimate);
    }
    let elapsed = recorded - previous["recorded"].as_f64()?;
    let delta = completed.checked_sub(previous["completed"].as_u64()?)?;
    if elapsed <= 0.0 || delta == 0 {
        return None;
    }
    if !(30.0..=900.0).contains(&elapsed) {
        return Some(estimate);
    }
    let finish_at = recorded + (total - completed) as f64 * elapsed / delta as f64;
    if finish_at <= now || finish_at > 8.64e12 {
        return None;
    }
    estimate["status"] = json!("estimated");
    estimate["finish_at"] = json!(finish_at);
    estimate["remaining_s"] = json!(finish_at - now);
    Some(estimate)
}

pub fn validate_plan(plan: &Value) -> Result<()> {
    contracts::validate(
        &json!({"type":"array","maxItems":256,"items":{"type":"object","properties":{"id":{"type":"string","pattern":"^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,80}$"},"label":{"type":"string","minLength":1,"maxLength":200}},"required":["id","label"],"additionalProperties":false}}),
        plan,
    )?;
    let mut ids = std::collections::BTreeSet::new();
    require(
        plan.as_array()
            .unwrap()
            .iter()
            .all(|s| ids.insert(s["id"].as_str().unwrap())),
        "steps",
        "Repeated internal Step identity",
        400,
    )
}
pub fn read(folder: Option<&Path>, spec: &Value) -> Value {
    let plan = spec["profile"]["steps"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if plan.is_empty() {
        return Value::Null;
    }
    let mut result = json!({"schema":"loom-job-steps/v1","authority":"producer_reported; execution and publication remain authoritative","status":"planned","steps":plan.iter().map(|s|{let mut v=s.clone();v["state"]=json!("pending");v}).collect::<Vec<_>>()});
    let Some(folder) = folder else { return result };
    let load = (|| -> Result<Value> {
        let report = read_report(folder, "steps.json", 262144)?;
        require(
            report["schema"] == "loom-job-steps/v1"
                && report["profile_sha256"] == spec["profile"]["sha256"]
                && report["steps"].is_array(),
            "progress",
            "Progress identity differs from accepted Profile",
            409,
        )?;
        let mut reported = BTreeMap::new();
        for step in report["steps"].as_array().unwrap() {
            let id = step["id"].as_str().unwrap_or("");
            require(
                plan.iter().any(|s| s["id"] == id)
                    && !reported.contains_key(id)
                    && [
                        "pending",
                        "running",
                        "completed",
                        "reused",
                        "failed",
                        "skipped",
                    ]
                    .iter()
                    .any(|state| step["state"] == *state)
                    && step
                        .get("detail")
                        .is_none_or(|s| s.as_str().is_some_and(|s| s.len() <= 1000)),
                "progress",
                "Invalid internal Step report",
                400,
            )?;
            if step["state"] == "reused" {
                require(
                    spec["_consolidation_sources"].as_array().is_some_and(|a| {
                        a.iter().any(|s| {
                            s["job"] == step["source_job"] && s["attempt"] == step["source_attempt"]
                        })
                    }),
                    "progress",
                    "Reused Step must identify a frozen source execution",
                    409,
                )?;
            }
            let mut value = plan.iter().find(|s| s["id"] == id).unwrap().clone();
            for field in ["state", "detail", "source_job", "source_attempt"] {
                if let Some(v) = step.get(field) {
                    value[field] = v.clone();
                }
            }
            reported.insert(id.to_string(), value);
        }
        Ok(json!(plan
            .iter()
            .map(|s| reported
                .get(s["id"].as_str().unwrap_or(""))
                .cloned()
                .unwrap_or_else(|| {
                    let mut v = s.clone();
                    v["state"] = json!("pending");
                    v
                }))
            .collect::<Vec<_>>()))
    })();
    match load {
        Ok(steps) => {
            result["status"] = json!("reported");
            result["steps"] = steps;
        }
        Err(_) => {
            if folder.join("steps.json").symlink_metadata().is_ok() {
                result["status"] = json!("unavailable");
                result["reason"] = json!("Invalid or unreadable producer progress");
            }
        }
    }
    result
}
