//! Browsing taxonomy is explicit Task context, independent of data derivation.
use crate::{
    auth::Actor, contracts::Target, error::require, research_metadata, store::*, Application,
    Error, Result,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

pub fn validate_path(path: &Value) -> Result<()> {
    require(
        path.as_array().is_some_and(|p| p.len() <= 12),
        "organization",
        "Use at most 12 subgroup levels",
        400,
    )?;
    for node in path.as_array().unwrap() {
        require(
            node.as_object()
                .is_some_and(|o| o.len() == 2 && o.contains_key("id") && o.contains_key("label"))
                && node["id"].as_str().is_some_and(|s| {
                    !s.is_empty()
                        && s.len() <= 64
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                })
                && node["label"].as_str().is_some_and(|s| {
                    !s.trim().is_empty()
                        && s.chars().count() <= 80
                        && !s.chars().any(char::is_control)
                }),
            "organization",
            "Each subgroup needs a stable local id and a short label",
            400,
        )?;
    }
    Ok(())
}
pub fn validate_declaration(value: &Value) -> Result<()> {
    require(
        value
            .as_object()
            .is_some_and(|o| o.keys().all(|k| ["path", "task_ref"].contains(&k.as_str()))),
        "organization",
        "Use path and optional exact task_ref",
        400,
    )?;
    validate_path(&value["path"])?;
    if let Some(task) = value.get("task_ref") {
        require(
            task.as_object().is_some_and(|o| o.len() == 2)
                && task["id"].as_str().is_some_and(|s| !s.is_empty())
                && task["revision"].as_i64().is_some_and(|r| r > 0),
            "organization_task",
            "Use an exact Task id and revision",
            400,
        )?;
    }
    Ok(())
}

pub(crate) fn validate_task(organization: &Value, task: &Value) -> Result<()> {
    let declared = &organization["task_ref"];
    require(
        task.is_object()
            && (declared.is_null()
                || (declared["id"] == task["id"] && declared["revision"] == task["revision"])),
        "organization_task",
        "Dataset organization must use the accepted Task revision",
        409,
    )
}

fn saved(app: &Application, target: &Target) -> Result<Value> {
    let text: Option<String> = app
        .db
        .query_row(
            "SELECT body FROM rust_annotations WHERE kind='dataset_organization' AND id=?",
            [research_metadata::snapshot_key(target)?],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or(Value::Null))
}
pub fn attach_context(app: &Application, entity: &mut Value) -> Result<()> {
    if !entity["scenario_ref"].is_object() {
        if let Some(job) = entity["job"].as_str() {
            if let Ok(record) = crate::jobs::load(&app.db, job) {
                entity["scenario_ref"] = record.spec["scenario_ref"].clone();
            }
        }
    }
    app.attach_task_context(std::slice::from_mut(entity))
}
fn context(app: &Application, target: &Target) -> Result<Vec<Value>> {
    let i = &target.identity;
    let mut entity = if target.kind == "entity" {
        let text: String = app.db.query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![i["id"].as_str(), i["revision"].as_i64()],
            |r| r.get(0),
        )?;
        let mut e: Value = serde_json::from_str(&text)?;
        e["loom_registry"] = i.clone();
        e
    } else {
        let text:String=app.db.query_row("SELECT body FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",params![i["source"].as_str(),i["kind"].as_str(),i["id"].as_str(),i["revision"].as_str(),i["snapshot"].as_str()],|r|r.get(0))?;
        let b: Value = serde_json::from_str(&text)?;
        let mut e = b
            .get("entity")
            .or_else(|| b.get("original"))
            .cloned()
            .unwrap_or(json!({}));
        e["loom_history"] = i.clone();
        e
    };
    attach_context(app, &mut entity)?;
    Ok(entity["task_refs"].as_array().cloned().unwrap_or_default())
}
pub fn attach(app: &Application, entity: &mut Value) -> Result<()> {
    if entity["kind"] != "dataset" {
        return Ok(());
    }
    let Some(target) = research_metadata::snapshot_target(entity) else {
        return Ok(());
    };
    let sha = crate::dataset_lineage::exact(app, &target)?;
    let old = saved(app, &target)?;
    if old["source_sha256"] == sha && old["target"] == json!(target) {
        if entity["task_refs"].as_array().is_some_and(|a| {
            a.iter().any(|t| {
                t["id"] == old["task_ref"]["id"] && t["revision"] == old["task_ref"]["revision"]
            })
        }) {
            entity["dataset_organization"] = old;
        }
        return Ok(());
    }
    let declared = &entity["metadata"]["research"]["organization"];
    if declared.is_object() && validate_path(&declared["path"]).is_ok() {
        let refs = entity["task_refs"].as_array().cloned().unwrap_or_default();
        let task = if declared["task_ref"].is_object() {
            refs.iter()
                .find(|r| {
                    r["id"] == declared["task_ref"]["id"]
                        && r["revision"] == declared["task_ref"]["revision"]
                })
                .cloned()
        } else if refs.len() == 1 {
            Some(refs[0].clone())
        } else {
            None
        };
        if let Some(task) = task {
            entity["dataset_organization"] = json!({"schema":"loom-dataset-organization/v1","task_ref":task,"path":declared["path"],"authority":"producer"});
        }
    }
    Ok(())
}
impl Application {
    pub fn organize_dataset(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let dry = args["dry_run"].as_bool().unwrap_or(true);
        if !dry {
            let key = args["key"]
                .as_str()
                .ok_or_else(|| Error::input("Request key required"))?;
            if let Some(old) = replay(&self.db, actor, key, "dataset.organization", &args)? {
                return Ok(old);
            }
        }
        let target: Target = crate::contracts::decode(args["target"].clone())?;
        let sha = crate::dataset_lineage::exact(self, &target)?;
        validate_path(&args["path"])?;
        let task = &args["task_ref"];
        require(
            context(self, &target)?
                .iter()
                .any(|r| r["id"] == task["id"] && r["revision"] == task["revision"]),
            "organization_task",
            "Reuse a Task already linked to this exact Dataset",
            409,
        )?;
        // Stable path IDs cannot silently acquire conflicting display labels.
        for row in rows(
            &self.db,
            "SELECT body FROM rust_annotations WHERE kind='dataset_organization'",
            &[],
        )? {
            let old: Value = serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;
            if old["task_ref"]["id"] != task["id"] || old["target"] == json!(target) {
                continue;
            }
            let old_target: Target = crate::contracts::decode(old["target"].clone())?;
            if !crate::dataset_lineage::exact(self, &old_target)
                .is_ok_and(|s| old["source_sha256"] == s)
            {
                continue;
            }
            for (a, b) in old["path"]
                .as_array()
                .into_iter()
                .flatten()
                .zip(args["path"].as_array().unwrap())
            {
                if a["id"] != b["id"] {
                    break;
                }
                require(
                    a["label"] == b["label"],
                    "organization_label",
                    "One subgroup path ID must retain the same label",
                    409,
                )?;
            }
        }
        let previous = saved(self, &target)?;
        let claim = json!({"schema":"loom-dataset-organization/v1","target":target,"source_sha256":sha,"task_ref":task,"path":args["path"]});
        let digest = hash(encode(&json!({"claim":claim,"previous":previous}))?);
        let mut result = json!({"dry_run":dry,"stored":false,"digest":digest,"organization":claim});
        if dry {
            return Ok(result);
        }
        require(
            args["expected_digest"] == digest,
            "source_changed",
            "Source or current organization changed; preview again",
            409,
        )?;
        let mut record = claim;
        record["actor"] = json!(actor.id);
        record["reason"] = args["reason"].clone();
        record["updated_at"] = json!(now());
        record["authority"] = json!("operator");
        let key = research_metadata::snapshot_key(&target)?;
        let tx = self.transaction()?;
        active(&tx)?;
        if !previous.is_null() {
            tx.execute("INSERT OR IGNORE INTO rust_annotations VALUES('dataset_organization_history',?,?,?)",params![hash(encode(&previous)?),encode(&previous)?,now()])?;
        }
        tx.execute("INSERT INTO rust_annotations VALUES('dataset_organization',?,?,?) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body,updated=excluded.updated",params![key,encode(&record)?,now()])?;
        result["stored"] = json!(true);
        receipt(
            &tx,
            actor,
            args["key"].as_str().unwrap(),
            "dataset.organization",
            &args,
            &result,
        )?;
        tx.commit()?;
        Ok(result)
    }
}
