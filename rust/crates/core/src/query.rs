//! Read models project committed facts and never schedule or repair work.
use crate::{
    auth::Actor,
    contracts,
    error::require,
    jobs::{self, Record},
    store::*,
    Error, Result,
};
use loom_domain::{union_intervals, JobState};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
impl Application {
    pub fn consolidation_target(&self, id: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT c.id FROM jobs c,json_each(c.spec,'$._consolidation_sources') src JOIN jobs original ON original.id=json_extract(src.value,'$.job') WHERE original.id=? AND original.state='complete' AND json_extract(src.value,'$.attempt') IS original.attempt AND c.state='complete' AND json_extract(c.result,'$.artifact_ready')=1 ORDER BY c.created,c.id LIMIT 1",[id],|r|r.get(0)).optional()?)
    }
    pub fn job_detail(&self, id: &str) -> Result<Value> {
        let workflow = rows(&self.db, "SELECT * FROM job_workflows WHERE id=?", &[&id])?
            .into_iter()
            .next();
        if let Some(workflow) = workflow {
            let request: Value =
                serde_json::from_str(workflow["request"].as_str().unwrap_or("{}"))?;
            let mut nodes = vec![];
            let mut intervals = vec![];
            let mut open_intervals = vec![];
            let mut states = vec![];
            let mut outputs = serde_json::Map::new();
            let mut errors = vec![];
            for n in rows(
                &self.db,
                "SELECT node,label,job FROM workflow_nodes WHERE root=? ORDER BY rowid",
                &[&id],
            )? {
                let child = n["job"].as_str().unwrap_or("");
                let row = jobs::load(&self.db, child)?;
                let detail = self.record_detail(&row)?;
                states.push(row.aggregate.state);
                open_intervals.extend(
                    detail["execution_intervals"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|i| i["start"].is_number() && i["end"].is_null())
                        .cloned(),
                );
                intervals.extend(
                    detail["execution_intervals"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|i| Some((i["start"].as_f64()?, i["end"].as_f64()?))),
                );
                if row.aggregate.state == JobState::Failed
                    || row.aggregate.state == JobState::ResultFailed
                {
                    errors.push(
                        json!({"job":child,"attempt":detail["attempt"],"result":detail["result"]}),
                    );
                }
                nodes.push(json!({"id":n["node"],"job":child,"label":n["label"],"state":detail["state"],"status":detail["status"],"attempt":detail["attempt"],"attempts":detail["attempts"],"profile":row.spec["kind"]}));
                for (alias, ref_) in request["outputs"].as_object().into_iter().flatten() {
                    if ref_["node"] == n["node"] {
                        if let Some(output) = row
                            .result
                            .as_ref()
                            .and_then(|r| r["entities"].as_array())
                            .and_then(|e| e.iter().find(|e| e["id"] == ref_["entity"]))
                        {
                            outputs.insert(alias.clone(), output.clone());
                        }
                    }
                }
            }
            let state = if states.iter().all(|s| *s == JobState::Complete) {
                JobState::Complete
            } else if states.contains(&JobState::Running) {
                JobState::Running
            } else if states.iter().any(|s| matches!(s, JobState::Processing)) {
                JobState::Processing
            } else if states.contains(&JobState::ResultFailed) {
                JobState::ResultFailed
            } else if states.contains(&JobState::Preparing) {
                JobState::Preparing
            } else if states.contains(&JobState::Failed) || states.contains(&JobState::Interrupted)
            {
                JobState::Failed
            } else if states.iter().all(|s| s.terminal()) {
                JobState::Cancelled
            } else {
                JobState::Queued
            };
            let mut execution_intervals: Vec<_> = union_intervals(&intervals)?
                .iter()
                .map(|(a, b)| json!({"start":a,"end":b}))
                .collect();
            execution_intervals.extend(open_intervals);
            let mut presentation = request.get("presentation").cloned().unwrap_or(Value::Null);
            if presentation.is_null() {
                presentation = json!({"scope":"research","role":"main"});
            }
            return self.decorate_owner("job",id,workflow["owner"].as_str().unwrap_or(""),
                json!({"id":id,"workflow":true,"owner":workflow["owner"],"notify":workflow["notify"],"created":workflow["created"],"state":state.as_str(),"status":state.public(),"spec":request,"executions":nodes,"execution_intervals":execution_intervals,"result":{"artifact_ready":state==JobState::Complete,"outputs":outputs},"errors":errors,"presentation":presentation,"scenario_ref":request["scenario_ref"]}),
            );
        }
        if let Some(record) = self.imported_job(id)? {
            return Ok(record);
        }
        self.record_detail(&jobs::load(&self.db, id)?)
    }
    pub fn record_detail(&self, row: &Record) -> Result<Value> {
        let id = row.aggregate.id.as_str();
        let mut spec = row.spec.clone();
        for k in [
            "_requested_presentation",
            "_requested_dependencies",
            "_output_storage_id",
            "_consolidation_sources",
        ] {
            spec.as_object_mut().unwrap().remove(k);
        }
        let mut attempts=rows(&self.db,"SELECT a.*,a.started reserved_at,CASE WHEN s.attempt IS NOT NULL THEN s.accepted ELSE a.started END started,r.result FROM attempts a LEFT JOIN rust_attempt_starts s ON s.attempt=a.id LEFT JOIN attempt_results r ON r.job=a.job AND r.attempt=a.id WHERE a.job=? ORDER BY a.started,a.id",&[&id])?;
        for a in &mut attempts {
            if let Some(r) = a["result"].as_str() {
                a["result"] = serde_json::from_str(r)?;
            }
        }
        let stages = rows(
            &self.db,
            "SELECT * FROM stage_runs WHERE job=? ORDER BY started,stage",
            &[&id],
        )?;
        let mut intervals = vec![];
        let mut open_intervals = vec![];
        for s in &stages {
            if !matches!(s["stage"].as_str(), Some("run" | "prepare")) {
                continue;
            }
            if let Some(start) = s["started"].as_f64() {
                if s["ended"].is_null() {
                    open_intervals.push(json!({"start":start,"end":null,"stage":s["stage"],"count_runtime":true,"authority":"stage_receipt"}));
                }
            }
            if let (Some(a), Some(b)) = (s["started"].as_f64(), s["ended"].as_f64()) {
                intervals.push((a, b));
            }
        }
        if stages.is_empty() {
            for a in &attempts {
                if let Some(start) = a["started"].as_f64() {
                    if let Some(end) = a["ended"].as_f64() {
                        intervals.push((start, end));
                    } else {
                        open_intervals.push(json!({"start":start,"end":null,"count_runtime":true,"may_include_waiting":true,"authority":"attempt_receipt"}));
                    }
                }
            }
        }
        let mut timing: Vec<_> = union_intervals(&intervals)?
            .iter()
            .map(|(a, b)| json!({"start":a,"end":b}))
            .collect();
        timing.extend(open_intervals);
        let recovery=rows(&self.db,"SELECT state,rounds,agent,deadline,hard_deadline,reason,updated FROM recovery WHERE job=?",&[&id])?.into_iter().next();
        let scheduling = self.readiness(row)?;
        let mut presentation=spec.get("presentation").cloned().unwrap_or_else(||json!({"scope":if spec["kind"]=="cpu.smoke"{"operations"}else{"research"},"role":if spec["kind"]=="cpu.smoke"{"test"}else{"main"}}));
        if let Some(overlay) = self
            .db
            .query_row(
                "SELECT presentation FROM research_overlays WHERE kind='job' AND id=?",
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            if let (Some(target), Some(source)) = (
                presentation.as_object_mut(),
                serde_json::from_str::<Value>(&overlay)?.as_object(),
            ) {
                target.extend(source.clone());
            }
        }
        let mut result = row.result.clone();
        let mut previous = Value::Null;
        if matches!(
            row.aggregate.state,
            JobState::Queued | JobState::Preparing | JobState::Ready | JobState::Running
        ) {
            previous = result.take().unwrap_or(Value::Null);
        }
        let internal_steps = if let Some(attempt) = row.aggregate.fence.attempt.as_ref() {
            let folder = crate::storage::store(
                &self.config,
                &self.bulk,
                row.spec["output_storage"].as_str().unwrap_or("data/0"),
                row.spec["_output_storage_id"].as_str(),
            )
            .ok()
            .map(|(root, _)| root.join("runs").join(id).join(attempt.as_str()));
            crate::progress::read(folder.as_deref(), &row.spec)
        } else {
            crate::progress::read(None, &row.spec)
        };
        let queue_history = rows(
            &self.db,
            "SELECT actor,action,recorded,reason FROM queue_history WHERE job=? ORDER BY seq",
            &[&id],
        )?;
        let mut details = json!({"id":id,"submitter":row.submitter,"owner":row.owner,"notify":row.notify,"state":row.aggregate.state.as_str(),"status":row.aggregate.state.public(),"attempt":row.aggregate.fence.attempt.as_ref().map(|a|a.as_str()),"created":row.created,"spec":spec,"result":result,"previous_result":previous,"attempts":attempts,"stages":stages,"execution_intervals":timing,"recovery":recovery,"presentation":presentation,"category":row.spec.get("category").unwrap_or(&json!("other")),"execution_mode":row.spec["execution_mode"],"scenario_ref":row.spec["scenario_ref"],"scheduling":{"eligible":scheduling.reason().is_none(),"waiting_reason":scheduling.reason(),"dependency_failed":scheduling.dependency_failed},"cancel":row.aggregate.cancel_requested,"queue_history":queue_history,"internal_steps":internal_steps});
        details = self.decorate_owner("job", id, &row.owner, details)?;
        let annotations = rows(
            &self.db,
            "SELECT payload FROM job_annotations WHERE job=? ORDER BY id",
            &[&id],
        )?
        .iter()
        .map(|r| serde_json::from_str::<Value>(r["payload"].as_str().unwrap_or("{}")))
        .collect::<std::result::Result<Vec<_>, _>>()?;
        if !annotations.is_empty() {
            details["annotations"] = json!(annotations);
        }
        details["consolidation"] = json!({"consolidated_into":self.consolidation_target(id)?,"sources":row.spec["_consolidation_sources"].as_array().into_iter().flatten().map(|s|json!({"job":s["job"],"attempt":s["attempt"]})).collect::<Vec<_>>()});
        if let Some(sources) = row.spec["_consolidation_sources"].as_array() {
            let mut executions = vec![];
            for source in sources {
                if let Some(id) = source["job"].as_str() {
                    let source = jobs::load(&self.db, id)?;
                    let detail = self.record_detail(&source)?;
                    executions.push(json!({"job":id,"attempt":source.aggregate.fence.attempt.as_ref().map(|a|a.as_str()),"label":source.spec["presentation"].get("label").unwrap_or(&source.spec["kind"]),"attempts":detail["attempts"],"stages":detail["stages"],"state":detail["state"],"execution_intervals":detail["execution_intervals"],"created":detail["created"],"source":"preserved execution"}));
                }
            }
            let mut all_intervals = details["execution_intervals"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for source in &executions {
                all_intervals.extend(
                    source["execution_intervals"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            let started = all_intervals
                .iter()
                .filter_map(|i| i["start"].as_f64())
                .min_by(f64::total_cmp);
            let ended = if all_intervals.iter().all(|i| i["end"].is_number()) {
                all_intervals
                    .iter()
                    .filter_map(|i| i["end"].as_f64())
                    .max_by(f64::total_cmp)
            } else {
                None
            };
            let queued = executions
                .iter()
                .filter_map(|e| e["created"].as_f64())
                .fold(row.created, f64::min);
            details["timeline"] = json!({"authority":"frozen_source_execution","queued":queued,"started":started,"ended":ended,"executions":executions});
            details["execution_intervals"] = json!(all_intervals);
            if details["internal_steps"].is_null() {
                details["internal_steps"] = json!({"schema":"loom-job-steps/v1","status":"preserved_execution","authority":"frozen source execution receipts","steps":executions.iter().map(|e|json!({"id":e["job"],"label":e["label"],"state":"reused","source_job":e["job"],"source_attempt":e["attempt"]})).collect::<Vec<_>>()});
            }
            details["source_executions"] = json!(executions);
        }
        Ok(details)
    }
    pub fn job_list(&self, args: &Value) -> Result<Value> {
        let offset = args["offset"].as_i64().unwrap_or(0).max(0);
        let limit = args["limit"].as_i64().unwrap_or(250).clamp(1, 1000);
        let view = args["view"].as_str().unwrap_or("all");
        let category = args["category"].as_str();
        let mode = args["execution_mode"].as_str();
        let consolidated = args["include_consolidated"] == true;
        let identities=rows(&self.db,"WITH folded AS (SELECT DISTINCT c.value id FROM jobs j,json_each(j.spec,'$.consolidates') c WHERE j.state='complete' AND json_extract(j.result,'$.artifact_ready')=1), candidates AS (SELECT j.id,j.created,CASE WHEN o.presentation IS NULL THEN j.spec ELSE json_patch(j.spec,json_object('presentation',json_patch(COALESCE(json_extract(j.spec,'$.presentation'),'{}'),o.presentation))) END spec,0 workflow FROM jobs j LEFT JOIN research_overlays o ON o.kind='job' AND o.id=j.id WHERE NOT EXISTS(SELECT 1 FROM workflow_nodes n WHERE n.job=j.id) AND (? OR NOT EXISTS(SELECT 1 FROM folded f WHERE f.id=j.id)) UNION ALL SELECT w.id,w.created,w.request,1 FROM job_workflows w UNION ALL SELECT h.id,COALESCE(json_extract(h.body,'$.record.created'),json_extract(h.body,'$.entity.created'),json_extract(h.body,'$.entity.properties.created_at'),0),COALESCE(json_extract(h.body,'$.record.spec'),json_extract(h.body,'$.entity.properties'),'{}'),2 FROM history_records h WHERE h.rowid IN(SELECT max(rowid) FROM history_records WHERE kind='job_view' AND source IN ('queue','loom-ops') GROUP BY id) AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.id=h.id)) SELECT id,workflow FROM candidates WHERE (?='all' OR COALESCE(json_extract(spec,'$.presentation.scope'),json_extract(spec,'$.scope'),CASE WHEN json_extract(spec,'$.kind')='cpu.smoke' THEN 'operations' ELSE 'research' END)=?) AND (? IS NULL OR COALESCE(json_extract(spec,'$.category'),'other')=?) AND (? IS NULL OR json_extract(spec,'$.execution_mode')=?) ORDER BY created DESC,id LIMIT ? OFFSET ?",&[&consolidated,&view,&view,&category,&category,&mode,&mode,&limit,&offset])?;
        let mut items = vec![];
        for identity in identities {
            let id = identity["id"].as_str().unwrap_or("");
            if identity["workflow"] != 0 {
                items.push(self.job_detail(id)?);
                continue;
            }
            let mut item=rows(&self.db,"SELECT id,state,owner,submitter,created,attempt,queue_order,queue_held,spec FROM jobs WHERE id=?",&[&id])?.remove(0);
            let spec: Value = serde_json::from_str(item["spec"].as_str().unwrap_or("{}"))?;
            item.as_object_mut().unwrap().remove("spec");
            let overlay: Option<String> = self
                .db
                .query_row(
                    "SELECT presentation FROM research_overlays WHERE kind='job' AND id=?",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            let mut presentation=spec.get("presentation").filter(|p|p.is_object()).cloned().unwrap_or(json!({"scope":if spec["kind"]=="cpu.smoke"{"operations"}else{"research"},"role":if spec["kind"]=="cpu.smoke"{"test"}else{"main"}}));
            if let Some(overlay) = overlay {
                presentation.as_object_mut().unwrap().extend(
                    serde_json::from_str::<Value>(&overlay)?
                        .as_object()
                        .unwrap()
                        .clone(),
                );
            }
            presentation["visible"] = json!(
                presentation["scope"] == "research"
                    && presentation["role"] == "main"
                    && !presentation["parent_job"].is_string()
            );
            item["consolidated_into"] = json!(self.consolidation_target(id)?);
            item["presentation"] = presentation;
            item["category"] = spec.get("category").cloned().unwrap_or(json!("other"));
            item["execution_mode"] = spec["execution_mode"].clone();
            item["scenario_ref"] = spec["scenario_ref"].clone();
            item["status"] = json!(JobState::parse(item["state"].as_str().unwrap_or(""))?.public());
            item["recovery"]=rows(&self.db,"SELECT state,rounds,agent,deadline,hard_deadline,reason,updated FROM recovery WHERE job=?",&[&id])?.into_iter().next().unwrap_or(Value::Null);
            let owner = item["owner"].as_str().unwrap_or("").to_string();
            items.push(self.decorate_owner("job", id, &owner, item)?);
        }
        Ok(json!(items))
    }
    pub fn queue(&self) -> Result<Value> {
        let mut entries = vec![];
        for row in rows(&self.db,"SELECT id FROM jobs WHERE id IN (SELECT id FROM jobs WHERE state IN ('queued','blocked','preparing','ready','running','processing','result_failed') UNION SELECT job FROM recovery WHERE state IN ('waiting','claimed','pending','executing')) ORDER BY COALESCE(queue_order,created),id",&[])? {let id=row["id"].as_str().unwrap_or("");let record=jobs::load(&self.db,id)?;let r=self.readiness(&record)?;let parent:Option<String>=self.db.query_row("SELECT root FROM workflow_nodes WHERE job=?",[id],|r|r.get(0)).optional()?;entries.push(json!({"id":id,"state":record.aggregate.state.as_str(),"status":record.aggregate.state.public(),"position":entries.len()+1,"owner":record.owner,"lane":record.spec["profile"]["runtime"]["lane"].as_str().unwrap_or("cpu"),"stage":if record.aggregate.state==JobState::Processing{"finalize"}else{"run"},"workflow":parent,"scheduling":{"eligible":r.reason().is_none(),"waiting_reason":r.reason(),"dependency_failed":r.dependency_failed}}));}
        let mut position = 0;
        for entry in &mut entries {
            let waiting = matches!(
                entry["state"].as_str(),
                Some("queued" | "ready" | "blocked")
            );
            let id = entry["id"].as_str().unwrap_or("").to_string();
            let record = jobs::load(&self.db, &id)?;
            let ready = self.readiness(&record)?;
            entry["position"] = if waiting {
                position += 1;
                json!(position)
            } else {
                Value::Null
            };
            entry["held"] = json!(ready.held);
            entry["reason"] = json!(
                if entry["state"] == "running" || entry["state"] == "preparing" {
                    "executing"
                } else if entry["state"] == "processing" {
                    "finalizing"
                } else {
                    match ready.reason() {
                        Some("operator_paused") => "queue_paused",
                        Some("lane_busy") => "resource_busy",
                        Some("backoff") => "retry_delay",
                        Some("recovery_reserved") => "recovery_waiting",
                        Some(reason) => reason,
                        None => "ready",
                    }
                }
            );
            entry["eligible"] = json!(waiting && ready.reason().is_none());
            entry["can_reorder"] = json!(waiting);
            entry["can_hold"] = json!(waiting && !ready.held);
            entry["can_resume"] = json!(waiting && ready.held);
            entry["presentation"] = record
                .spec
                .get("presentation")
                .cloned()
                .unwrap_or(json!({}));
        }
        let paused=self.db.query_row("SELECT paused,reason,recorded FROM dispatch_control WHERE id=1",[],|r|Ok(json!({"paused":r.get::<_,i64>(0)?!=0,"reason":r.get::<_,Option<String>>(1)?,"recorded":r.get::<_,Option<f64>>(2)?})))?;
        let mut logical = vec![];
        for row in rows(&self.db, "SELECT id FROM job_workflows", &[])? {
            let mut d = self.job_detail(row["id"].as_str().unwrap_or(""))?;
            let children: Vec<_> = entries
                .iter()
                .filter(|e| e["workflow"] == d["id"])
                .collect();
            if children.is_empty() {
                continue;
            }
            d["reason"] = json!(if d["state"] == "running" || d["state"] == "preparing" {
                "executing"
            } else {
                "recovery_required"
            });
            d["position"] = children
                .iter()
                .filter_map(|e| e["position"].as_i64())
                .min()
                .map(|n| json!(n))
                .unwrap_or(Value::Null);
            if !matches!(d["state"].as_str(), Some("complete" | "cancelled")) {
                logical.push(d);
            }
        }
        Ok(
            json!({"entries":entries,"logical_entries":logical,"dispatch":paused,"queue_policy":{"preemption":false,"purpose_filters":"history only"}}),
        )
    }
    pub fn event_action(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        match action {
            "list" => {
                let after = args["after"].as_i64().unwrap_or(0);
                let kind = args["kind"].as_str().unwrap_or("errors");
                let mut events=rows(&self.db,"SELECT e.*,d.state delivery_state,d.receipt,d.version FROM events e LEFT JOIN delivery d ON d.event=e.seq WHERE target=? AND seq>? AND ack=0 AND (?!='errors' OR code IN ('failed','interrupted','result_failed')) ORDER BY seq LIMIT 100",&[&actor.id,&after,&kind])?;
                if kind == "errors" {
                    events.retain(|e| {
                        matches!(
                            e["code"].as_str(),
                            Some("failed" | "interrupted" | "result_failed")
                        )
                    });
                }
                Ok(json!(events))
            }
            "ack" => {
                let seq = args["seq"]
                    .as_i64()
                    .ok_or_else(|| Error::input("Event sequence required"))?;
                let changed = self.db.execute(
                    "UPDATE events SET ack=1 WHERE seq=? AND target=?",
                    params![seq, actor.id],
                )?;
                require(changed == 1, "event", "Event outside recipient grant", 403)?;
                Ok(json!({"seq":seq,"acked":true}))
            }
            _ => Err(Error::input("Unknown event action")),
        }
    }
    pub fn system_action(&self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        match action {
            "status" => {
                let mut status = self.status()?;
                status["principal"] = actor.public();
                status["onboarding"] = crate::onboarding::identity();
                status["data_stores"] = self.data_stores();
                Ok(status)
            }
            "describe" => {
                if let Some(group) = args["group"].as_str() {
                    if let Some(action) = args["action"].as_str() {
                        crate::onboarding::describe(actor, group, action)
                    } else {
                        let reference = crate::onboarding::reference(actor)?;
                        reference["groups"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|g| g["name"] == group)
                            .cloned()
                            .ok_or_else(|| Error::input("Group unavailable"))
                    }
                } else {
                    crate::onboarding::reference(actor)
                }
            }
            "guide" => crate::onboarding::read(&args),
            "contract" => Ok(serde_json::from_str(include_str!(
                "../../../contracts/job-contract.json"
            ))?),
            "users" => Ok(json!(rows(
                &self.db,
                "SELECT id,provider,login,display_name FROM user_profiles ORDER BY display_name,id",
                &[]
            )?)),
            "ssh" => {
                let configured = self
                    .config
                    .extra
                    .get("ssh_info")
                    .and_then(Value::as_str)
                    .map(std::path::PathBuf::from)
                    .or_else(|| std::env::var_os("LOOM_SSH_INFO").map(Into::into))
                    .unwrap_or_else(|| "/etc/loom/ssh-info/connections.json".into());
                if !configured.exists() {
                    return Ok(json!({"state":"not_configured"}));
                }
                let info: Value = serde_json::from_slice(&std::fs::read(&configured)?)?;
                let mut machine = std::fs::read(configured.with_file_name("machine.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .filter(Value::is_object)
                    .unwrap_or(json!({}));
                machine.as_object_mut().unwrap().extend(
                    info.get(&actor.id)
                        .cloned()
                        .unwrap_or(json!({"state":"not_enabled"}))
                        .as_object()
                        .ok_or_else(|| Error::input("Invalid SSH descriptor"))?
                        .clone(),
                );
                Ok(machine)
            }
            "docker" => self.docker_observation(),
            "updates" | "check_updates" => {
                if action == "check_updates" && !self.read_only {
                    crate::storage::atomic(
                        &self.local.join("release-check-request.json"),
                        &serde_json::to_vec(&json!({"requested_at":now()}))?,
                        0o600,
                    )?;
                }
                let path = self.local.join("release-status.json");
                if path.exists() {
                    let status: Value = serde_json::from_slice(&std::fs::read(path)?)?;
                    if action == "check_updates" {
                        Ok(
                            json!({"accepted":!self.read_only&&status["enabled"]==true,"reason":if self.read_only{"read_only"}else if status["enabled"]==true{"queued"}else{"disabled"},"updates":status}),
                        )
                    } else {
                        Ok(status)
                    }
                } else {
                    Ok(json!({"status":"not_checked","automatic_deployment":false}))
                }
            }
            _ => Err(Error::input("Unknown system action")),
        }
    }
    fn data_stores(&self) -> Value {
        let mut stores = self.config.data_stores.clone();
        stores.insert(
            "data/0".into(),
            json!({"storage_id":self.config.storage_id,"mode":"rw"}),
        );
        json!(stores)
    }
    pub fn curate_research(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let changes = args["changes"]
            .as_array()
            .ok_or_else(|| Error::input("Changes required"))?;
        let key = args["key"]
            .as_str()
            .ok_or_else(|| Error::input("Key required"))?;
        let catalog = self.dashboard_catalog()?;
        let known: std::collections::BTreeSet<(String, String)> = catalog["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    (if e.get("loom_registry").is_some() {
                        "entity"
                    } else if e["kind"] == "execution_profile" {
                        "profile"
                    } else {
                        e["kind"].as_str().unwrap_or("")
                    })
                    .to_string(),
                    e["id"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        let mut proposed: std::collections::BTreeMap<(String, String), Value> = catalog["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    (
                        e["kind"].as_str().unwrap_or("").to_string(),
                        e["id"].as_str().unwrap_or("").to_string(),
                    ),
                    e["presentation"].clone(),
                )
            })
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        for change in changes {
            let key = (
                change["kind"].as_str().unwrap_or("").to_string(),
                change["id"].as_str().unwrap_or("").to_string(),
            );
            require(
                known.contains(&key) && seen.insert(key.clone()),
                "presentation",
                "Unknown or repeated research identity",
                400,
            )?;
            proposed.insert(key, change["presentation"].clone());
        }
        for ((kind, id), presentation) in &proposed {
            let mut cursor = presentation["parent_job"].as_str();
            let mut visited = std::collections::BTreeSet::new();
            if kind == "job" {
                visited.insert(id.as_str());
            }
            while let Some(parent) = cursor {
                require(
                    known.contains(&("job".into(), parent.into())) && visited.insert(parent),
                    "presentation",
                    "Invalid or cyclic parent Job binding",
                    400,
                )?;
                cursor = proposed
                    .get(&("job".into(), parent.into()))
                    .and_then(|v| v["parent_job"].as_str());
            }
        }
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, key, "research.curate", &args)? {
            return Ok(r);
        }
        active(&tx)?;
        for change in changes {
            let kind = change["kind"]
                .as_str()
                .ok_or_else(|| Error::input("Kind required"))?;
            let id = change["id"]
                .as_str()
                .ok_or_else(|| Error::input("ID required"))?;

            if let Some(parent) = change["presentation"]["parent_job"].as_str() {
                require(parent != id, "presentation", "Cannot reference self", 400)?;
                let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=? UNION SELECT 1 FROM job_workflows WHERE id=?)",params![parent,parent],|r|r.get(0))?;
                require(exists, "presentation", "Unknown parent reference", 400)?;
            }
            if args["dry_run"] == false {
                tx.execute("INSERT INTO research_overlays VALUES(?,?,?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET presentation=excluded.presentation,actor=excluded.actor,reason=excluded.reason,updated=excluded.updated",params![kind,id,encode(&change["presentation"])?,actor.id,args["reason"].as_str(),now()])?;
            }
        }
        let result =
            json!({"dry_run":args["dry_run"]!=false,"changes":changes,"execution_unchanged":true});
        if args["dry_run"] == false {
            receipt(&tx, actor, key, "research.curate", &args, &result)?;
        }
        tx.commit()?;
        Ok(result)
    }
    pub fn assign_job_owner(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Job ID required"))?;
        let owner = args["owner"]
            .as_str()
            .ok_or_else(|| Error::input("Owner required"))?;
        let detail = self.job_detail(id)?;
        let fallback = detail
            .get("recorded_owner")
            .unwrap_or(&detail["owner"])
            .as_str()
            .unwrap_or("")
            .to_string();
        self.assign_responsibility(actor, "job", id, owner, &fallback)
    }
    pub fn job_logs(&self, args: Value) -> Result<Value> {
        let row = jobs::load(
            &self.db,
            args["id"]
                .as_str()
                .ok_or_else(|| Error::input("Job ID required"))?,
        )?;
        let attempt = args["attempt"]
            .as_str()
            .or(row.aggregate.fence.attempt.as_ref().map(|a| a.as_str()))
            .ok_or_else(|| Error::input("Attempt required"))?;
        crate::log_index::legacy_guard(self, row.aggregate.id.as_str(), attempt)?;
        if let Some(value) = &self.live_logs {
            return value.clone();
        }
        let (base, _) = crate::storage::store(
            &self.config,
            &self.bulk,
            row.spec["output_storage"].as_str().unwrap_or("data/0"),
            row.spec["_output_storage_id"].as_str(),
        )?;
        let path = crate::storage::relative(
            &base,
            &format!("runs/{}/{attempt}/worker.log", row.aggregate.id),
        )?;
        use std::io::{Read, Seek, SeekFrom};
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        require(
            file.metadata()?.is_file(),
            "logs",
            "Regular log file required",
            400,
        )?;
        let total = file.metadata()?.len();
        let offset = args["offset"].as_u64().unwrap_or(0).min(total);
        let limit = args["limit"].as_u64().unwrap_or(65536).min(1024 * 1024);
        file.seek(SeekFrom::Start(offset))?;
        let mut content = vec![];
        file.take(limit).read_to_end(&mut content)?;
        let end = offset + content.len() as u64;
        Ok(
            json!({"id":row.aggregate.id.as_str(),"attempt":attempt,"offset":offset,"next_offset":end,"text":String::from_utf8_lossy(&content),"eof":end==total,"total_bytes":total}),
        )
    }
}
pub fn guide(id: Option<&str>) -> Result<Value> {
    crate::onboarding::read(&id.map(|id| json!({"id":id})).unwrap_or(json!({})))
}
impl Application {
    pub fn record_delivery(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        actor.require("events")?;
        contracts::validate(
            &json!({"type":"object","properties":{"event":{"type":"integer"},"state":{"type":"string"},"receipt":{"type":"string","minLength":1,"maxLength":512},"version":{"type":"integer","minimum":0}},"required":["event","state","receipt","version"],"additionalProperties":false}),
            &args,
        )?;
        let event = args["event"].as_i64().unwrap();
        let version = args["version"].as_i64().unwrap();
        let state = args["state"].as_str().unwrap();
        let receipt = args["receipt"].as_str().unwrap();
        let tx = self.transaction()?;
        active(&tx)?;
        require(
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE seq=? AND target=?)",
                params![event, actor.id],
                |r| r.get(0),
            )?,
            "recipient",
            "Event outside recipient grant",
            403,
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO delivery(event,state) VALUES(?,'queued')",
            [event],
        )?;
        if let Some((prior_state, prior_receipt)) = tx
            .query_row(
                "SELECT state,receipt FROM delivery_history WHERE event=? AND version=?",
                params![event, version + 1],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            require(
                prior_state == state && prior_receipt == receipt,
                "delivery",
                "Delivery version conflict",
                409,
            )?;
            return Ok(json!({"event":event,"state":state,"version":version+1}));
        }
        let (previous, current): (String, i64) = tx.query_row(
            "SELECT state,version FROM delivery WHERE event=?",
            [event],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let valid = match previous.as_str() {
            "queued" | "send_failed" => matches!(state, "sent" | "send_failed"),
            "sent" => state == "native_input",
            "native_input" => state == "claimed",
            "claimed" | "progress" => matches!(state, "progress" | "completed"),
            _ => false,
        };
        require(
            current == version && valid,
            "delivery",
            "Invalid delivery transition",
            409,
        )?;
        tx.execute(
            "UPDATE delivery SET state=?,receipt=?,version=version+1 WHERE event=? AND version=?",
            params![state, receipt, event, version],
        )?;
        tx.execute(
            "INSERT INTO delivery_history VALUES(?,?,?,?,?)",
            params![event, version + 1, state, receipt, now()],
        )?;
        tx.commit()?;
        Ok(json!({"event":event,"state":state,"version":version+1}))
    }
}

/// Capability projection never grants mutation authority.
pub fn queue_permissions(value: &mut Value, write: bool) {
    if !write {
        match value {
            Value::Object(fields) => {
                for (name, value) in fields {
                    if ["can_reorder", "can_hold", "can_resume"].contains(&name.as_str()) {
                        *value = json!(false)
                    } else {
                        queue_permissions(value, write)
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    queue_permissions(item, write)
                }
            }
            _ => {}
        }
    }
}
