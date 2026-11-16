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
        self.dashboard_catalog_selected(None)
    }
    pub fn dashboard_catalog_selected(&self, selected: Option<&[Value]>) -> Result<Value> {
        let all = selected.is_none();
        let ids_for = |kind: &str| -> Result<String> {
            encode(&json!(selected
                .unwrap_or_default()
                .iter()
                .filter(|v| v["native_ref"]["kind"] == kind)
                .map(|v| v["native_ref"]["identity"]["id"].clone())
                .collect::<Vec<_>>()))
        };
        let job_ids = ids_for("job")?;
        let selected_json = encode(&json!(selected.unwrap_or_default()))?;
        let history_ids = ids_for("historical")?;
        let revision = self.dashboard_revision()?;
        if let Some((cached, value)) = self
            .catalog_cache
            .lock()
            .map_err(|_| Error::new("projection", "Projection cache unavailable", 503))?
            .as_ref()
        {
            if all && cached == &revision {
                return Ok(value.clone());
            }
        }

        let mut entities = vec![];
        let mut ids = std::collections::BTreeSet::new();
        for r in rows(&self.db, "SELECT id FROM jobs WHERE ? OR id IN(SELECT value FROM json_each(?)) ORDER BY created,id", &[&all,&job_ids])? {
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
            "SELECT id FROM job_workflows WHERE ? OR id IN(SELECT value FROM json_each(?)) ORDER BY created,id",
            &[&all,&job_ids],
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
            "SELECT * FROM all_entity_revisions e WHERE ? OR EXISTS(SELECT 1 FROM json_each(?) x WHERE json_extract(x.value,'$.native_ref.kind')='entity' AND json_extract(x.value,'$.native_ref.identity.id')=e.id AND json_extract(x.value,'$.native_ref.identity.revision')=e.revision) ORDER BY id,revision",
            &[&all,&selected_json],
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
            entity["presentation"] = entity
                .get("presentation")
                .cloned()
                .unwrap_or(json!({"scope":"research","role":"main"}));
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
                "SELECT * FROM all_entity_locations WHERE id=? AND revision=?",
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
        if all {
            for entity in
                self.imported_entities_selected(if all { None } else { Some(&history_ids) })?
            {
                ids.insert(entity["id"].as_str().unwrap_or("").into());
                entities.push(entity);
            }
            let snapshot:Option<String>=self.db.query_row("SELECT snapshot FROM history_records WHERE source='catalog' ORDER BY rowid DESC LIMIT 1",[],|r|r.get(0)).optional()?;
            if let Some(snapshot) = snapshot {
                for row in rows(&self.db,"SELECT * FROM history_records WHERE source='catalog' AND snapshot=? AND (? OR id IN(SELECT value FROM json_each(?))) ORDER BY rowid DESC",&[&snapshot,&all,&history_ids])?{let id=row["id"].as_str().unwrap_or("");if ids.contains(id){continue;}let body:Value=serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;let mut entity=body.get("entity").or_else(||body.get("original")).cloned().unwrap_or(json!({}));if !entity.is_object(){continue}entity["id"]=json!(id);entity["kind"]=row["kind"].clone();entity["namespace"]=json!("history");entity["loom_history"]=json!({"source":row["source"],"kind":row["kind"],"id":id,"revision":row["revision"],"snapshot":snapshot});let (manifest,locations)=self.historical_files(&entity["loom_history"])?;let(files,media)=crate::media::file_dtos(&entity["loom_history"],&manifest,&locations,&self.config);entity["loom_files"]=files;entity["loom_media"]=media;entity["payload_status"]=body["payload_status"].clone();if entity.get("label").is_none(){entity["label"]=json!(id);}entities.push(entity);ids.insert(id.into());}
            }
        } else {
            for item in selected
                .unwrap_or_default()
                .iter()
                .filter(|v| v["native_ref"]["kind"] == "historical")
            {
                let identity = &item["native_ref"]["identity"];
                let row=rows(&self.db,"SELECT body FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",&[&identity["source"].as_str(),&identity["kind"].as_str(),&identity["id"].as_str(),&identity["revision"].as_str(),&identity["snapshot"].as_str()])?.into_iter().next().ok_or_else(||Error::new("history","Exact historical snapshot unavailable",404))?;
                let body: Value = serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;
                let mut entity = body
                    .get("entity")
                    .or_else(|| body.get("original"))
                    .filter(|v| v.is_object())
                    .cloned()
                    .unwrap_or(json!({}));
                entity["id"] = json!(format!(
                    "history:{}",
                    item["item_id"].as_str().unwrap_or("")
                ));
                entity["kind"] = if identity["kind"] == "run" {
                    json!("run")
                } else {
                    item["kind"].clone()
                };
                entity["namespace"] = json!("history");
                entity["loom_history"] = identity.clone();
                entity["migration"] = json!({"read_only":true,"source":identity["source"],"snapshot":identity["snapshot"]});
                if !entity["label"].is_string() {
                    entity["label"] = identity["id"].clone();
                }
                let (mut manifest, locations) = self.historical_files(identity)?;
                let count = manifest["components"].as_array().map(Vec::len).unwrap_or(0);
                if let Some(components) = manifest["components"].as_array_mut() {
                    components.truncate(200);
                }
                let (files, media) =
                    crate::media::file_dtos(identity, &manifest, &locations, &self.config);
                entity["loom_files"] = files;
                entity["loom_media"] = media;
                entity["files_complete"] = json!(count <= 200);
                let who = entity["owner"]
                    .as_str()
                    .or_else(|| entity["properties"]["owner"].as_str())
                    .unwrap_or("")
                    .to_string();
                entity = self.decorate_owner(
                    "entity",
                    identity["id"].as_str().unwrap_or(""),
                    &who,
                    entity,
                )?;
                entities.push(entity);
            }
        }
        let mut profiles = if all {
            self.profile_listing()?
        } else {
            let mut profiles = vec![];
            for item in selected
                .unwrap_or_default()
                .iter()
                .filter(|v| v["native_ref"]["kind"] == "profile")
            {
                let native = &item["native_ref"]["identity"];
                if let Some(kind) = native["kind"]
                    .as_str()
                    .or_else(|| item["profile_kind"].as_str())
                {
                    profiles.extend(
                        self.profile_listing_qualified(
                            Some(kind),
                            native["sha256"]
                                .as_str()
                                .or_else(|| item["profile_sha256"].as_str()),
                        )?
                        .as_array()
                        .cloned()
                        .unwrap_or_default(),
                    );
                }
            }
            json!(profiles)
        };
        if !all {
            let mut related = std::collections::BTreeMap::new();
            for item in selected.unwrap_or_default().iter().filter(|v| {
                matches!(
                    v["native_ref"]["kind"].as_str(),
                    Some("component" | "tool" | "runtime")
                )
            }) {
                if let Ok(page)=self.item_graph(&crate::auth::person_actor("catalog-reader"),&json!({"item_id":item["item_id"],"revision_id":item["revision_id"],"limit":50})) {
                    for edge in page["items"].as_array().into_iter().flatten().filter(|e|e["native_ref"]["kind"]=="profile") {
                        let native=&edge["native_ref"]["identity"];
                        let body=edge["item_id"].as_str().and_then(|id|self.get_item(&json!({"item_id":id,"revision_id":edge["revision_id"]})).ok()).unwrap_or(Value::Null);
                        if let Some(kind)=native["kind"].as_str().or_else(||body["profile_kind"].as_str()) {
                            related.insert(kind.to_string(),native["sha256"].as_str().or_else(||body["profile_sha256"].as_str()).map(String::from));
                        }
                    }
                }
            }
            for (kind, sha) in related {
                if !profiles
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["kind"] == kind)
                {
                    if let Ok(value) = self.profile_listing_qualified(Some(&kind), sha.as_deref()) {
                        profiles
                            .as_array_mut()
                            .unwrap()
                            .extend(value.as_array().cloned().unwrap_or_default());
                    }
                }
            }
        }
        let mut combinations = vec![];
        for p in profiles.as_array().unwrap() {
            let kind = p["kind"].as_str().unwrap_or("");
            entities.push(json!({"id":format!("profile:{kind}"),"kind":"execution_profile","namespace":"loom","label":kind,"loom_execution_profile":p,"presentation":if p["group"]=="platform-check"&&p["origin"]["type"]!="builtin"{json!({"scope":"operations","role":"test"})}else{json!({"scope":"research","role":"main"})},"properties":{"revision":p["revision"],"used_by_jobs":p["used_by_jobs"]},"relations":[],"artifacts":[]}));
            combinations.push(p.clone());
        }
        let software = self.extension_inventory_selected(&profiles, selected)?;
        entities.extend(software.clone());
        let mut runtime_catalog = self.runtime_inventory()?;
        let observed_runtimes = runtime_catalog["runtimes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if let Some(selected) = selected {
            if let Some(items) = runtime_catalog["runtimes"].as_array_mut() {
                items.retain(|r| {
                    selected.iter().any(|v| {
                        v["native_ref"]["kind"] == "runtime"
                            && v["native_ref"]["identity"]["id"] == r["id"]
                            && v["native_ref"]["identity"]["revision"] == r["revision"]
                    })
                });
            }
            let representatives = runtime_catalog["dependency_catalog"]["dependencies"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let runtimes = observed_runtimes;
            let mut dependencies = vec![];
            for item in selected
                .iter()
                .filter(|v| v["native_ref"]["kind"] == "dependency")
            {
                let exact = &item["native_ref"]["identity"];
                let category = representatives
                    .iter()
                    .find(|d| {
                        d["label"].as_str().is_some_and(|name| {
                            name.eq_ignore_ascii_case(item["label"].as_str().unwrap_or(""))
                        })
                    })
                    .map(|d| d["category"].clone())
                    .unwrap_or_else(|| {
                        item.get("category")
                            .filter(|v| v.is_string())
                            .cloned()
                            .unwrap_or(json!("libraries"))
                    });
                let mut uses = vec![];
                for runtime in &runtimes {
                    if runtime["manifest"]["dependencies"]
                        .as_array()
                        .is_some_and(|deps| {
                            deps.iter()
                                .any(|d| crate::items::dependency_ref(runtime, d) == *exact)
                        })
                    {
                        uses.push(json!({"id":runtime["id"],"revision":runtime["revision"],"entity":format!("runtime:{}:{}",runtime["id"].as_str().unwrap_or(""),runtime["revision"]),"label":runtime["manifest"].get("label").unwrap_or(&runtime["id"]),"scope":exact["scope"]}));
                    }
                }
                dependencies.push(json!({"id":item["item_id"],"name":exact["name"],"label":item["label"],"category":category,"versions":[{"version":exact["version"],"installations":[{"origin":exact["origin"],"runtimes":uses}]}],"runtimes":uses,"source_status":if uses.is_empty(){"unavailable"}else{"observed"}}));
            }
            runtime_catalog["dependency_catalog"]["dependencies"] = json!(dependencies);
        }
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
        for scenario in rows(&self.db,"SELECT s.*,r.revision AS displayed_revision,r.definition,r.sha256 FROM scenarios s JOIN scenario_revisions r ON r.scenario_id=s.id AND r.revision=CASE WHEN ? THEN s.current_revision ELSE (SELECT json_extract(value,'$.native_ref.identity.revision') FROM json_each(?) WHERE json_extract(value,'$.native_ref.kind')='scenario' AND json_extract(value,'$.native_ref.identity.id')=s.id LIMIT 1) END ORDER BY s.created",&[&all,&selected_json])?{entities.push(json!({"id":scenario["id"],"kind":"scenario","namespace":"loom","label":serde_json::from_str::<Value>(scenario["definition"].as_str().unwrap_or("{}"))?["label"],"created_by":scenario["created_by"],"loom_scenario":{"id":scenario["id"],"revision":scenario["displayed_revision"],"archived":scenario["archived"]==1,"links":rows(&self.db,"SELECT revision,target_kind,target,superseded_by FROM scenario_links WHERE scenario_id=?",&[&scenario["id"].as_str()])?.into_iter().map(|mut r|{if let Some(t)=r["target"].as_str(){r["target"]=json!({"kind":r["target_kind"],"identity":serde_json::from_str::<Value>(t).unwrap_or(Value::Null)});}r}).collect::<Vec<_>>(),"definition":serde_json::from_str::<Value>(scenario["definition"].as_str().unwrap_or("{}"))?},"properties":{"revision":scenario["displayed_revision"],"archived":scenario["archived"]},"relations":[],"artifacts":[]}));}
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
        let value = json!({"entities":entities,"queue":queue,"revision":revision,"sources":[{"id":"loom-ops","status":"ok"},{"id":"catalog","status":"ok"}],"capabilities":{"curate":true,"scenarios":true,"item_search":true},"host":self.dashboard_host()?,"job_contract":serde_json::from_str::<Value>(include_str!("../../../contracts/job-contract.json"))?,"runtime_catalog":runtime_catalog});
        if all {
            *self
                .catalog_cache
                .lock()
                .map_err(|_| Error::new("projection", "Projection cache unavailable", 503))? =
                Some((revision, value.clone()));
        }
        Ok(value)
    }
    pub fn dashboard_bootstrap(&self) -> Result<Value> {
        let mut selected = vec![];
        for row in rows(&self.db,"SELECT r.body FROM catalog_items i JOIN catalog_revisions r ON r.id=i.current_revision WHERE i.deleted=0 AND i.kind IN('scenario','job','dataset','model','experiment','artifact','profile','component','tool','runtime','dependency') ORDER BY i.rowid DESC LIMIT 50",&[])? {
            selected.push(serde_json::from_str::<Value>(row["body"].as_str().unwrap_or("{}"))?);
        }
        let queue = self.queue()?;
        for entry in queue["entries"].as_array().into_iter().flatten().take(200) {
            let id = entry
                .get("workflow")
                .filter(|v| v.is_string())
                .unwrap_or(&entry["id"]);
            if let Ok(resolved) =
                self.resolve_item(&json!({"native_ref":{"kind":"job","identity":{"id":id}}}))
            {
                if let Ok(item) = self.get_item(&json!({"item_id":resolved["item_id"]})) {
                    if !selected.iter().any(|v| v["item_id"] == item["item_id"]) {
                        selected.push(item);
                    }
                }
            }
        }
        let mut catalog = self.dashboard_catalog_selected(Some(&selected))?;
        catalog["summary"] = self.dashboard_summary()?;
        catalog["activity"] = self.dashboard_activity("research")?;
        catalog["capabilities"]["catalog_pages"] = json!(true);
        catalog["initial_page_complete"] = json!(false);
        Ok(catalog)
    }
    pub fn dashboard_entity(&self, args: &Value) -> Result<Value> {
        let item = self.get_item(args)?;
        let mut catalog = self.dashboard_catalog_selected(Some(std::slice::from_ref(&item)))?;
        for entity in catalog["entities"].as_array_mut().unwrap() {
            entity["item_id"] = item["item_id"].clone();
            entity["item_revision_id"] = item["revision_id"].clone();
        }
        Ok(catalog)
    }
    /// List DTOs are projected only for the selected index page; payload trees stay lazy.
    pub fn dashboard_page(&self, actor: &crate::auth::Actor, args: &Value) -> Result<Value> {
        let mut query = args.clone();
        if query["kinds"].as_array().is_none_or(Vec::is_empty) {
            query["kinds"] = json!([
                "scenario",
                "job",
                "dataset",
                "model",
                "experiment",
                "artifact",
                "profile",
                "component",
                "tool",
                "runtime",
                "dependency"
            ]);
        }
        let page = self.search_items(actor, &query)?;
        let selected = page["items"]
            .as_array()
            .ok_or_else(|| Error::input("Search page missing"))?;
        let mut catalog = self.dashboard_catalog_selected(Some(selected))?;
        catalog["next_cursor"] = page["next_cursor"].clone();
        catalog["partial"] = page["partial"].clone();
        catalog["consistency"] = page["consistency"].clone();
        catalog["warnings"] = page["warnings"].clone();
        catalog["capabilities"]["catalog_pages"] = json!(true);
        for entity in catalog["entities"].as_array_mut().unwrap() {
            let item = selected.iter().find(|item| {
                let native = &item["native_ref"];
                let id = &native["identity"];
                match native["kind"].as_str().unwrap_or("") {
                    "job" | "entity" | "scenario" => entity["id"] == id["id"],
                    "historical" => entity["loom_history"] == *id,
                    "profile" => {
                        entity["id"]
                            == format!(
                                "profile:{}",
                                id["kind"]
                                    .as_str()
                                    .or_else(|| item["profile_kind"].as_str())
                                    .unwrap_or("")
                            )
                    }
                    "runtime" => {
                        entity["loom_runtime"]["id"] == id["id"]
                            && entity["loom_runtime"]["revision"] == id["revision"]
                    }
                    "component" | "tool" => {
                        entity["loom_extension"]["owner"] == id["owner"]
                            && entity["loom_extension"]["id"] == id["id"]
                    }
                    "dependency" => entity["loom_dependency"]["name"] == id["name"],
                    _ => false,
                }
            });
            if let Some(item) = item {
                entity["item_id"] = item["item_id"].clone();
                entity["item_revision_id"] = item["revision_id"].clone();
            }
        }
        // Search pagination follows authority items, not extra related DTOs.
        catalog["entities"]
            .as_array_mut()
            .unwrap()
            .retain(|e| e["item_id"].is_string());
        Ok(catalog)
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
    pub fn dashboard_host(&self) -> Result<Value> {
        let mut host = json!({"machine_id":self.config.machine_id,"name":self.config.machine_id,
            "hostname":null,"inventory":{},
            "status":"unavailable"});
        let observation = self.docker_observation()?;
        if observation_fresh(&observation, now()) {
            host["inventory"]["docker_engine"] = observation["engine"]["version"].clone();
        }
        if let Ok(bytes) = std::fs::read(self.observation_path("dashboard-gpus.json")) {
            if let Ok(gpus) = serde_json::from_slice::<Value>(&bytes) {
                if observation_fresh(&gpus, now()) && gpus["status"] == "ok" {
                    host["inventory"]["gpu_driver"] = gpus["data"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find_map(|gpu| gpu["driver_version"].as_str())
                        .map(|v| json!(v))
                        .unwrap_or(Value::Null);
                }
            }
        }
        if observation["host"].is_object() {
            for key in [
                "hostname",
                "ip",
                "ip_kind",
                "lan_ips",
                "os",
                "kernel",
                "logical_cpus",
            ] {
                if let Some(value) = observation["host"].get(key) {
                    host[key] = value.clone();
                }
            }
            if let Some(name) = host["hostname"].as_str().filter(|s| !s.is_empty()) {
                host["name"] = json!(name);
            }
            host["observed_at"] = observation["observed_at"].clone();
            host["status"] = json!(if observation_fresh(&observation, now()) {
                "ok"
            } else {
                "stale"
            });
        }
        Ok(host)
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
        // The workspace consumes the established sections envelope. Retain the
        // native flat fields for callers released before the Dashboard adapter.
        let mut sections = result.clone();
        sections.as_object_mut().unwrap().remove("observed_at");
        let queue = &result["leases"]["data"];
        let entries = queue["entries"].as_array().cloned().unwrap_or_default();
        let registered: Vec<_> = entries.iter().filter(|entry| matches!(entry["state"].as_str(),
            Some("running" | "preparing" | "processing"))).map(|entry| json!({
                "job_id":entry["id"],"label":entry.get("label").unwrap_or(&entry["id"]),
                "resource":format!("{} 실행 슬롯",entry["lane"].as_str().unwrap_or("cpu").to_uppercase()),"units":1
            })).collect();
        sections["leases"]["data"] = json!({"registered":registered,
            "queued_jobs":entries.iter().filter(|entry| matches!(entry["state"].as_str(),Some("queued" | "ready" | "blocked"))).count(),
            "scope":"visible_node_jobs","mechanism":"worker_lanes"});
        result["sections"] = sections;
        result["host"] = self.dashboard_host()?;
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
