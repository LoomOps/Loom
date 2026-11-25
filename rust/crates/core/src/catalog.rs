//! Browser DTOs preserve native/historical identity and truthful time provenance.
use crate::query::JOB_LINKS_CTE;
use crate::{jobs, store::*, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::BTreeMap;

// Typed input aliases from accepted contracts and preserved catalog links.
fn catalog_input_kind(relation: &Value, declared: &Value, output: &str) -> Option<&'static str> {
    match relation["kind"].as_str()? {
        "trained_on" if output == "model" => Some("dataset"),
        "evaluated_on" if output != "model" => Some("dataset"),
        "evaluated_model" | "evaluates" if output != "model" => Some("model"),
        "consumes" => {
            let input = relation["input"].as_str()?;
            match declared[input].as_str() {
                Some("dataset") => Some("dataset"),
                Some("model") if output != "model" => Some("model"),
                Some(_) => None,
                None => match input {
                    "dataset" | "training_dataset" if output == "model" => Some("dataset"),
                    "dataset" | "eval_dataset" | "evaluation_dataset" if output != "model" => {
                        Some("dataset")
                    }
                    "policy" | "model" | "evaluated_policy" if output != "model" => Some("model"),
                    _ => None,
                },
            }
        }
        _ => None,
    }
}

impl Application {
    fn catalog_input_descriptor(
        &self,
        identity: &Value,
        kind: &str,
        history: bool,
    ) -> Result<Option<Value>> {
        let mut dataset_mirror_key = Value::Null;
        let mut record = if history {
            let row = rows(&self.db,
                "SELECT revision, body FROM history_records WHERE source=? AND kind=? AND id=? AND snapshot=? AND (? IS NULL OR revision=?) LIMIT 2",
                &[&identity["source"].as_str(), &kind, &identity["id"].as_str(),
                    &identity["snapshot"].as_str(), &identity["revision"].as_str(), &identity["revision"].as_str()])?;
            let [record] = row.as_slice() else {
                return Ok(None);
            };
            let body: Value = serde_json::from_str(record["body"].as_str().unwrap_or("{}"))?;
            let Some(mut entity) = body
                .get("entity")
                .or_else(|| body.get("original"))
                .filter(|v| v.is_object())
                .cloned()
            else {
                return Ok(None);
            };
            entity["loom_history"] = json!({"source":identity["source"],"snapshot":identity["snapshot"],"kind":kind,"id":identity["id"],"revision":record["revision"]});
            if kind == "dataset" {
                dataset_mirror_key = crate::dataset_lineage::mirror_key(&identity["id"], body)?;
            }
            entity
        } else {
            let manifest: Option<String> = self.db.query_row(
                "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=? AND kind=?",
                params![identity["id"].as_str(),identity["revision"].as_i64(),kind], |r| r.get(0)).optional()?;
            let Some(manifest) = manifest else {
                return Ok(None);
            };
            let mut entity: Value = serde_json::from_str(&manifest)?;
            entity["loom_registry"] = identity.clone();
            entity
        };
        record["kind"] = json!(kind);
        crate::dataset_organization::attach_context(self, &mut record)?;
        crate::dataset_organization::attach(self, &mut record)?;
        let native = crate::research_metadata::snapshot_target(&record).unwrap();
        let label = record
            .get("label")
            .or_else(|| record.get("name"))
            .filter(|v| v.is_string())
            .unwrap_or(&identity["id"]);
        let label = self.output_label(
            if history { kind } else { "entity" },
            identity["id"].as_str().unwrap_or(""),
            label,
        )?;
        let metadata = record
            .get("metadata")
            .or_else(|| record.get("properties"))
            .unwrap_or(&Value::Null);
        let mut descriptor = json!({"ref":native,"label":label,"task_refs":record["task_refs"],
            "organization":record["dataset_organization"],"dataset_mirror_key":dataset_mirror_key});
        match self.resolve_item(&json!({"native_ref":native})) {
            Ok(pin) => {
                descriptor["item_id"] = pin["item_id"].clone();
                descriptor["revision_id"] = pin["revision_id"].clone();
            }
            Err(error) if error.status == 404 => {} // The exact source may still be indexing.
            Err(error) => return Err(error),
        }
        if kind == "dataset" {
            let split = metadata
                .pointer("/research/collection/split")
                .or_else(|| metadata.get("split_clips"))
                .or_else(|| metadata.get("splits"))
                .or_else(|| record.pointer("/dataset_overview/splits"))
                .or_else(|| metadata.get("split"))
                .and_then(crate::research_metadata::canonical_split);
            if let Some(split) = split {
                descriptor["split"] = split;
            }
        }
        if kind == "model" {
            descriptor["family"] = metadata
                .get("model_family")
                .or_else(|| metadata.get("family"))
                .cloned()
                .unwrap_or(Value::Null);
            descriptor["architecture"] = metadata
                .pointer("/research/model/architecture")
                .or_else(|| metadata.get("architecture"))
                .or_else(|| metadata.get("policy_type"))
                .cloned()
                .unwrap_or(Value::Null);
        }
        Ok(Some(descriptor))
    }

    fn attach_catalog_inputs(
        &self,
        entity: &mut Value,
        cache: &mut BTreeMap<String, Option<Value>>,
    ) -> Result<()> {
        let Some(output @ ("model" | "experiment" | "run")) = entity["kind"].as_str() else {
            return Ok(());
        };
        let mut datasets = Vec::new();
        let mut policies = Vec::new();
        for relation in entity["relations"].as_array().into_iter().flatten() {
            let Some(kind) = catalog_input_kind(relation, &entity["catalog_input_kinds"], output)
            else {
                continue;
            };
            let Some(id) = relation["id"]
                .as_str()
                .or_else(|| relation["target"].as_str())
            else {
                continue;
            };
            let history = entity["loom_history"].is_object();
            let identity = if history {
                let source = &entity["loom_history"];
                json!({"source":source["source"],"snapshot":source["snapshot"],"id":id,
                    "revision":relation["revision"].as_str().map(str::to_owned).or_else(||relation["revision"].as_i64().map(|n|n.to_string()))})
            } else {
                json!({"id":id,"revision":relation["revision"]})
            };
            let key = encode(&json!([history, kind, identity]))?;
            if !cache.contains_key(&key) {
                cache.insert(
                    key.clone(),
                    self.catalog_input_descriptor(&identity, kind, history)?,
                );
            }
            if let Some(descriptor) = &cache[&key] {
                let list = if kind == "dataset" {
                    &mut datasets
                } else {
                    &mut policies
                };
                if !list.contains(descriptor) {
                    list.push(descriptor.clone());
                }
            }
        }
        entity["catalog_inputs"] = json!({"datasets":datasets,"policies":policies});
        entity
            .as_object_mut()
            .unwrap()
            .remove("catalog_input_kinds");
        Ok(())
    }
}

enum CatalogContext {
    Full,
    SoftwareList,
    SoftwareOverview,
    OutputList,
    OutputOverview,
    History,
    Records,
    Queue(Value),
    CompactQueue(Value),
}

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
    pub(crate) fn dashboard_domains(&self) -> Result<Value> {
        if !self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='rust_dashboard_versions')",
            [],
            |r| r.get::<_, bool>(0),
        )? {
            return Ok(Value::Null);
        }
        let mut versions = serde_json::Map::new();
        for row in rows(
            &self.db,
            "SELECT domain,version FROM rust_dashboard_versions",
            &[],
        )? {
            versions.insert(
                row["domain"].as_str().unwrap_or("").to_owned(),
                row["version"].clone(),
            );
        }
        let fresh: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM rust_runtime_observation WHERE observed>?)",
            [now() - 30.],
            |r| r.get(0),
        )?;
        versions.insert("runtime_fresh".into(), json!(fresh));
        Ok(Value::Object(versions))
    }
    pub fn dashboard_catalog(&self) -> Result<Value> {
        self.dashboard_catalog_selected(None)
    }
    pub fn dashboard_catalog_selected(&self, selected: Option<&[Value]>) -> Result<Value> {
        self.catalog_projection(selected, CatalogContext::Full)
    }

    fn catalog_projection(
        &self,
        selected: Option<&[Value]>,
        context: CatalogContext,
    ) -> Result<Value> {
        let all = selected.is_none();
        let output_list = matches!(context, CatalogContext::OutputList);
        let output_overview = matches!(context, CatalogContext::OutputOverview);
        let history = matches!(context, CatalogContext::History);
        let compact_queue = if let CatalogContext::CompactQueue(ref queue) = context {
            Some(queue)
        } else {
            None
        };
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
            let d = if history { self.history_job_detail(id)? } else { self.job_detail(id)? };
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
                .query_row("SELECT root FROM execution_memberships WHERE job=?", [id], |r| {
                    r.get(0)
                })
                .optional()?;
            entities.push(json!({"id":id,"kind":"job","namespace":"loom","label":d["presentation"]["label"].as_str().unwrap_or(spec["kind"].as_str().unwrap_or(id)),"state":d["state"],"created":d["created"],"attempt":d["attempt"],"properties":{"id":id,"state":d["state"],"status":d["status"],"created":d["created"],"attempt":d["attempt"],"owner":d["owner"],"record_role":"loom_job","category":d["category"],"execution_mode":d["execution_mode"],"pilot":spec["pilot"],"queued_at":d["created"],"started_at":d["work_timeline"].get("started").or_else(||d["timeline"].get("started")).unwrap_or(&started),"process_exited_at":d["work_timeline"].get("ended").or_else(||d["timeline"].get("ended")).unwrap_or(&ended),"artifact_ready":d["result"]["artifact_ready"],"recovery":d["recovery"],"output_storage":spec["output_storage"],"consolidated_into":d["consolidation"]["consolidated_into"],"internal_step_count":d["internal_steps"]["steps"].as_array().map(|a|a.len()).unwrap_or(0),"attempt_count":d["attempt_count"],"workflow_root":root},"attempts":attempts,"internal_steps":d["internal_steps"],"execution_intervals":d["execution_intervals"],"loom_job":{"id":id},"presentation":d["presentation"],"scenario_ref":d["scenario_ref"],"relations":[],"artifacts":[]}));
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
            let d = if let Some(queue) = compact_queue {
                queue["logical_entries"].as_array().into_iter().flatten()
                    .find(|entry| entry["id"]==id).cloned().unwrap_or(Value::Null)
            } else { self.job_detail(id)? };
            let members = d.get("units").unwrap_or(&d["executions"]);
            let node_count = members.as_array().map(Vec::len).unwrap_or(0);
            entities.push(json!({"id":id,"kind":"job","namespace":"loom","label":d["spec"]["label"],"state":d["state"],"created":d["created"],"properties":{"record_role":"loom_job","id":id,"state":d["state"],"status":d["status"],"queued_at":d["created"],"started_at":d["execution_intervals"].as_array().and_then(|a|a.iter().filter_map(|i|i["start"].as_f64()).min_by(f64::total_cmp)),"process_exited_at":if d["state"]=="complete"{d["execution_intervals"].as_array().and_then(|a|a.iter().filter_map(|i|i["end"].as_f64()).max_by(f64::total_cmp))}else{None},"owner":d["owner"],"execution_node_count":node_count,"internal_step_count":node_count},"loom_workflow":d,"loom_job":{"id":id},"steps":members,"execution_intervals":d["execution_intervals"],"presentation":d["presentation"],"scenario_ref":d["scenario_ref"],"relations":[],"artifacts":[]}));
            if let Some(entity) = entities.last_mut() {
                for key in ["owner", "owner_name", "owner_profile"] {
                    entity[key] = d[key].clone();
                }
            }
            ids.insert(id.to_string());
        }
        // Discover actual groups once. Running the recursive family query for
        // every unrelated Job makes a flat catalog quadratic in its Job count.
        if !history && compact_queue.is_none() {
            let parents_with_children: std::collections::BTreeSet<String> = rows(
            &self.db,
            &format!("WITH {JOB_LINKS_CTE} SELECT DISTINCT parent FROM job_links WHERE parent IS NOT NULL"),
            &[],
        )?.into_iter().filter_map(|r| r["parent"].as_str().map(String::from)).collect();
            for entity in &mut entities {
                let id = entity["id"].as_str().unwrap_or("").to_string();
                let root = self.job_family_root(&id)?;
                entity["group_root"] = json!(root);
                if root == id {
                    entity["group_summary"] = if parents_with_children.contains(&id) {
                        self.job_group_summary(&id)?
                    } else {
                        Value::Null
                    };
                }
            }
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
            let producer_job = row["job"].as_str().or_else(|| entity["job"].as_str()).map(str::to_owned);
            entity["loom_job"] = json!({"id":producer_job});
            let registered: Option<f64> = self.db.query_row(
                "SELECT MIN(created) FROM events WHERE job=? AND attempt=? AND code='complete'",
                rusqlite::params![producer_job, entity["attempt"].as_str()],
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
                if entity["kind"] == "experiment" && entity["reporting"]["checkpoint_sweep"].is_null() {
                    if let Some(summary) = self.legacy_checkpoint_evaluation(&entity, &record.spec["params"]) {
                        entity["reporting"]["checkpoint_sweep"] = summary;
                        entity["reporting"]["primary_outcome"] = Value::Null;
                    }
                }
                entity["presentation"] = record
                    .spec
                    .get("presentation")
                    .cloned()
                    .unwrap_or(json!({"scope":"research","role":"main"}));
                entity["scenario_ref"] = record.spec["scenario_ref"].clone();
                entity["catalog_input_kinds"] = record.spec["profile"]["reporting"]["input_kinds"].clone();
            }
            if !output_list && !output_overview {
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
            }
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
                if matches!(
                    identity["kind"].as_str(),
                    Some("model" | "experiment" | "run")
                ) {
                    // The catalog envelope pins the exact publication revision.
                    // Fold the two copies only if the normalized original and
                    // recorded file inventory also match.
                    let publication = match identity["source"].as_str() {
                        Some("catalog") => body
                            .pointer("/original/publication/entity_revision")
                            .and_then(Value::as_u64),
                        Some("publication") => identity["revision"]
                            .as_str()
                            .and_then(|v| v.parse::<u64>().ok()),
                        _ => None,
                    };
                    let key = if let Some(revision) = publication {
                        json!([
                            identity["kind"],
                            identity["id"],
                            revision,
                            crate::dataset_lineage::mirror_record(body, true)
                        ])
                    } else {
                        json!([
                            identity["source"],
                            identity["kind"],
                            identity["id"],
                            identity["revision"],
                            body
                        ])
                    };
                    entity["catalog_mirror_key"] = json!(hash(encode(&key)?));
                }
                entity["migration"] = json!({"read_only":true,"source":identity["source"],"snapshot":identity["snapshot"]});
                // The index descriptor already resolved legacy presentation overlays.
                // Keep list filters, labels and the displayed role consistent.
                if !entity["presentation"].is_object() {
                    entity["presentation"] = json!({});
                }
                for field in ["scope", "role"] {
                    if item[field].is_string() {
                        entity["presentation"][field] = item[field].clone();
                    }
                }
                if item["label"].is_string() {
                    entity["label"] = item["label"].clone();
                }

                if !entity["label"].is_string() {
                    entity["label"] = identity["id"].clone();
                }
                if !output_list && !output_overview && !history {
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
                }
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
        let software_list = matches!(context, CatalogContext::SoftwareList);
        let software_overview = matches!(context, CatalogContext::SoftwareOverview);
        let has_profile_selection = selected.is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item["native_ref"]["kind"].as_str(),
                    Some("profile" | "component" | "tool" | "runtime")
                )
            })
        });
        let profile_runtimes = if has_profile_selection {
            self.runtime_snapshot()?
        } else {
            Value::Null
        };
        let profile_pins: Vec<_> = selected
            .unwrap_or_default()
            .iter()
            .filter(|item| item["native_ref"]["kind"] == "profile")
            .filter_map(|item| {
                let native = &item["native_ref"]["identity"];
                Some((
                    native["kind"]
                        .as_str()
                        .or_else(|| item["profile_kind"].as_str())?
                        .to_owned(),
                    native["sha256"]
                        .as_str()
                        .or_else(|| item["profile_sha256"].as_str())
                        .map(str::to_owned),
                ))
            })
            .collect();
        let profile_usage = if software_list || software_overview {
            self.browse_profile_usage(&profile_pins)?
        } else {
            BTreeMap::new()
        };
        let mut profiles = if all {
            self.profile_listing()?
        } else {
            let mut profiles = vec![];
            for (kind, sha) in profile_pins {
                let value = if software_list || software_overview {
                    let empty = json!({});
                    let usage = profile_usage
                        .get(&(kind.clone(), sha.clone()))
                        .unwrap_or(&empty);
                    self.browse_profile(
                        &kind,
                        sha.as_deref(),
                        &profile_runtimes,
                        usage,
                        software_list,
                    )?
                } else {
                    self.profile_listing_with_runtime_snapshot(
                        Some(&kind),
                        sha.as_deref(),
                        &profile_runtimes,
                    )?
                };
                if let Value::Array(items) = value {
                    profiles.extend(items);
                }
            }
            json!(profiles)
        };
        if !all && !software_list {
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
                    if let Ok(value) = self.profile_listing_with_runtime_snapshot(
                        Some(&kind),
                        sha.as_deref(),
                        &profile_runtimes,
                    ) {
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
        let wants_runtime = selected.is_none_or(|items| {
            items.iter().any(|item| {
                matches!(
                    item["native_ref"]["kind"].as_str(),
                    Some("runtime" | "dependency")
                )
            })
        });
        let mut runtime_catalog = if wants_runtime {
            let pins = selected.map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        if item["native_ref"]["kind"] != "runtime" {
                            return None;
                        }
                        let pin = &item["native_ref"]["identity"];
                        Some((pin["id"].as_str()?.to_owned(), pin["revision"].as_i64()?))
                    })
                    .collect::<Vec<_>>()
            });
            self.runtime_projection(pins.as_deref(), !(software_list || software_overview))?
        } else {
            json!({"runtimes":[],"dependency_catalog":{"dependencies":[]}})
        };
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
        let runtime_counts = if software_list {
            self.runtime_started_counts(runtime_catalog["runtimes"].as_array().unwrap())?
        } else {
            Vec::new()
        };
        for item in runtime_catalog["runtimes"].as_array_mut().unwrap() {
            if let Some(count) = runtime_counts
                .iter()
                .find(|r| r["id"] == item["id"] && r["revision"] == item["revision"])
            {
                item["started_job_count"] = count["started"].clone();
            }
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
            if software_list || software_overview {
                if software_list {
                    item["summary_only"] = json!(true);
                }
                item["manifest"]
                    .as_object_mut()
                    .map(|manifest| manifest.remove("dependencies"));
            }
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
            // A preserved DTO has a common Item ID, while its curation key is
            // the original source identity. Task membership remains independent.
            let historical = entity.get("loom_history").filter(|h| h.is_object());
            let overlay_kind = if historical.is_some() && keykind == "run" {
                "experiment"
            } else {
                keykind
            };
            let overlay_id = historical
                .and_then(|h| h["id"].as_str())
                .or_else(|| entity["id"].as_str());
            let overlay: Option<String> = self
                .db
                .query_row(
                    "SELECT presentation FROM research_overlays WHERE kind=? AND id=?",
                    rusqlite::params![overlay_kind, overlay_id],
                    |r| r.get(0),
                )
                .optional()?;
            let mut presentation = entity
                .get("presentation")
                .cloned()
                .unwrap_or_else(|| json!({"scope":"research","role":"main"}));
            if let Some(overlay) = overlay {
                let curated: Value = serde_json::from_str(&overlay)?;
                if matches!(
                    entity["kind"].as_str(),
                    Some("model" | "dataset" | "experiment" | "run")
                ) && curated["label"].is_string()
                {
                    entity["label"] = curated["label"].clone();
                }
                presentation
                    .as_object_mut()
                    .unwrap()
                    .extend(curated.as_object().unwrap().clone());
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
        // Whole-catalog compatibility/curation reads stay metadata-only. File
        // enrichment belongs to bounded pages on the independent query lane.
        for entity in &mut entities {
            if all || output_list || output_overview {
                if matches!(
                    entity["kind"].as_str(),
                    Some("model" | "dataset" | "experiment" | "run")
                ) {
                    let mut summary = crate::research_metadata::summarize(entity, &[]);
                    summary["sources"] = json!([]);
                    entity["research_summary"] = summary;
                }
            } else {
                crate::research_metadata::enrich(self, entity);
            }
            if matches!(
                entity["kind"].as_str(),
                Some("model" | "dataset" | "experiment" | "run")
            ) {
                crate::research_metadata::attach_materialized(self, entity)?;
                crate::metadata_templates::attach(self, entity)?;
                crate::dataset_lineage::attach(self, entity)?;
            }
        }
        if (output_list || output_overview)
            && self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='rust_browse_summaries')",
                [],
                |r| r.get::<_, bool>(0),
            )?
        {
            for entity in &mut entities {
                if let Some(target) = crate::research_metadata::snapshot_target(entity) {
                    if let Some(item) = selected.unwrap_or_default().iter().find(|item| {
                        item["native_ref"]["kind"] == target.kind
                            && item["native_ref"]["identity"] == target.identity
                    }) {
                        self.attach_browse_summary(entity, &item["revision_id"])?;
                    }
                }
            }
        }
        self.attach_task_context(&mut entities)?;
        let mut input_descriptors = BTreeMap::new();
        for entity in &mut entities {
            crate::dataset_organization::attach(self, entity)?;
            self.attach_catalog_inputs(entity, &mut input_descriptors)?;
            self.attach_output_disposition(entity)?;
            if let Some(id) = entity["loom_registry"]["id"].as_str().map(str::to_owned) {
                if matches!(entity["kind"].as_str(), Some("dataset" | "model")) {
                    self.attach_output_edit(entity, &id)?;
                }
            }
        }
        let mut visible = Vec::with_capacity(entities.len());
        for entity in entities {
            if !self.dashboard_entity_is_system(&entity)? {
                visible.push(entity);
            }
        }
        let mut entities = visible;
        for job in entities.iter_mut().filter(|entity| entity["kind"] == "job") {
            let properties = &mut job["properties"];
            // The Job's exact spec retains older mode values. A Dashboard label
            // needs the recorded purpose and reduced scope of an actual pilot.
            if properties["execution_mode"] == "pilot"
                && !["preflight_for", "reduced_scope"].into_iter().all(|field| {
                    properties["pilot"][field]
                        .as_str()
                        .is_some_and(|value| !value.trim().is_empty())
                })
            {
                properties["execution_mode"] = Value::Null;
            }
        }
        for entity in entities
            .iter_mut()
            .filter(|e| e["kind"] == "job" && e["loom_history"].is_null())
        {
            let id = entity["id"].as_str().unwrap_or("");
            if let Some(unit) = self.unit_summary(id)? {
                entity["entry_type"] = json!("unit");
                entity["unit"] = unit;
                entity["presentation"]
                    .as_object_mut()
                    .unwrap()
                    .remove("parent_job");
                entity["properties"]
                    .as_object_mut()
                    .unwrap()
                    .remove("workflow_root");
            } else if self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM research_jobs WHERE id=?)",
                [id],
                |r| r.get::<_, bool>(0),
            )? {
                entity["entry_type"] = json!("job");
                if let Some(count) = entity["properties"]
                    .as_object_mut()
                    .unwrap()
                    .remove("execution_node_count")
                {
                    entity["properties"]["unit_count"] = count;
                }
            }
        }

        for entity in entities
            .iter_mut()
            .filter(|e| e["kind"] == "job" && e["entry_type"].is_null())
        {
            let id = entity["loom_history"]["id"]
                .as_str()
                .or_else(|| entity["id"].as_str())
                .unwrap_or("");
            if let Some(review) = self.work_review(id)? {
                entity["entry_type"] = review["entry_type"].clone();
                if review["entry_type"] == "unit" {
                    entity["unit"] = json!({"job_id":review["job_id"]});
                }
                entity["reviewed_work"] = review;
            }
        }

        if let Some(runtimes) = runtime_catalog["runtimes"].as_array_mut() {
            runtimes.retain(|r| r["owner_kind"] != "system");
        }
        if compact_queue.is_some() {
            for entity in &mut entities {
                let fields = entity.as_object_mut().unwrap();
                for key in ["loom_workflow", "steps", "attempts", "internal_steps"] {
                    fields.remove(key);
                }
                fields.insert("summary_only".into(), json!(true));
            }
        }
        if software_list {
            for entity in &mut entities {
                entity["summary_only"] = json!(true);
            }
        }
        if output_list {
            for entity in &mut entities {
                let fields = entity.as_object_mut().unwrap();
                fields.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "id" | "kind"
                            | "label"
                            | "namespace"
                            | "presentation"
                            | "origin"
                            | "summary"
                            | "keywords"
                            | "owner"
                            | "owner_name"
                            | "owner_kind"
                            | "owner_profile"
                            | "loom_registry"
                            | "loom_history"
                            | "loom_job"
                            | "migration"
                            | "scenario_ref"
                            | "task_refs"
                            | "catalog_inputs"
                            | "relations"
                            | "catalog_mirror_key"
                            | "dataset_organization"
                            | "dataset_lineage"
                            | "dataset_overview"
                            | "dataset_mirror_key"
                            | "disposition"
                            | "output_edit"
                            | "research_summary"
                            | "properties"
                            | "reporting"
                            | "comparison"
                            | "stage_sequence"
                            | "outcome_highlights"
                            | "model_role"
                            | "metrics"
                    )
                });
                if let Some(properties) =
                    fields.get_mut("properties").and_then(Value::as_object_mut)
                {
                    properties.retain(|key, _| {
                        matches!(
                            key.as_str(),
                            "model_family"
                                | "family"
                                | "architecture"
                                | "policy_type"
                                | "model_role"
                                | "role"
                                | "importance"
                                | "registered_at"
                                | "selected_step"
                        )
                    });
                }
                if let Some(relations) = fields.get_mut("relations").and_then(Value::as_array_mut) {
                    relations.retain(|relation| {
                        matches!(
                            relation["kind"].as_str(),
                            Some("evaluated_model" | "evaluates" | "evaluated_on" | "consumes")
                        )
                    });
                }
                if let Some(reporting) = fields.get_mut("reporting").and_then(Value::as_object_mut)
                {
                    reporting.retain(|key, _| {
                        matches!(
                            key.as_str(),
                            "checkpoint_sweep" | "primary_outcome" | "origin" | "activity"
                        )
                    });
                }
                if let Some(overview) = fields
                    .get_mut("dataset_overview")
                    .and_then(Value::as_object_mut)
                {
                    overview.retain(|key, _| key == "role");
                }
                if let Some(summary) = fields
                    .get_mut("research_summary")
                    .and_then(Value::as_object_mut)
                {
                    summary.remove("sources");
                }
                fields.insert("summary_only".into(), json!(true));
            }
        }
        let mut value = json!({"entities":entities,"revision":revision});
        match context {
            CatalogContext::Full => {
                value["queue"] = self.dashboard_queue()?;
                value["research_tasks"] = self.dashboard_tasks()?;
                value["sources"] =
                    json!([{"id":"loom-ops","status":"ok"},{"id":"catalog","status":"ok"}]);
                value["capabilities"] = json!({"curate":true,"scenarios":true,"item_search":true,"metadata_templates":true,"reporting":true,"output_disposition":true});
                value["host"] = self.dashboard_host()?;
                value["job_contract"] =
                    serde_json::from_str(include_str!("../../../contracts/job-contract.json"))?;
                value["runtime_catalog"] = runtime_catalog;
            }
            CatalogContext::Records
            | CatalogContext::SoftwareList
            | CatalogContext::SoftwareOverview
            | CatalogContext::OutputList
            | CatalogContext::OutputOverview => {
                value["research_tasks"] = self.dashboard_tasks()?;
                if wants_runtime {
                    value["runtime_catalog"] = runtime_catalog;
                }
            }
            CatalogContext::History => {}
            CatalogContext::Queue(queue) => value["queue"] = queue,
            CatalogContext::CompactQueue(mut queue) => {
                for field in ["entries", "logical_entries", "job_entries", "unit_entries"] {
                    for entry in queue[field].as_array_mut().into_iter().flatten() {
                        let fields = entry.as_object_mut().unwrap();
                        if let Some(spec) = fields.remove("spec") {
                            fields.insert("label".into(), spec["label"].clone());
                        }
                        for key in [
                            "result",
                            "errors",
                            "execution_intervals",
                            "executions",
                            "annotations",
                        ] {
                            fields.remove(key);
                        }
                        if let Some(progress) = fields.get_mut("progress") {
                            let steps = progress["steps"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(|s| json!({"label":s["label"],"state":s["state"]}))
                                .collect::<Vec<_>>();
                            *progress = json!({"steps":steps});
                        }
                    }
                }
                value["queue"] = queue;
            }
        }
        self.decorate_registry(&mut value)?;
        if all {
            *self
                .catalog_cache
                .lock()
                .map_err(|_| Error::new("projection", "Projection cache unavailable", 503))? =
                Some((revision, value.clone()));
        }
        Ok(value)
    }
    /// Login and revision refresh need no Job, Activity, file or runtime projection.
    pub fn dashboard_shell(&self) -> Result<Value> {
        Ok(json!({
            "revision":self.dashboard_revision()?, "domains":self.dashboard_domains()?, "entities":[], "host":self.dashboard_host()?,
            "research_tasks":self.dashboard_tasks()?, "sources":[{"id":"loom-ops","status":"ok"},{"id":"catalog","status":"ok"}],
            "job_contract":serde_json::from_str::<Value>(include_str!("../../../contracts/job-contract.json"))?,
            "capabilities":{"catalog_pages":true,"live_queue":true,"job_units":true,"scenarios":true,"item_search":true,"metadata_templates":true,"reporting":true,"output_disposition":true,"compact_lists":true},
            "initial_page_complete":false
        }))
    }

    pub fn dashboard_occupancy(&self, actor: &crate::auth::Actor) -> Result<Value> {
        self.dashboard_occupancy_with_detail(actor, false)
    }
    pub(crate) fn dashboard_occupancy_with_detail(
        &self,
        actor: &crate::auth::Actor,
        compact: bool,
    ) -> Result<Value> {
        let mut queue = self.dashboard_queue_with_detail(compact)?;
        crate::query::queue_permissions(
            &mut queue,
            !self.read_only && actor.scopes.iter().any(|s| s == "submit"),
        );
        crate::query::project_queue_types(&mut queue);
        let ids: std::collections::BTreeSet<_> = ["entries", "logical_entries"]
            .iter()
            .flat_map(|key| queue[*key].as_array().into_iter().flatten())
            .filter_map(|entry| entry["id"].as_str())
            .collect();
        let selected: Vec<_> = ids
            .into_iter()
            .map(|id| json!({"native_ref":{"kind":"job","identity":{"id":id}}}))
            .collect();
        if selected.is_empty() {
            return Ok(json!({"queue":queue,"entities":[],"revision":self.dashboard_revision()?}));
        }
        let context = if compact {
            CatalogContext::CompactQueue(queue)
        } else {
            CatalogContext::Queue(queue)
        };
        self.catalog_projection(Some(&selected), context)
    }

    pub fn dashboard_bootstrap(&self) -> Result<Value> {
        let mut selected = vec![];
        for row in rows(&self.db,"SELECT r.body FROM catalog_items i JOIN catalog_revisions r ON r.id=i.current_revision WHERE i.deleted=0 AND i.kind IN('scenario','job','dataset','model','experiment','artifact','profile','component','tool','runtime','dependency') ORDER BY i.rowid DESC LIMIT 50",&[])? {
            selected.push(serde_json::from_str::<Value>(row["body"].as_str().unwrap_or("{}"))?);
        }
        selected.retain(|v| {
            crate::registry_lifecycle::dto_target(v).is_none_or(|t| {
                crate::registry_lifecycle::state(&self.db, &t)
                    .is_ok_and(|s| s["state"] != "archived")
            })
        });
        let queue = self.dashboard_queue()?;
        for entry in ["entries", "logical_entries"]
            .iter()
            .flat_map(|key| queue[key].as_array().into_iter().flatten())
            .take(200)
        {
            let id = &entry["id"];
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
        catalog["activity"] = self.dashboard_activity("all")?;
        catalog["activity"]["view"] = json!("all");
        catalog["capabilities"]["catalog_pages"] = json!(true);
        catalog["initial_page_complete"] = json!(false);
        Ok(catalog)
    }
    pub fn dashboard_entity(&self, args: &Value) -> Result<Value> {
        let item = self.get_item(args)?;
        let section = args["section"].as_str();
        if let Some(section) = section.filter(|section| *section != "overview") {
            if self.system_native(&item["native_ref"])? {
                return Err(Error::new("not_found", "Item not found", 404));
            }
            return match section {
                "files" | "training" => self.dashboard_output_section(&item, section, args),
                _ => self.dashboard_software_section(&item, section, args),
            };
        }
        let software_overview = section == Some("overview")
            && matches!(
                item["native_ref"]["kind"].as_str(),
                Some("runtime" | "profile")
            );
        let output_overview = section == Some("overview")
            && matches!(
                item["kind"].as_str(),
                Some("dataset" | "model" | "experiment" | "run")
            )
            && matches!(
                item["native_ref"]["kind"].as_str(),
                Some("entity" | "historical")
            );
        let mode = if software_overview {
            CatalogContext::SoftwareOverview
        } else if output_overview {
            CatalogContext::OutputOverview
        } else if args["records_only"] == true || section == Some("overview") {
            CatalogContext::Records
        } else {
            CatalogContext::Full
        };
        let mut catalog = self.catalog_projection(Some(std::slice::from_ref(&item)), mode)?;
        for entity in catalog["entities"].as_array_mut().unwrap() {
            if software_overview {
                entity["deferred_software_sections"] = json!(true);
            }
            if output_overview {
                entity["deferred_output_sections"] = json!(true);
            }
            if let Some(id) = entity["loom_registry"]["id"].as_str().map(str::to_owned) {
                self.attach_policy_checkpoints(entity, &id, None)?;
            }
            if !output_overview {
                self.attach_policy_training_revisions(entity)?;
            }
            entity["item_id"] = item["item_id"].clone();
            entity["item_revision_id"] = item["revision_id"].clone();
            if item["catalog_class"].is_string() {
                entity["catalog_class"] = item["catalog_class"].clone();
            }
        }
        Ok(catalog)
    }
    fn dashboard_output_section(&self, item: &Value, section: &str, args: &Value) -> Result<Value> {
        let native = &item["native_ref"];
        let pin = &native["identity"];
        if section == "training" && native["kind"] == "entity" && item["kind"] == "model" {
            let mut model = json!({"kind":"model","loom_registry":pin});
            self.attach_policy_training_revisions(&mut model)?;
            return Ok(json!({"section":section,"items":model["policy_training_revisions"]}));
        }
        if section != "files" || !matches!(native["kind"].as_str(), Some("entity" | "historical")) {
            return Err(Error::input("Section does not belong to the selected Item"));
        }
        let offset = args["offset"].as_u64().unwrap_or(0).min(1_000_000) as usize;
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 100) as usize;
        let (manifest, mut locations) = if native["kind"] == "entity" {
            let manifest: String = self.db.query_row(
                "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
                rusqlite::params![pin["id"].as_str(), pin["revision"].as_i64()],
                |r| r.get(0),
            )?;
            let locations = rows(
                &self.db,
                "SELECT * FROM all_entity_locations WHERE id=? AND revision=?
                ORDER BY component,storage_id,machine_id,ref LIMIT ? OFFSET ?",
                &[
                    &pin["id"].as_str(),
                    &pin["revision"].as_i64(),
                    &((limit + 1) as i64),
                    &(offset as i64),
                ],
            )?;
            (serde_json::from_str(&manifest)?, locations)
        } else {
            let (manifest, mut locations) = self.historical_files(pin)?;
            locations.sort_by(|a, b| {
                [
                    a["component"].as_str(),
                    a["storage_id"].as_str(),
                    a["ref"].as_str(),
                ]
                .cmp(&[
                    b["component"].as_str(),
                    b["storage_id"].as_str(),
                    b["ref"].as_str(),
                ])
            });
            (
                manifest,
                locations.into_iter().skip(offset).take(limit + 1).collect(),
            )
        };
        let next = (locations.len() > limit).then_some(offset + limit);
        locations.truncate(limit);
        let (files, media) = crate::media::file_dtos(pin, &manifest, &locations, &self.config);
        Ok(json!({"section":section,"items":files,"media":media,"next_offset":next}))
    }

    fn dashboard_software_section(
        &self,
        item: &Value,
        section: &str,
        args: &Value,
    ) -> Result<Value> {
        let native = &item["native_ref"];
        let pin = &native["identity"];
        let offset = args["offset"].as_u64().unwrap_or(0).min(1_000_000) as usize;
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as usize;
        if section == "profile_usage" && native["kind"] == "profile" {
            let kind = pin["kind"]
                .as_str()
                .or_else(|| item["profile_kind"].as_str())
                .ok_or_else(|| Error::input("Profile identity required"))?;
            let profiles = self.profile_listing_qualified(Some(kind), pin["sha256"].as_str())?;
            let profile = &profiles[0];
            let jobs = profile["jobs"].as_array().map(Vec::as_slice).unwrap_or(&[]);
            return Ok(
                json!({"section":section,"items":jobs.iter().skip(offset).take(limit).collect::<Vec<_>>(),
                "next_offset":(offset+limit<jobs.len()).then_some(offset+limit),
                "complete":profile["jobs_complete"]}),
            );
        }
        if native["kind"] != "runtime" || !matches!(section, "runtime_usage" | "runtime_inventory")
        {
            return Err(Error::input("Section does not belong to the selected Item"));
        }
        let snapshot = self.runtime_snapshot()?;
        let mut runtime = snapshot["runtimes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| {
                r["id"] == pin["id"]
                    && r["revision"] == pin["revision"]
                    && (!pin["sha256"].is_string() || r["sha256"] == pin["sha256"])
            })
            .cloned()
            .ok_or_else(|| Error::new("not_found", "Runtime revision is unavailable", 404))?;
        if section == "runtime_usage" {
            self.attach_runtime_usage_page(&mut runtime, offset, limit)?;
            return Ok(json!({"section":section,"items":runtime["job_usage"],
                "next_offset":(runtime["jobs_complete"]==false).then_some(offset+limit)}));
        }
        let origin = args["origin"].as_str().unwrap_or("managed");
        let text = args["text"].as_str().unwrap_or("").to_lowercase();
        let matching = runtime["manifest"]["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| {
                let source = match (
                    d["deprecated"].as_bool(),
                    d["origin"]["type"].as_str(),
                    d["scope"].as_str(),
                ) {
                    (Some(true), _, _) => "deprecated",
                    (_, Some("managed-directory"), _) => "managed",
                    (_, Some("external-mount"), _) => "external",
                    (_, Some("image" | "bundled-extension"), _)
                    | (_, _, Some("image" | "image-os" | "isaac-extension")) => "image",
                    _ => "unknown",
                };
                (origin == "all" || origin == source)
                    && (text.is_empty()
                        || ["name", "version", "scope"]
                            .into_iter()
                            .filter_map(|key| d[key].as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                            .to_lowercase()
                            .contains(&text))
            })
            .skip(offset)
            .take(limit + 1)
            .collect::<Vec<_>>();
        Ok(
            json!({"section":section,"items":matching.iter().take(limit).collect::<Vec<_>>(),
            "next_offset":(matching.len()>limit).then_some(offset+limit)}),
        )
    }

    fn dashboard_directory(&self, args: &Value) -> Result<Value> {
        let directory = args["directory"].as_str().unwrap_or("");
        let mut parts = directory.splitn(3, '/');
        let selector = match (parts.next(), parts.next()) {
            (Some("data"), Some(name)) => format!("data/{name}"),
            _ => return Err(Error::input("Registered storage selector required")),
        };
        let prefix = parts.next().unwrap_or("");
        crate::error::require(
            !prefix.contains('\0')
                && (prefix.is_empty()
                    || prefix
                        .split('/')
                        .all(|part| !matches!(part, "" | "." | ".."))),
            "input",
            "Invalid directory",
            400,
        )?;
        let prefix = if prefix.is_empty() {
            "".to_owned()
        } else {
            format!("{prefix}/")
        };
        let storage_id = if selector == "data/0" {
            self.config.storage_id.as_str()
        } else {
            self.config
                .data_stores
                .get(&selector)
                .and_then(|store| store["storage_id"].as_str())
                .ok_or_else(|| Error::input("Unknown storage selector"))?
        };
        let db = crate::index_store::read(self)?;
        let system = crate::system_ownership::native_sql(
            "json_extract(parent.body,'$.native_ref')",
            "authority.",
        );
        let offset = args["offset"].as_u64().unwrap_or(0).min(1_000_000) as i64;
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 100) as i64;
        let sql = format!("WITH local_files AS (
            SELECT substr(json_extract(c.value,'$.ref'),length(?1)+1) relative,
                   json_extract(parent.body,'$.native_ref') native_ref,
                   json_extract(parent.body,'$.item_id') item_id,json_extract(parent.body,'$.revision_id') revision_id,
                   parent.kind,parent.label,
                   json_extract(f.body,'$.entry_key') component,json_extract(f.body,'$.size') size,
                   json_extract(c.value,'$.ref') ref,json_extract(c.value,'$.status') status,
                   f.recorded, f.id file_id
            FROM docs f JOIN docs parent ON parent.id=f.parent AND parent.valid_to IS NULL
            JOIN authority.catalog_items current ON current.id=parent.id AND current.deleted=0 AND current.current_revision=parent.revision
            JOIN json_each(f.body,'$.copies') c WHERE f.kind='file' AND f.valid_to IS NULL
              AND parent.kind IN ('dataset','model','experiment','artifact','run')
              AND json_extract(f.body,'$.owner_revision_id')=json_extract(parent.body,'$.revision_id')
              AND json_extract(c.value,'$.machine_id')=?2 AND json_extract(c.value,'$.storage_id')=?3
              AND substr(json_extract(c.value,'$.ref'),1,length(?1))=?1
              AND NOT COALESCE(({system}),0)
        ), children AS (
            SELECT CASE WHEN instr(relative,'/')>0 THEN substr(relative,1,instr(relative,'/')-1)
                        ELSE relative END name, instr(relative,'/')>0 folder,local_files.* FROM local_files
        ), chosen AS (
            SELECT *,row_number() OVER(PARTITION BY name,folder ORDER BY (status='available') DESC,recorded DESC,file_id) n
            FROM children WHERE name NOT IN ('','.','..')
        ) SELECT name,folder,native_ref,item_id,revision_id,kind,label,component,size,ref,status
          FROM chosen WHERE n=1 ORDER BY folder DESC,name LIMIT ?4 OFFSET ?5");
        let candidates = rows(
            &db,
            &sql,
            &[
                &prefix,
                &self.config.machine_id,
                &storage_id,
                &(limit + 1),
                &offset,
            ],
        )?;
        let mut children = Vec::new();
        for row in candidates.iter().take(limit as usize) {
            if row["folder"] == 1 {
                children.push(json!({"name":row["name"],"folder":true}));
                continue;
            }
            let native: Value = serde_json::from_str(row["native_ref"].as_str().unwrap_or("{}"))?;
            let (files, media) = crate::media::file_dtos(
                &native["identity"],
                &json!({"components":[{"ref":row["component"],"size":row["size"]}]}),
                &[
                    json!({"ref":row["ref"],"component":row["component"],"storage_id":storage_id,
                         "machine_id":self.config.machine_id,"status":row["status"]}),
                ],
                &self.config,
            );
            children.push(json!({"name":row["name"],"folder":false,"file":files[0],
                "entity":{"id":format!("revision:{}:{}",row["item_id"].as_str().unwrap_or(""),row["revision_id"].as_str().unwrap_or("")),
                    "item_id":row["item_id"],"item_revision_id":row["revision_id"],"kind":row["kind"],
                    "label":row["label"],"loom_media":media,"summary_only":true}}));
        }
        Ok(
            json!({"directory":directory,"children":children,"next_offset":
            (candidates.len()>limit as usize).then_some(offset+limit)}),
        )
    }

    fn dashboard_history(&self, actor: &crate::auth::Actor, args: &Value) -> Result<Value> {
        use rusqlite::named_params;
        let (key, access) = crate::index_store::context(self, actor)?;
        let cursor = args["cursor"]
            .as_str()
            .map(|text| crate::index_store::cursor(text, &key))
            .transpose()?;
        let clock = now();
        if let Some(cursor) = &cursor {
            crate::error::require(
                cursor["projection"] == "history"
                    && cursor["access"] == access
                    && cursor["expires"].as_f64().is_some_and(|t| t >= clock),
                "cursor_expired",
                "History page expired",
                409,
            )?;
        }
        let as_of = cursor
            .as_ref()
            .and_then(|c| c["as_of"].as_f64())
            .unwrap_or(clock);
        let time = cursor.as_ref().and_then(|c| c["time"].as_f64());
        let last = cursor.as_ref().and_then(|c| c["id"].as_str()).unwrap_or("");
        let limit = args["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize;
        let system = crate::system_ownership::native_sql(
            "json_object('kind','job','identity',json_object('id',candidates.id))",
            "",
        );
        let sql = format!("{}, finishes AS MATERIALIZED (
            SELECT job,attempt,ended FROM stage_runs WHERE stage IN ('prepare','run')
            UNION ALL
            SELECT a.job,a.id,a.ended FROM attempts a
             WHERE NOT EXISTS(SELECT 1 FROM stage_runs s WHERE s.job=a.job AND s.attempt=a.id AND s.stage IN ('prepare','run'))
        ), own_finish AS MATERIALIZED (
            SELECT job,max(ended) finish FROM finishes GROUP BY job
        ), frozen_finish AS MATERIALIZED (
            SELECT c.target job,max(f.ended) finish FROM consolidation_paths c JOIN finishes f
              ON f.job=c.source AND f.attempt=c.source_attempt GROUP BY c.target
        ), workflow_finish AS MATERIALIZED (
            SELECT m.root,max(max(COALESCE(t.finish,0),COALESCE(f.finish,0))) finish
            FROM execution_memberships m LEFT JOIN own_finish t ON t.job=m.job
            LEFT JOIN frozen_finish f ON f.job=m.job GROUP BY m.root
        ), preserved AS (
            SELECT h.id,h.source,h.revision,h.snapshot,
                COALESCE(json_extract(h.body,'$.entity.properties.state'),json_extract(h.body,'$.record.state')) state,
                COALESCE(json_extract(h.body,'$.entity.properties.process_exited_at'),
                         json_extract(h.body,'$.entity.properties.ended_at'),
                         json_extract(h.body,'$.entity.properties.completed_at'),
                         json_extract(h.body,'$.entity.properties.finished_at')) ended,
                COALESCE(json_extract(h.body,'$.entity.created'),json_extract(h.body,'$.record.created'),0) created,
                row_number() OVER(PARTITION BY h.id ORDER BY (h.source='queue') DESC,h.rowid DESC) preferred
            FROM history_records h WHERE h.kind='job_view' AND h.source IN ('queue','loom-ops','catalog')
                AND json_extract(h.body,'$.entity.properties.record_role') IN ('work','loom_job')
                AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.id=h.id)
                AND NOT EXISTS(SELECT 1 FROM job_workflows w WHERE w.id=h.id)
        ), candidates AS (
            SELECT j.id,NULL source,NULL revision,NULL snapshot,j.created,
                   NULLIF(max(COALESCE(o.finish,0),COALESCE(f.finish,0)),0) ended
            FROM jobs j LEFT JOIN execution_units u ON u.id=j.id
            LEFT JOIN own_finish o ON o.job=j.id LEFT JOIN frozen_finish f ON f.job=j.id
            WHERE j.state IN ('complete','failed','interrupted','cancelled','result_failed')
                AND u.job_id IS NULL
                AND (j.state IN ('complete','cancelled') OR NOT EXISTS(
                  SELECT 1 FROM recovery r WHERE r.job=j.id AND r.state IN ('pending','waiting','claimed','executing')))
                AND NOT EXISTS(SELECT 1 FROM consolidation_roots c WHERE c.source=j.id
                    AND c.source_attempt IS j.attempt AND j.state='complete')
            UNION ALL
            SELECT w.id,NULL,NULL,NULL,w.created,NULLIF(f.finish,0)
            FROM job_workflows w LEFT JOIN workflow_finish f ON f.root=w.id
            WHERE (EXISTS(SELECT 1 FROM research_jobs r WHERE r.id=w.id)
                   OR COALESCE((SELECT visible FROM reviewed_work r WHERE r.id=w.id),0)=1
                   OR NOT EXISTS(SELECT 1 FROM execution_memberships m WHERE m.root=w.id))
                AND (w.state IN ('complete','failed','cancelled') OR
                   (json_type(w.request,'$.plan_revision') IS NULL AND NOT EXISTS(
                     SELECT 1 FROM execution_memberships m JOIN jobs j ON j.id=m.job
                     WHERE m.root=w.id AND j.state NOT IN ('complete','failed','interrupted','cancelled','result_failed'))))
                AND NOT EXISTS(SELECT 1 FROM execution_memberships m JOIN recovery r ON r.job=m.job
                     WHERE m.root=w.id AND r.state IN ('pending','waiting','claimed','executing'))
            UNION ALL
            SELECT id,source,revision,snapshot,created,ended FROM preserved
             WHERE preferred=1 AND state IN ('complete','completed','succeeded','success','failed','interrupted','cancelled','canceled','result_failed','timed_out','stopped','aborted')
        ), visible AS (
            SELECT id,source,revision,snapshot,COALESCE(
                CASE WHEN typeof(ended) IN ('integer','real') THEN ended / CASE WHEN ended>=100000000000 THEN 1000. ELSE 1. END
                     ELSE CAST(strftime('%s',ended) AS REAL) END,
                CASE WHEN typeof(created) IN ('integer','real') THEN created / CASE WHEN created>=100000000000 THEN 1000. ELSE 1. END
                     ELSE CAST(strftime('%s',created) AS REAL) END,0) sort_time
            FROM candidates WHERE NOT COALESCE(({system}),0)
              AND NOT EXISTS(SELECT 1 FROM reviewed_work r WHERE r.id=candidates.id
                  AND (r.visible=0 OR (r.entry_type='unit' AND r.job_id IS NOT NULL)))
        ) SELECT * FROM visible WHERE sort_time<=:as_of AND (:time IS NULL OR sort_time<:time OR (sort_time=:time AND id>:last))
          ORDER BY sort_time DESC,id LIMIT :limit", crate::query::CONSOLIDATION_CTE);
        let mut query = self.db.prepare(&sql)?;
        let rows = query
            .query_map(
                named_params! {":as_of":as_of,":time":time,":last":last,":limit":(limit+1) as i64},
                json_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut selected = Vec::new();
        let mut order = BTreeMap::new();
        for (position, row) in rows.iter().take(limit).enumerate() {
            let id = row["id"].as_str().unwrap_or("");
            if row["source"].is_string() {
                let identity = json!({"source":row["source"],"kind":"job_view","id":id,
                    "revision":row["revision"],"snapshot":row["snapshot"]});
                let native = json!({"kind":"historical","identity":identity});
                let item = self.resolve_item(&json!({"native_ref":native}));
                match item {
                    Ok(item) => {
                        order.insert(
                            format!("history:{}", item["item_id"].as_str().unwrap_or("")),
                            position,
                        );
                        selected.push(item);
                    }
                    Err(error) if error.status == 404 => continue,
                    Err(error) => return Err(error),
                }
            } else {
                order.insert(id.to_owned(), position);
                selected
                    .push(json!({"kind":"job","native_ref":{"kind":"job","identity":{"id":id}}}));
            }
        }
        let mut catalog = self.catalog_projection(Some(&selected), CatalogContext::History)?;
        let entities = catalog["entities"].as_array_mut().unwrap();
        entities.sort_by_key(|e| {
            order
                .get(e["id"].as_str().unwrap_or(""))
                .copied()
                .unwrap_or(usize::MAX)
        });
        for entity in entities {
            if let Some(hist) = entity.get("loom_history") {
                if let Some(item) = selected
                    .iter()
                    .find(|i| &i["native_ref"]["identity"] == hist)
                {
                    entity["item_id"] = item["item_id"].clone();
                    entity["item_revision_id"] = item["revision_id"].clone();
                }
            }
            let fields = entity.as_object_mut().unwrap();
            fields.retain(|k, _| {
                matches!(
                    k.as_str(),
                    "id" | "kind"
                        | "namespace"
                        | "label"
                        | "properties"
                        | "state"
                        | "owner"
                        | "owner_name"
                        | "owner_profile"
                        | "presentation"
                        | "loom_history"
                        | "loom_job"
                        | "entry_type"
                        | "unit"
                        | "execution_intervals"
                        | "task_refs"
                        | "scenario_ref"
                        | "item_id"
                        | "item_revision_id"
                )
            });
            fields.insert("summary_only".into(), json!(true));
        }
        if rows.len() > limit {
            let last = &rows[limit - 1];
            catalog["next_cursor"] = json!(crate::index_store::sign(
                &json!({"projection":"history","access":access,
                "as_of":as_of,"expires":cursor.as_ref().map_or(clock+300.,|c|c["expires"].as_f64().unwrap_or(clock)),
                "time":last["sort_time"],"id":last["id"]}),
                &key
            )?);
        }
        catalog["partial"] = json!(false);
        Ok(catalog)
    }

    /// List DTOs are projected only for the selected index page; payload trees stay lazy.
    pub fn dashboard_page(&self, actor: &crate::auth::Actor, args: &Value) -> Result<Value> {
        if args["projection"] == "history" {
            return self.dashboard_history(actor, args);
        }
        if args["projection"] == "files" {
            return self.dashboard_directory(args);
        }
        let mut query = args.clone();
        let list =
            query.as_object_mut().and_then(|v| v.remove("projection")) == Some(json!("list"));
        let records_only =
            query.as_object_mut().and_then(|v| v.remove("records_only")) == Some(json!(true));
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
        query["_dashboard_job_rows"] = json!(true);
        query["include_system"] = json!(false);
        if query.get("include_archived").is_none() {
            query["include_archived"] = json!(false);
        }
        if query["task_id"].is_null()
            && query["scenario_ref"].is_null()
            && matches!(query["kinds"].as_array().map(Vec::as_slice), Some([kind])
                if matches!(kind.as_str(), Some("dataset" | "model" | "experiment")))
        {
            query["_dashboard_task_index"] = json!(true);
        }
        let page = self.search_items(actor, &query)?;
        let selected = page["items"]
            .as_array()
            .ok_or_else(|| Error::input("Search page missing"))?;
        let all_kinds = |allowed: &[&str]| {
            query["kinds"].as_array().is_some_and(|kinds| {
                kinds
                    .iter()
                    .all(|kind| kind.as_str().is_some_and(|kind| allowed.contains(&kind)))
            })
        };
        let software = all_kinds(&["profile", "component", "tool", "runtime", "dependency"]);
        let mode = if software && (list || !records_only) {
            CatalogContext::SoftwareList
        } else if list && all_kinds(&["dataset", "model", "experiment", "run"]) {
            CatalogContext::OutputList
        } else if records_only || list {
            CatalogContext::Records
        } else {
            CatalogContext::Full
        };
        let mut catalog = self.catalog_projection(Some(selected), mode)?;
        catalog["next_cursor"] = page["next_cursor"].clone();
        catalog["partial"] = page["partial"].clone();
        catalog["consistency"] = page["consistency"].clone();
        catalog["warnings"] = page["warnings"].clone();
        if let Some(tasks) = page.get("task_index") {
            catalog["task_index"] = tasks.clone();
        }
        catalog["capabilities"]["catalog_pages"] = json!(true);
        for entity in catalog["entities"].as_array_mut().unwrap() {
            let item = selected.iter().find(|item| {
                let native = &item["native_ref"];
                let id = &native["identity"];
                match native["kind"].as_str().unwrap_or("") {
                    "entity" => {
                        entity["loom_registry"]["id"] == id["id"]
                            && entity["loom_registry"]["revision"] == id["revision"]
                    }
                    "job" | "scenario" => entity["id"] == id["id"],
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
                if item["catalog_class"].is_string() {
                    entity["catalog_class"] = item["catalog_class"].clone();
                }
                if query["output_state"] == "discarded"
                    && item["native_ref"]["kind"] == "entity"
                    && matches!(
                        entity["kind"].as_str(),
                        Some("dataset" | "model" | "experiment" | "run")
                    )
                {
                    let item_id = item["item_id"].as_str().unwrap_or("");
                    let revision_id = item["revision_id"].as_str().unwrap_or("");
                    entity["id"] = json!(format!("revision:{item_id}:{revision_id}"));
                    entity["catalog_exact"] = json!(false);
                }
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
    fn docker_observation_path(&self) -> std::path::PathBuf {
        self.config
            .extra
            .get("docker_observation")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| self.observation_path("docker-status.json"))
    }

    pub fn docker_observation(&self) -> Result<Value> {
        let mut value = std::fs::read(self.docker_observation_path())
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
        #[derive(serde::Deserialize)]
        struct DockerHeader {
            #[serde(default)]
            observed_at: Value,
            #[serde(default)]
            host: Value,
            #[serde(default)]
            engine: DockerEngine,
        }
        #[derive(Default, serde::Deserialize)]
        struct DockerEngine {
            #[serde(default)]
            version: Value,
        }
        let docker = std::fs::read(self.docker_observation_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<DockerHeader>(&bytes).ok());
        let observation = docker.as_ref().map_or(
            Value::Null,
            |docker| json!({"observed_at":docker.observed_at,"host":docker.host}),
        );
        if observation_fresh(&observation, now()) {
            if let Some(docker) = docker {
                host["inventory"]["docker_engine"] = docker.engine.version;
            }
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
        self.resource_snapshot(None)
    }

    pub(crate) fn dashboard_resources(&self, scope: &str) -> Result<Value> {
        self.resource_snapshot(Some(scope))
    }

    fn resource_snapshot(&self, scope: Option<&str>) -> Result<Value> {
        if scope == Some("host") {
            return Ok(json!({"observed_at":now(),"host":self.dashboard_host()?,"sections":{}}));
        }
        let storage = self.observed_storage()?;
        if scope == Some("storage") {
            return Ok(
                json!({"observed_at":now(),"host":self.dashboard_host()?,"sections":{"storage":storage}}),
            );
        }
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
        let mut result = json!({"observed_at":now(),"cpu":{"status":if cpu.is_some(){"ok"}else{"unavailable"},"source":"linux:/proc/loadavg","data":{"load_1m":cpu,"logical_cpus":std::thread::available_parallelism().ok().map(|n|n.get())}},"memory":mem,"gpus":{"status":"unavailable","data":null},"storage":storage,"sensors":{"status":"unavailable","data":null}});
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
        if scope == Some("inspector") {
            result["leases"] = json!({"status":"ok","data":self.dashboard_slots()?});
            let observed_at = result.as_object_mut().unwrap().remove("observed_at");
            return Ok(
                json!({"observed_at":observed_at,"sections":result,"host":self.dashboard_host()?}),
            );
        }

        result["leases"] = json!({"status":"ok","data":self.queue()?});
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
    fn observed_storage(&self) -> Result<Value> {
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
        Ok(
            json!({"status": if disks.iter().any(|v|v["status"]=="ok") {"ok"} else {"unavailable"},
            "source":"linux:statvfs","data":disks}),
        )
    }

    fn dashboard_slots(&self) -> Result<Value> {
        let registered = rows(&self.db,
            "SELECT id AS job_id,COALESCE(json_extract(spec,'$.presentation.label'),json_extract(spec,'$.kind'),id) AS label,
                upper(CASE WHEN state IN ('preparing','processing') THEN 'cpu'
                           ELSE COALESCE(json_extract(spec,'$.profile.runtime.lane'),'cpu') END)
                    || ' 실행 슬롯' AS resource,1 AS units
             FROM jobs WHERE state IN ('running','preparing','processing')
             ORDER BY COALESCE(queue_order,created),id", &[])?;
        let queued_jobs: i64 = self.db.query_row(
            "SELECT count(*) FROM jobs WHERE state IN ('queued','ready','blocked')",
            [],
            |r| r.get(0),
        )?;
        Ok(json!({"registered":registered,"queued_jobs":queued_jobs,
            "scope":"visible_node_jobs","mechanism":"worker_lanes"}))
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
