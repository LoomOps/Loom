//! Producer reports are presentation evidence, never scheduling or return authority.
use crate::{contracts, error::require, storage, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Read, path::Path};
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
        let path = storage::relative(folder, "steps.json")?;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        require(
            file.metadata()?.is_file() && file.metadata()?.len() <= 262144,
            "progress",
            "Invalid producer progress file",
            400,
        )?;
        let mut bytes = vec![];
        file.by_ref().take(262145).read_to_end(&mut bytes)?;
        require(
            bytes.len() <= 262144,
            "progress",
            "Producer progress exceeds limit",
            400,
        )?;
        let report: Value = serde_json::from_slice(&bytes)?;
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
