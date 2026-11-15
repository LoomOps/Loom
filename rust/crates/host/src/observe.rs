//! Bounded observations are evidence with source/time, never execution-return authority.
use crate::executor::{Docker, DockerCli};
use loom_core::{store::now, Result};
use serde_json::{json, Value};
pub fn collect() -> Result<Value> {
    let cpu = std::fs::read_to_string("/proc/stat")?
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let memory = std::fs::read_to_string("/proc/meminfo")?
        .lines()
        .filter(|s| s.starts_with("MemTotal:") || s.starts_with("MemAvailable:"))
        .map(String::from)
        .collect::<Vec<_>>();
    let docker = DockerCli.call(&["ps".into(), "--format".into(), "{{json .}}".into()]);
    Ok(
        json!({"recorded":now(),"cpu_raw":cpu,"memory_raw":memory,"docker":match docker{Ok(lines)=>json!({"status":"available","containers":lines.lines().filter_map(|l|serde_json::from_str::<Value>(l).ok()).collect::<Vec<_>>()}),Err(_)=>json!({"status":"unavailable"})}}),
    )
}
