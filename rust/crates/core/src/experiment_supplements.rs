//! Immutable Experiment revisions assembled from verified evaluation batches.
use crate::{error::require, reporting, store::*, Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn case_report(record: &Value) -> Result<&Value> {
    let mut reports = record["reporting"]["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["role"] == "evaluation_cases" && r["state"] == "recorded");
    let report = reports.next().filter(|_| reports.next().is_none());
    report
        .ok_or_else(|| Error::conflict("Supplement requires one validated evaluation case report"))
}

pub(crate) fn target(db: &Connection, pin: &Value, owner: &str) -> Result<Value> {
    let (id, revision) = (pin["id"].as_str(), pin["revision"].as_i64());
    let mut candidates = rows(
        db,
        "SELECT e.revision,e.manifest,COALESCE(a.owner,e.owner) owner,
         COALESCE(r.catalog_class,json_extract(e.manifest,'$.catalog_class')) catalog_class
         FROM all_entity_revisions e
         LEFT JOIN item_owners a ON a.kind='entity' AND a.id=e.id
         LEFT JOIN reviewed_outputs r ON r.source='entity' AND r.id=e.id AND r.kind=e.kind
           AND r.revision=CAST(e.revision AS TEXT) AND r.snapshot=''
         WHERE e.id=? AND e.kind='experiment'
         ORDER BY e.revision DESC LIMIT 1",
        &[&id],
    )?;
    let row = candidates
        .pop()
        .ok_or_else(|| Error::new("supplement_target", "Experiment not found", 404))?;
    require(
        row["owner"] == owner && owner != "system",
        "forbidden",
        "A supplement must use the target Experiment's owner",
        403,
    )?;
    require(
        row["revision"].as_i64() == revision && row["catalog_class"] == "regular",
        "supplement_target",
        "Use the current revision of a Regular Experiment",
        409,
    )?;
    let record: Value = serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;
    case_report(&record)?;
    Ok(record)
}

fn input_pins(record: &Value) -> BTreeSet<String> {
    record["reporting"]["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| json!([r["input"], r["id"], r["revision"], r["member"]]).to_string())
        .collect()
}

fn recorded_split(record: &Value) -> Option<&str> {
    record
        .pointer("/metadata/research/evaluation/split")
        .and_then(Value::as_str)
        .and_then(|v| v.split_whitespace().next())
        .filter(|v| matches!(*v, "development" | "validation" | "test"))
}

fn merge_cases(mut previous: Value, record: &Value, batch: &Value) -> Result<Value> {
    require(
        previous["protocol_ref"].is_object()
            && previous["protocol_ref"] == batch["protocol_ref"]
            && previous["scope"] == batch["scope"],
        "supplement_protocol",
        "Evaluation protocol and scope differ",
        409,
    )?;
    let old_selection = &previous["checkpoint_selection"];
    let new_selection = &batch["checkpoint_selection"];
    let selection = if old_selection.is_object() {
        require(
            new_selection.is_null() || new_selection == old_selection,
            "supplement_selection",
            "Checkpoint selection differs from the accepted Experiment",
            409,
        )?;
        old_selection.clone()
    } else {
        require(
            new_selection.is_object() && new_selection["split"].as_str() == recorded_split(record),
            "supplement_selection",
            "A legacy Experiment needs a recorded split and a compatible selection rule",
            409,
        )?;
        new_selection.clone()
    };
    let unique = |data: &Value| -> Result<BTreeSet<u64>> {
        data["cases"]
            .as_array()
            .ok_or_else(|| Error::conflict("Evaluation report has no cases"))?
            .iter()
            .map(|r| {
                r["factors"]["update"]
                    .as_u64()
                    .ok_or_else(|| Error::conflict("Case has no checkpoint step"))
            })
            .collect()
    };
    require(
        unique(&previous)?.is_disjoint(&unique(batch)?),
        "supplement_overlap",
        "Supplement evaluates a step already recorded in this Experiment",
        409,
    )?;
    let batch_cases = batch["cases"].as_array().unwrap();
    let cases = previous["cases"].as_array_mut().unwrap();
    require(
        cases.len() + batch_cases.len() <= 20_000,
        "supplement_size",
        "Evaluation series exceeds the supported case limit",
        409,
    )?;
    cases.extend(batch_cases.iter().cloned());
    let distinct: BTreeSet<_> = cases
        .iter()
        .map(|r| (r["id"].as_str(), r["seed"].as_i64()))
        .collect();
    require(
        distinct.len() == cases.len(),
        "supplement_cases",
        "Case IDs must be unique across checkpoint batches",
        409,
    )?;
    cases.sort_by_key(|r| (r["factors"]["update"].as_u64(), r["seed"].as_i64()));
    for key in [
        "planned",
        "executed",
        "valid",
        "successes",
        "infra",
        "invalid",
        "unexecuted",
    ] {
        previous["counts"][key] = json!(
            previous["counts"][key].as_u64().unwrap_or(0)
                + batch["counts"][key].as_u64().unwrap_or(0)
        );
    }
    previous["checkpoint_selection"] = selection;
    let sweep = reporting::checkpoint_evaluation(&previous, false)
        .filter(|v| v["comparable"] == true)
        .ok_or_else(|| Error::conflict("Checkpoint batches must use distinct digests and the same complete valid seed/reset cohort"))?;
    require(
        sweep["rule"].is_string(),
        "supplement_selection",
        "A checkpoint selection rule is required",
        409,
    )?;
    Ok(previous)
}

pub(crate) fn is_series_report(entity: &Value, report: &Value) -> bool {
    entity["supplement"]["base"].is_object() && report["role"] == "evaluation_cases"
}

impl Application {
    pub(crate) fn attach_evaluation_group(&self, record: &mut Value) -> Result<()> {
        let kind = match record["kind"].as_str() {
            Some("experiment") => "evaluation_group_experiment",
            Some("model") => "evaluation_group_policy",
            _ => return Ok(()),
        };
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM rust_annotations WHERE kind=? AND id=?",
                params![kind, record["id"].as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(body) = body {
            record["evaluation_group"] = serde_json::from_str(&body)?;
        }
        Ok(())
    }

    pub(crate) fn group_evaluations(
        &mut self,
        actor: &crate::auth::Actor,
        args: Value,
    ) -> Result<Value> {
        let key = args["key"].as_str().unwrap();
        if let Some(value) = replay(&self.db, actor, key, "results.group_evaluations", &args)? {
            return Ok(value);
        }
        let pins = args["experiments"].as_array().unwrap();
        let mut basis = None;
        let mut experiment_ids = BTreeSet::new();
        let mut policy_refs = BTreeMap::new();
        let mut owners = BTreeSet::new();
        for pin in pins {
            let id = pin["id"].as_str().unwrap();
            require(
                experiment_ids.insert(id),
                "evaluation_group",
                "Each Experiment may appear once",
                400,
            )?;
            let owner:String=self.db.query_row(
                "SELECT COALESCE(a.owner,e.owner) FROM all_entity_revisions e LEFT JOIN item_owners a ON a.kind='entity' AND a.id=e.id WHERE e.id=? AND e.revision=?",
                params![id,pin["revision"].as_i64()],|r|r.get(0),
            )?;
            if owner != actor.id && !actor.owners.contains(&owner) {
                crate::system_ownership::require_admin(&self.config, actor)?;
            }
            let record = target(&self.db, pin, &owner)?;
            owners.insert(owner);
            let payload = self.evaluation_series_payload(&record)?;
            let data = &payload["data"];
            require(
                reporting::checkpoint_evaluation(data, false)
                    .is_some_and(|s| s["comparable"] == true),
                "evaluation_group",
                "Each evaluation needs a verified, complete matched reset cohort",
                409,
            )?;
            let resets: BTreeSet<_> = data["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| json!([r["seed"], r["factors"]["reset_state_sha256"]]).to_string())
                .collect();
            let mut inputs = BTreeSet::new();
            let mut policies = vec![];
            for input in record["reporting"]["inputs"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let kind: Option<String> = self
                    .db
                    .query_row(
                        "SELECT kind FROM all_entity_revisions WHERE id=? AND revision=?",
                        params![input["id"].as_str(), input["revision"].as_i64()],
                        |r| r.get(0),
                    )
                    .optional()?;
                match kind.as_deref() {
                    Some("model") => {
                        policies.push(json!({"id":input["id"],"revision":input["revision"]}))
                    }
                    _ => {
                        inputs.insert(
                            json!([
                                input["input"],
                                input["id"],
                                input["revision"],
                                input["member"]
                            ])
                            .to_string(),
                        );
                    }
                }
            }
            require(
                policies.len() == 1 && !inputs.is_empty(),
                "evaluation_group",
                "Each member needs one exact Policy and matching evaluation inputs",
                409,
            )?;
            let policy = policies.pop().unwrap();
            policy_refs.insert(policy["id"].as_str().unwrap().to_owned(), policy);
            let signature = json!([data["protocol_ref"], data["scope"], inputs, resets]);
            require(
                data["protocol_ref"].is_object() && basis.as_ref().is_none_or(|v| v == &signature),
                "evaluation_group",
                "Protocol, scope, evaluation inputs and reset cohort must match",
                409,
            )?;
            basis = Some(signature);
        }
        let group = json!({"id":id(),"label":args["label"],"experiments":pins,"policies":policy_refs.values().collect::<Vec<_>>()});
        let body = encode(&group)?;
        let tx = self.transaction()?;
        if let Some(value) = replay(&tx, actor, key, "results.group_evaluations", &args)? {
            return Ok(value);
        }
        require(
            owners.len() == 1,
            "evaluation_group",
            "Experiment members must share an owner",
            403,
        )?;
        let owner = owners.first().unwrap();
        for pin in pins {
            target(&tx, pin, owner)?;
        }
        for (kind, ids) in [
            ("evaluation_group_experiment", experiment_ids),
            (
                "evaluation_group_policy",
                policy_refs.keys().map(String::as_str).collect(),
            ),
        ] {
            for id in ids {
                let inserted = tx.execute(
                    "INSERT OR IGNORE INTO rust_annotations VALUES(?,?,?,?)",
                    params![kind, id, body, now()],
                )?;
                require(
                    inserted == 1,
                    "evaluation_group",
                    "An item already has a comparison group",
                    409,
                )?;
            }
        }
        tx.execute(
            "UPDATE rust_projection_version SET version=version+1 WHERE id=1",
            [],
        )?;
        receipt(&tx, actor, key, "results.group_evaluations", &args, &group)?;
        tx.commit()?;
        Ok(group)
    }
    pub(crate) fn evaluation_series_payload(&self, latest: &Value) -> Result<Value> {
        let id = latest["id"]
            .as_str()
            .ok_or_else(|| Error::input("Experiment ID missing"))?;
        let mut records = vec![latest.clone()];
        while let Some(base) = records.last().and_then(|r| r.pointer("/supplement/base")) {
            let rev = base["revision"]
                .as_u64()
                .filter(|v| Some(*v + 1) == records.last().unwrap()["revision"].as_u64());
            require(
                records.len() < 64 && base["id"] == id && rev.is_some(),
                "supplement_chain",
                "Invalid Experiment revision chain",
                409,
            )?;
            let body: String = self.db.query_row(
                "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
                params![id, rev.map(|n| n as i64)],
                |r| r.get(0),
            )?;
            records.push(serde_json::from_str(&body)?);
        }
        let mut records = records.iter().rev();
        let first = records.next().unwrap();
        let read = |r: &Value| {
            self.read_bound_report(id, r["revision"].as_u64().unwrap(), case_report(r)?)
        };
        let mut payload = read(first)?;
        let mut previous = first;
        for record in records {
            let mut next = read(record)?;
            next["data"] = merge_cases(payload["data"].take(), previous, &next["data"])?;
            payload = next;
            previous = record;
        }
        Ok(payload)
    }

    pub(crate) fn prepare_experiment_supplement(
        &self,
        base: &Value,
        owner: &str,
        batch: &mut Value,
        folder: &Path,
        proofs: &[crate::storage::VerifiedFile],
    ) -> Result<()> {
        let original = target(&self.db, base, owner)?;
        let inputs = input_pins(batch);
        require(
            !inputs.is_empty() && input_pins(&original) == inputs,
            "supplement_inputs",
            "Freeze the same exact Policy, Dataset and evaluation inputs",
            409,
        )?;
        let report = case_report(batch)?;
        let path = crate::storage::relative(folder, report["component"].as_str().unwrap_or(""))?;
        let proof = proofs
            .iter()
            .find(|p| p.path == path && report["sha256"] == p.sha256)
            .ok_or_else(|| Error::conflict("Verifiable evaluation case report required"))?;
        let payload = reporting::read_json(&path, proof)?;
        let old = self.evaluation_series_payload(&original)?;
        let cases = merge_cases(old["data"].clone(), &original, &payload["data"])?;
        batch["id"] = base["id"].clone();
        batch["revision"] = json!(base["revision"].as_u64().unwrap() + 1);
        batch["supplement"] = json!({"base":base,"batch_evaluation":batch.pointer("/metadata/research/evaluation"),"batch_metrics":batch["metrics"]});
        if let Some(label) = original.get("label") {
            batch["label"] = label.clone();
        }
        batch["metrics"] = json!([]);
        batch["reporting"]["primary_outcome"] = Value::Null;
        batch["reporting"]["checkpoint_sweep"] =
            reporting::checkpoint_evaluation(&cases, false).unwrap();
        batch["reporting"]["evaluation_summary"]["counts"] = cases["counts"].clone();
        let mut failure = std::collections::BTreeMap::new();
        for row in cases["cases"].as_array().unwrap() {
            if row["valid"] == true && row["outcome"] != "success" {
                if let Some(stage) = row["failure_stage"].as_str() {
                    *failure.entry(stage).or_insert(0usize) += 1;
                }
            }
        }
        batch["reporting"]["evaluation_summary"]["recorded_failure_subtasks"] = json!(failure);
        if let Some(evaluation) = batch
            .pointer_mut("/metadata/research/evaluation")
            .and_then(Value::as_object_mut)
        {
            for (key, total) in [
                ("planned", "planned"),
                ("valid", "valid"),
                ("successes", "successes"),
                ("infra", "infra"),
                ("pending", "unexecuted"),
            ] {
                if evaluation.contains_key(key) {
                    evaluation.insert(key.into(), cases["counts"][total].clone());
                }
            }
        }
        for r in batch["reporting"]["reports"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|r| r["role"] == "evaluation_cases")
        {
            for f in r["fields"].as_array_mut().into_iter().flatten() {
                if let Some(value) = cases.pointer(f["pointer"].as_str().unwrap_or("")) {
                    f["value"] = match value.as_array() {
                        Some(rows) => json!({"rows":rows.len()}),
                        None => value.clone(),
                    };
                }
            }
        }
        batch["relations"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind":"supplements","id":base["id"],"revision":base["revision"]}));
        Ok(())
    }
}
