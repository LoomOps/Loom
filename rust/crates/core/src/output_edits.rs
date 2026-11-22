use crate::{auth::Actor, error::require, storage, store::*, Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

pub(crate) fn authorize(value: &Value) -> Result<()> {
    require(
        matches!(value["basis"].as_str(), Some("explicit_request" | "confirmed"))
            && value["evidence"].as_str().is_some_and(|evidence| !evidence.trim().is_empty()),
        "edit_confirmation_required",
        "Ask the user whether to revise the specific existing output. Record their request or confirmation.",
        409,
    )
}

fn intent(db: &Connection, unit: &str) -> Result<Option<(String, Value)>> {
    db.query_row(
        "SELECT n.root,json_extract(w.request,'$.outcome.deliverables[0].revises')
         FROM execution_memberships n JOIN job_workflows w ON w.id=n.root
         WHERE n.job=? AND json_type(w.request,'$.outcome.deliverables[0].edit_authorization')='object'",
        [unit],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .optional()?
    .map(|(root, base)| Ok((root, serde_json::from_str(&base)?)))
    .transpose()
}

pub(crate) fn wait_reason(db: &Connection, unit: &str) -> Result<Option<&'static str>> {
    let Some((root, base)) = intent(db, unit)? else {
        return Ok(None);
    };
    let latest: Option<i64> = db.query_row(
        "SELECT max(revision) FROM all_entity_revisions WHERE id=?",
        [base["id"].as_str()],
        |r| r.get(0),
    )?;
    let owner: Option<String> = db
        .query_row(
            "SELECT root FROM output_edit_claims WHERE target=?",
            [base["id"].as_str()],
            |r| r.get(0),
        )
        .optional()?;
    if owner.as_ref() == Some(&root) {
        return Ok(None);
    }
    Ok(if latest != base["revision"].as_i64() {
        Some("output_edit_base_changed")
    } else if owner.is_some() {
        Some("output_edit_reserved")
    } else {
        None
    })
}

pub(crate) fn claim(db: &Connection, unit: &str) -> Result<bool> {
    let Some((root, base)) = intent(db, unit)? else {
        return Ok(true);
    };
    if wait_reason(db, unit)?.is_some() {
        return Ok(false);
    }
    db.execute(
        "INSERT INTO output_edit_claims VALUES(?,?,?,?) ON CONFLICT(target) DO NOTHING",
        params![base["id"].as_str(), root, base["revision"].as_i64(), now()],
    )?;
    verify_claim(db, unit, &base)?;
    Ok(true)
}

pub(crate) fn verify_claim(db: &Connection, unit: &str, base: &Value) -> Result<()> {
    let Some((root, accepted)) = intent(db, unit)? else {
        return Ok(());
    };
    let valid: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM output_edit_claims WHERE target=? AND root=? AND base=?)",
        params![base["id"].as_str(), root, base["revision"].as_i64()],
        |r| r.get(0),
    )?;
    require(
        valid && accepted["id"] == base["id"] && accepted["revision"] == base["revision"],
        "output_edit_claim",
        "Job no longer owns this output edit",
        409,
    )
}

pub(crate) fn require_source(record: &Value, base: &Value) -> Result<()> {
    require(
        record["relations"].as_array().is_some_and(|relations| {
            relations.iter().any(|r| {
                r["kind"] == "consumes"
                    && r["id"] == base["id"]
                    && r["revision"] == base["revision"]
            })
        }),
        "output_edit_source",
        "The producing Unit must consume the exact base output",
        409,
    )
}

fn manifest(db: &Connection, pin: &Value) -> Result<Value> {
    let body = db
        .query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![pin["id"].as_str(), pin["revision"].as_i64()],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| Error::new("output_revision", "Exact output not found", 404))?;
    Ok(serde_json::from_str(&body)?)
}

fn membership_report(record: &Value) -> Result<&Value> {
    record["reporting"]["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|report| report["role"] == "dataset_statistics" && report["state"] == "recorded")
        .ok_or_else(|| Error::conflict("A verified Dataset membership report is required"))
}

fn members(body: &Value) -> Result<BTreeMap<&str, &str>> {
    let rows = body["data"]["members"]
        .as_array()
        .ok_or_else(|| Error::conflict("Registered Dataset membership is missing"))?;
    let mut seen = BTreeMap::new();
    for row in rows {
        let id = row["id"]
            .as_str()
            .ok_or_else(|| Error::conflict("Missing Dataset member ID"))?;
        let split = row["split"]
            .as_str()
            .ok_or_else(|| Error::conflict("Missing Dataset member split"))?;
        require(
            seen.insert(id, split).is_none(),
            "output_edit_membership",
            "Duplicate Dataset member",
            409,
        )?;
    }
    Ok(seen)
}

fn verify_append(base: &Value, next: &Value) -> Result<()> {
    let old = members(base)?;
    let new = members(next)?;
    require(!old.is_empty() && new.len() > old.len() && old.iter().all(|(id,split)| new.get(id) == Some(split)),
        "output_edit_membership", "Retain existing member IDs and splits. Publish a separate Dataset to repartition membership.", 409)
}

fn require_unused_dataset(db: &Connection, base: &Value, source: &Value, producer: &str) -> Result<()> {
    // Read accepted inputs and source records in the publication transaction. The
    // replacement producer necessarily consumes its base, so exclude that Unit
    // and its published candidate, while still checking its other Units.
    let used: bool = db.query_row(
        "WITH documents(body) AS (
             SELECT manifest FROM all_entity_revisions
             WHERE id <> ?1 AND NOT (id = ?3 AND revision = ?4)
             UNION ALL SELECT spec FROM jobs WHERE id <> ?5
                 AND (state <> 'cancelled' OR attempt IS NOT NULL)
             UNION ALL SELECT request FROM job_workflows w WHERE state <> 'cancelled'
                 AND NOT EXISTS(SELECT 1 FROM execution_memberships m WHERE m.root=w.id AND m.job=?5)
             UNION ALL SELECT json_extract(body,'$.parent') FROM rust_annotations WHERE kind='dataset_lineage'
             UNION ALL SELECT json_extract(body,'$.entity.relations') FROM history_records
                 WHERE kind IN ('dataset','model','experiment','run')
             UNION ALL SELECT json_extract(body,'$.original.relations') FROM history_records
                 WHERE kind IN ('dataset','model','experiment','run')
         )
         SELECT EXISTS(SELECT 1 FROM documents d JOIN json_tree(d.body) node
             WHERE CASE WHEN node.type='object' THEN json_extract(node.value,'$.id')=?1
               AND CAST(json_extract(node.value,'$.revision') AS TEXT)=CAST(?2 AS TEXT) ELSE 0 END)",
        params![base["id"].as_str(), base["revision"].as_i64(), source["id"].as_str(), source["revision"].as_i64(), producer],
        |r| r.get(0),
    )?;
    require(!used, "dataset_in_use", "This Dataset revision has references. Keep the completed candidate as a separate Dataset.", 409)
}

impl Application {
    fn registered_membership(&self, pin: &Value, record: &Value) -> Result<Value> {
        self.read_bound_report(
            pin["id"].as_str().unwrap(),
            pin["revision"].as_u64().unwrap(),
            membership_report(record)?,
        )
    }

    pub(crate) fn verify_dataset_append(
        &self,
        base: &Value,
        next: &Value,
        folder: &Path,
        proofs: &[storage::VerifiedFile],
    ) -> Result<()> {
        let old = manifest(&self.db, base)?;
        let previous = self.registered_membership(base, &old)?;
        let report = membership_report(next)?;
        let path = storage::relative(folder, report["component"].as_str().unwrap())?;
        let proof = proofs
            .iter()
            .find(|proof| proof.path == path && report["sha256"] == proof.sha256)
            .ok_or_else(|| {
                Error::conflict("Membership report is outside the verified publication")
            })?;
        verify_append(&previous, &crate::reporting::read_json(&path, proof)?)
    }

    pub(crate) fn attach_output_edit(&self, record: &mut Value, id: &str) -> Result<()> {
        let claim = rows(
            &self.db,
            "SELECT c.root AS job_id,c.base AS revision,c.acquired,w.state AS job_state
             FROM output_edit_claims c JOIN job_workflows w ON w.id=c.root WHERE c.target=?",
            &[&id],
        )?;
        if let Some(claim) = claim.into_iter().next() {
            record["output_edit"] = claim;
        }
        Ok(())
    }

    pub(crate) fn attach_policy_checkpoints(
        &self,
        record: &mut Value,
        id: &str,
        through: Option<i64>,
    ) -> Result<()> {
        if record["kind"] != "model" {
            return Ok(());
        }
        let mut checkpoints = Vec::new();
        for row in rows(
            &self.db,
            "SELECT revision,manifest FROM all_entity_revisions WHERE id=? AND (?2 IS NULL OR revision<=?2) ORDER BY revision",
            &[&id, &through],
        )? {
            let source: Value = serde_json::from_str(row["manifest"].as_str().unwrap())?;
            let selected = source
                .pointer("/metadata/research/model/selected_step")
                .and_then(Value::as_u64);
            let mut seen = std::collections::BTreeSet::new();
            for (index, component) in source["components"].as_array().into_iter().flatten().enumerate() {
                let (Some(name), Some(digest)) = (component["ref"].as_str(), component["sha256"].as_str()) else {
                    continue;
                };
                let Some((stem, extension)) = name.rsplit('/').next().and_then(|file| file.rsplit_once('.')) else {
                    continue;
                };
                if !matches!(extension, "safetensors" | "ckpt" | "pth") || !seen.insert(digest) {
                    continue;
                }
                let named_step = stem.rsplit_once(['-', '_']).and_then(|(prefix, number)| {
                    if matches!(prefix, "model" | "policy" | "checkpoint" | "ckpt" | "step") {
                        number.parse::<u64>().ok()
                    } else {
                        None
                    }
                });
                let step = if index == 0 { selected.or(named_step) } else { named_step };
                checkpoints.push(json!({"name":name,"sha256":digest,"step":step,"revision":row["revision"],"unit":source["job"]}));
            }
        }
        if !checkpoints.is_empty() {
            checkpoints.sort_by_key(|c| (c["step"].as_u64().unwrap_or(u64::MAX), c["revision"].as_u64()));
            record["policy_checkpoints"] = json!(checkpoints);
        }
        Ok(())
    }

    pub(crate) fn attach_policy_training_revisions(&self, record: &mut Value) -> Result<()> {
        let Some(id) = record["loom_registry"]["id"].as_str() else {
            return Ok(());
        };
        if record["kind"] != "model" {
            return Ok(());
        }
        let mut revisions = Vec::new();
        for row in rows(
            &self.db,
            "SELECT revision,json_extract(manifest,'$.reporting.reports') AS reports FROM all_entity_revisions WHERE id=? ORDER BY revision",
            &[&id],
        )? {
            let Some(reports) = row["reports"].as_str() else { continue };
            let reports: Vec<Value> = serde_json::from_str(reports)?;
            let reports: Vec<Value> = reports
                .into_iter()
                .filter(|report| matches!(report["role"].as_str(), Some("learning_series" | "action_metrics")))
                .collect();
            if !reports.is_empty() {
                revisions.push(json!({"revision":row["revision"],"reports":reports}));
            }
        }
        let spans_revisions = revisions.len() > 1
            || revisions.first().is_some_and(|history| history["revision"] != record["loom_registry"]["revision"]);
        record["policy_training_revisions"] = if spans_revisions { json!(revisions) } else { json!([]) };
        Ok(())
    }

    pub(crate) fn adopt_output_revision(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        authorize(&args["authorization"])?;
        let base = &args["base"];
        let source = &args["source"];
        let (base_id, source_id) = (base["id"].as_str().unwrap(), source["id"].as_str().unwrap());
        require(
            base_id != source_id,
            "output_revision",
            "Use separate existing record IDs",
            400,
        )?;
        let key = args["key"].as_str().unwrap();
        let operation = "results.adopt_output_revision";
        if let Some(result) = replay(&self.db, actor, key, operation, &args)? {
            return Ok(result);
        }
        let mut record = manifest(&self.db, source)?;
        require_source(&record, base)?;
        let kind = record["kind"].as_str().unwrap_or_default().to_owned();
        let replace_unused = args["mode"] == "replace_unused";
        require(
            matches!(kind.as_str(), "dataset" | "model") && record["catalog_class"] == "regular",
            "output_revision",
            "Use a Regular Dataset or Policy",
            409,
        )?;
        if replace_unused {
            require(kind == "dataset", "output_revision", "Only a Regular Dataset supports unused replacement", 400)?;
            let report = self.registered_membership(source, &record)?;
            members(&report)?;
        } else if kind == "dataset" {
            let previous = manifest(&self.db, base)?;
            verify_append(
                &self.registered_membership(base, &previous)?,
                &self.registered_membership(source, &record)?,
            )?;
        }
        let tx = self.transaction()?;
        active(&tx)?;
        if let Some(result) = replay(&tx, actor, key, operation, &args)? {
            return Ok(result);
        }
        let (owner, submitter, state, producer): (String, String, String, String) = tx.query_row(
            "SELECT e.owner,e.submitter,j.state,j.id FROM entity_revisions e JOIN jobs j ON j.id=e.job WHERE e.id=? AND e.revision=?",
            params![source_id,source["revision"].as_i64()],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
        )?;
        require(
            state == "complete" && actor.id == submitter,
            "forbidden",
            "Use the submitting principal of the completed output",
            403,
        )?;
        let revision = crate::results::output_revision_base(
            &tx,
            base,
            &owner,
            &record["scenario_ref"],
            &kind,
        )?;
        require(
            !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM output_edit_claims WHERE target IN (?,?))",
                params![base_id, source_id],
                |r| r.get::<_, bool>(0),
            )?,
            "output_edit_reserved",
            "An output is being revised by another Job",
            409,
        )?;
        let latest: i64 = tx.query_row(
            "SELECT max(revision) FROM all_entity_revisions WHERE id=?",
            [source_id],
            |r| r.get(0),
        )?;
        require(
            Some(latest) == source["revision"].as_i64(),
            "output_revision",
            "Use the current revision of the completed continuation",
            409,
        )?;
        if replace_unused {
            let base_key = crate::results::output_disposition_key(&tx, base, &kind)?;
            let discarded: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM rust_annotations WHERE kind='output_disposition' AND id=? AND json_extract(body,'$.state')='discarded')",
                [base_key], |row| row.get(0),
            )?;
            require(!discarded, "output_revision", "Use a Current Dataset as the replacement target", 409)?;
            require_unused_dataset(&tx, base, source, &producer)?;
            record["revision_mode"] = json!("replace_unused");
        }
        let source_type = if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM asset_revisions WHERE id=? AND revision=?)",
            params![base_id, base["revision"].as_i64()],
            |r| r.get::<_, bool>(0),
        )? {
            "asset_revisions"
        } else {
            "entity_revisions"
        };
        crate::items::adopt(&tx, source_type, base)?;
        let label = manifest(&tx, base)?["label"].clone();
        if label.is_string() {
            record["label"] = label;
        }
        record["id"] = json!(base_id);
        record["revision"] = json!(revision);
        record["relations"].as_array_mut().unwrap().extend([
            json!({"kind":"revises","id":base_id,"revision":base["revision"]}),
            json!({"kind":"adopted_from","id":source_id,"revision":source["revision"]}),
        ]);
        record["revision_authorization"] = args["authorization"].clone();
        // Keep the original Unit's output set and exact aliases intact.
        tx.execute(
            "INSERT INTO asset_revisions VALUES(?,?,NULL,?,?,?,?)",
            params![base_id, revision, kind, owner, submitter, encode(&record)?],
        )?;
        if kind == "dataset" {
            crate::dataset_organization::retain_presentation(&tx, base, &record, &record["scenario_ref"])?;
        }
        let copied = tx.execute("INSERT INTO asset_locations SELECT ?,?,component,machine_id,storage_id,ref,status FROM entity_locations WHERE id=? AND revision=?",
            params![base_id,revision,source_id,source["revision"].as_i64()])?;
        require(
            copied > 0,
            "output_edit_source",
            "Completed continuation has no registered locations",
            409,
        )?;
        let disposition_key = crate::results::output_disposition_key(&tx, source, &kind)?;
        let disposition = json!({"state":"discarded","reason":"Consolidated into the original output's next revision",
            "actor":actor.id,"superseded_by":{"id":base_id,"revision":revision}});
        tx.execute("INSERT INTO rust_annotations VALUES('output_disposition',?,?,?) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body,updated=excluded.updated",
            params![disposition_key,encode(&disposition)?,now()])?;
        tx.execute(
            "UPDATE rust_projection_version SET version=version+1 WHERE id=1",
            [],
        )?;
        tx.execute("UPDATE reviewed_clock SET version=version+1 WHERE id=1", [])?;
        crate::items::adopt(
            &tx,
            "asset_revisions",
            &json!({"id":base_id,"revision":revision}),
        )?;
        let result = json!({"id":base_id,"revision":revision,"source":source,"source_disposition":"discarded"});
        receipt(&tx, actor, key, operation, &args, &result)?;
        tx.commit()?;
        Ok(result)
    }
}
