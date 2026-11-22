//! Outcome admission shares the workflow root, pinned execution and scheduler transaction.
use crate::{
    auth::Actor,
    contracts::{Node, Workflow},
    error::require,
    jobs,
    store::*,
    Error, Result,
};
use loom_domain::{JobId, NodeId, PlanNode};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn preview_outcome(db: &Connection, outcome: &Value) -> Result<Value> {
    let mut coverage = Map::new();
    let mut promises = BTreeMap::new();
    for promise in outcome["deliverables"].as_array().into_iter().flatten() {
        let alias = promise["alias"].as_str().unwrap();
        require(
            promises.insert(alias, promise).is_none(),
            "deliverable",
            "Duplicate deliverable alias",
            400,
        )?;
    }
    let science = &outcome["science"];
    if science.is_null() {
        return Ok(json!({"mode":"exploratory","coverage":{},"science_digest":null}));
    }
    let expectations = science["deliverables"].as_object().unwrap();
    let hypothesis = &science["hypothesis"];
    for field in ["statement", "analysis", "population", "metric"] {
        require(
            hypothesis.is_null()
                || hypothesis[field]
                    .as_str()
                    .is_some_and(|v| !v.trim().is_empty()),
            "hypothesis",
            "A declared hypothesis needs a statement, analysis, population and metric",
            400,
        )?;
    }
    for alias in hypothesis["aliases"].as_array().into_iter().flatten() {
        require(
            expectations.contains_key(alias.as_str().unwrap()),
            "hypothesis",
            "Hypothesis references an undeclared scientific deliverable",
            400,
        )?;
    }
    for (alias, request) in expectations {
        let kind = promises
            .get(alias.as_str())
            .and_then(|d| d["kind"].as_str())
            .unwrap_or("");
        require(
            matches!(kind, "dataset" | "model" | "experiment"),
            "science",
            "Scientific expectations require a typed deliverable alias",
            400,
        )?;
        let template = crate::metadata_templates::resolve(db, &request["template_ref"])?;
        let contract = crate::reporting::resolve(db, "report_contract", &request["contract_ref"])?;
        require(
            template["output_kind"] == kind
                && contract["output_kind"] == kind
                && contract["activity"] == request["activity"],
            "science",
            "Template, report contract and activity must match the promised output",
            409,
        )?;
        for pin in request["inputs"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
        {
            require(
                db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM all_entity_revisions WHERE id=? AND revision=?)",
                    params![pin["id"].as_str(), pin["revision"].as_i64()],
                    |r| r.get(0),
                )?,
                "science",
                "Pinned scientific input revision does not exist",
                409,
            )?;
        }
        let mut sessions = BTreeSet::new();
        for session in request["sessions"].as_array().into_iter().flatten() {
            require(
                sessions.insert(session["id"].as_str().unwrap()),
                "science",
                "Duplicate setup session",
                400,
            )?;
            let setup = &session["configuration"];
            let raw: Option<String> = db.query_row("SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=? AND kind='artifact'",
                params![setup["id"].as_str(),setup["revision"].as_i64()], |r| r.get(0)).optional()?;
            let manifest: Value =
                serde_json::from_str(&raw.ok_or_else(|| {
                    Error::new("science", "Setup artifact revision not found", 404)
                })?)?;
            require(
                manifest.pointer("/metadata/experimental_setup/domain") == Some(&session["domain"])
                    && manifest["components"]
                        .as_array()
                        .is_some_and(|c| c.iter().any(|v| v["sha256"] == setup["sha256"])),
                "science",
                "Setup artifact must declare the same domain and exact component hash",
                409,
            )?;
        }
        let requirements: Vec<_> = crate::reporting::effective(&contract).into_iter().map(|r| json!({"id":r.id,"role":r.role,"required":r.required,"definitions":r.definitions})).collect();
        coverage.insert(alias.clone(),json!({"kind":kind,"activity":request["activity"],"template_ref":request["template_ref"],
            "contract_ref":request["contract_ref"],"requirements":requirements,"sessions":request["sessions"],
            "analysis_required":hypothesis["aliases"].as_array().is_some_and(|a|a.contains(&json!(alias)))}));
    }
    Ok(
        json!({"mode":if hypothesis.is_null(){"exploratory"}else{"hypothesis"},"coverage":coverage,
        "science_digest":hash(crate::compat::seal(science))}),
    )
}

pub(crate) struct ScientificPlan {
    root: String,
    node: String,
    doc: Value,
}

pub(crate) fn scientific_plan(db: &Connection, job: &str) -> Result<Option<ScientificPlan>> {
    let parent: Option<(String,String,String)> = db.query_row(
        "SELECT w.id,n.node,w.request FROM execution_memberships n JOIN job_workflows w ON w.id=n.root WHERE n.job=? AND json_type(w.request,'$.outcome.science')='object'",
        [job], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if parent.is_none() {
        let direct: Option<String> = db
            .query_row(
                "SELECT spec FROM jobs WHERE id=? AND json_type(spec,'$.outcome.science')='object'",
                [job],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(spec) = direct {
            return Ok(Some(ScientificPlan {
                root: job.to_owned(),
                node: String::new(),
                doc: serde_json::from_str(&spec)?,
            }));
        }
    }
    parent
        .map(|(root, node, raw)| {
            Ok(ScientificPlan {
                root,
                node,
                doc: serde_json::from_str(&raw)?,
            })
        })
        .transpose()
}

// Standalone Jobs publish their own entity. Older commitments explicitly name its producer.
fn produced_entity<'a>(promise: &'a Value, producer: &str) -> Option<&'a str> {
    if producer.is_empty() {
        promise["entity"].as_str()
    } else if promise["from"]["node"] == producer {
        promise["from"]["entity"].as_str()
    } else {
        None
    }
}

impl ScientificPlan {
    fn deliverables<'a>(&'a self, entity: &'a Value) -> impl Iterator<Item = (&'a str, &'a Value)> {
        let expected = &self.doc["outcome"]["science"]["deliverables"];
        self.doc["outcome"]["deliverables"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| produced_entity(p, &self.node).is_some_and(|id| entity["id"] == id))
            .filter_map(move |p| {
                let alias = p["alias"].as_str()?;
                Some((alias, expected.get(alias)?))
            })
    }

    pub(crate) fn accepts_component(&self, entity: &Value, component: &Value) -> bool {
        self.deliverables(entity).any(|(alias, e)| {
            self.needs_evidence(alias, e)
                && entity["metadata"]["science_evidence"][alias]["component"] == *component
        })
    }

    fn needs_evidence(&self, alias: &str, expected: &Value) -> bool {
        expected["sessions"]
            .as_array()
            .is_some_and(|s| !s.is_empty())
            || self.has_hypothesis(alias)
    }

    fn has_hypothesis(&self, alias: &str) -> bool {
        self.doc["outcome"]["science"]["hypothesis"]["aliases"]
            .as_array()
            .is_some_and(|a| a.contains(&json!(alias)))
    }
}

fn science_member_key(value: &Value, evaluation: bool) -> Option<String> {
    if evaluation {
        let id = value["id"].as_str().filter(|v| !v.is_empty())?;
        let seed = &value["seed"];
        (seed.is_i64() || seed.is_u64()).then(|| crate::compat::seal(&json!([id, seed])))
    } else {
        value.as_str().filter(|v| !v.is_empty()).map(str::to_owned)
    }
}

pub(crate) fn verify_output_science(
    plan: &ScientificPlan,
    job: &str,
    attempt: &str,
    entity: &Value,
    folder: &std::path::Path,
    proofs: &[crate::storage::VerifiedFile],
    published: &Value,
) -> Result<Option<Value>> {
    let doc = &plan.doc;
    let science = &doc["outcome"]["science"];
    let mut claims = Map::new();
    for (alias, expectation) in plan.deliverables(entity) {
        for (input, pin) in expectation["inputs"].as_object().into_iter().flatten() {
            require(
                published["relations"].as_array().is_some_and(|items| {
                    items.iter().any(|r| {
                        r["kind"] == "consumes"
                            && r["input"] == *input
                            && r["id"] == pin["id"]
                            && r["revision"] == pin["revision"]
                    })
                }),
                "science",
                "Published lineage differs from the accepted scientific input",
                409,
            )?;
        }
        let hypothesis = plan.has_hypothesis(alias);
        let sessions = expectation["sessions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut result = json!({"activity":expectation["activity"],"template_ref":expectation["template_ref"],"contract_ref":expectation["contract_ref"],
            "hypothesis":if hypothesis{science["hypothesis"].clone()}else{Value::Null}});
        if plan.needs_evidence(alias, expectation) {
            let binding = &entity["metadata"]["science_evidence"][alias];
            let rel = binding["component"].as_str().ok_or_else(|| {
                Error::input("Declare a verified science evidence component for the deliverable")
            })?;
            let exact = published["components"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["ref"] == rel && p["sha256"] == binding["sha256"])
                .ok_or_else(|| {
                    Error::conflict("Science evidence component is not in the verified output")
                })?;
            let path = crate::storage::relative(folder, rel)?;
            let proof = proofs
                .iter()
                .find(|p| p.path == path)
                .ok_or_else(|| Error::conflict("Science evidence proof missing"))?;
            let value = crate::reporting::read_json(&path, proof)?;
            require(
                value["schema"] == "loom-science-evidence/v1"
                    && value["job"] == job
                    && value["attempt"] == attempt
                    && value["alias"] == alias
                    && value["science_digest"] == doc["science_digest"],
                "science",
                "Evidence does not match the accepted outcome and current Attempt",
                409,
            )?;
            if hypothesis {
                require(
                    value["analysis"]["summary"]
                        .as_str()
                        .is_some_and(|v| !v.trim().is_empty() && v.len() <= 8192)
                        && value["analysis"]["metric"] == science["hypothesis"]["metric"],
                    "science",
                    "The declared analysis needs an observed result for the accepted metric",
                    409,
                )?;
            }
            let observed = value["sessions"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            let evaluation = entity["kind"] == "experiment";
            let mut members = BTreeSet::new();
            let mut session_receipts = Vec::new();
            require(
                observed.len() == sessions.len(),
                "science",
                "Session coverage differs from the accepted scope",
                409,
            )?;
            for expected in sessions {
                let observed = observed
                    .iter()
                    .find(|s| {
                        s["id"] == expected["id"]
                            && s["domain"] == expected["domain"]
                            && s["configuration"] == expected["configuration"]
                    })
                    .ok_or_else(|| {
                        Error::conflict("Setup application does not match the accepted session")
                    })?;
                let ids = observed["members"]
                    .as_array()
                    .ok_or_else(|| Error::input("Session member IDs are required"))?;
                require(
                    !ids.is_empty()
                        && ids.iter().all(|id| {
                            science_member_key(id, evaluation)
                                .is_some_and(|key| members.insert(key))
                        })
                        && observed["readback"]
                            .as_object()
                            .is_some_and(|m| !m.is_empty())
                        && crate::compat::seal(&observed["readback"]).len() <= 16384,
                    "science",
                    "Session needs non-overlapping recorded members and a bounded readback",
                    409,
                )?;
                session_receipts.push(json!({"id":expected["id"],"domain":expected["domain"],"configuration":expected["configuration"],
                    "member_count":ids.len(),"readback":observed["readback"]}));
            }
            if !sessions.is_empty() {
                let expected_role = match entity["kind"].as_str() {
                    Some("dataset") => Some(("dataset_statistics", "members")),
                    Some("experiment") => Some(("evaluation_cases", "cases")),
                    _ => None,
                };
                if let Some((role, field)) = expected_role {
                    let reports = published["reporting"]["reports"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|r| r["state"] == "recorded" && r["role"] == role);
                    let mut cohort = BTreeSet::new();
                    for report in reports {
                        let file = crate::storage::relative(
                            folder,
                            report["component"].as_str().unwrap(),
                        )?;
                        let proof = proofs
                            .iter()
                            .find(|p| p.path == file)
                            .ok_or_else(|| Error::conflict("Population report evidence missing"))?;
                        let verified = crate::reporting::read_json(&file, proof)?;
                        for member in verified["data"][field].as_array().into_iter().flatten() {
                            let identity = if evaluation { member } else { &member["id"] };
                            if let Some(key) = science_member_key(identity, evaluation) {
                                cohort.insert(key);
                            }
                        }
                    }
                    require(
                        !cohort.is_empty() && members.iter().all(|id| cohort.contains(id)),
                        "science",
                        "Session references members outside the verified report population",
                        409,
                    )?;
                }
            }
            result["source"] = exact.clone();
            result["sessions"] = json!(session_receipts);
            if hypothesis {
                result["analysis"] = json!({"metric":value["analysis"]["metric"],"summary":value["analysis"]["summary"]});
            }
        }
        claims.insert(alias.into(), result);
    }
    Ok((!claims.is_empty())
        .then(|| json!({"outcome":plan.root,"digest":doc["science_digest"],"deliverables":claims})))
}

fn units(plan: &Value) -> BTreeMap<&str, &Value> {
    plan["units"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|u| (u["id"].as_str().unwrap(), u))
        .collect()
}

fn runtime_receipt(db: &Connection, target: &Value) -> Result<Value> {
    let row = db.query_row("SELECT json_extract(i.value,'$.sha256'),json_extract(i.value,'$.manifest.validation'),o.observed
        FROM rust_runtime_observation o,json_each(o.value,'$.runtimes') i
        WHERE o.id=1 AND o.observed>=? AND json_extract(i.value,'$.id')=?
        AND json_extract(i.value,'$.revision')=? AND coalesce(json_extract(i.value,'$.legacy'),0)=0
        AND json_extract(i.value,'$.manifest.validation.status')='passed'",
        params![now()-30.,target["id"].as_str(),target["revision"].as_i64()],
        |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,f64>(2)?))).optional()?
        .ok_or_else(||Error::new("runtime_pending","Current instance grant and passed Runtime validation required",409))?;
    require(
        target.get("sha256").is_none_or(|s| s == &row.0),
        "runtime_pin",
        "Runtime digest mismatch",
        409,
    )?;
    Ok(
        json!({"id":target["id"],"revision":target["revision"],"sha256":row.0,"validation":serde_json::from_str::<Value>(&row.1)?,"observed_at":row.2}),
    )
}

fn validate_revision(old: &Value, next: &Value, bound: &Map<String, Value>) -> Result<()> {
    require(
        old["sealed"] != true || old == next,
        "plan",
        "A sealed plan cannot change its execution graph",
        409,
    )?;
    let updated = units(next);
    for (id, prior) in units(old) {
        let slot = updated
            .get(id)
            .ok_or_else(|| Error::new("plan", "Existing slot cannot be removed", 409))?;
        if bound.contains_key(id) {
            require(
                prior == *slot,
                "plan",
                "Accepted execution cannot change",
                409,
            )?;
        } else {
            for field in ["depends_on", "requires"] {
                require(
                    prior[field]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .all(|v| slot[field].as_array().is_some_and(|a| a.contains(v))),
                    "plan",
                    "Existing prerequisites cannot be removed",
                    409,
                )?;
            }
        }
    }
    Ok(())
}

fn output_kind(spec: &Value, entity: &str) -> Option<String> {
    if spec["kind"] == "cpu.smoke" && entity == "observation" {
        return Some("artifact".into());
    }
    spec["profile"]["outputs"][entity]
        .as_str()
        .map(str::to_owned)
}

pub(crate) fn check_execution(doc: &Value, node: &str, spec: &Value) -> Result<()> {
    for (d, entity) in doc["outcome"]["deliverables"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| produced_entity(d, node).map(|entity| (d, entity)))
    {
        require(
            output_kind(spec, entity).as_deref() == d["kind"].as_str(),
            "deliverable",
            "Pinned Profile must publish the promised entity and kind",
            409,
        )?;
        let Some(expected) =
            doc["outcome"]["science"]["deliverables"].get(d["alias"].as_str().unwrap())
        else {
            continue;
        };
        let profile = &spec["profile"];
        let selector = format!("output:{entity}");
        let kind = d["kind"].as_str().unwrap();
        let template = profile["metadata_template_pins"]
            .get(&selector)
            .or_else(|| profile["metadata_template_pins"].get(kind));
        let reporting = profile["reporting_pins"]
            .get(&selector)
            .or_else(|| profile["reporting_pins"].get(kind));
        require(
            template.is_some_and(|t| {
                crate::metadata_templates::reference(t) == expected["template_ref"]
            }) && reporting
                .is_some_and(|r| crate::reporting::reference(r) == expected["contract_ref"]),
            "science",
            "Profile must bind the accepted template and reporting contract",
            409,
        )?;
        for (input, pin) in expected["inputs"].as_object().into_iter().flatten() {
            require(
                spec["params"][input]["artifact"] == *pin,
                "science",
                "Execution parameters must bind the accepted scientific input revision",
                409,
            )?;
        }
    }
    for g in doc["stage_receipts"][node].as_array().into_iter().flatten() {
        let pin = &spec["profile"][g["stage"].as_str().unwrap()]["runtime_pin"];
        require(
            ["id", "revision", "sha256"]
                .iter()
                .all(|k| pin[k] == g["runtime"][k]),
            "runtime_pin",
            "Profile stage must select the verified Runtime revision",
            409,
        )?;
    }
    Ok(())
}

// Returns true for a managed plan, including when it cannot yet complete.
pub(crate) fn finish(db: &Connection, root: &str) -> Result<bool> {
    let Some((raw,state,notify))=db.query_row("SELECT request,state,notify FROM job_workflows WHERE id=? AND json_type(request,'$.plan_revision')='integer'",[root],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional()? else {return Ok(false)};
    if matches!(state.as_str(), "complete" | "cancelled" | "failed") {
        return Ok(true);
    }
    let doc: Value = serde_json::from_str(&raw)?;
    let members=rows(db,"SELECT n.node,j.id,j.state,j.result,r.state AS recovery_state FROM execution_memberships n JOIN jobs j ON j.id=n.job LEFT JOIN recovery r ON r.job=j.id WHERE n.root=?",&[&root])?;
    let exhausted = members
        .iter()
        .any(|member| member["recovery_state"] == "exhausted")
        && db.query_row(
            "SELECT EXISTS(SELECT 1 FROM research_jobs WHERE id=?)",
            [root],
            |row| row.get::<_, bool>(0),
        )?;
    let terminal = if exhausted && doc["cancel_requested"] != true {
        for member in &members {
            if matches!(
                member["state"].as_str(),
                Some("queued" | "ready" | "blocked")
            ) {
                let id = member["id"].as_str().unwrap();
                let row = jobs::load(db, id)?;
                jobs::transition(db, &row, loom_domain::Event::RequestCancel)?;
                db.execute("UPDATE attempts SET state='cancelled',ended=COALESCE(ended,?) WHERE job=? AND id=? AND state='running'",
                        params![now(),id,row.aggregate.fence.attempt.as_ref().map(|attempt| attempt.as_str())])?;
            }
        }
        if members.iter().any(|m| {
            matches!(
                m["state"].as_str(),
                Some("preparing" | "running" | "processing")
            )
        }) {
            return Ok(true);
        }
        "failed"
    } else if doc["cancel_requested"] == true || members.iter().any(|m| m["state"] == "cancelled") {
        if !members.iter().all(|m| {
            matches!(
                m["state"].as_str(),
                Some("cancelled" | "complete" | "failed" | "interrupted" | "result_failed")
            )
        }) {
            return Ok(true);
        }
        "cancelled"
    } else {
        if doc["plan"]["sealed"] != true || !members.iter().all(|m| m["state"] == "complete") {
            return Ok(true);
        }
        for d in doc["outcome"]["deliverables"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if d["kind"] == "runtime_receipt" {
                if !doc["receipts"][d["alias"].as_str().unwrap()].is_object() {
                    return Ok(true);
                }
            } else {
                let result = members
                    .iter()
                    .find(|m| m["node"] == d["from"]["node"])
                    .and_then(|m| m["result"].as_str())
                    .map(serde_json::from_str::<Value>)
                    .transpose()?
                    .unwrap_or_default();
                if result["artifact_ready"] != true
                    || !result["entities"].as_array().is_some_and(|entities| {
                        entities
                            .iter()
                            .any(|e| e["id"] == d["from"]["entity"] && e["kind"] == d["kind"])
                    })
                {
                    return Ok(true);
                }
            }
        }
        "complete"
    };
    db.execute(
        "UPDATE job_workflows SET state=? WHERE id=?",
        params![terminal, root],
    )?;
    db.execute("DELETE FROM output_edit_claims WHERE root=?", [root])?;
    db.execute(
        "INSERT INTO events(job,target,code,created,attempt) SELECT ?1,?2,?3,?4,NULL WHERE NOT EXISTS(SELECT 1 FROM events WHERE job=?1 AND target=?2 AND code=?3 AND attempt IS NULL)",
        params![root, notify, terminal, now()],
    )?;
    Ok(true)
}

impl Application {
    pub(crate) fn write_plan(
        &mut self,
        actor: &Actor,
        action: &str,
        mut raw: Value,
    ) -> Result<Value> {
        if raw["plan"].get("units").is_none() {
            let mut entries = Vec::new();
            if let Some(mut run) = raw["plan"].get("run").cloned() {
                run["id"] = json!("run");
                entries.push(run);
            }
            raw["plan"] = json!({"units":entries,"sealed":raw["plan"]["sealed"]});
            for promise in raw
                .pointer_mut("/outcome/deliverables")
                .and_then(Value::as_array_mut)
                .into_iter()
                .flatten()
            {
                if promise["kind"] != "runtime_receipt" {
                    let entity = promise
                        .as_object_mut()
                        .unwrap()
                        .remove("entity")
                        .ok_or_else(|| {
                            Error::input("Name the output entity of the reserved Job")
                        })?;
                    promise["from"] = json!({"node":"run","entity":entity});
                }
            }
            let receipt = self.write_accepted_plan(actor, action, raw, false)?;
            return Ok(
                json!({"id":receipt["id"],"revision":receipt["revision"],"sealed":receipt["sealed"],
                "execution_job":receipt["nodes"]["run"],"receipts":receipt["receipts"],"science_digest":receipt["science_digest"]}),
            );
        }
        require(
            !raw["outcome"]["deliverables"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|d| d.get("entity").is_some()),
            "deliverable",
            "Historical plan format requires its original producer reference",
            400,
        )?;
        self.write_accepted_plan(actor, action, raw, false)
    }
    pub(crate) fn write_research_plan(
        &mut self,
        actor: &Actor,
        action: &str,
        mut raw: Value,
    ) -> Result<Value> {
        let creating = action == "submit_research";
        if creating {
            require(
                raw["presentation"]["parent_job"].is_null(),
                "execution_model",
                "Use Unit membership or an exact dependency for related execution",
                400,
            )?;
            let deliverables = raw["outcome"]["deliverables"].as_array_mut().unwrap();
            require(
                deliverables.len() == 1,
                "deliverable",
                "A research Job declares exactly one independent research output",
                400,
            )?;
            let output = &mut deliverables[0];
            output["from"]["node"] = output["from"]
                .as_object_mut()
                .unwrap()
                .remove("unit")
                .unwrap();
        }
        for slot in raw["plan"]["units"].as_array_mut().unwrap() {
            if let Some(params) = slot
                .pointer_mut("/execution/params")
                .and_then(Value::as_object_mut)
            {
                for value in params.values_mut() {
                    require(
                        value.get("node").is_none(),
                        "execution_model",
                        "Use a Unit name for a local output reference",
                        400,
                    )?;
                    if value.as_object().is_some_and(|o| o.len() == 2)
                        && value["unit"].is_string()
                        && value["entity"].is_string()
                    {
                        *value = json!({"node":value["unit"], "entity":value["entity"]});
                    }
                }
            }
        }
        let receipt = self.write_accepted_plan(
            actor,
            if creating { "create" } else { "revise_plan" },
            raw,
            true,
        )?;
        Ok(
            json!({"id":receipt["id"],"kind":"job","revision":receipt["revision"],
            "sealed":receipt["sealed"],"units":receipt["nodes"],"science_digest":receipt["science_digest"]}),
        )
    }
    fn write_accepted_plan(
        &mut self,
        actor: &Actor,
        action: &str,
        raw: Value,
        research: bool,
    ) -> Result<Value> {
        let config = self.config.clone();
        let bulk = self.bulk.clone();
        let tx = self.transaction()?;
        let key = raw["key"].as_str().unwrap();
        let creating = action == "create";
        let prior = if creating {
            tx.query_row(
                "SELECT id FROM job_workflows WHERE submitter=? AND key=?",
                params![actor.id, key],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        } else {
            Some(raw["id"].as_str().unwrap().to_string())
        };
        let root = prior.clone().unwrap_or_else(id);
        let typed: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM research_jobs WHERE id=?)",
            [&root],
            |r| r.get(0),
        )?;
        if prior.is_some() {
            require(
                typed == research,
                "execution_model",
                "Use the original Job plan contract",
                409,
            )?;
        }
        let mut nodes = jobs::workflow_ids(&tx, &root)?.as_object().unwrap().clone();
        let plan = &raw["plan"];
        let mut doc = if prior.is_some() {
            let (original, submitter, state) = tx
                .query_row(
                    "SELECT request,submitter,state FROM job_workflows WHERE id=?",
                    [&root],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| Error::new("job", "Outcome not found", 404))?;
            require(
                submitter == actor.id,
                "forbidden",
                "Only the submitting principal may revise this outcome",
                403,
            )?;
            if let Some((request,response))=tx.query_row("SELECT request,response FROM rust_plan_revisions WHERE root=? AND actor=? AND key=?",params![root,actor.id,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{
                require(serde_json::from_str::<Value>(&request)?==raw,"idempotency_conflict","Plan key already used for another request",409)?;
                return Ok(serde_json::from_str(&response)?);
            }
            require(
                !creating,
                "idempotency_conflict",
                "Submission key already used",
                409,
            )?;
            let doc: Value = serde_json::from_str(&original)?;
            require(
                doc["plan_revision"] == raw["expected_revision"],
                "plan_revision",
                "Plan revision changed or Job has no editable plan",
                409,
            )?;
            require(
                doc["cancel_requested"] != true
                    && !matches!(state.as_str(), "complete" | "cancelled" | "failed"),
                "plan_closed",
                "Outcome is closing or terminal",
                409,
            )?;
            validate_revision(&doc["plan"], plan, &nodes)?;
            require(
                research || units(plan).len() <= units(&doc["plan"]).len().max(1),
                "separate_jobs",
                "Submit additional executions as separate Jobs with jobs.submit and depends_on",
                409,
            )?;
            doc
        } else {
            require(research || units(plan).len() <= 1, "separate_jobs",
                "Submit each independently scheduled execution with jobs.submit and link their Job IDs", 409)?;
            for field in ["owner", "notify"] {
                JobId::new(raw[field].as_str().unwrap())?;
            }
            if raw["owner"] == "system" {
                crate::system_ownership::require_admin(&config, actor)?;
            }
            let mut doc = json!({"key":key,"label":raw["outcome"]["goal"],"owner":raw["owner"],"notify":raw["notify"],"outcome":raw["outcome"],"outputs":{},"stage_receipts":{},"receipts":{}});
            let catalog_deliverable = doc["outcome"]["deliverables"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|d| matches!(d["kind"].as_str(), Some("dataset" | "model" | "experiment")));
            require(
                raw["scenario_ref"].is_object() || (!research && !catalog_deliverable),
                "task_required",
                "Pin a Task revision for a research Job or a Catalog deliverable",
                400,
            )?;
            if research {
                let output = &doc["outcome"]["deliverables"][0];
                if let Some(target) = output.get("revises") {
                    require(
                        matches!(output["kind"].as_str(), Some("dataset" | "model"))
                            && output.get("supplements").is_none(),
                        "output_revision",
                        "Only a Regular Dataset or Policy supports this edit",
                        400,
                    )?;
                    crate::output_edits::authorize(&output["edit_authorization"])?;
                    crate::results::output_revision_base(
                        &tx,
                        target,
                        raw["owner"].as_str().unwrap(),
                        &raw["scenario_ref"],
                        output["kind"].as_str().unwrap(),
                    )?;
                } else {
                    require(
                        output.get("edit_authorization").is_none(),
                        "output_revision",
                        "Edit authorization requires a revision target",
                        400,
                    )?;
                }
                if let Some(target) = output.get("supplements") {
                    require(
                        output["kind"] == "experiment",
                        "supplement_kind",
                        "Only a checkpoint Experiment can be supplemented",
                        400,
                    )?;
                    let previous = crate::experiment_supplements::target(
                        &tx,
                        target,
                        raw["owner"].as_str().unwrap(),
                    )?;
                    require(
                        previous["scenario_ref"].is_null()
                            || previous["scenario_ref"]["id"] == raw["scenario_ref"]["id"],
                        "supplement_task",
                        "Use the Task of the Experiment being supplemented",
                        409,
                    )?;
                }
            }
            if !raw["outcome"]["science"].is_null() {
                doc["science_digest"] =
                    preview_outcome(&tx, &raw["outcome"])?["science_digest"].clone();
            }
            for field in ["presentation", "scenario_ref", "category"] {
                if let Some(v) = raw.get(field) {
                    doc[field] = v.clone();
                }
            }
            if let Some(reference) = doc.get("scenario_ref") {
                crate::scenario::pin(&tx, &serde_json::from_value(reference.clone())?)?;
            }
            tx.execute("INSERT INTO job_workflows(id,submitter,key,request,owner,notify,created,state) VALUES(?,?,?,?,?,?,?,'queued')",params![root,actor.id,key,"{}",raw["owner"].as_str(),raw["notify"].as_str(),now()])?;
            if research {
                enable_job_units(&tx)?;
                tx.execute("INSERT INTO research_jobs(id) VALUES(?)", [&root])?;
            }
            doc
        };
        active(&tx)?;
        let revision = doc["plan_revision"].as_i64().unwrap_or(0) + 1;
        let slots = units(plan);
        let order = if slots.is_empty() {
            Vec::new()
        } else {
            let graph = plan["units"]
                .as_array()
                .unwrap()
                .iter()
                .map(|u| {
                    Ok(PlanNode {
                        id: NodeId::new(u["id"].as_str().unwrap())?,
                        dependencies: u["depends_on"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|v| NodeId::new(v.as_str().unwrap()))
                            .collect::<loom_domain::Result<_>>()?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            loom_domain::order_plan(&graph)?
        };
        let sealed = plan["sealed"] == true;
        let outcome = doc["outcome"].clone();
        let deliverables = outcome["deliverables"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        require(
            !sealed || !slots.is_empty() || !deliverables.is_empty(),
            "plan_pending",
            "A sealed outcome needs executable work or a Runtime receipt",
            409,
        )?;
        let mut aliases = BTreeSet::new();
        for d in deliverables {
            let alias = d["alias"].as_str().unwrap();
            require(
                aliases.insert(alias.to_string()),
                "deliverable",
                "Duplicate deliverable alias",
                400,
            )?;
            if d["kind"] == "runtime_receipt" {
                if sealed && doc["receipts"].get(alias).is_none() {
                    doc["receipts"][alias] = runtime_receipt(&tx, &d["runtime"])?;
                }
            } else {
                require(
                    slots.contains_key(d["from"]["node"].as_str().unwrap()),
                    "deliverable",
                    "Deliverable references unknown execution slot",
                    400,
                )?;
                doc["outputs"][alias] = d["from"].clone();
            }
        }
        let workflow = Workflow {
            key: root.clone(),
            label: doc["label"].as_str().unwrap().into(),
            owner: doc["owner"].as_str().unwrap().into(),
            notify: doc["notify"].as_str().unwrap().into(),
            nodes: Vec::new(),
            outputs: Default::default(),
            presentation: serde_json::from_value(doc["presentation"].clone())?,
            category: doc["category"].as_str().unwrap_or("other").into(),
            scenario_ref: serde_json::from_value(doc["scenario_ref"].clone())?,
        };
        for node in order {
            let name = node.as_str();
            let slot = slots[name];
            let mut stages = BTreeSet::new();
            for gate in slot["requires"].as_array().into_iter().flatten() {
                require(
                    stages.insert(gate["stage"].as_str().unwrap()),
                    "runtime_pin",
                    "Each Stage may require one Runtime",
                    400,
                )?;
            }
            if !slot["execution"].is_object() {
                require(
                    !sealed,
                    "plan_pending",
                    "Sealing requires a binding for every slot",
                    409,
                )?;
                continue;
            }
            let n = Node {
                id: name.into(),
                label: slot["label"].as_str().unwrap().into(),
                depends_on: serde_json::from_value(
                    slot.get("depends_on").cloned().unwrap_or(json!([])),
                )?,
                spec: slot["execution"].clone(),
            };
            if nodes.contains_key(name) {
                continue;
            }
            jobs::validate_pilot(&n.spec)?;
            if research && deliverables.iter().any(|d| d["from"]["node"] == name) {
                require(
                    !matches!(n.spec["execution_mode"].as_str(), Some("pilot" | "smoke")),
                    "deliverable",
                    "Pilot and Smoke execution cannot satisfy a Regular deliverable",
                    400,
                )?;
            }
            require(
                n.depends_on.iter().all(|d| nodes.contains_key(d)),
                "plan_pending",
                "Bind predecessor slots before admitting this execution",
                409,
            )?;
            let mut gates = vec![];
            for g in slot["requires"].as_array().into_iter().flatten() {
                gates.push(
                    json!({"stage":g["stage"],"runtime":runtime_receipt(&tx,&g["runtime"])?}),
                );
            }
            doc["stage_receipts"][name] = json!(gates);
            let phase = research.then(|| slot["phase"].as_str().unwrap_or("run"));
            let child = jobs::admit_node(
                &tx,
                (&config, &bulk),
                actor,
                (&root, phase),
                &workflow,
                &n,
                &nodes,
            )?;
            let frozen = jobs::load(&tx, &child)?;
            check_execution(&doc, name, &frozen.spec)?;
            let cycle:bool=tx.query_row("WITH RECURSIVE deps(id) AS (SELECT value FROM json_each(?,'$.depends_on') UNION SELECT d.value FROM deps JOIN jobs j ON j.id=deps.id JOIN json_each(j.spec,'$.depends_on') d UNION SELECT n.job FROM deps JOIN execution_memberships n ON n.root=deps.id) SELECT EXISTS(SELECT 1 FROM deps WHERE id IN (?,?))",params![encode(&frozen.spec)?,root,child],|r|r.get(0))?;
            require(
                !cycle,
                "plan",
                "Dependencies lead back to the owning outcome",
                409,
            )?;
            if doc["held"] == true {
                tx.execute("UPDATE jobs SET queue_held=1 WHERE id=?", [&child])?;
            }
            nodes.insert(name.into(), json!(child));
        }
        doc["plan_revision"] = json!(revision);
        doc["plan"] = plan.clone();
        tx.execute(
            "UPDATE job_workflows SET request=? WHERE id=?",
            params![encode(&doc)?, root],
        )?;
        finish(&tx, &root)?;
        let response = json!({"id":root,"revision":revision,"sealed":sealed,"nodes":nodes,"receipts":doc["receipts"],"science_digest":doc["science_digest"]});
        tx.execute(
            "INSERT INTO rust_plan_revisions VALUES(?,?,?,?,?,?,?)",
            params![
                root,
                revision,
                actor.id,
                key,
                encode(&raw)?,
                encode(&response)?,
                now()
            ],
        )?;
        tx.commit()?;
        Ok(response)
    }
}
