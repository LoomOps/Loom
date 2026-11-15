//! Bounded release metadata observer. It cannot download, migrate or deploy a release.
use loom_core::{error::require, storage, store::now, Error, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[derive(Clone)]
pub struct Policy {
    pub repository: String,
    pub installed: String,
    pub channel: String,
    pub interval: f64,
    pub enabled: bool,
}
pub fn version(input: &str) -> Option<(u64, u64, u64, u8, u64)> {
    if let Some((_, metadata)) = input.split_once('+') {
        if metadata.is_empty()
            || !metadata
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
        {
            return None;
        }
    }
    let input = input.strip_prefix('v').unwrap_or(input).split('+').next()?;
    let (base, pre) = input
        .split_once('-')
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((input, None));
    let mut fields = base.split('.');
    let mut number = || {
        let s = fields.next()?;
        if s.len() > 1 && s.starts_with('0') {
            return None;
        }
        s.parse::<u64>().ok()
    };
    let a = number()?;
    let b = number()?;
    let c = number()?;
    if fields.next().is_some() {
        return None;
    }
    let (stage, n) = if let Some(pre) = pre {
        let (s, n) = pre.split_once('.')?;
        if n.len() > 1 && n.starts_with('0') {
            return None;
        }
        (
            match s {
                "alpha" => 0,
                "beta" => 1,
                "rc" => 2,
                _ => return None,
            },
            n.parse().ok()?,
        )
    } else {
        (3, 0)
    };
    Some((a, b, c, stage, n))
}
impl Policy {
    pub fn read(installed: String) -> Result<Self> {
        let repository = std::env::var("LOOM_RELEASE_REPOSITORY").unwrap_or("LoomOps/Loom".into());
        let names = repository.split('/').collect::<Vec<_>>();
        require(
            names.len() == 2
                && names.iter().all(|s| {
                    !s.is_empty()
                        && s.len() <= 100
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                        && s.as_bytes()[0].is_ascii_alphanumeric()
                }),
            "release_config",
            "Invalid release repository",
            400,
        )?;
        let channel = std::env::var("LOOM_RELEASE_CHANNEL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or(
                if version(&installed).is_some_and(|v| v.3 == 3) {
                    "stable"
                } else {
                    "alpha"
                }
                .into(),
            );
        require(
            ["alpha", "beta", "rc", "stable"].contains(&channel.as_str()),
            "release_config",
            "Invalid release channel",
            400,
        )?;
        let interval = std::env::var("LOOM_RELEASE_INTERVAL")
            .unwrap_or("21600".into())
            .parse::<u64>()
            .map_err(|_| Error::input("Invalid release interval"))?;
        require(
            (300..=604800).contains(&interval),
            "release_config",
            "Release interval outside limits",
            400,
        )?;
        let enabled = std::env::var("LOOM_RELEASE_CHECK_ENABLED").unwrap_or("1".into());
        require(
            ["0", "1"].contains(&enabled.as_str()),
            "release_config",
            "Invalid release check flag",
            400,
        )?;
        Ok(Self {
            repository,
            installed: installed.clone(),
            channel,
            interval: interval as f64,
            enabled: enabled == "1" && version(&installed).is_some(),
        })
    }
}
pub fn select(rows: &[Value], policy: &Policy) -> Option<Value> {
    let rank = match policy.channel.as_str() {
        "stable" => 3,
        "rc" => 2,
        "beta" => 1,
        _ => 0,
    };
    rows.iter().filter_map(|r|{let tag=r["tag_name"].as_str()?;let v=version(tag)?;if r["draft"]==true||!r["published_at"].is_string()||v.3<rank||rank==3&&r["prerelease"]==true{return None}let mut release=json!({"tag":tag,"url":format!("https://github.com/{}/releases/tag/{tag}",policy.repository),"published_at":r["published_at"]});if r["assets"].as_array().into_iter().flatten().any(|a|a["name"]=="upgrade.json"){release["upgrade"]=json!({"descriptor_url":format!("https://github.com/{}/releases/download/{tag}/upgrade.json",policy.repository),"instructions_url":format!("https://github.com/{}/blob/{tag}/docs/UPGRADE.md",policy.repository)});}Some((v,release))}).max_by_key(|(v,_)|*v).map(|(_,r)|r)
}
fn fetch(
    client: &reqwest::blocking::Client,
    policy: &Policy,
    pages: &BTreeMap<String, Value>,
    retry: &mut f64,
    stopping: &AtomicBool,
) -> Result<(Vec<Value>, BTreeMap<String, Value>)> {
    let path = Path::new("/etc/loom/releases/github-token");
    let token = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    let token = token.trim();
    require(
        token.len() <= 4096 && !token.chars().any(char::is_whitespace),
        "credential_invalid",
        "Invalid release credential",
        400,
    )?;
    let mut all = vec![];
    let mut saved = BTreeMap::new();
    for page in 1..=10 {
        require(
            !stopping.load(Ordering::Acquire),
            "interrupted",
            "Release observer stopping",
            503,
        )?;
        let key = page.to_string();
        let previous = pages.get(&key);
        let mut request = client
            .get(format!(
                "https://api.github.com/repos/{}/releases?per_page=100&page={page}",
                policy.repository
            ))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if !token.is_empty() {
            request = request.bearer_auth(token)
        }
        if let Some(tag) = previous.and_then(|p| p["etag"].as_str()) {
            request = request.header("If-None-Match", tag)
        }
        let mut response = request
            .send()
            .map_err(|_| Error::new("network_unavailable", "Release metadata unavailable", 503))?;
        let item = if response.status().as_u16() == 304 {
            previous
                .cloned()
                .ok_or_else(|| Error::input("Unmatched release cache response"))?
        } else {
            if !response.status().is_success() {
                let after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|s| s.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|s| now() + s)
                    .unwrap_or(0.);
                let reset = response
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|s| s.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.);
                *retry = after.max(reset);
            }
            require(
                response.status().is_success(),
                if response.status().as_u16() == 403 || response.status().as_u16() == 429 {
                    "rate_limited"
                } else {
                    "upstream_unavailable"
                },
                "Release metadata unavailable",
                503,
            )?;
            let etag = response
                .headers()
                .get("etag")
                .and_then(|h| h.to_str().ok())
                .map(String::from);
            let next = response
                .headers()
                .get("link")
                .and_then(|h| h.to_str().ok())
                .is_some_and(|s| s.contains("rel=\"next\""));
            let mut bytes = vec![];
            response
                .by_ref()
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            require(
                bytes.len() <= 4 * 1024 * 1024,
                "release_bound",
                "Release page exceeds limit",
                503,
            )?;
            let rows: Value = serde_json::from_slice(&bytes)?;
            require(
                rows.is_array(),
                "release_shape",
                "Invalid release list",
                503,
            )?;
            json!({"etag":etag,"next":next,"rows":rows})
        };
        let next = item["next"] == true;
        all.extend(
            item["rows"]
                .as_array()
                .ok_or_else(|| Error::input("Invalid cached release rows"))?
                .clone(),
        );
        saved.insert(key, item);
        if !next {
            return Ok((all, saved));
        }
    }
    Err(Error::new(
        "release_list_incomplete",
        "Release metadata pagination limit reached",
        503,
    ))
}
pub async fn run(state: PathBuf, policy: Policy, stopping: Arc<AtomicBool>) {
    let result = tokio::task::spawn_blocking(move || observe(state, policy, stopping)).await;
    if result.is_err() {
        eprintln!("Loom release observer stopped");
    }
}
fn observe(state: PathBuf, policy: Policy, stopping: Arc<AtomicBool>) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Loom-release-check")
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| Error::new("release_client", "Release TLS client unavailable", 503))?;
    let cache = state.join("release-check.json");
    let saved = std::fs::read(&cache)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(json!({}));
    let matching = saved["repository"] == policy.repository && saved["channel"] == policy.channel;
    let mut pages = if matching {
        saved["pages"]
            .as_object()
            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let mut status = if matching {
        saved.get("state").cloned().unwrap_or(json!({}))
    } else {
        json!({})
    };
    let mut next = status["next_check_at"].as_f64().unwrap_or(0.);
    let mut failures = 0;
    let mut last_request = 0.;
    loop {
        if stopping.load(Ordering::Acquire) {
            return Ok(());
        }
        let request = std::fs::read(state.join("release-check-request.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["requested_at"].as_f64())
            .unwrap_or(0.);
        let requested =
            request > last_request && now() >= status["checked_at"].as_f64().unwrap_or(0.) + 60.;
        if policy.enabled && (now() >= next || requested) {
            last_request = request;
            let time = now();
            let mut retry = 0.;
            match fetch(&client, &policy, &pages, &mut retry, &stopping) {
                Ok((rows, new_pages)) => {
                    pages = new_pages;
                    status["latest"] = select(&rows, &policy).unwrap_or(Value::Null);
                    status["error"] = Value::Null;
                    status["last_success_at"] = json!(time);
                    failures = 0;
                    next = time + policy.interval;
                }
                Err(error) => {
                    failures += 1;
                    status["error"] = json!(error.code);
                    next = (time + (60. * 2f64.powi(failures.min(6))).min(3600.)).max(retry);
                }
            }
            status["retry_at"] = json!(if status["error"].is_string() {
                next
            } else {
                time + 60.
            });
            status["checked_at"] = json!(time);
            status["next_check_at"] = json!(next);
            storage::atomic(
                &cache,
                &serde_json::to_vec(
                    &json!({"repository":policy.repository,"channel":policy.channel,"state":status,"pages":pages}),
                )?,
                0o600,
            )?;
        }
        let available = policy.enabled
            && status["latest"]["tag"]
                .as_str()
                .and_then(version)
                .zip(version(&policy.installed))
                .is_some_and(|(new, old)| new > old);
        let mut public = status.clone();
        public.as_object_mut().unwrap().extend(json!({"schema":"loom-release-check/v1","installed":policy.installed,"repository":policy.repository,"channel":policy.channel,"enabled":policy.enabled,"interval":policy.interval,"update_available":available,"status":if !policy.enabled{"disabled"}else if status["error"].is_string(){"error"}else if !status["checked_at"].is_number(){"not_checked"}else if available{"update_available"}else{"up_to_date"},"checking":false,"automatic_install":false,"stale":status["last_success_at"].is_number()&&(status["error"].is_string()||now()>next)}).as_object().unwrap().clone());
        storage::atomic(
            &state.join("release-status.json"),
            &serde_json::to_vec(&public)?,
            0o600,
        )?;
        if !policy.enabled {
            return Ok(());
        }
        for _ in 0..10 {
            if stopping.load(Ordering::Acquire) {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
#[cfg(test)]
#[path = "../../../../tests/rust/updates.rs"]
mod tests;
