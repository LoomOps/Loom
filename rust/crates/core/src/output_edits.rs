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

    pub(crate) fn attach_policy_checkpoints(&self, record: &mut Value, pin: &Value) -> Result<()> {
        if record["kind"] != "model" || !pin["revision"].as_i64().is_some_and(|n| n > 1) {
            return Ok(());
        }
        let mut checkpoints = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for row in rows(&self.db,
            "SELECT revision,manifest FROM all_entity_revisions WHERE id=? AND revision<=? ORDER BY revision DESC LIMIT 32",
            &[&pin["id"].as_str(),&pin["revision"].as_i64()])? {
            let source: Value = serde_json::from_str(row["manifest"].as_str().unwrap())?;
            for component in source["components"].as_array().into_iter().flatten() {
                let Some(name) = component["ref"].as_str() else { continue };
                let Some(digest) = component["sha256"].as_str() else { continue };
                if ![".safetensors", ".ckpt", ".pth"].iter().any(|suffix| name.ends_with(suffix))
                    || !seen.insert(digest.to_owned()) { continue }
                checkpoints.push(json!({"name":name,"sha256":digest,"revision":row["revision"],"unit":source["job"]}));
                if checkpoints.len() == 64 { break }
            }
            if checkpoints.len() == 64 { break }
        }
        if !checkpoints.is_empty() {
            record["policy_checkpoints"] = json!(checkpoints);
        }
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
        require(
            matches!(kind.as_str(), "dataset" | "model") && record["catalog_class"] == "regular",
            "output_revision",
            "Use a Regular Dataset or Policy",
            409,
        )?;
        if kind == "dataset" {
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
        let (owner, submitter, state): (String, String, String) = tx.query_row(
            "SELECT e.owner,e.submitter,j.state FROM entity_revisions e JOIN jobs j ON j.id=e.job WHERE e.id=? AND e.revision=?",
            params![source_id,source["revision"].as_i64()],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
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
