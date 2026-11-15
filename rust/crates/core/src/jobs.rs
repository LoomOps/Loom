//! One requested outcome owns a typed plan. External execution follows durable intents.
use crate::{
    auth::Actor,
    compat,
    contracts::{self, Submit, Workflow},
    error::require,
    store::*,
    Error, Result,
};
use loom_domain::{
    AttemptId, Event, Fence, Job, JobId, JobState, Lane, NodeId, PlanNode, PrincipalId, Readiness,
    RecoveryLease, RecoveryState,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
#[derive(Debug, Clone)]
pub struct Record {
    pub aggregate: Job,
    pub submitter: String,
    pub owner: String,
    pub notify: String,
    pub spec: Value,
    pub result: Option<Value>,
    pub created: f64,
    pub available: f64,
    pub order: f64,
}
pub fn load(db: &Connection, id: &str) -> Result<Record> {
    let row=rows(db,"SELECT j.*,COALESCE(f.revision,r.revision,0) rust_revision,COALESCE(f.epoch,r.epoch,1) rust_epoch FROM jobs j LEFT JOIN rust_job_requests r ON r.job=j.id LEFT JOIN rust_job_fences f ON f.job=j.id WHERE j.id=?",&[&id])?.into_iter().next().ok_or_else(||Error::new("job","Job not found",404))?;
    Ok(Record {
        aggregate: Job {
            id: JobId::new(id)?,
            state: JobState::parse(row["state"].as_str().unwrap_or(""))?,
            fence: Fence {
                attempt: row["attempt"].as_str().map(AttemptId::new).transpose()?,
                revision: row["rust_revision"].as_u64().unwrap_or(0),
                epoch: row["rust_epoch"].as_u64().unwrap_or(1),
            },
            cancel_requested: row["cancel"].as_i64() == Some(1),
        },
        submitter: row["submitter"].as_str().unwrap_or("").into(),
        owner: row["owner"].as_str().unwrap_or("").into(),
        notify: row["notify"].as_str().unwrap_or("").into(),
        spec: serde_json::from_str(row["spec"].as_str().unwrap_or("{}"))?,
        result: row["result"]
            .as_str()
            .map(serde_json::from_str)
            .transpose()?,
        created: row["created"].as_f64().unwrap_or(0.),
        available: row["available_at"].as_f64().unwrap_or(0.),
        order: row["queue_order"]
            .as_f64()
            .unwrap_or(row["created"].as_f64().unwrap_or(0.)),
    })
}
pub fn transition(db: &Connection, record: &Record, event: Event) -> Result<JobState> {
    let decision = record.aggregate.decide(&record.aggregate.fence, event)?;
    let n=db.execute("UPDATE jobs SET state=?,cancel=? WHERE id=? AND state=? AND attempt IS ? AND COALESCE((SELECT revision FROM rust_job_fences WHERE job=jobs.id),(SELECT revision FROM rust_job_requests WHERE job=jobs.id),0)=? AND EXISTS(SELECT 1 FROM rust_authority WHERE epoch=?)",params![decision.state.as_str(),i64::from(decision.cancel_requested),record.aggregate.id.as_str(),record.aggregate.state.as_str(),record.aggregate.fence.attempt.as_ref().map(|s|s.as_str()),record.aggregate.fence.revision as i64,record.aggregate.fence.epoch as i64])?;
    require(n == 1, "fence", "Execution changed during transition", 409)?;
    Ok(decision.state)
}
pub fn event(db: &Connection, record: &Record, code: &str) -> Result<()> {
    let id = record.aggregate.id.as_str();
    let parent: Option<String> = db
        .query_row("SELECT root FROM workflow_nodes WHERE job=?", [id], |r| {
            r.get(0)
        })
        .optional()?;
    let target = if let Some(root) = parent {
        let inserted = db.execute(
            "INSERT OR IGNORE INTO rust_node_events VALUES(?,?,?,?,?)",
            params![
                id,
                record
                    .aggregate
                    .fence
                    .attempt
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or(""),
                code,
                now(),
                root
            ],
        )?;
        if inserted == 0 {
            return Ok(());
        }
        if code == "complete" {
            let pending:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM workflow_nodes n JOIN jobs j ON j.id=n.job WHERE n.root=? AND j.state!='complete')",[&root],|r|r.get(0))?;
            if pending {
                return Ok(());
            }
            db.execute(
                "UPDATE job_workflows SET state='complete' WHERE id=?",
                [&root],
            )?;
            let done: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE job=? AND code='complete')",
                [&root],
                |r| r.get(0),
            )?;
            if done {
                return Ok(());
            }
        } else if !matches!(code, "failed" | "interrupted" | "result_failed") {
            return Ok(());
        }
        let existing: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE job=? AND code=? AND attempt IS NULL)",
            params![root, code],
            |r| r.get(0),
        )?;
        if existing {
            return Ok(());
        }
        root
    } else {
        id.to_string()
    };
    db.execute(
        "INSERT OR IGNORE INTO events(job,target,code,created,attempt) VALUES(?,?,?,?,?)",
        params![
            target,
            record.notify,
            code,
            now(),
            if target == id {
                record.aggregate.fence.attempt.as_ref().map(|s| s.as_str())
            } else {
                None
            }
        ],
    )?;
    Ok(())
}

fn defaults(base: &Value, override_: &Value) -> Value {
    if let (Some(base), Some(over_)) = (base.as_object(), override_.as_object()) {
        let mut result = base.clone();
        for (k, v) in over_ {
            result.insert(
                k.clone(),
                defaults(result.get(k).unwrap_or(&Value::Null), v),
            );
        }
        json!(result)
    } else {
        override_.clone()
    }
}
fn requested_match(db: &Connection, old: &Record, v: &Submit) -> Result<bool> {
    // Compare the accepted request, independent of later Profile or Scenario revisions.
    if let Some(raw) = db
        .query_row(
            "SELECT request FROM rust_job_requests WHERE job=?",
            [old.aggregate.id.as_str()],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        let previous: Submit = contracts::decode(serde_json::from_str(&raw)?)?;
        return Ok(serde_json::to_value(previous)? == serde_json::to_value(v)?);
    }
    let source: Option<String> = db
        .query_row(
            "SELECT spec FROM job_revisions WHERE job=? AND revision=0",
            [old.aggregate.id.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    let source: Value = source
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_else(|| old.spec.clone());
    let params = defaults(
        source["profile"].get("defaults").unwrap_or(&json!({})),
        &v.params,
    );
    let mut deps = v.depends_on.clone();
    deps.sort();
    let recorded = source
        .get("_requested_dependencies")
        .or_else(|| source.get("depends_on"))
        .cloned()
        .unwrap_or(json!([]));
    Ok(source["kind"] == v.kind
        && source["owner"] == v.owner
        && source["notify"] == v.notify
        && source["params"] == params
        && source.get("category").unwrap_or(&json!("other")) == &json!(v.category)
        && source["execution_mode"] == json!(v.execution_mode)
        && source.get("execution").unwrap_or(&Value::Null) == &v.execution
        && source.get("output_storage").unwrap_or(&json!("data/0")) == &json!(v.output_storage)
        && recorded == json!(deps)
        && source
            .get("_requested_presentation")
            .unwrap_or(&source["presentation"])
            == &json!(v.presentation)
        && source.get("consolidates").unwrap_or(&json!([])) == &json!(v.consolidates)
        && source
            .get("scenario_ref")
            .filter(|r| !r.is_null())
            .map(|r| json!({"id":r["id"],"revision":r["revision"]}))
            == v.scenario_ref.as_ref().map(|r| json!(r)))
}
fn prepare(
    db: &Connection,
    config: &Config,
    bulk: &std::path::Path,
    actor: &Actor,
    request: &Value,
) -> Result<Value> {
    let v: Submit = contracts::decode(request.clone())?;
    require(
        v.params.is_object(),
        "params",
        "Job params must be an object",
        400,
    )?;
    for identity in [&v.key, &v.owner, &v.notify] {
        JobId::new(identity.clone())?;
    }
    let mut spec = serde_json::to_value(&v)?;
    let mut presentation = v.presentation.clone().unwrap_or_default();
    if v.presentation.is_none() && v.kind == "cpu.smoke" {
        presentation.scope = "operations".into();
        presentation.role = "test".into();
    }
    spec["presentation"] = json!(presentation);
    spec["_requested_presentation"] = json!(v.presentation);
    let mut requested = v.depends_on.clone();
    requested.sort();
    spec["_requested_dependencies"] = json!(requested);
    if let Some(reference) = v.scenario_ref.as_ref() {
        spec["scenario_ref"] = crate::scenario::pin(db, reference)?;
    }
    let (_, store) = crate::storage::output_store(config, bulk, &v.output_storage, None)?;
    spec["_output_storage_id"] = json!(store);
    if v.kind == "cpu.smoke" {
        contracts::validate(
            &json!({"type":"object","properties":{"seconds":{"type":"integer","minimum":0,"maximum":20},"fail":{"type":"boolean"}},"required":["seconds","fail"],"additionalProperties":false}),
            &v.params,
        )?;
    } else {
        let p = crate::software::profile(db, config, &v.kind)?
            .ok_or_else(|| Error::input("Installed Profile not found"))?;
        crate::software::validate_profile(&p)?;
        spec["params"] = defaults(p.get("defaults").unwrap_or(&json!({})), &v.params);
        contracts::validate(&p["params_schema"], &spec["params"])?;
        spec["profile"] = p;
    }
    let mut dependencies = v.depends_on.clone();
    for (name, prefix) in spec["profile"]["inputs"].as_object().into_iter().flatten() {
        let value = &spec["params"][name];
        let object = value
            .as_object()
            .ok_or_else(|| Error::input("Declared input must be a locator or upstream entity"))?;
        if object.len() == 2 && value["job"].is_string() && value["entity"].is_string() {
            let job = value["job"].as_str().unwrap();
            if !dependencies.iter().any(|d| d == job) {
                dependencies.push(job.into());
            }
        } else {
            require(
                object.len() == 4
                    && ["machine_id", "storage_id", "ref", "sha256"]
                        .iter()
                        .all(|k| object.contains_key(*k)),
                "input",
                "Invalid input locator fields",
                400,
            )?;
            require(
                value["sha256"].as_str().is_some_and(|s| {
                    s.len() == 64
                        && s.bytes()
                            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                }),
                "input",
                "Input SHA256 required",
                400,
            )?;
            let file = crate::storage::locator(config, bulk, value)?;
            let root = if value["storage_id"] == config.storage_id {
                bulk.to_path_buf()
            } else {
                let selector = config
                    .data_stores
                    .iter()
                    .find(|(_, s)| s["storage_id"] == value["storage_id"])
                    .map(|(key, _)| key)
                    .ok_or_else(|| Error::input("Input store not configured"))?;
                crate::storage::store(config, bulk, selector, value["storage_id"].as_str())?.0
            };
            let allowed = crate::storage::prefix(
                &root,
                prefix
                    .as_str()
                    .ok_or_else(|| Error::input("Input prefix must be a string"))?,
            )?;
            require(
                file.is_file() && file.starts_with(allowed),
                "input",
                "Input unavailable or outside Profile prefix",
                403,
            )?;
        }
    }
    for d in &dependencies {
        let workflow: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM job_workflows WHERE id=?)",
            [d],
            |r| r.get(0),
        )?;
        if !workflow {
            load(db, d)?;
        }
    }
    let mut preserved = vec![];
    let unique: std::collections::BTreeSet<_> = v.consolidates.iter().collect();
    require(
        unique.len() == v.consolidates.len(),
        "consolidation",
        "Duplicate source Jobs",
        400,
    )?;
    for source in &v.consolidates {
        let claimed: bool=db.query_row("SELECT EXISTS(SELECT 1 FROM jobs j,json_each(j.spec,'$.consolidates') c WHERE c.value=? AND j.state!='cancelled' AND NOT(j.submitter=? AND j.idem=?))",params![source,actor.id,v.key],|r|r.get(0))?;
        require(
            !claimed,
            "consolidation",
            "Source Job already belongs to a consolidation",
            409,
        )?;
        let row = load(db, source)?;
        require(
            row.aggregate.state == JobState::Complete
                && row.owner == v.owner
                && row
                    .result
                    .as_ref()
                    .is_some_and(|r| r["artifact_ready"] == true),
            "consolidation",
            "Only completed source execution can be reused",
            409,
        )?;
        preserved.push(json!({"job":source,"attempt":row.aggregate.fence.attempt.as_ref().map(|a|a.as_str()),"result":row.result}));
        if !dependencies.contains(source) {
            dependencies.push(source.clone());
        }
    }
    if !preserved.is_empty() {
        spec["_consolidation_sources"] = json!(preserved);
    }
    spec["depends_on"] = json!(dependencies);
    let policy = spec["execution"].as_object().cloned().unwrap_or_default();
    let max = spec["profile"]["limits"]["max_attempts"]
        .as_u64()
        .unwrap_or(2);
    let timeout = spec["profile"]["limits"]["timeout_seconds"]
        .as_u64()
        .unwrap_or(30);
    require(
        policy
            .get("max_attempts")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            <= max
            && policy
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(timeout)
                <= timeout,
        "limits",
        "Execution exceeds Profile limits",
        400,
    )?;
    require(
        !actor.id.is_empty(),
        "principal",
        "Authenticated actor required",
        403,
    )?;
    Ok(spec)
}
fn insert(db: &Connection, actor: &Actor, raw: &Value, spec: &Value) -> Result<String> {
    active(db)?;
    let key = spec["key"]
        .as_str()
        .ok_or_else(|| Error::input("Submission key required"))?;
    let existing: Option<String> = db
        .query_row(
            "SELECT id FROM jobs WHERE submitter=? AND idem=?",
            params![actor.id, key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        require(
            requested_match(db, &load(db, &existing)?, &contracts::decode(raw.clone())?)?,
            "idempotency_conflict",
            "Submission key reused for another request",
            409,
        )?;
        return Ok(existing);
    }
    let id = id();
    let created = now();
    let mut order: f64 = db.query_row(
        "SELECT max(COALESCE(queue_order,created)) FROM jobs",
        [],
        |r| Ok(r.get::<_, Option<f64>>(0)?.unwrap_or(created).max(created) + 1.),
    )?;
    if spec["queue_position"] == "next" {
        order=db.query_row("SELECT COALESCE(min(COALESCE(queue_order,created)),?)-1 FROM jobs WHERE state IN ('queued','ready','blocked')",[created],|r|r.get(0))?;
    }
    db.execute("INSERT INTO jobs(id,submitter,idem,spec,owner,notify,state,created,queue_order) VALUES(?,?,?,?,?,?,'queued',?,?)",params![id,actor.id,key,compat::dump(spec),spec["owner"].as_str(),spec["notify"].as_str(),created,order])?;
    let epoch: i64 = db.query_row("SELECT epoch FROM rust_authority WHERE id=1", [], |r| {
        r.get(0)
    })?;
    db.execute(
        "INSERT INTO rust_job_requests VALUES(?,?,?,?,0,?)",
        params![id, actor.id, key, encode(raw)?, epoch],
    )?;
    db.execute(
        "INSERT INTO rust_job_fences VALUES(?,0,?)",
        params![id, epoch],
    )?;
    db.execute(
        "INSERT INTO job_revisions VALUES(?,0,?,'submission',?)",
        params![id, compat::dump(spec), created],
    )?;
    Ok(id)
}
impl Application {
    pub fn submit(&mut self, actor: &Actor, raw: Value) -> Result<Value> {
        let v: Submit = contracts::decode(raw.clone())?;
        if let Some(old) = self
            .db
            .query_row(
                "SELECT id FROM jobs WHERE submitter=? AND idem=?",
                params![actor.id, v.key],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            require(
                requested_match(&self.db, &load(&self.db, &old)?, &v)?,
                "idempotency_conflict",
                "Submission key conflict",
                409,
            )?;
            active(&self.db)?;
            return Ok(json!({"id":old}));
        }
        let spec = prepare(&self.db, &self.config, &self.bulk, actor, &raw)?;
        let tx = self.transaction()?;
        let id = insert(&tx, actor, &raw, &spec)?;
        tx.commit()?;
        Ok(json!({"id":id}))
    }
    pub fn submit_workflow(&mut self, actor: &Actor, raw: Value) -> Result<Value> {
        let v: Workflow = contracts::decode(raw.clone())?;
        let order = loom_domain::order_plan(
            &v.nodes
                .iter()
                .map(|n| {
                    Ok(PlanNode {
                        id: NodeId::new(n.id.clone())?,
                        dependencies: n
                            .depends_on
                            .iter()
                            .map(|s| NodeId::new(s.clone()))
                            .collect::<loom_domain::Result<Vec<_>>>()?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        if let Some((id, request)) = self
            .db
            .query_row(
                "SELECT id,request FROM job_workflows WHERE submitter=? AND key=?",
                params![actor.id, v.key],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            require(
                serde_json::from_str::<Value>(&request)? == raw,
                "idempotency_conflict",
                "Workflow key conflict",
                409,
            )?;
            return Ok(json!({"id":id,"nodes":workflow_ids(&self.db,&id)?}));
        }
        let config = self.config.clone();
        let bulk = self.bulk.clone();
        let tx = self.transaction()?;
        active(&tx)?;
        let root = id();
        tx.execute("INSERT INTO job_workflows(id,submitter,key,request,owner,notify,created,state) VALUES(?,?,?,?,?,?,?,'queued')",params![root,actor.id,v.key,compat::dump(&raw),v.owner,v.notify,now()])?;
        let mut nodes = serde_json::Map::new();
        for node in order {
            let n = v.nodes.iter().find(|n| n.id == node.as_str()).unwrap();
            let mut spec = n.spec.clone();
            require(
                spec.is_object()
                    && ["key", "owner", "notify", "presentation", "consolidates"]
                        .iter()
                        .all(|k| spec.get(k).is_none()),
                "workflow",
                "Execution identity is inherited from logical Job",
                400,
            )?;
            spec["key"] = json!(format!(
                "workflow-{}",
                hash(format!("{}:{}:{}", actor.id, v.key, n.id))
            ));
            spec["owner"] = json!(v.owner);
            spec["notify"] = json!(v.notify);
            spec["presentation"] = json!({"scope":v.presentation.as_ref().map(|p|p.scope.as_str()).unwrap_or("research"),"role":"support","parent_job":root,"label":n.label});
            if let Some(r) = &v.scenario_ref {
                spec["scenario_ref"] = json!(r);
            }
            let mut dependencies = spec["depends_on"].as_array().cloned().unwrap_or_default();
            for dependency in &n.depends_on {
                dependencies.push(
                    nodes
                        .get(dependency)
                        .cloned()
                        .ok_or_else(|| Error::input("Unknown node dependency"))?,
                );
            }
            spec["depends_on"] = json!(dependencies);
            if let Some(params) = spec["params"].as_object_mut() {
                for value in params.values_mut() {
                    if value.as_object().is_some_and(|o| o.len() == 2)
                        && value["node"].is_string()
                        && value["entity"].is_string()
                    {
                        let dependency = value["node"].as_str().unwrap();
                        require(
                            n.depends_on.iter().any(|d| d == dependency),
                            "workflow",
                            "Input node must be a dependency",
                            400,
                        )?;
                        *value = json!({"job":nodes[dependency],"entity":value["entity"]});
                    }
                }
            }
            let frozen = prepare(&tx, &config, &bulk, actor, &spec)?;
            let child = insert(&tx, actor, &spec, &frozen)?;
            tx.execute(
                "INSERT INTO workflow_nodes VALUES(?,?,?,?)",
                params![root, n.id, n.label, child],
            )?;
            nodes.insert(n.id.clone(), json!(child));
        }
        for output in v.outputs.values() {
            let child = nodes
                .get(&output.node)
                .and_then(Value::as_str)
                .ok_or_else(|| Error::input("Unknown output node"))?;
            let row = load(&tx, child)?;
            require(
                row.spec["kind"] == "cpu.smoke" && output.entity == "observation"
                    || row.spec["profile"]["outputs"].get(&output.entity).is_some(),
                "workflow",
                "Output alias outside pinned Profile",
                400,
            )?;
        }
        tx.commit()?;
        Ok(json!({"id":root,"nodes":nodes}))
    }
    pub fn job_action(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        match action {
            "submit" => self.submit(actor, args),
            "submit_workflow" => self.submit_workflow(actor, args),
            "get" => self.job_detail(
                args["id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Job ID required"))?,
            ),
            "list" => self.job_list(&args),
            "queue" => self.queue(),
            "cancel" => {
                let id = args["id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Job ID required"))?;
                let tx = self.transaction()?;
                let root: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM job_workflows WHERE id=?)",
                    [id],
                    |r| r.get(0),
                )?;
                let ids = if root {
                    rows(&tx, "SELECT job FROM workflow_nodes WHERE root=?", &[&id])?
                        .iter()
                        .filter_map(|r| r["job"].as_str().map(String::from))
                        .collect()
                } else {
                    vec![id.to_string()]
                };
                for id in ids {
                    let row = load(&tx, &id)?;
                    if !matches!(
                        row.aggregate.state,
                        JobState::Complete | JobState::Cancelled
                    ) {
                        let next = transition(&tx, &row, Event::RequestCancel)?;
                        tx.execute("UPDATE recovery SET state='closed',token=NULL,deadline=NULL,hard_deadline=NULL,updated=? WHERE job=?",params![now(),id])?;
                        if next == JobState::Cancelled {
                            tx.execute("UPDATE attempts SET state='cancelled',ended=COALESCE(ended,?) WHERE id=? AND state='running'",params![now(),row.aggregate.fence.attempt.as_ref().map(|a|a.as_str())])?;
                            tx.execute("DELETE FROM result_outbox WHERE job=?", [&id])?;
                            tx.execute("UPDATE rust_effects SET state='returned' WHERE job=? AND stage='publish' AND state!='returned'",[&id])?;
                            event(&tx, &row, "cancelled")?;
                            if row.aggregate.fence.attempt.is_some() {
                                crate::execution::enqueue_hook(&tx, &load(&tx, &id)?, "on_cancel")?;
                            }
                        }
                    }
                }
                tx.commit()?;
                self.job_detail(id)
            }
            "reorder" | "hold" | "resume" => self.queue_mutation(actor, action, args),
            "curate" => self.curate_research(actor, args),
            "assign_owner" => self.assign_job_owner(actor, args),
            "logs" => self.job_logs(args),
            _ => Err(Error::input("Unknown Job action")),
        }
    }
    fn queue_mutation(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Job ID required"))?;
        let tx = self.transaction()?;
        active(&tx)?;
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=? UNION SELECT 1 FROM job_workflows WHERE id=?)",params![id,id],|r|r.get(0))?;
        require(exists, "job", "Job not found", 404)?;
        let before=json!(rows(&tx,"SELECT id,queue_order,queue_held FROM jobs WHERE state IN ('queued','ready','blocked') ORDER BY COALESCE(queue_order,created),created,id",&[])?);
        let moving=rows(&tx,"SELECT j.id FROM jobs j WHERE (j.id=? OR EXISTS(SELECT 1 FROM workflow_nodes n WHERE n.job=j.id AND n.root=?)) AND j.state IN ('queued','ready','blocked') ORDER BY COALESCE(j.queue_order,j.created),j.created,j.id",&[&id,&id])?.iter().filter_map(|r|r["id"].as_str().map(String::from)).collect::<Vec<_>>();
        require(
            !moving.is_empty(),
            "queue",
            "Job has no waiting execution nodes",
            409,
        )?;
        if action == "hold" || action == "resume" {
            for member in &moving {
                tx.execute(
                    "UPDATE jobs SET queue_held=? WHERE id=?",
                    params![i64::from(action == "hold"), member],
                )?;
            }
        } else {
            let mut ids = before
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|r| r["id"].as_str().map(String::from))
                .filter(|s| !moving.contains(s))
                .collect::<Vec<_>>();
            let index = match args["position"].as_str().unwrap_or("tail") {
                "first" | "next" => 0,
                "last" | "tail" => ids.len(),
                "before" | "after" => {
                    let reference = args["reference"]
                        .as_str()
                        .ok_or_else(|| Error::input("Queue reference required"))?;
                    let references=rows(&tx,"SELECT j.id FROM jobs j WHERE j.id=? OR EXISTS(SELECT 1 FROM workflow_nodes n WHERE n.job=j.id AND n.root=?)",&[&reference,&reference])?.iter().filter_map(|r|r["id"].as_str().map(String::from)).collect::<Vec<_>>();
                    let positions = ids
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| references.contains(s))
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    require(
                        !positions.is_empty(),
                        "queue",
                        "Queue reference is not waiting",
                        400,
                    )?;
                    if args["position"] == "before" {
                        positions[0]
                    } else {
                        positions[positions.len() - 1] + 1
                    }
                }
                _ => return Err(Error::input("Invalid queue placement")),
            };
            ids.splice(index..index, moving);
            let base = now();
            for (i, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE jobs SET queue_order=? WHERE id=?",
                    params![base + i as f64, id],
                )?;
            }
        }
        let after=json!(rows(&tx,"SELECT id,queue_order,queue_held FROM jobs WHERE state IN ('queued','ready','blocked') ORDER BY COALESCE(queue_order,created),created,id",&[])?);
        if before != after {
            tx.execute("INSERT INTO queue_history(job,actor,action,recorded,reason,before_order,after_order) VALUES(?,?,?,?,?,?,?)",params![id,actor.id,action,now(),args["reason"].as_str(),encode(&before)?,encode(&after)?])?;
        }
        tx.commit()?;
        self.queue()
    }
    pub fn recover(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Job ID required"))?;
        let config = self.config.clone();
        let bulk = self.bulk.clone();
        let tx = self.transaction()?;
        active(&tx)?;
        tx.execute("UPDATE recovery SET state='deferred',token=NULL,updated=? WHERE job=? AND state IN ('waiting','claimed') AND deadline<=?",params![now(),id,now()])?;
        let row = load(&tx, id)?;
        if action == "resume" {
            let lease = args["lease"]
                .as_str()
                .ok_or_else(|| Error::input("Lease required"))?;
            if let Some((request,result))=tx.query_row("SELECT request,result FROM rust_recovery_receipts WHERE job=? AND actor=? AND lease_hash=?",params![id,actor.id,hash(lease)],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()? {
                require(serde_json::from_str::<Value>(&request)?==args,"idempotency_conflict","Recovery lease reused with different request",409)?;
                return Ok(serde_json::from_str(&result)?);
            }
        }
        let rec = rows(&tx, "SELECT * FROM recovery WHERE job=?", &[&id])?
            .into_iter()
            .next()
            .ok_or_else(|| Error::conflict("No recovery incident"))?;
        let state = rec["state"].as_str().unwrap_or("");
        let rounds = rec["rounds"].as_u64().unwrap_or(0);
        let agent = PrincipalId::new(actor.id.clone())?;
        let mut lease_out = None;
        match action {
            "claim" => {
                if state == "claimed" && rec["agent"] == actor.id {
                    lease_out = rec["token"].as_str().map(String::from);
                } else {
                    require(
                        state == "waiting" && rounds < 3,
                        "recovery",
                        "Recovery is not available",
                        409,
                    )?;
                    let token = crate::store::id();
                    tx.execute("UPDATE recovery SET state='claimed',agent=?,token=?,deadline=?,hard_deadline=?,updated=? WHERE job=?",params![actor.id,token,now()+900.,now()+1800.,now(),id])?;
                    lease_out = Some(token);
                }
            }
            "defer" => {
                require(
                    matches!(state, "waiting" | "claimed" | "deferred"),
                    "recovery",
                    "No active reservation",
                    409,
                )?;
                tx.execute(
                    "UPDATE recovery SET state='deferred',token=NULL,updated=? WHERE job=?",
                    params![now(), id],
                )?;
            }
            "reactivate" => {
                require(
                    state == "deferred",
                    "recovery",
                    "Only deferred recovery may be reactivated",
                    409,
                )?;
                tx.execute("UPDATE recovery SET state='waiting',agent=NULL,token=NULL,deadline=?,hard_deadline=NULL,updated=? WHERE job=?",params![now()+900.,now(),id])?;
            }
            "progress" | "resume" => {
                let token = args["lease"]
                    .as_str()
                    .ok_or_else(|| Error::input("Lease required"))?;
                RecoveryLease {
                    state: RecoveryState::parse(state)?,
                    rounds: rounds as u32,
                    agent: rec["agent"].as_str().map(PrincipalId::new).transpose()?,
                    token: rec["token"].as_str().map(String::from),
                    deadline: rec["deadline"].as_f64(),
                    hard_deadline: rec["hard_deadline"].as_f64(),
                }
                .authorize(&agent, token, now())?;
                if action == "progress" {
                    tx.execute(
                        "UPDATE recovery SET deadline=min(?,hard_deadline),updated=? WHERE job=?",
                        params![now() + 900., now(), id],
                    )?;
                } else {
                    require(rounds < 3, "recovery", "Recovery budget exhausted", 409)?;
                    let patch = args.get("patch").cloned().unwrap_or(json!({}));
                    require(
                        patch.is_object()
                            && patch
                                .as_object()
                                .unwrap()
                                .keys()
                                .all(|k| matches!(k.as_str(), "params" | "kind" | "execution")),
                        "recovery",
                        "Invalid recovery patch",
                        400,
                    )?;
                    let finalization = row.aggregate.state == JobState::ResultFailed;
                    if finalization {
                        require(
                            patch == json!({}),
                            "recovery",
                            "Result-only retry cannot change execution inputs",
                            409,
                        )?;
                        transition(&tx, &row, Event::RetryPublication)?;
                        tx.execute(
                            "UPDATE result_outbox SET retries=0,next_try=0 WHERE job=?",
                            [id],
                        )?;
                    } else {
                        let mut requested = row.spec.clone();
                        if let Some(reference) = requested
                            .get_mut("scenario_ref")
                            .and_then(Value::as_object_mut)
                        {
                            reference.remove("sha256");
                        }
                        requested.as_object_mut().unwrap().retain(|k, _| {
                            !matches!(
                                k.as_str(),
                                "profile"
                                    | "_requested_presentation"
                                    | "_requested_dependencies"
                                    | "_output_storage_id"
                                    | "_consolidation_sources"
                            )
                        });
                        for (k, v) in patch.as_object().unwrap() {
                            requested[k] = v.clone();
                        }
                        let spec = prepare(&tx, &config, &bulk, actor, &requested)?;
                        transition(&tx, &row, Event::RetryExecution)?;
                        if let Some(old) = &row.result {
                            tx.execute(
                                "INSERT OR IGNORE INTO attempt_results VALUES(?,?,?)",
                                params![
                                    id,
                                    row.aggregate.fence.attempt.as_ref().map(|a| a.as_str()),
                                    encode(old)?
                                ],
                            )?;
                        }
                        tx.execute(
                            "UPDATE jobs SET spec=?,result=NULL,cancel=0,available_at=0 WHERE id=?",
                            params![compat::dump(&spec), id],
                        )?;
                    }
                    let revision: i64 = tx.query_row(
                        "SELECT COALESCE(max(revision),0)+1 FROM job_revisions WHERE job=?",
                        [id],
                        |r| r.get(0),
                    )?;
                    tx.execute(
                        "INSERT INTO job_revisions SELECT id,?,spec,?,? FROM jobs WHERE id=?",
                        params![
                            revision,
                            args["reason"].as_str().unwrap_or("agent recovery"),
                            now(),
                            id
                        ],
                    )?;
                    tx.execute(
                        "UPDATE rust_job_fences SET revision=? WHERE job=?",
                        params![revision, id],
                    )?;
                    tx.execute(
                        "UPDATE rust_job_requests SET revision=? WHERE job=?",
                        params![revision, id],
                    )?;
                    tx.execute(
                        "UPDATE recovery SET state='pending',token=NULL,updated=? WHERE job=?",
                        params![now(), id],
                    )?;
                }
            }
            _ => return Err(Error::input("Unknown recovery action")),
        }
        tx.execute("INSERT INTO recovery_history(job,action,agent,round,recorded,detail) VALUES(?,?,?,?,?,?)",params![id,action,actor.id,rounds as i64,now(),encode(&json!({"reason":args["reason"]}))?])?;
        let mut result=rows(&tx,"SELECT state,rounds,agent,deadline,hard_deadline,reason,updated FROM recovery WHERE job=?",&[&id])?.remove(0);
        if let Some(lease) = lease_out {
            result["lease"] = json!(lease);
        }
        if action == "resume" {
            tx.execute(
                "INSERT INTO rust_recovery_receipts VALUES(?,?,?,?,?)",
                params![
                    id,
                    actor.id,
                    hash(args["lease"].as_str().unwrap_or("")),
                    encode(&args)?,
                    encode(&result)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(result)
    }
    pub fn readiness(&self, record: &Record) -> Result<Readiness> {
        readiness(&self.db, record)
    }
}
pub fn readiness(db: &Connection, record: &Record) -> Result<Readiness> {
    let mut r = Readiness {
        admission_active: true,
        delayed: record.available > now(),
        ..Readiness::default()
    };
    let mode: String =
        db.query_row("SELECT mode FROM node_control WHERE id=1", [], |r| r.get(0))?;
    r.admission_active = mode == "active";
    r.operator_paused = db
        .query_row("SELECT paused FROM dispatch_control WHERE id=1", [], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
        .unwrap_or(0)
        != 0;
    r.held = db
        .query_row(
            "SELECT queue_held FROM jobs WHERE id=?",
            [record.aggregate.id.as_str()],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0)
        != 0;
    let dependencies = if record.aggregate.state == JobState::Queued
        && record.spec["profile"].get("prepare").is_some()
    {
        record.spec["profile"]["prepare"]
            .get("depends_on")
            .unwrap_or(&record.spec["depends_on"])
    } else {
        &record.spec["depends_on"]
    };
    for dependency in dependencies.as_array().into_iter().flatten() {
        if let Some(id) = dependency.as_str() {
            let members = rows(db, "SELECT job FROM workflow_nodes WHERE root=?", &[&id])?;
            if !members.is_empty() {
                for member in members {
                    let upstream = load(db, member["job"].as_str().unwrap_or(""))?;
                    let available = loom_domain::dependency_available(
                        upstream.aggregate.state,
                        upstream
                            .result
                            .as_ref()
                            .is_some_and(|r| r["artifact_ready"] == true),
                    );
                    r.dependency_pending |= !available;
                    r.dependency_failed |= !available && upstream.aggregate.state.terminal();
                }
                continue;
            }
            let upstream = load(db, id)?;
            let available = loom_domain::dependency_available(
                upstream.aggregate.state,
                upstream
                    .result
                    .as_ref()
                    .is_some_and(|r| r["artifact_ready"] == true),
            );
            r.dependency_pending |= !available;
            r.dependency_failed |= !available && upstream.aggregate.state.terminal();
        }
    }
    r.recovery_reserved=db.query_row("SELECT EXISTS(SELECT 1 FROM recovery WHERE job=? AND state IN ('waiting','claimed') AND deadline>?)",params![record.aggregate.id.as_str(),now()],|r|r.get(0))?;
    let lane = if record.aggregate.state == JobState::Queued
        && record.spec["profile"].get("prepare").is_some()
    {
        "cpu"
    } else {
        record.spec["profile"]["runtime"]["lane"]
            .as_str()
            .unwrap_or("cpu")
    };
    Lane::parse(lane)?;
    r.lane_busy=db.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id!=? AND state IN ('running','preparing') AND CASE WHEN state='preparing' THEN 'cpu' ELSE COALESCE(json_extract(spec,'$.profile.runtime.lane'),'cpu') END=?)",params![record.aggregate.id.as_str(),lane],|r|r.get(0))?;
    r.lane_busy |= crate::execution::lane_in_use(db, lane)?;
    Ok(r)
}
pub fn workflow_ids(db: &Connection, root: &str) -> Result<Value> {
    let mut ids = serde_json::Map::new();
    for row in rows(
        db,
        "SELECT node,job FROM workflow_nodes WHERE root=? ORDER BY rowid",
        &[&root],
    )? {
        ids.insert(
            row["node"].as_str().unwrap_or("").into(),
            row["job"].clone(),
        );
    }
    Ok(json!(ids))
}
