//! Exact, read-only software composition and revision-scoped display metadata.
use crate::{auth::Actor, error::require, runtime, software, store::*, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
impl Application {
    pub fn classification(
        &self,
        kind: &str,
        owner: &str,
        id: &str,
        revision: i64,
        manifest: &Value,
    ) -> Result<Value> {
        let text:Option<String>=self.db.query_row("SELECT value FROM software_metadata WHERE kind=? AND owner=? AND id=? AND revision=?",params![kind,owner,id,revision],|r|r.get(0)).optional()?;
        Ok(text.map(|s|serde_json::from_str(&s)).transpose()?.unwrap_or_else(||json!({"tags":manifest.get("tags").cloned().unwrap_or(json!([])),"label":manifest["label"]})))
    }
    pub fn owner_projection(&self, kind: &str, namespace: &str, id: &str) -> Result<Value> {
        let who: String = self
            .db
            .query_row(
                "SELECT owner FROM item_owners WHERE kind=? AND id=?",
                params![kind, format!("{namespace}:{id}")],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(namespace.into());
        let person = if who == "built-in" {
            json!({"id":who,"provider":"builtin","display_name":"Built-in"})
        } else {
            rows(
                &self.db,
                "SELECT id,provider,login,display_name FROM user_profiles WHERE id=?",
                &[&who],
            )?
            .into_iter()
            .next()
            .unwrap_or_else(|| json!({"id":who,"provider":"local","display_name":who}))
        };
        Ok(
            json!({"namespace":namespace,"owner_name":person["display_name"],"owner_kind":if who=="built-in"{"built-in"}else{"user"},"owner_profile":person}),
        )
    }
    pub fn profile_listing(&self) -> Result<Value> {
        self.profile_listing_selected(None)
    }
    pub fn profile_listing_selected(&self, only: Option<&str>) -> Result<Value> {
        self.profile_listing_qualified(only, None)
    }
    pub fn profile_listing_qualified(
        &self,
        only: Option<&str>,
        only_sha: Option<&str>,
    ) -> Result<Value> {
        let mut definitions: BTreeMap<String, Value> = self
            .config
            .profiles
            .iter()
            .filter(|(k, _)| only.is_none_or(|kind| *k == kind))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let selected = only.unwrap_or("");
        let parent: Value = if let Some(kind) = only {
            self.db
                .query_row(
                    "SELECT body FROM catalog_sources WHERE kind='profile' AND identity=?",
                    [encode(&json!({"kind":kind}))?],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .map(|v| v["component_ref"].clone())
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        let mut origins = BTreeMap::new();
        let mut components = BTreeMap::new();
        for row in rows(
            &self.db,
            "SELECT * FROM components WHERE ?='' OR (owner=? AND id=? AND revision=?) ORDER BY owner,id,revision",
            &[&selected,&parent["owner"].as_str(),&parent["id"].as_str(),&parent["revision"].as_i64()],
        )? {
            let (owner, id, revision) = (
                row["owner"].as_str().unwrap_or(""),
                row["id"].as_str().unwrap_or(""),
                row["revision"].as_i64().unwrap_or(0),
            );
            let manifest: Value = serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;
            let path = self
                .components
                .join(owner)
                .join(id)
                .join("revisions")
                .join(revision.to_string())
                .to_string_lossy()
                .to_string();
            components.insert(path,json!({"owner":owner,"id":id,"revision":revision,"entity":format!("component:{owner}:{id}:r{revision}"),"requires":manifest["requires"],"classification":self.classification("component",owner,id,revision,&manifest)?}));
            let profiles: Value = serde_json::from_str(row["profiles"].as_str().unwrap_or("{}"))?;
            for (kind, p) in profiles.as_object().into_iter().flatten() {
                definitions.insert(kind.clone(), p.clone());
                origins.insert(
                    kind.clone(),
                    json!({"type":"component","owner":owner,"id":id,"revision":revision}),
                );
            }
        }
        for row in rows(
            &self.db,
            "SELECT * FROM execution_profiles WHERE ?='' OR json_extract(profile,'$.revision')=? ORDER BY owner,id,revision",
            &[&selected,&selected],
        )? {
            let p: Value = serde_json::from_str(row["profile"].as_str().unwrap_or("{}"))?;
            if let Some(kind) = p["revision"].as_str() {
                origins.insert(kind.into(),json!({"type":"standalone","owner":row["owner"],"id":row["id"],"revision":row["revision"]}));
                definitions.insert(kind.into(), p);
            }
        }
        definitions.insert("cpu.smoke".into(),json!({"params_schema":{"type":"object","properties":{"seconds":{"type":"integer","minimum":0,"maximum":20},"fail":{"type":"boolean"}},"required":["seconds","fail"],"additionalProperties":false},"outputs":{"observation":"artifact"},"usage":{"categories":["other"],"execution_modes":["smoke"]}}));
        origins.insert(
            "cpu.smoke".into(),
            json!({"type":"builtin","owner":"built-in","id":"cpu.smoke","revision":null}),
        );
        if let (Some(kind), Some(sha)) = (only, only_sha) {
            let exact = software::exact_profile(
                &self.db,
                &self.config,
                &json!({"kind":kind,"sha256":sha}),
            )?;
            if let Some(path) = exact["component_path"].as_str() {
                if let Ok(relative) = std::path::Path::new(path).strip_prefix(&self.components) {
                    let pieces = relative
                        .components()
                        .map(|v| v.as_os_str().to_string_lossy().to_string())
                        .collect::<Vec<_>>();
                    if pieces.len() == 4 && pieces[2] == "revisions" {
                        if let Ok(revision) = pieces[3].parse::<i64>() {
                            origins.insert(kind.into(),json!({"type":"component","owner":pieces[0],"id":pieces[1],"revision":revision}));
                            components.insert(path.to_string(),json!({"owner":pieces[0],"id":pieces[1],"revision":revision,"entity":format!("component:{}:{}:r{revision}",pieces[0],pieces[1])}));
                        }
                    }
                }
            }
            definitions.insert(kind.into(), exact);
        }
        let catalog = self.runtime_inventory()?;
        let mut items = vec![];
        for (kind, p) in definitions {
            if only.is_some_and(|value| value != kind) {
                continue;
            }
            let origin = origins
                .get(&kind)
                .cloned()
                .unwrap_or_else(|| json!({"type":"installation","id":kind,"revision":null}));
            let curation = self.curation("profile", &kind, 0)?;
            let component = p["component_path"]
                .as_str()
                .and_then(|s| components.get(s))
                .cloned();
            let mut tools = vec![];
            for (alias, pin) in p["tool_pins"].as_object().into_iter().flatten() {
                let manifest = self
                    .db
                    .query_row(
                        "SELECT manifest FROM tools WHERE owner=? AND id=? AND revision=?",
                        params![
                            pin["owner"].as_str(),
                            pin["id"].as_str(),
                            pin["revision"].as_i64()
                        ],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                    .map(|s| serde_json::from_str::<Value>(&s))
                    .transpose()?
                    .unwrap_or(json!({}));
                tools.push(json!({"alias":alias,"owner":pin["owner"],"id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"],"entity":format!("tool:{}:{}:r{}",pin["owner"].as_str().unwrap_or(""),pin["id"].as_str().unwrap_or(""),pin["revision"]),"label":manifest.get("label").unwrap_or(&pin["id"]),"classification":self.classification("tool",pin["owner"].as_str().unwrap_or(""),pin["id"].as_str().unwrap_or(""),pin["revision"].as_i64().unwrap_or(0),&manifest)?,"role":curation["tool_roles"][alias],"availability":"all_container_stages","invocation":"runner_defined"}));
            }
            let mut stages = vec![];
            for stage in runtime::STAGES {
                let Some(runner) = p.get(stage) else { continue };
                let pin = &runner["runtime_pin"];
                let mut environment = if !pin.is_null() {
                    json!({"type":"registered","id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"],"image":pin["image"],"validation":pin["validation"],"entity":format!("runtime:{}:{}",pin["id"].as_str().unwrap_or(""),pin["revision"]),"dependencies":pin["dependencies"]})
                } else {
                    json!({"type":if runner.get("runtime_id").is_some(){"legacy"}else{"image"},"id":runner["runtime_id"],"image":runner["image"],"dependencies":null})
                };
                let observed = catalog["runtimes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|r| {
                        r["legacy"] != true
                            && ((!pin.is_null()
                                && r["id"] == pin["id"]
                                && r["revision"] == pin["revision"])
                                || (pin.is_null()
                                    && r["manifest"]["runtime"]["image"] == runner["image"]
                                    && runner["image"].is_string()))
                    });
                if let Some(item) = observed {
                    if pin.is_null() {
                        environment = json!({"type":"observed","id":item["id"],"revision":item["revision"],"image":item["manifest"]["runtime"]["image"],"entity":format!("runtime:{}:{}",item["id"].as_str().unwrap_or(""),item["revision"]),"dependencies":item["manifest"]["dependencies"]});
                    }
                }
                let deps = environment["dependencies"].as_array().cloned();
                environment["dependency_count"] = json!(deps.as_ref().map(Vec::len));
                let selectors = curation["stage_dependencies"].get(stage);
                environment["primary_dependencies"] = json!(runtime::primary(
                    &json!({"dependencies":deps.clone().unwrap_or_default()}),
                    selectors
                )?);
                environment["dependencies"] =
                    json!(deps
                        .as_ref()
                        .map(|a| a.iter().take(16).cloned().collect::<Vec<_>>()));
                environment["dependencies_complete"] =
                    json!(deps.as_ref().is_some_and(|a| a.len() <= 16));
                stages.push(json!({"stage":stage,"lane":runner["lane"].as_str().unwrap_or("cpu"),"environment":environment,"storage":runner.get("storage").cloned().unwrap_or(json!([])),"inputs":p["inputs"].as_object().map(|o|o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()}));
            }
            if kind == "cpu.smoke" {
                stages.push(json!({"stage":"runtime","lane":"cpu","environment":{"type":"builtin","id":"Rust producer","dependencies":[],"primary_dependencies":[],"dependency_count":0},"storage":[],"inputs":[]}));
            }
            let job_sql = format!(
                r#"{} SELECT j.id,j.state,j.created,json_extract(j.spec,'$.category') category,json_extract(j.spec,'$.execution_mode') execution_mode,json_extract(j.spec,'$.profile.sha256') profile_sha256,COALESCE((SELECT target FROM consolidation_roots WHERE source=j.id AND source_attempt IS j.attempt AND j.state='complete' ORDER BY target LIMIT 1),w.root,j.id) logical_job,EXISTS(SELECT 1 FROM attempts a LEFT JOIN rust_attempt_starts s ON s.attempt=a.id WHERE a.job=j.id AND (CASE WHEN s.attempt IS NOT NULL THEN s.accepted ELSE a.started END) IS NOT NULL) executed FROM jobs j LEFT JOIN workflow_nodes w ON w.job=j.id WHERE json_extract(j.spec,'$.kind')=? AND (? IS NULL OR json_extract(j.spec,'$.profile.sha256')=?)"#,
                crate::query::CONSOLIDATION_CTE
            );
            let jobs = rows(
                &self.db,
                &format!("{job_sql} ORDER BY created DESC LIMIT 200"),
                &[&kind, &only_sha, &only_sha],
            )?;
            let counts=rows(&self.db,&format!("SELECT count(*) recorded_jobs,count(DISTINCT logical_job) logical_jobs,count(DISTINCT CASE WHEN executed=1 THEN logical_job END) executed_jobs,COALESCE(sum(executed),0) recorded_executions FROM ({job_sql})"),&[&kind,&only_sha,&only_sha])?.into_iter().next().unwrap_or(json!({}));
            let logical_count = counts["logical_jobs"].as_u64().unwrap_or(0);
            let run_count = counts["executed_jobs"].as_u64().unwrap_or(0);
            let family = if origin["type"] == "standalone" {
                format!(
                    "profile:{}:{}",
                    origin["owner"].as_str().unwrap_or(""),
                    origin["id"].as_str().unwrap_or("")
                )
            } else if origin["type"] == "component" {
                kind.rsplit_once(".r")
                    .map(|(k, _)| k)
                    .unwrap_or(&kind)
                    .into()
            } else {
                kind.clone()
            };
            let owner = self.owner_projection(
                "profile",
                origin["owner"].as_str().unwrap_or("unrecorded"),
                origin["id"].as_str().unwrap_or(&kind),
            )?;
            let mut usage = p.get("usage").cloned().unwrap_or(json!({}));
            for field in ["categories", "execution_modes"] {
                if let Some(value) = curation.get(field) {
                    usage[field] = value.clone();
                }
            }
            let label = curation
                .get("label")
                .or_else(|| usage.get("label"))
                .and_then(Value::as_str)
                .unwrap_or(&kind);
            let mut item = json!({"kind":kind,"label":label,"sha256":p["sha256"],"family":family,"origin":origin,"curation":curation,"group":curation["group"].as_str().or_else(||usage["group"].as_str()).unwrap_or(if kind=="cpu.smoke"{"platform-check"}else{"research"}),"description":usage["description"].as_str().unwrap_or(""),"usage":usage,"usage_status":if p.get("usage").is_some(){"declared"}else{"unrecorded"},"component":component,"tools":tools,"stages":stages,"limits":p.get("limits").cloned().unwrap_or(json!({})),"inputs":p.get("inputs").cloned().unwrap_or(json!({})),"outputs":p["outputs"],"metadata_policy":p["metadata_policy"],"metadata_template_pins":p["metadata_template_pins"],"reporting_pins":p["reporting_pins"],"reporting_policy":p["reporting_policy"],"steps":p.get("steps").cloned().unwrap_or(json!([])),"jobs":jobs,"jobs_complete":counts["recorded_jobs"].as_u64().unwrap_or(0)<=200,"used_by_jobs":logical_count,"usage_stats":{"recorded_jobs":counts["recorded_jobs"],"logical_jobs":logical_count,"executed_jobs":run_count,"reuses":run_count.saturating_sub(1),"recorded_executions":counts["recorded_executions"]},"params_schema":p["params_schema"],"defaults":p.get("defaults").cloned().unwrap_or(json!({})),"observed_usage":{"categories":jobs.iter().filter_map(|j|j["category"].as_str()).collect::<BTreeSet<_>>(),"execution_modes":jobs.iter().filter_map(|j|j["execution_mode"].as_str()).collect::<BTreeSet<_>>()}});
            item.as_object_mut()
                .unwrap()
                .extend(owner.as_object().unwrap().clone());
            items.push(item);
        }
        Ok(json!(items))
    }
    pub fn inspect_profiles(&self, action: &str, args: Value) -> Result<Value> {
        if action == "compare_profiles" {
            let left =
                software::profile(&self.db, &self.config, args["left"].as_str().unwrap_or(""))?
                    .ok_or_else(|| Error::input("Profile not found"))?;
            let right =
                software::profile(&self.db, &self.config, args["right"].as_str().unwrap_or(""))?
                    .ok_or_else(|| Error::input("Profile not found"))?;
            fn compare(a: &Value, b: &Value, path: &str, out: &mut Vec<Value>) {
                if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
                    for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                        if path.is_empty()
                            && ["revision", "sha256", "usage", "owners", "submitters"]
                                .contains(&key.as_str())
                        {
                            continue;
                        }
                        let p = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        };
                        if !a.contains_key(key) || !b.contains_key(key) {
                            out.push(json!({"path":p,"change":if a.contains_key(key){"removed"}else{"added"}}));
                        } else {
                            compare(&a[key], &b[key], &p, out);
                        }
                    }
                } else if a != b {
                    out.push(json!({"path":path,"change":"changed"}));
                }
            }
            let mut changes = vec![];
            compare(&left, &right, "", &mut changes);
            return Ok(
                json!({"left":args["left"],"right":args["right"],"execution_equal":changes.is_empty(),"changes":changes.iter().take(200).collect::<Vec<_>>(),"truncated":changes.len()>200}),
            );
        }
        let list = self.profile_listing()?;
        let combo = list
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == args["kind"])
            .ok_or_else(|| Error::input("Profile not found"))?;
        let mut checks:Vec<_>=combo["stages"].as_array().unwrap().iter().map(|s|json!({"stage":s["stage"],"check":"runtime","status":match s["environment"]["type"].as_str(){Some("registered")=>"pinned",Some("builtin")=>"builtin",Some("observed")=>"observed",_=>"unverified"}})).collect();
        let definition =
            software::profile(&self.db, &self.config, args["kind"].as_str().unwrap_or(""))?
                .unwrap_or_else(|| json!({"defaults":{}}));
        if let Some(component) = combo.get("component").filter(|v| !v.is_null()) {
            if let Some(text) = self
                .db
                .query_row(
                    "SELECT manifest FROM components WHERE owner=? AND id=? AND revision=?",
                    params![
                        component["owner"].as_str(),
                        component["id"].as_str(),
                        component["revision"].as_i64()
                    ],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
            {
                checks.extend(runtime::requirement_report(
                    &serde_json::from_str(&text)?,
                    &definition,
                )?);
            }
        }
        for tool in combo["tools"].as_array().into_iter().flatten() {
            let text: String = self.db.query_row(
                "SELECT manifest FROM tools WHERE owner=? AND id=? AND revision=?",
                params![
                    tool["owner"].as_str(),
                    tool["id"].as_str(),
                    tool["revision"].as_i64()
                ],
                |r| r.get(0),
            )?;
            checks.extend(runtime::requirement_report(
                &serde_json::from_str(&text)?,
                &definition,
            )?);
        }
        fn merge(a: &Value, b: &Value) -> Value {
            if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
                let mut result = a.clone();
                for (k, v) in b {
                    result.insert(k.clone(), merge(result.get(k).unwrap_or(&Value::Null), v));
                }
                json!(result)
            } else {
                b.clone()
            }
        }
        let params = merge(
            combo.get("defaults").unwrap_or(&json!({})),
            args.get("params").unwrap_or(&json!({})),
        );
        let validation = if args.get("params").is_some() {
            if crate::contracts::validate(&combo["params_schema"], &params).is_ok() {
                "valid"
            } else {
                "invalid"
            }
        } else {
            "not_checked"
        };
        Ok(
            json!({"kind":args["kind"],"profile_sha256":combo["sha256"],"combination":combo,"checks":checks,"params":{"status":validation},"params_schema":combo["params_schema"],"defaults":combo["defaults"],"inputs":combo["inputs"],"outputs":combo["outputs"],"draft":{"kind":args["kind"],"params":params}}),
        )
    }
    pub fn curate_software(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Identity required"))?;
        if action == "annotate" {
            let kind = args["kind"].as_str().unwrap_or("");
            let owner = args["owner"].as_str().unwrap_or("");
            let revision = args["revision"].as_i64().unwrap_or(0);
            let table = if kind == "component" {
                "components"
            } else {
                "tools"
            };
            require(self.db.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE owner=? AND id=? AND revision=?)"),params![owner,id,revision],|r|r.get(0))?,"software","Software revision not found",404)?;
            let value =
                json!({"tags":args["tags"],"label":args["label"],"evidence":args["evidence"]});
            let tx = self.transaction()?;
            active(&tx)?;
            tx.execute("INSERT INTO software_metadata VALUES(?,?,?,?,?,?,?) ON CONFLICT(kind,owner,id,revision) DO UPDATE SET value=excluded.value,actor=excluded.actor,updated=excluded.updated",params![kind,owner,id,revision,encode(&value)?,actor.id,now()])?;
            tx.commit()?;
            return Ok(value);
        }
        if action == "assign_owner" {
            let kind = args["kind"].as_str().unwrap_or("");
            let namespace = args["namespace"].as_str().unwrap_or("");
            let owner = args["owner"].as_str().unwrap_or("");
            let table = match kind {
                "component" => "components",
                "tool" => "tools",
                _ => "execution_profiles",
            };
            require(
                self.db.query_row(
                    &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE owner=? AND id=?)"),
                    params![namespace, id],
                    |r| r.get(0),
                )?,
                "software",
                "Software not found",
                404,
            )?;
            require(
                owner == "built-in"
                    || self.db.query_row(
                        "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE id=?)",
                        [owner],
                        |r| r.get(0),
                    )?,
                "owner",
                "Owner profile not registered",
                400,
            )?;
            let tx = self.transaction()?;
            active(&tx)?;
            let key = format!("{namespace}:{id}");
            let previous: Option<String> = tx
                .query_row(
                    "SELECT owner FROM item_owners WHERE kind=? AND id=?",
                    params![kind, key],
                    |r| r.get(0),
                )
                .optional()?;
            let changed = previous.as_deref() != Some(owner);
            if changed {
                tx.execute("INSERT INTO ownership_history(kind,id,previous_owner,owner,changed_by,changed) VALUES(?,?,?,?,?,?)",params![kind,key,previous,owner,actor.id,now()])?;
                tx.execute("INSERT INTO item_owners VALUES(?,?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET owner=excluded.owner,updated=excluded.updated,updated_by=excluded.updated_by",params![kind,key,owner,now(),actor.id])?;
            }
            tx.commit()?;
            let mut result = self.owner_projection(kind, namespace, id)?;
            result["changed"] = json!(changed);
            return Ok(result);
        }
        let kind = args["type"].as_str().unwrap_or("");
        let revision = args["revision"].as_i64().unwrap_or(0);
        let mut value = self.curation(kind, id, revision)?;
        if kind == "profile" {
            require(
                revision == 0 && args.get("primary_dependencies").is_none(),
                "curation",
                "Select exact Profile kind; dependency highlights belong to Runtime",
                400,
            )?;
            let profiles = self.profile_listing()?;
            let p = profiles
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["kind"] == id)
                .ok_or_else(|| Error::input("Profile not found"))?;
            for (alias, role) in args["tool_roles"].as_object().into_iter().flatten() {
                require(
                    p["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["alias"] == *alias)
                        && role["stages"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .all(|stage| {
                                p["stages"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|s| s["stage"] == *stage)
                            }),
                    "curation",
                    "Unknown Tool alias or stage",
                    400,
                )?;
            }
            for (stage, selectors) in args["stage_dependencies"].as_object().into_iter().flatten() {
                let s = p["stages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["stage"] == *stage)
                    .ok_or_else(|| Error::input("Unknown dependency stage"))?;
                let definition = software::profile(&self.db, &self.config, id)?
                    .ok_or_else(|| Error::input("Profile unavailable"))?;
                let dependencies = definition[stage]["runtime_pin"]
                    .get("dependencies")
                    .cloned()
                    .unwrap_or_else(|| s["environment"]["dependencies"].clone());
                runtime::primary(&json!({"dependencies":dependencies}), Some(selectors))?;
            }
            for (key, v) in args.as_object().unwrap() {
                if !["type", "id", "revision"].contains(&key.as_str()) {
                    value[key] = v.clone();
                }
            }
            require(
                !(value["preferred"] == true && value["state"] == "archived"),
                "curation",
                "Archived Profile cannot be preferred",
                400,
            )?;
            if value["preferred"] == true {
                require(
                    !profiles.as_array().unwrap().iter().any(|other| {
                        other["kind"] != id
                            && other["family"] == p["family"]
                            && other["curation"]["preferred"] == true
                    }),
                    "curation",
                    "Clear existing preferred revision first",
                    409,
                )?;
            }
        } else {
            require(
                revision > 0
                    && [
                        "categories",
                        "execution_modes",
                        "group",
                        "preferred",
                        "tool_roles",
                        "stage_dependencies",
                    ]
                    .iter()
                    .all(|k| args.get(k).is_none()),
                "curation",
                "Select exact Runtime revision and Runtime metadata",
                400,
            )?;
            let inventory = self.runtime_inventory()?;
            let item = inventory["runtimes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id && r["revision"] == revision)
                .ok_or_else(|| Error::input("Runtime outside instance grant"))?;
            runtime::primary(&item["manifest"], args.get("primary_dependencies"))?;
            for (key, v) in args.as_object().unwrap() {
                if !["type", "id", "revision"].contains(&key.as_str()) {
                    value[key] = v.clone();
                }
            }
        }
        let tx = self.transaction()?;
        active(&tx)?;
        tx.execute("INSERT INTO runtime_curation VALUES(?,?,?,?,?,?) ON CONFLICT(kind,id,revision) DO UPDATE SET value=excluded.value,actor=excluded.actor,updated=excluded.updated",params![kind,id,revision,encode(&value)?,actor.id,now()])?;
        tx.commit()?;
        Ok(value)
    }
}
impl Application {
    pub fn extension_inventory(&self, profiles: &Value) -> Result<Vec<Value>> {
        self.extension_inventory_selected(profiles, None)
    }
    pub fn extension_inventory_selected(
        &self,
        profiles: &Value,
        selected: Option<&[Value]>,
    ) -> Result<Vec<Value>> {
        let mut entities = vec![];
        for (kind, table) in [("component", "components"), ("tool", "tools")] {
            for latest in rows(
                &self.db,
                &format!("SELECT owner,id,max(revision) revision FROM {table} WHERE ? OR json_array(owner,id,revision) IN (SELECT json_array(json_extract(value,'$.native_ref.identity.owner'),json_extract(value,'$.native_ref.identity.id'),json_extract(value,'$.native_ref.identity.revision')) FROM json_each(?)) GROUP BY owner,id"),
                &[&selected.is_none(),&encode(&json!(selected.unwrap_or_default().iter().filter(|v|v["native_ref"]["kind"]==kind).collect::<Vec<_>>()))?],
            )? {
                let (owner, id) = (
                    latest["owner"].as_str().unwrap_or(""),
                    latest["id"].as_str().unwrap_or(""),
                );
                let mut revisions = vec![];
                let mut latest_manifest = json!({});
                for row in rows(
                    &self.db,
                    &format!("SELECT * FROM {table} WHERE owner=? AND id=? AND (? OR revision<=?) ORDER BY revision DESC LIMIT 200"),
                    &[&owner, &id,&selected.is_none(),&latest["revision"].as_i64()],
                )? {
                    let revision = row["revision"].as_i64().unwrap_or(0);
                    let manifest: Value =
                        serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;
                    if revisions.is_empty() {
                        latest_manifest = manifest.clone();
                    }
                    let connected:Vec<_>=profiles.as_array().unwrap().iter().filter(|p|if kind=="component"{p["component"]["owner"]==owner&&p["component"]["id"]==id&&p["component"]["revision"]==revision}else{p["tools"].as_array().is_some_and(|a|a.iter().any(|t|t["owner"]==owner&&t["id"]==id&&t["revision"]==revision))}).map(|p|json!({"kind":p["kind"],"stages":p["stages"].as_array().unwrap().iter().map(|s|s["stage"].clone()).collect::<Vec<_>>() })).collect();
                    let kinds: Vec<_> = connected.iter().map(|p| p["kind"].clone()).collect();
                    let mut pinned = std::collections::BTreeSet::new();
                    for p in profiles
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|p| kinds.contains(&p["kind"]))
                    {
                        for j in p["jobs"].as_array().into_iter().flatten() {
                            if j["executed"] == 1 && j["profile_sha256"] == p["sha256"] {
                                if let Some(id) = j["logical_job"].as_str() {
                                    pinned.insert(id.to_string());
                                }
                            }
                        }
                    }
                    revisions.push(json!({"revision":revision,"sha256":row["sha256"],"label":manifest.get("label").unwrap_or(&latest["id"]),"description":manifest["description"].as_str().unwrap_or(""),"classification":self.classification(kind,owner,id,revision,&manifest)?,"pinned_jobs":pinned,"invocations":null,"source_pins":manifest.get("source_pins").cloned().unwrap_or(json!({})),"source_pins_recorded":manifest.get("source_pins").is_some(),"profiles":connected,"connected_profiles":connected,"used_by":connected,"origin":manifest["origin"]}));
                }
                let who = self.owner_projection(kind, owner, id)?;
                let mut entity = json!({"id":format!("{kind}:{owner}:{id}:r{}",latest["revision"]),"kind":kind,"namespace":"loom","label":latest_manifest.get("label").unwrap_or(&latest["id"]),"summary":latest_manifest["description"].as_str().unwrap_or(""),"properties":{"revision":latest["revision"]},"loom_extension":{"owner":owner,"id":id,"revision":latest["revision"],"manifest":latest_manifest,"classification":revisions.first().map(|r|&r["classification"]),"revisions":revisions},"relations":[],"artifacts":[]});
                entity
                    .as_object_mut()
                    .unwrap()
                    .extend(who.as_object().unwrap().clone());
                entities.push(entity);
            }
        }
        Ok(entities)
    }
}

impl Application {
    pub fn decorate_owner(
        &self,
        kind: &str,
        id: &str,
        fallback: &str,
        mut value: Value,
    ) -> Result<Value> {
        let owner = self
            .db
            .query_row(
                "SELECT owner FROM item_owners WHERE kind=? AND id=?",
                params![kind, id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or(fallback.into());
        let person = rows(
            &self.db,
            "SELECT id,provider,login,display_name FROM user_profiles WHERE id=?",
            &[&owner],
        )?
        .into_iter()
        .next()
        .unwrap_or_else(|| json!({"id":owner,"provider":"local","display_name":owner}));
        value["recorded_owner"] = json!(fallback);
        value["owner"] = json!(owner);
        value["owner_profile"] = person.clone();
        value["owner_name"] = person["display_name"].clone();
        Ok(value)
    }
    pub fn assign_responsibility(
        &mut self,
        actor: &Actor,
        kind: &str,
        id: &str,
        owner: &str,
        fallback: &str,
    ) -> Result<Value> {
        let known: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM user_profiles WHERE id=?)",
            [owner],
            |r| r.get(0),
        )?;
        require(
            known || self.config.actors.iter().any(|a| a.id == owner),
            "owner",
            "Select a registered responsible user",
            404,
        )?;
        let tx = self.transaction()?;
        active(&tx)?;
        let previous = tx
            .query_row(
                "SELECT owner FROM item_owners WHERE kind=? AND id=?",
                params![kind, id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or(fallback.into());
        if previous != owner {
            tx.execute("INSERT INTO item_owners VALUES(?,?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET owner=excluded.owner,updated=excluded.updated,updated_by=excluded.updated_by",params![kind,id,owner,now(),actor.id])?;
            tx.execute("INSERT INTO ownership_history(kind,id,previous_owner,owner,changed_by,changed) VALUES(?,?,?,?,?,?)",params![kind,id,previous,owner,actor.id,now()])?;
        }
        tx.commit()?;
        self.decorate_owner(
            kind,
            id,
            fallback,
            json!({"id":id,"execution_pins":"preserved"}),
        )
    }
}
