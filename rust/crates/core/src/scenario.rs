//! Versioned research context; linking never creates an execution or duplicates data.
use crate::{auth::Actor, contracts::*, error::require, store::*, Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
fn definition(value: &Value) -> Result<String> {
    require(
        value.is_object()
            && value["label"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 200),
        "scenario",
        "Task label required",
        400,
    )?;
    require(
        serde_json::to_vec(value)?.len() <= 128 * 1024,
        "scenario",
        "Task definition exceeds bound",
        400,
    )?;
    for (k, v) in value.as_object().unwrap() {
        require(
            matches!(
                k.as_str(),
                "label" | "description" | "protocol" | "references" | "metadata"
            ),
            "scenario",
            "Unknown Task field",
            400,
        )?;
        if k == "description" {
            require(
                v.as_str().is_some_and(|s| s.len() <= 8192),
                "scenario",
                "Invalid Task description",
                400,
            )?;
        }
    }
    encode(value)
}
pub fn pin(db: &Connection, reference: &ScenarioRef) -> Result<Value> {
    let row:Option<(String,i64)>=db.query_row("SELECT r.sha256,s.archived FROM scenario_revisions r JOIN scenarios s ON s.id=r.scenario_id WHERE r.scenario_id=? AND r.revision=?",params![reference.id,reference.revision],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let Some((sha, archived)) = row else {
        return Err(Error::new("scenario", "Task revision not found", 404));
    };
    let _ = archived; // Archive changes discovery; exact historical context remains usable.
    Ok(json!({"id":reference.id,"revision":reference.revision,"sha256":sha}))
}
impl Application {
    pub fn create_scenario(&mut self, actor: &Actor, v: ScenarioCreate) -> Result<Value> {
        let raw = json!({"definition":v.definition});
        let encoded = definition(&v.definition)?;
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, &v.key, "scenario.create", &raw)? {
            return Ok(r);
        }
        active(&tx)?;
        let identity = id();
        let created = now();
        let sha = hash(&encoded);
        tx.execute(
            "INSERT INTO scenarios VALUES(?,?,1,0,?,?)",
            params![identity, v.definition["label"].as_str(), created, actor.id],
        )?;
        tx.execute(
            "INSERT INTO scenario_revisions VALUES(?,1,?,?,?,?,?)",
            params![identity, encoded, sha, created, actor.id, "created"],
        )?;
        let result = json!({"id":identity,"revision":1,"sha256":sha});
        receipt(&tx, actor, &v.key, "scenario.create", &raw, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn update_scenario(&mut self, actor: &Actor, v: ScenarioUpdate) -> Result<Value> {
        let raw = json!({"id":v.id,"expected_revision":v.expected_revision,"definition":v.definition,"reason":v.reason});
        let encoded = definition(&v.definition)?;
        require(
            !v.reason.trim().is_empty(),
            "reason",
            "Revision reason required",
            400,
        )?;
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, &v.key, "scenario.update", &raw)? {
            return Ok(r);
        }
        active(&tx)?;
        let changed=tx.execute("UPDATE scenarios SET current_revision=current_revision+1,label=? WHERE id=? AND current_revision=? AND archived=0",params![v.definition["label"].as_str(),v.id,v.expected_revision])?;
        require(
            changed == 1,
            "revision",
            "Task revision changed or archived",
            409,
        )?;
        let revision = v.expected_revision + 1;
        let sha = hash(&encoded);
        tx.execute(
            "INSERT INTO scenario_revisions VALUES(?,?,?,?,?,?,?)",
            params![v.id, revision, encoded, sha, now(), actor.id, v.reason],
        )?;
        let result = json!({"id":v.id,"revision":revision,"sha256":sha});
        receipt(&tx, actor, &v.key, "scenario.update", &raw, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn archive_scenario(&mut self, actor: &Actor, v: Value) -> Result<Value> {
        let key = v["key"]
            .as_str()
            .ok_or_else(|| Error::input("Key required"))?;
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, key, "scenario.archive", &v)? {
            return Ok(r);
        }
        active(&tx)?;
        let n = tx.execute(
            "UPDATE scenarios SET archived=1 WHERE id=? AND current_revision=? AND archived=0",
            params![v["id"].as_str(), v["expected_revision"].as_i64()],
        )?;
        require(n == 1, "revision", "Task revision changed", 409)?;
        let result = json!({"id":v["id"],"archived":true});
        receipt(&tx, actor, key, "scenario.archive", &v, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn link_scenario(&mut self, actor: &Actor, v: ScenarioLink) -> Result<Value> {
        let raw = json!({"scenario_ref":v.scenario_ref,"target":v.target,"relation":v.relation,"provenance":v.provenance});
        let config = self.config.clone();
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, &v.key, "scenario.link", &raw)? {
            return Ok(r);
        }
        active(&tx)?;
        pin(&tx, &v.scenario_ref)?;
        crate::results::verify_target(&tx, &v.target, &config)?;
        let identity = id();
        tx.execute(
            "INSERT INTO scenario_links VALUES(?,?,?,?,?,?,?,?,?,NULL,NULL)",
            params![
                identity,
                v.scenario_ref.id,
                v.scenario_ref.revision,
                v.target.kind,
                encode(&v.target.identity)?,
                v.relation,
                encode(&v.provenance)?,
                now(),
                actor.id
            ],
        )?;
        let result = json!({"id":identity,"scenario_ref":v.scenario_ref,"target":v.target});
        receipt(&tx, actor, &v.key, "scenario.link", &raw, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn supersede_scenario_link(&mut self, actor: &Actor, v: Value) -> Result<Value> {
        let key = v["key"]
            .as_str()
            .ok_or_else(|| Error::input("Key required"))?;
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, key, "scenario.supersede", &v)? {
            return Ok(r);
        }
        active(&tx)?;
        let replacement = v["replacement"].as_str().unwrap_or("withdrawn");
        if replacement != "withdrawn" {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM scenario_links replacement JOIN scenario_links old ON replacement.scenario_id=old.scenario_id WHERE replacement.id=? AND old.id=? AND replacement.superseded_by IS NULL)",
                params![replacement,v["id"].as_str()],
                |r| r.get(0),
            )?;
            require(
                exists && Some(replacement) != v["id"].as_str(),
                "link",
                "Invalid replacement link",
                409,
            )?;
        }
        let n=tx.execute("UPDATE scenario_links SET superseded_by=?,reason=? WHERE id=? AND superseded_by IS NULL",params![replacement,v["reason"].as_str(),v["id"].as_str()])?;
        require(n == 1, "link", "Link already changed", 409)?;
        let result = json!({"id":v["id"],"superseded_by":replacement});
        receipt(&tx, actor, key, "scenario.supersede", &v, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn scenario_query(&self, operation: &str, v: Value) -> Result<Value> {
        let limit = v["limit"].as_i64().unwrap_or(100).clamp(1, 200);
        let offset = v["offset"].as_i64().unwrap_or(0).max(0);
        if operation == "scenarios" {
            return Ok(json!(rows(&self.db,"SELECT * FROM scenarios WHERE (? OR archived=0) ORDER BY created DESC,id LIMIT ? OFFSET ?",&[&v["include_archived"].as_bool().unwrap_or(false),&limit,&offset])?));
        }
        let id = v["id"]
            .as_str()
            .ok_or_else(|| Error::input("Task ID required"))?;
        let scenario = rows(&self.db, "SELECT * FROM scenarios WHERE id=?", &[&id])?
            .into_iter()
            .next()
            .ok_or_else(|| Error::new("scenario", "Task not found", 404))?;
        let revision = v["revision"]
            .as_i64()
            .or(scenario["current_revision"].as_i64())
            .unwrap_or(1);
        let mut record = rows(
            &self.db,
            "SELECT * FROM scenario_revisions WHERE scenario_id=? AND revision=?",
            &[&id, &revision],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("scenario", "Revision not found", 404))?;
        record["definition"] = serde_json::from_str(record["definition"].as_str().unwrap_or("{}"))?;
        let mut result = json!({"scenario":scenario,"revision":record});
        if operation == "scenario_graph" {
            let mut links=rows(&self.db,"SELECT * FROM scenario_links WHERE scenario_id=? AND revision=? ORDER BY created,id LIMIT ? OFFSET ?",&[&id,&revision,&limit,&offset])?;
            for link in &mut links {
                for field in ["target", "provenance"] {
                    link[field] = serde_json::from_str(link[field].as_str().unwrap_or("{}"))?;
                }
            }
            for link in &mut links {
                link["target"] = json!({"kind":link["target_kind"],"identity":link["target"]});
            }
            result["links"] = json!(links);
            result["execution"] = json!("context only");
        }
        Ok(result)
    }
}

/// Compact Task metadata is derived from exact accepted references and additive links.
/// It never changes the original Job, result, or historical snapshot.
impl Application {
    pub fn dashboard_tasks(&self) -> Result<Value> {
        Ok(json!(rows(&self.db,"SELECT s.id,s.current_revision AS revision,json_extract(r.definition,'$.label') AS label FROM scenarios s JOIN scenario_revisions r ON r.scenario_id=s.id AND r.revision=s.current_revision WHERE s.archived=0 ORDER BY s.label,s.id",&[])?))
    }
    pub fn attach_task_context(&self, entities: &mut [Value]) -> Result<()> {
        let links=rows(&self.db,"SELECT l.scenario_id AS id,l.revision,l.target_kind,l.target,json_extract(r.definition,'$.label') AS label FROM scenario_links l JOIN scenario_revisions r ON r.scenario_id=l.scenario_id AND r.revision=l.revision WHERE l.superseded_by IS NULL ORDER BY label,l.scenario_id,l.revision",&[])?;
        for entity in entities {
            let target = dashboard_target(entity);
            let mut refs = vec![];
            if entity["scenario_ref"].is_object() {
                let pin = &entity["scenario_ref"];
                let label:Option<String>=self.db.query_row("SELECT json_extract(definition,'$.label') FROM scenario_revisions WHERE scenario_id=? AND revision=?",params![pin["id"].as_str(),pin["revision"].as_i64()],|r|r.get(0)).optional()?;
                if let Some(label) = label {
                    refs.push(json!({"id":pin["id"],"revision":pin["revision"],"label":label}));
                }
            }
            for link in &links {
                let kind = if link["target_kind"] == "protocol_artifact" {
                    "entity"
                } else {
                    link["target_kind"].as_str().unwrap_or("")
                };
                let identity: Value =
                    serde_json::from_str(link["target"].as_str().unwrap_or("{}"))?;
                if target["kind"] == kind
                    && identity.as_object().is_some_and(|o| {
                        o.iter().all(|(k, v)| target["identity"].get(k) == Some(v))
                    })
                {
                    let reference =
                        json!({"id":link["id"],"revision":link["revision"],"label":link["label"]});
                    if !refs.contains(&reference) {
                        refs.push(reference);
                    }
                }
            }
            refs.sort_by_key(|v| v["label"].as_str().unwrap_or("").to_string());
            entity["task_refs"] = json!(refs);
        }
        Ok(())
    }
}
fn dashboard_target(e: &Value) -> Value {
    if e["loom_registry"].is_object() {
        return json!({"kind":"entity","identity":e["loom_registry"]});
    }
    if e["loom_history"].is_object() {
        return json!({"kind":"historical","identity":e["loom_history"]});
    }
    if e["loom_extension"].is_object() {
        let x = &e["loom_extension"];
        return json!({"kind":e["kind"],"identity":{"owner":x["owner"],"id":x["id"],"revision":x["revision"]}});
    }
    if e["loom_runtime"].is_object() {
        let x = &e["loom_runtime"];
        return json!({"kind":"runtime","identity":{"id":x["id"],"revision":x["revision"],"sha256":x["sha256"]}});
    }
    if e["loom_execution_profile"].is_object() {
        let x = &e["loom_execution_profile"];
        return json!({"kind":"profile","identity":{"kind":x["kind"],"sha256":x["sha256"]}});
    }
    if e["kind"] == "job" {
        return json!({"kind":"job","identity":{"id":e["id"]}});
    }
    Value::Null
}
