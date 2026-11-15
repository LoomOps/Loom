//! Browser DTOs preserve native/historical identity and truthful time provenance.
use crate::{jobs, store::*, Error, Result};
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
impl Application {
    pub fn dashboard_revision(&self) -> Result<String> {
        let version: i64 = self.db.query_row(
            "SELECT version FROM rust_projection_version WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        let fresh: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM rust_runtime_observation WHERE observed>?)",
            [now() - 30.],
            |r| r.get(0),
        )?;
        Ok(hash(encode(&json!([
            self.config.machine_id,
            self.config.storage_id,
            self.config.profiles,
            self.config.data_stores,
            self.config.extra,
            crate::onboarding::identity()["source_sha256"],
            version,
            fresh
        ]))?))
    }
    pub fn dashboard_catalog(&self) -> Result<Value> {
        let revision = self.dashboard_revision()?;
        if let Some((cached, value)) = self
            .catalog_cache
            .lock()
            .map_err(|_| Error::new("projection", "Projection cache unavailable", 503))?
            .as_ref()
        {
            if cached == &revision {
                return Ok(value.clone());
            }
        }

        let mut entities = vec![];
        let mut ids = std::collections::BTreeSet::new();
        for r in rows(&self.db, "SELECT id FROM jobs ORDER BY created,id", &[])? {
            let id = r["id"].as_str().unwrap_or("");
            let d = self.job_detail(id)?;
            let spec = &d["spec"];
            let attempts = d["attempts"].as_array().cloned().unwrap_or_default();
            let started = attempts
                .first()
                .map(|a| a["started"].clone())
                .unwrap_or(Value::Null);
            let ended = attempts
                .last()
                .map(|a| a["ended"].clone())
                .unwrap_or(Value::Null);
            let root: Option<String> = self
                .db
                .query_row("SELECT root FROM workflow_nodes WHERE job=?", [id], |r| {
                    r.get(0)
                })
                .optional()?;
            entities.push(json!({"id":id,"kind":"job","namespace":"loom","label":d["presentation"]["label"].as_str().unwrap_or(spec["kind"].as_str().unwrap_or(id)),"state":d["state"],"created":d["created"],"attempt":d["attempt"],"properties":{"id":id,"state":d["state"],"status":d["status"],"created":d["created"],"attempt":d["attempt"],"owner":d["owner"],"record_role":"loom_job","category":d["category"],"execution_mode":d["execution_mode"],"queued_at":d["created"],"started_at":d["timeline"].get("started").unwrap_or(&started),"process_exited_at":d["timeline"].get("ended").unwrap_or(&ended),"artifact_ready":d["result"]["artifact_ready"],"recovery":d["recovery"],"output_storage":spec["output_storage"],"internal_step_count":d["internal_steps"]["steps"].as_array().map(|a|a.len()).unwrap_or(0),"workflow_root":root},"attempts":attempts,"internal_steps":d["internal_steps"],"execution_intervals":d["execution_intervals"],"loom_job":{"id":id},"presentation":d["presentation"],"scenario_ref":d["scenario_ref"],"relations":[],"artifacts":[]}));
            if let Some(entity) = entities.last_mut() {
                for key in ["owner", "owner_name", "owner_profile"] {
                    entity[key] = d[key].clone();
                }
            }
            ids.insert(id.to_string());
        }
        for w in rows(
            &self.db,
            "SELECT id FROM job_workflows ORDER BY created,id",
            &[],
        )? {
            let id = w["id"].as_str().unwrap_or("");
            let d = self.job_detail(id)?;
            entities.push(json!({"id":id,"kind":"job","namespace":"loom","label":d["spec"]["label"],"state":d["state"],"created":d["created"],"properties":{"record_role":"loom_job","id":id,"state":d["state"],"status":d["status"],"queued_at":d["created"],"started_at":d["execution_intervals"].as_array().and_then(|a|a.iter().filter_map(|i|i["start"].as_f64()).min_by(f64::total_cmp)),"process_exited_at":if d["state"]=="complete"{d["execution_intervals"].as_array().and_then(|a|a.iter().filter_map(|i|i["end"].as_f64()).max_by(f64::total_cmp))}else{None},"owner":d["owner"],"internal_step_count":d["executions"].as_array().map(|a|a.len()).unwrap_or(0)},"loom_workflow":d,"loom_job":{"id":id},"steps":d["executions"],"execution_intervals":d["execution_intervals"],"presentation":d["presentation"],"scenario_ref":d["scenario_ref"],"relations":[],"artifacts":[]}));
            if let Some(entity) = entities.last_mut() {
                for key in ["owner", "owner_name", "owner_profile"] {
                    entity[key] = d[key].clone();
                }
            }
            ids.insert(id.to_string());
        }
        for row in rows(
            &self.db,
            "SELECT * FROM entity_revisions ORDER BY id,revision",
            &[],
        )? {
            let mut entity: Value = serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;
            let id = row["id"].as_str().unwrap_or("");
            entity["kind"] = row["kind"].clone();
            entity["namespace"] = json!("loom");
            if entity.get("label").is_none() {
                entity["label"] = json!(id);
            }
            entity["properties"] = entity.get("metadata").cloned().unwrap_or(json!({}));
            entity["loom_registry"] = json!({"id":id,"revision":row["revision"]});
            entity["loom_job"] = json!({"id":row["job"]});
            let registered: Option<f64> = self.db.query_row(
                "SELECT MIN(created) FROM events WHERE job=? AND attempt=? AND code='complete'",
                rusqlite::params![row["job"].as_str(), entity["attempt"].as_str()],
                |r| r.get(0),
            )?;
            entity["properties"]["registered_at"] = json!(registered);
            entity["properties"]["activity_time_source"] = json!("publication registration");
            entity["registered_at"] = json!(registered);
            entity["contribution"] = json!({"eligible":registered.is_some()&&matches!(entity["kind"].as_str(),Some("dataset"|"model"|"experiment")),"key":entity["id"],"time_kind":"registered"});
            entity["metrics"] = entity.get("metrics").cloned().unwrap_or(json!([]));
            entity["presentation"] = json!({"scope":"research","role":"main"});
            if let Some(job) = row["job"].as_str() {
                let record = jobs::load(&self.db, job)?;
                entity["presentation"] = record
                    .spec
                    .get("presentation")
                    .cloned()
                    .unwrap_or(json!({"scope":"research","role":"main"}));
                entity["scenario_ref"] = record.spec["scenario_ref"].clone();
            }
            let locations = rows(
                &self.db,
                "SELECT * FROM entity_locations WHERE id=? AND revision=?",
                &[&id, &row["revision"].as_i64()],
            )?;
            let (files, media) = crate::media::file_dtos(
                &json!({"id":id,"revision":row["revision"]}),
                &entity,
                &locations,
                &self.config,
            );
            entity["loom_files"] = files;
            entity["loom_media"] = media;
            let entity =
                self.decorate_owner("entity", id, row["owner"].as_str().unwrap_or(""), entity)?;
            entities.push(entity);
            ids.insert(id.into());
        }
        for entity in self.imported_entities()? {
            ids.insert(entity["id"].as_str().unwrap_or("").into());
            entities.push(entity);
        }
        let snapshot:Option<String>=self.db.query_row("SELECT snapshot FROM history_records WHERE source='catalog' ORDER BY rowid DESC LIMIT 1",[],|r|r.get(0)).optional()?;
        if let Some(snapshot) = snapshot {
            for row in rows(&self.db,"SELECT * FROM history_records WHERE source='catalog' AND snapshot=? ORDER BY rowid DESC",&[&snapshot])?{let id=row["id"].as_str().unwrap_or("");if ids.contains(id){continue;}let body:Value=serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;let mut entity=body.get("entity").or_else(||body.get("original")).cloned().unwrap_or(json!({}));if !entity.is_object(){continue}entity["id"]=json!(id);entity["kind"]=row["kind"].clone();entity["namespace"]=json!("history");entity["loom_history"]=json!({"source":row["source"],"kind":row["kind"],"id":id,"revision":row["revision"],"snapshot":snapshot});let (manifest,locations)=self.historical_files(&entity["loom_history"])?;let(files,media)=crate::media::file_dtos(&entity["loom_history"],&manifest,&locations,&self.config);entity["loom_files"]=files;entity["loom_media"]=media;entity["payload_status"]=body["payload_status"].clone();if entity.get("label").is_none(){entity["label"]=json!(id);}entities.push(entity);ids.insert(id.into());}
        }
        let profiles = self.profile_listing()?;
        let mut combinations = vec![];
        for p in profiles.as_array().unwrap() {
            let kind = p["kind"].as_str().unwrap_or("");
            entities.push(json!({"id":format!("profile:{kind}"),"kind":"execution_profile","namespace":"loom","label":kind,"loom_execution_profile":p,"presentation":if p["group"]=="platform-check"&&p["origin"]["type"]!="builtin"{json!({"scope":"operations","role":"test"})}else{json!({"scope":"research","role":"main"})},"properties":{"revision":p["revision"],"used_by_jobs":p["used_by_jobs"]},"relations":[],"artifacts":[]}));
            combinations.push(p.clone());
        }
        let software = self.extension_inventory(&profiles)?;
        entities.extend(software.clone());
        let mut runtime_catalog = self.runtime_inventory()?;
        for item in runtime_catalog["runtimes"].as_array_mut().unwrap() {
            let mut consumers = vec![];
            for profile in profiles.as_array().unwrap() {
                let stages: Vec<_> = profile["stages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|s| {
                        s["environment"]["id"] == item["id"]
                            && s["environment"]["revision"] == item["revision"]
                    })
                    .map(|s| s["stage"].clone())
                    .collect();
                if !stages.is_empty() {
                    consumers.push(json!({"kind":profile["kind"],"stages":stages}));
                }
            }
            item["used_by_profiles"] = json!(consumers);
            entities.push(json!({"id":format!("runtime:{}:{}",item["id"].as_str().unwrap_or(""),item["revision"].as_i64().map(|n|n.to_string()).unwrap_or("legacy".into())),"kind":"runtime","namespace":"loom","label":item["curation"].get("label").or_else(||item["manifest"].get("label")).unwrap_or(&item["id"]),"summary":item["manifest"]["description"].as_str().unwrap_or(""),"loom_runtime":item,"properties":{"revision":item["revision"]},"relations":[],"artifacts":[]}));
        }
        for dependency in runtime_catalog["dependency_catalog"]["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
        {
            entities.push(json!({"id":format!("dependency:{}",dependency["id"].as_str().unwrap_or("")),"kind":"dependency","namespace":"loom","label":dependency["label"],"loom_dependency":dependency,"properties":{"category":dependency["category"]},"relations":[],"artifacts":[]}));
        }
        runtime_catalog["profiles"] = json!(combinations);
        runtime_catalog["components"] = json!(software
            .iter()
            .filter(|e| e["kind"] == "component")
            .collect::<Vec<_>>());
        runtime_catalog["tools"] = json!(software
            .iter()
            .filter(|e| e["kind"] == "tool")
            .collect::<Vec<_>>());
        for scenario in rows(&self.db,"SELECT s.*,r.definition,r.sha256 FROM scenarios s JOIN scenario_revisions r ON r.scenario_id=s.id AND r.revision=s.current_revision ORDER BY s.created",&[])?{entities.push(json!({"id":scenario["id"],"kind":"scenario","namespace":"loom","label":scenario["label"],"created_by":scenario["created_by"],"loom_scenario":{"id":scenario["id"],"revision":scenario["current_revision"],"archived":scenario["archived"]==1,"links":rows(&self.db,"SELECT revision,target_kind,target,superseded_by FROM scenario_links WHERE scenario_id=?",&[&scenario["id"].as_str()])?.into_iter().map(|mut r|{if let Some(t)=r["target"].as_str(){r["target"]=json!({"kind":r["target_kind"],"identity":serde_json::from_str::<Value>(t).unwrap_or(Value::Null)});}r}).collect::<Vec<_>>(),"definition":serde_json::from_str::<Value>(scenario["definition"].as_str().unwrap_or("{}"))?},"properties":{"revision":scenario["current_revision"],"archived":scenario["archived"]},"relations":[],"artifacts":[]}));}
        for entity in &mut entities {
            if entity["kind"] == "scenario" {
                let identity = entity["id"].as_str().unwrap_or("").to_string();
                let creator = entity["created_by"].as_str().unwrap_or("").to_string();
                *entity = self.decorate_owner("scenario", &identity, &creator, entity.clone())?;
            }
            let keykind = if entity.get("loom_registry").is_some() {
                "entity"
            } else if entity["kind"] == "execution_profile" {
                "profile"
            } else {
                entity["kind"].as_str().unwrap_or("")
            };
            let overlay: Option<String> = self
                .db
                .query_row(
                    "SELECT presentation FROM research_overlays WHERE kind=? AND id=?",
                    rusqlite::params![keykind, entity["id"].as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            let mut presentation = entity
                .get("presentation")
                .cloned()
                .unwrap_or_else(|| json!({"scope":"research","role":"main"}));
            if let Some(overlay) = overlay {
                presentation.as_object_mut().unwrap().extend(
                    serde_json::from_str::<Value>(&overlay)?
                        .as_object()
                        .unwrap()
                        .clone(),
                );
            }
            if entity["properties"]["workflow_root"].is_string() {
                presentation["parent_job"] = entity["properties"]["workflow_root"].clone();
                presentation["role"] = json!("support");
            }
            presentation["visible"] = json!(
                presentation["scope"] == "research"
                    && presentation["role"] == "main"
                    && !(entity["kind"] == "job" && presentation["parent_job"].is_string())
            );
            entity["presentation"] = presentation;
            if entity["kind"] == "execution_profile" {
                let p = entity["loom_execution_profile"].clone();
                for key in ["owner_name", "owner_kind", "owner_profile"] {
                    entity[key] = p[key].clone();
                }
            }
        }
        let queue = self.queue()?;
        let value = json!({"entities":entities,"queue":queue,"revision":revision,"sources":[{"id":"loom-ops","status":"ok"},{"id":"catalog","status":"ok"}],"capabilities":{"curate":true,"scenarios":true},"host":{"machine_id":self.config.machine_id},"job_contract":serde_json::from_str::<Value>(include_str!("../../../contracts/job-contract.json"))?,"runtime_catalog":runtime_catalog});
        *self
            .catalog_cache
            .lock()
            .map_err(|_| Error::new("projection", "Projection cache unavailable", 503))? =
            Some((revision, value.clone()));
        Ok(value)
    }
    pub fn observation_path(&self, name: &str) -> std::path::PathBuf {
        std::env::var_os("LOOM_OBSERVATIONS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| self.local.clone())
            .join(name)
    }
    pub fn docker_observation(&self) -> Result<Value> {
        let path = self
            .config
            .extra
            .get("docker_observation")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| self.observation_path("docker-status.json"));
        let mut value = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .filter(Value::is_object)
            .unwrap_or(json!({"status":"unavailable","containers":[],"errors":[]}));
        let show = self.config.extra.get("show_dev_builds") == Some(&json!(true));
        if let Some(rows) = value["containers"].as_array_mut() {
            rows.retain(|r| {
                let project = r["compose_project"].as_str().unwrap_or("");
                show || r["deployment_channel"] != "dev-build"
                    && project != "loom-dev-build"
                    && !project.starts_with("loom-dev-build-")
            });
        }
        if !observation_fresh(&value, now()) {
            value["status"] = json!("stale");
        }
        value["visibility"] = json!({"show_dev_builds":show});
        Ok(value)
    }
    pub fn resources(&self) -> Result<Value> {
        let memory = std::fs::read_to_string("/proc/meminfo").ok();
        let cpu = std::fs::read_to_string("/proc/loadavg").ok().and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        });
        let mut values = std::collections::BTreeMap::new();
        if let Some(memory) = memory {
            for line in memory.lines() {
                let mut parts = line.split_whitespace();
                if let (Some(name), Some(size)) = (parts.next(), parts.next()) {
                    if let Ok(n) = size.parse::<u64>() {
                        values.insert(name.trim_end_matches(':').to_string(), n * 1024);
                    }
                }
            }
        }
        let mem = if let (Some(total), Some(available)) =
            (values.get("MemTotal"), values.get("MemAvailable"))
        {
            json!({"status":"ok","source":"linux:/proc/meminfo","data":{"total_bytes":total,"available_bytes":available,"used_bytes":total-available}})
        } else {
            json!({"status":"unavailable","data":null})
        };
        let mut result = json!({"observed_at":now(),"cpu":{"status":if cpu.is_some(){"ok"}else{"unavailable"},"source":"linux:/proc/loadavg","data":{"load_1m":cpu,"logical_cpus":std::thread::available_parallelism().ok().map(|n|n.get())}},"memory":mem,"gpus":{"status":"unavailable","data":null},"storage":{"status":"unavailable","data":null},"sensors":{"status":"unavailable","data":null},"leases":{"status":"ok","data":self.queue()?}});
        for (file, section) in [
            ("dashboard-gpus.json", "gpus"),
            ("dashboard-host-metrics.json", ""),
        ] {
            if let Ok(bytes) = std::fs::read(self.observation_path(file)) {
                if bytes.len() <= 4 * 1024 * 1024 {
                    if let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) {
                        if !observation_fresh(&value, now()) {
                            value["status"] = json!("stale");
                            if let Some(sections) = value["sections"].as_object_mut() {
                                for (_, section) in sections {
                                    section["status"] = json!("stale");
                                }
                            }
                        }
                        if section.is_empty() {
                            for key in ["cpu", "sensors"] {
                                if let Some(v) = value["sections"].get(key) {
                                    result[key] = v.clone();
                                }
                            }
                        } else {
                            result[section] = value;
                        }
                    }
                }
            }
        }
        let mut stores = vec!["data/0".to_string()];
        stores.extend(
            self.config
                .data_stores
                .keys()
                .filter(|k| k.as_str() != "data/0")
                .cloned(),
        );
        let mut disks = vec![];
        for selector in stores {
            let data = crate::storage::store(&self.config, &self.bulk, &selector, None);
            if let Ok((path, _)) = data {
                let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
                    .map_err(|_| Error::input("Invalid store path"))?;
                let mut info = std::mem::MaybeUninit::<libc::statvfs>::uninit();
                if unsafe { libc::statvfs(name.as_ptr(), info.as_mut_ptr()) } == 0 {
                    let info = unsafe { info.assume_init() };
                    disks.push(json!({"path":selector,"status":"ok","total_bytes":info.f_blocks*info.f_frsize,"free_bytes":info.f_bavail*info.f_frsize,"used_bytes":(info.f_blocks-info.f_bfree)*info.f_frsize}));
                    continue;
                }
            }
            disks.push(json!({"path":selector,"status":"unavailable"}));
        }
        result["storage"] = json!({"status":if disks.iter().any(|v|v["status"]=="ok"){"ok"}else{"unavailable"},"source":"linux:statvfs","data":disks});
        Ok(result)
    }
}

/// Parse the observer's RFC3339 UTC/offset clock rather than trusting file mtime.
pub fn observation_fresh(value: &Value, clock: f64) -> bool {
    let timestamp = if let Some(n) = value["observed_at"].as_f64() {
        Some(n)
    } else {
        value["observed_at"].as_str().and_then(|s| {
            if !s.is_ascii() || s.len() < 20 {
                return None;
            }
            let text = std::ffi::CString::new(&s[..19]).ok()?;
            let format = c"%Y-%m-%dT%H:%M:%S";
            let mut time = std::mem::MaybeUninit::<libc::tm>::zeroed();
            let parsed =
                unsafe { libc::strptime(text.as_ptr(), format.as_ptr(), time.as_mut_ptr()) };
            if parsed.is_null() {
                return None;
            }
            let mut time = unsafe { time.assume_init() };
            let seconds = unsafe { libc::timegm(&mut time) } as f64;
            let mut suffix = &s[19..];
            let mut fractional = 0.;
            if let Some(rest) = suffix.strip_prefix('.') {
                let count = rest.bytes().take_while(u8::is_ascii_digit).count();
                if count == 0 {
                    return None;
                }
                fractional = format!("0.{}", &rest[..count]).parse::<f64>().ok()?;
                suffix = &rest[count..];
            }
            let offset = if suffix == "Z" {
                0.
            } else {
                if suffix.len() != 6
                    || !matches!(suffix.as_bytes()[0], b'+' | b'-')
                    || suffix.as_bytes()[3] != b':'
                {
                    return None;
                }
                let hours = suffix[1..3].parse::<u32>().ok()?;
                let minutes = suffix[4..6].parse::<u32>().ok()?;
                if hours > 23 || minutes > 59 {
                    return None;
                }
                (hours * 3600 + minutes * 60) as f64
                    * if suffix.starts_with('-') { -1. } else { 1. }
            };
            Some(seconds + fractional - offset)
        })
    };
    timestamp.is_some_and(|stamp| clock - stamp >= 0. && clock - stamp <= 120.)
}
