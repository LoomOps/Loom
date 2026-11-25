//! Grant-scoped host observations. A cached observation never rewrites an accepted pin.
use crate::{compat, error::require, store::*, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
pub const STAGES: [&str; 6] = [
    "prepare",
    "runtime",
    "finalize",
    "on_complete",
    "on_error",
    "on_cancel",
];
pub(crate) fn profile_runtime_refs(profile: &Value) -> Vec<Value> {
    STAGES.iter().filter_map(|stage| {
        let pin = &profile[stage]["runtime_pin"];
        pin["id"].as_str()?;
        Some(json!({"stage":stage,"id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"]}))
    }).collect()
}
pub use loom_primitives::runtime::primary;
fn normalized(name: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for c in name.trim().to_lowercase().chars() {
        if matches!(c, '-' | '_' | '.') {
            if !separator {
                out.push('-');
            }
            separator = true;
        } else {
            out.push(c);
            separator = false;
        }
    }
    out
}
fn category(name: &str) -> Option<&'static str> {
    match name {
        "python" | "isaac-python" | "pypy" | "node" | "nodejs" | "java" | "openjdk" | "rust"
        | "go" | "julia" | "r" => Some("languages"),
        "isaacsim" | "isaac-sim" | "isaaclab" | "isaac-lab" | "mujoco" | "pybullet" | "gazebo"
        | "genesis" => Some("simulation"),
        "torch" | "pytorch" | "tensorflow" | "jax" | "jaxlib" | "lerobot" | "transformers"
        | "onnxruntime" | "onnxruntime-gpu" => Some("machine-learning"),
        "cuda" | "cuda-toolkit" | "cudnn" | "tensorrt" | "nvidia driver" | "nvidia-driver"
        | "cupy" => Some("gpu"),
        _ => None,
    }
}
pub fn dependencies(catalog: &Value) -> Value {
    let all = catalog["runtimes"].as_array().cloned().unwrap_or_default();
    let mut units: BTreeMap<String, Value> = BTreeMap::new();
    for item in &all {
        if item["curation"]["state"] == "archived"
            || all.iter().any(|other| {
                other["id"] == item["id"] && other["revision"].as_i64() > item["revision"].as_i64()
            })
        {
            continue;
        }
        for dep in item["manifest"]["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let (Some(name), Some(version)) = (dep["name"].as_str(), dep["version"].as_str())
            else {
                continue;
            };
            if name.is_empty()
                || version.is_empty()
                || dep["catalog"] == false
                || dep["deprecated"] == true
            {
                continue;
            }
            let name = normalized(name);
            let class = category(&name);
            let selected = item["primary_dependencies"]
                .as_array()
                .is_some_and(|a| a.iter().any(|d| d["name"] == dep["name"]));
            if class.is_none()
                && dep["catalog"] != true
                && !selected
                && dep.get("category").is_none()
            {
                continue;
            }
            let id = format!("dependency-{}", &hash(&name)[..24]);
            let unit=units.entry(id.clone()).or_insert_with(||json!({"id":id,"name":name,"label":dep["name"],"category":dep["category"].as_str().or(class).unwrap_or("libraries"),"versions":[],"runtimes":[]}));
            let runtime = json!({"id":item["id"],"revision":item["revision"],"entity":format!("runtime:{}:{}",item["id"].as_str().unwrap_or(""),item["revision"]),"label":item["manifest"].get("label").unwrap_or(&item["id"]),"current":true,"scope":dep["scope"].as_str().unwrap_or("")});
            if !unit["runtimes"].as_array().unwrap().contains(&runtime) {
                unit["runtimes"]
                    .as_array_mut()
                    .unwrap()
                    .push(runtime.clone());
            }
            let versions = unit["versions"].as_array_mut().unwrap();
            let index = versions
                .iter()
                .position(|v| v["version"] == version)
                .unwrap_or_else(|| {
                    versions.push(json!({"version":version,"installations":[]}));
                    versions.len() - 1
                });
            let origin = dep.get("origin").cloned().unwrap_or_else(|| {
                if dep["scope"] == "image" {
                    json!({"type":"image","image":item["manifest"]["runtime"]["image"]})
                } else {
                    json!({"type":"unrecorded"})
                }
            });
            let installs = versions[index]["installations"].as_array_mut().unwrap();
            let n = installs
                .iter()
                .position(|i| i["origin"] == origin)
                .unwrap_or_else(|| {
                    installs.push(json!({"origin":origin,"runtimes":[]}));
                    installs.len() - 1
                });
            if !installs[n]["runtimes"]
                .as_array()
                .unwrap()
                .contains(&runtime)
            {
                installs[n]["runtimes"]
                    .as_array_mut()
                    .unwrap()
                    .push(runtime);
            }
        }
    }
    let deps: Vec<_> = units.into_values().collect();
    let categories:Vec<_>=[("languages","Languages"),("simulation","Simulation"),("machine-learning","Machine Learning"),("gpu","GPU & Drivers"),("libraries","Libraries & Tools")].into_iter().map(|(id,label)|json!({"id":id,"label":label,"count":deps.iter().filter(|d|d["category"]==id).count()})).collect();
    json!({"status":catalog["status"],"dependencies":deps,"categories":categories,"basis":"declared software units; membership does not prove execution use"})
}
// Match actual stage receipts to the exact submitted Runtime in both the
// inventory counters and detailed usage pages.
const FROZEN_RECEIPT_SPEC: &str = r#"COALESCE(
                            h.spec_snapshot,
                            CASE WHEN NOT EXISTS(SELECT 1 FROM job_revisions WHERE job=s.job AND revision>0)
                                 THEN j.spec
                                 ELSE COALESCE(
                                     (SELECT json_extract(e.request,'$.spec') FROM rust_effects e
                                      WHERE e.job=s.job AND e.attempt=s.attempt AND e.stage=s.stage
                                      ORDER BY e.created DESC LIMIT 1),
                                     (SELECT r.spec FROM job_revisions r WHERE r.job=s.job AND r.created<=a.started
                                      ORDER BY r.revision DESC LIMIT 1)) END
                        )"#;

impl Application {
    pub fn observe_runtimes(&mut self, observation: Value) -> Result<()> {
        require(
            observation["runtimes"].is_array(),
            "runtime",
            "Invalid host Runtime observation",
            503,
        )?;
        for item in observation["runtimes"].as_array().unwrap() {
            if item["legacy"] == true {
                continue;
            }
            require(
                item["id"].is_string()
                    && item["revision"].as_i64().is_some_and(|n| n > 0)
                    && item["sha256"] == hash(compat::seal(&item["manifest"])),
                "runtime",
                "Host Runtime observation failed integrity check",
                503,
            )?;
        }
        let tx = self.transaction()?;
        tx.execute("INSERT INTO rust_runtime_observation VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value,observed=excluded.observed",params![encode(&observation)?,now()])?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn runtime_snapshot(&self) -> Result<Value> {
        let observation: Option<(String, f64)> = self
            .db
            .query_row(
                "SELECT value,observed FROM rust_runtime_observation WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(if let Some((value, observed)) = observation {
            let mut v: Value = serde_json::from_str(&value)?;
            v["observed_at"] = json!(observed);
            v["status"] = json!(if now() - observed > 30. {
                "stale"
            } else {
                "ok"
            });
            v
        } else {
            json!({"status":"unavailable","runtimes":[]})
        })
    }

    pub fn runtime_inventory(&self) -> Result<Value> {
        self.runtime_projection(None, true)
    }

    pub(crate) fn runtime_projection(
        &self,
        selected: Option<&[(String, i64)]>,
        include_usage: bool,
    ) -> Result<Value> {
        let mut catalog = self.runtime_snapshot()?;
        for item in catalog["runtimes"].as_array_mut().unwrap() {
            let selected = selected.is_none_or(|pins| {
                pins.iter().any(|(id, revision)| {
                    item["id"] == id.as_str() && item["revision"] == *revision
                })
            });
            if include_usage && selected {
                self.attach_runtime_usage(item)?;
            }
            let curated = self.curation(
                "runtime",
                item["id"].as_str().unwrap_or(""),
                item["revision"].as_i64().unwrap_or(0),
            )?;
            item["primary_dependencies"] = json!(primary(
                &item["manifest"],
                curated.get("primary_dependencies")
            )?);
            item["curation"] = curated;
            if self.owner_projection("runtime", "host", item["id"].as_str().unwrap_or(""))?
                ["owner_kind"]
                == "system"
            {
                item["owner"] = json!("system");
                item["owner_kind"] = json!("system");
                item["owner_profile"] = crate::system_ownership::person();
            }
        }
        catalog["dependency_catalog"] = dependencies(&catalog);
        Ok(catalog)
    }
    pub(crate) fn runtime_started_counts(&self, runtimes: &[Value]) -> Result<Vec<Value>> {
        let pins = runtimes
            .iter()
            .filter(|r| r["sha256"].is_string())
            .map(|r| json!({"id":r["id"],"revision":r["revision"],"sha256":r["sha256"]}))
            .collect::<Vec<_>>();
        if pins.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!("WITH starts AS MATERIALIZED (
            SELECT s.job,s.attempt,s.stage,CASE s.stage WHEN 'run' THEN 'runtime' ELSE s.stage END runtime_stage,
                {FROZEN_RECEIPT_SPEC} spec
            FROM stage_runs s JOIN jobs j ON j.id=s.job
            LEFT JOIN attempts a ON a.id=s.attempt AND a.job=s.job
            LEFT JOIN hooks h ON h.job=s.job AND h.attempt=s.attempt AND h.stage=s.stage
            WHERE s.started IS NOT NULL
        ), bound AS (
            SELECT job,json_extract(spec,'$.profile.'||runtime_stage||'.runtime_pin') pin FROM starts
        )
        SELECT json_extract(w.value,'$.id') id,json_extract(w.value,'$.revision') revision,
            count(DISTINCT COALESCE(m.root,b.job)) started
        FROM bound b JOIN json_each(?) w ON
            json_extract(b.pin,'$.id')=json_extract(w.value,'$.id')
            AND json_extract(b.pin,'$.revision')=json_extract(w.value,'$.revision')
            AND json_extract(b.pin,'$.sha256')=json_extract(w.value,'$.sha256')
        LEFT JOIN execution_memberships m ON m.job=b.job
        GROUP BY w.value");
        rows(&self.db, &sql, &[&encode(&json!(pins))?])
    }

    fn attach_runtime_usage(&self, item: &mut Value) -> Result<()> {
        self.attach_runtime_usage_page(item, 0, 200)
    }
    pub(crate) fn attach_runtime_usage_page(
        &self,
        item: &mut Value,
        offset: usize,
        limit: usize,
    ) -> Result<()> {
        // Frozen submissions, not current Profile definitions or UI scope,
        // are the authority for an environment's actual consumers.
        let consumers = rows(&self.db,
            "WITH versions(job,version,spec) AS (
                SELECT id, NULL, spec FROM jobs UNION ALL
                SELECT r.job,r.revision,r.spec FROM job_revisions r
                WHERE EXISTS(SELECT 1 FROM job_revisions later WHERE later.job=r.job AND later.revision>r.revision)
             ), matches AS (
                SELECT v.*,row_number() OVER(PARTITION BY job ORDER BY version IS NULL DESC,version DESC) AS preferred
                FROM versions v WHERE EXISTS(
                   SELECT 1 FROM json_each(v.spec,'$.profile') p
                   WHERE p.key IN('prepare','runtime','finalize','on_complete','on_error','on_cancel')
                     AND json_extract(p.value,'$.runtime_pin.id')=?
                     AND json_extract(p.value,'$.runtime_pin.revision')=?
                     AND json_extract(p.value,'$.runtime_pin.sha256')=?)
             )
             SELECT j.id,j.attempt,j.state,json_extract(m.spec,'$.kind') profile,
                    json_extract(m.spec,'$.profile.sha256') profile_sha256,
                    (SELECT json_group_array(p.key) FROM json_each(m.spec,'$.profile') p
                     WHERE p.key IN('prepare','runtime','finalize','on_complete','on_error','on_cancel')
                     AND json_extract(p.value,'$.runtime_pin.id')=?1
                     AND json_extract(p.value,'$.runtime_pin.revision')=?2
                     AND json_extract(p.value,'$.runtime_pin.sha256')=?3) stages,
                    coalesce(w.root,j.id) execution_owner
             FROM matches m JOIN jobs j ON j.id=m.job LEFT JOIN execution_memberships w ON w.job=j.id
             WHERE m.preferred=1 ORDER BY j.created DESC,j.id LIMIT ?4 OFFSET ?5",
            &[&item["id"].as_str(),&item["revision"].as_i64(),&item["sha256"].as_str(),&((limit+1) as i64),&(offset as i64)])?;
        item["jobs_complete"] = json!(consumers.len() <= limit);
        let mut jobs = vec![];
        for row in consumers.into_iter().take(limit) {
            let stages: Vec<String> = serde_json::from_str(row["stages"].as_str().unwrap_or("[]"))?;
            let stages: Vec<_> = STAGES
                .iter()
                .copied()
                .filter(|stage| stages.iter().any(|v| v == *stage))
                .collect();
            // A recovery may switch Profile before the next Attempt. Resolve
            // each start against its own frozen work (or its original revision).
            let executed = rows(&self.db,
                &format!("WITH starts AS (
                    SELECT s.attempt,CASE s.stage WHEN 'run' THEN 'runtime' ELSE s.stage END stage,s.started,
                        {FROZEN_RECEIPT_SPEC} spec
                    FROM stage_runs s JOIN jobs j ON j.id=s.job
                    LEFT JOIN attempts a ON a.id=s.attempt AND a.job=s.job
                    LEFT JOIN hooks h ON h.job=s.job AND h.attempt=s.attempt AND h.stage=s.stage
                    WHERE s.job=? AND s.started IS NOT NULL
                 )
                 SELECT attempt,stage,json_extract(spec,'$.kind') profile,
                        json_extract(spec,'$.profile.sha256') profile_sha256 FROM starts
                 WHERE json_extract(spec,'$.profile.'||stage||'.runtime_pin.id')=?
                   AND json_extract(spec,'$.profile.'||stage||'.runtime_pin.revision')=?
                   AND json_extract(spec,'$.profile.'||stage||'.runtime_pin.sha256')=?
                 ORDER BY started,stage"),
                &[&row["id"].as_str(), &item["id"].as_str(), &item["revision"].as_i64(), &item["sha256"].as_str()])?;
            jobs.push(json!({"id":row["id"],"attempt":row["attempt"],"state":row["state"],"execution_owner":row["execution_owner"],"executed":!executed.is_empty(),"execution_receipts":executed,"group_root":self.job_family_root(row["id"].as_str().unwrap())?,"profile":row["profile"],"profile_sha256":row["profile_sha256"],"stages":stages}));
        }
        item["executed_jobs"] = json!(jobs
            .iter()
            .filter(|j| j["executed"] == true)
            .map(|j| j["id"].clone())
            .collect::<Vec<_>>());
        if offset == 0 && item["jobs_complete"] == true {
            let owners: BTreeSet<_> = jobs
                .iter()
                .filter(|j| j["executed"] == true)
                .filter_map(|j| j["execution_owner"].as_str())
                .collect();
            item["started_job_count"] = json!(owners.len());
        }
        item["job_usage"] = json!(jobs);
        Ok(())
    }
    pub fn curation(&self, kind: &str, id: &str, revision: i64) -> Result<Value> {
        let text: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM runtime_curation WHERE kind=? AND id=? AND revision=?",
                params![kind, id, revision],
                |r| r.get(0),
            )
            .optional()?;
        Ok(text
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or(json!({})))
    }
}
pub fn bind(config: &Value, profile: &mut Value) -> Result<()> {
    for stage in STAGES {
        if let Some(runner) = profile.get_mut(stage) {
            require(
                runner.get("runtime_pin").is_none(),
                "runtime",
                "Runtime pins are server assigned",
                400,
            )?;
            if let Some(image) = runner
                .get("image")
                .and_then(Value::as_str)
                .map(String::from)
            {
                require(
                    config["status"] == "ok" && config["declared_policy"]["enabled"] == true,
                    "runtime",
                    "Fresh declared-image capability required",
                    503,
                )?;
                let digest = config["declared_images"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|v| v["id"] == image || v["tag"] == image)
                    .and_then(|v| v["id"].as_str())
                    .ok_or_else(|| {
                        Error::new(
                            "image",
                            "Cached image outside current declared-image observation",
                            409,
                        )
                    })?;
                let policy = &config["declared_policy"];
                let resources = runner.get("resources").cloned().unwrap_or(json!({}));
                let cpus = resources["cpus"].as_f64().unwrap_or(1.);
                let pids = resources["pids"].as_i64().unwrap_or(512);
                require(
                    cpus > 0.
                        && cpus <= policy["max_cpus"].as_f64().unwrap_or(4.)
                        && pids >= 16
                        && pids <= policy["max_pids"].as_i64().unwrap_or(1024),
                    "resources",
                    "Requested build resources exceed grant",
                    403,
                )?;
                runner["image"] = json!(digest);
                runner["executor"] = json!("container");
            }
            if let Some(id) = runner.get("runtime_id").and_then(Value::as_str) {
                require(
                    config["status"] == "ok"
                        && config["runtimes"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|r| {
                                (r["legacy"] == true && r["id"] == id)
                                    || r["aliases"]
                                        .as_array()
                                        .is_some_and(|a| a.iter().any(|alias| alias == id))
                            }),
                    "runtime",
                    "Compatibility Runtime ID outside current instance grant",
                    403,
                )?;
            }
            if let Some(reference) = runner.get("runtime_ref").cloned() {
                require(
                    config["status"] == "ok",
                    "runtime",
                    "Fresh protected Runtime observation required",
                    503,
                )?;
                let item = config["runtimes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|r| {
                        r["id"] == reference["id"]
                            && r["revision"] == reference["revision"]
                            && r["legacy"] != true
                    })
                    .ok_or_else(|| Error::new("runtime", "Runtime outside instance grant", 403))?;
                runner["runtime_pin"] = json!({"id":item["id"],"revision":item["revision"],"sha256":item["sha256"],"image":item["manifest"]["runtime"]["image"],"dependencies":item["manifest"]["dependencies"],"validation":item["manifest"]["validation"]});
                runner["executor"] = json!("container");
            }
        }
    }
    Ok(())
}
pub fn requirement_report(manifest: &Value, profile: &Value) -> Result<Vec<Value>> {
    require(
        manifest.get("requires").is_none_or(Value::is_object)
            && manifest.get("stage_requires").is_none_or(Value::is_array),
        "requires",
        "Requirements must be a mapping and stage list",
        400,
    )?;
    let mut requirements = vec![];
    for (name, version) in manifest["requires"].as_object().into_iter().flatten() {
        require(
            version.is_string(),
            "requires",
            "Requirements must name exact versions",
            400,
        )?;
        requirements.push(json!({"stage":"runtime","name":name,"version":version}));
    }
    requirements.extend(
        manifest["stage_requires"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    );
    let mut result = vec![];
    for req in requirements {
        let stage = req["stage"].as_str().unwrap_or("");
        require(
            STAGES.contains(&stage) && req["name"].is_string() && req["version"].is_string(),
            "requires",
            "Invalid dependency requirement",
            400,
        )?;
        let versions: BTreeSet<_> = profile[stage]["runtime_pin"]["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| {
                d["name"].as_str().map(str::to_lowercase)
                    == req["name"].as_str().map(str::to_lowercase)
                    && (req.get("scope").is_none() || d["scope"] == req["scope"])
            })
            .filter_map(|d| d["version"].as_str())
            .collect();
        let status = if profile.get(stage).is_none() {
            "missing_stage"
        } else if profile[stage]["runtime_pin"].is_null() {
            "unverified"
        } else if versions.is_empty() {
            "missing"
        } else if versions.len() > 1 {
            "ambiguous"
        } else if versions.contains(req["version"].as_str().unwrap_or("")) {
            "matched"
        } else {
            "mismatch"
        };
        let mut row = req;
        row["status"] = json!(status);
        row["actual_versions"] = json!(versions);
        result.push(row);
    }
    Ok(result)
}
pub fn require_dependencies(manifest: &Value, profile: &Value) -> Result<()> {
    require(
        requirement_report(manifest, profile)?
            .iter()
            .all(|r| r["status"] == "matched"),
        "requires",
        "Runtime dependency requirement mismatch",
        409,
    )
}
