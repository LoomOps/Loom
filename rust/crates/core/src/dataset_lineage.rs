//! Exact, additive Dataset derivation claims. Original manifests remain immutable.
use crate::{
    auth::Actor, contracts::Target, error::require, research_metadata, store::*, Application,
    Error, Result,
};
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub use loom_contracts::TRANSFORMS;

pub fn validate_transform(value: &Value) -> Result<()> {
    require(
        value.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                ["kind", "details", "tool_ref", "config_ref", "member_count"].contains(&k.as_str())
            })
        }) && value["kind"]
            .as_str()
            .is_some_and(|s| TRANSFORMS.contains(&s))
            && value["details"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 2000)
            && value
                .get("member_count")
                .is_none_or(|n| n.as_u64().is_some())
            && serde_json::to_vec(value)?.len() <= 8192,
        "lineage",
        "A typed Dataset transform and its recorded meaning are required",
        400,
    )
}

pub(crate) fn exact(app: &Application, target: &Target) -> Result<String> {
    let fields: &[&str] = match target.kind.as_str() {
        "entity" => &["id", "revision"],
        "historical" => &["source", "kind", "id", "revision", "snapshot"],
        _ => {
            return Err(Error::input(
                "Exact entity or historical Dataset selector required",
            ))
        }
    };
    require(
        target
            .identity
            .as_object()
            .is_some_and(|o| o.len() == fields.len() && fields.iter().all(|k| o.contains_key(*k)))
            && target.identity["id"]
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.len() <= 2048)
            && if target.kind == "entity" {
                target.identity["revision"].as_i64().is_some_and(|n| n > 0)
            } else {
                fields.iter().all(|k| {
                    target.identity[*k]
                        .as_str()
                        .is_some_and(|s| !s.is_empty() && s.len() <= 2048)
                })
            },
        "lineage_target",
        "Use one canonical exact Dataset selector without extra fields",
        400,
    )?;
    let (sha, kind) = research_metadata::source_identity(app, target)?;
    require(
        kind == "dataset",
        "lineage_kind",
        "Both lineage endpoints must be exact Datasets",
        400,
    )?;
    Ok(sha)
}

fn stored(app: &Application) -> Result<Vec<Value>> {
    rows(
        &app.db,
        "SELECT body FROM rust_annotations WHERE kind='dataset_lineage' ORDER BY id",
        &[],
    )?
    .iter()
    .map(|r| Ok(serde_json::from_str(r["body"].as_str().unwrap_or("{}"))?))
    .collect()
}

fn parents(app: &Application, target: &Target, annotations: &[Value]) -> Result<Vec<Value>> {
    let sha = exact(app, target)?;
    let mut result = vec![];
    for annotation in annotations
        .iter()
        .filter(|v| v["target"] == json!(target) && v["source_sha256"] == sha)
    {
        let parent: Target = crate::contracts::decode(annotation["parent"].clone())?;
        if exact(app, &parent).is_ok_and(|s| annotation["parent_sha256"] == s) {
            result.push(annotation.clone());
        }
    }
    if target.kind == "entity" {
        let text: String = app.db.query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![
                target.identity["id"].as_str(),
                target.identity["revision"].as_i64()
            ],
            |r| r.get(0),
        )?;
        let manifest: Value = serde_json::from_str(&text)?;
        for relation in manifest["relations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["kind"] == "derived_from")
        {
            result.push(json!({"target":target,"parent":{"kind":"entity","identity":{"id":relation["id"],"revision":relation["revision"]}},"transformation":relation["transformation"],"authority":"producer","input":relation["input"],"primary":relation["primary"]}));
        }
    }
    Ok(result)
}

pub(crate) fn declared(app: &Application, target: &Target) -> Result<Vec<Value>> {
    if research_metadata::source_identity(app, target).is_ok_and(|(_, k)| k == "dataset") {
        parents(app, target, &stored(app)?)
    } else {
        Ok(vec![])
    }
}
pub(crate) fn mirror_record(mut body: Value, include_missing_artifact_ids: bool) -> Value {
    // These two legacy envelopes index the same original definition. Retain
    // publication state, but normalize its placement and generated bindings.
    let state = body
        .get("publication_state")
        .cloned()
        .or_else(|| body["original"]["publication"].get("state").cloned());
    let binding = &body["original"]["publication"];
    let known_binding = binding.is_null()
        || binding.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "state",
                    "contract_revision",
                    "delivery_status",
                    "entity_revision",
                    "publication_id",
                ]
                .contains(&k.as_str())
            })
        });
    if matches!(
        state.as_ref().and_then(Value::as_str),
        Some("legacy_import" | "final")
    ) && body["original"].is_object()
        && known_binding
    {
        body["publication_state"] = state.unwrap();
        let locations = body["locations"].as_array().cloned().unwrap_or_default();
        body["original"]
            .as_object_mut()
            .unwrap()
            .remove("publication");
        if let Some(artifacts) = body["original"]["artifacts"].as_array_mut() {
            for artifact in artifacts {
                let generated = artifact["id"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("artifact-"))
                    .is_some_and(|s| s.len() == 20 && s.bytes().all(|b| b.is_ascii_hexdigit()));
                let bound = locations.iter().any(|l| {
                    let direct_or_member = l["source_ref"]
                        .as_str()
                        .zip(artifact["path"].as_str())
                        .is_some_and(|(source, path)| {
                            !path.is_empty()
                                && (source == path
                                    || source
                                        .strip_prefix(path)
                                        .is_some_and(|rest| rest.starts_with('/')))
                        });
                    direct_or_member
                        && (l["sha256"].as_str().is_some_and(|s| {
                            s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
                        }) || (include_missing_artifact_ids
                            && l["source_ref"] == artifact["path"]
                            && l["status"] == "missing"))
                });
                if generated && bound {
                    artifact.as_object_mut().unwrap().remove("id");
                }
            }
        }
    }
    body
}
pub(crate) fn mirror_key(id: &Value, body: Value) -> Result<Value> {
    Ok(json!(hash(encode(&json!([
        id,
        mirror_record(body, false)
    ]))?)))
}

pub fn attach(app: &Application, entity: &mut Value) -> Result<()> {
    if entity["kind"] != "dataset" {
        return Ok(());
    }
    let Some(target) = research_metadata::snapshot_target(entity) else {
        return Ok(());
    };
    if target.kind == "historical" {
        let i = &target.identity;
        let body:String=app.db.query_row("SELECT body FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",params![i["source"].as_str(),i["kind"].as_str(),i["id"].as_str(),i["revision"].as_str(),i["snapshot"].as_str()],|r|r.get(0))?;
        entity["dataset_mirror_key"] = mirror_key(&i["id"], serde_json::from_str(&body)?)?;
    }
    let mut links = declared(app, &target)?;
    for link in &mut links {
        if let Ok(item) = app.resolve_item(&json!({"native_ref":link["parent"]})) {
            link["item_id"] = item["item_id"].clone();
            link["revision_id"] = item["revision_id"].clone();
            // A historical read returns the original label. The exact Item read
            // applies current display curation while retaining the same revision.
            if let Ok(parent) =
                app.get_item(&json!({"item_id":item["item_id"],"revision_id":item["revision_id"]}))
            {
                link["label"] = parent["label"].clone();
            }
        }
    }
    entity["dataset_lineage"] = json!({"schema":"loom-dataset-lineage/v1","parents":links});
    Ok(())
}

impl Application {
    pub fn link_dataset(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let target: Target = crate::contracts::decode(args["target"].clone())?;
        let parent: Target = crate::contracts::decode(args["parent"].clone())?;
        let source_sha = exact(self, &target)?;
        let parent_sha = exact(self, &parent)?;
        let target_key = research_metadata::snapshot_key(&target)?;
        let parent_key = research_metadata::snapshot_key(&parent)?;
        require(
            target_key != parent_key,
            "lineage_cycle",
            "Dataset cannot derive from itself",
            409,
        )?;
        validate_transform(&args["transformation"])?;
        let evidence = args["evidence"]
            .as_array()
            .ok_or_else(|| Error::input("Registered lineage evidence required"))?;
        require(
            !evidence.is_empty() && evidence.len() <= 16,
            "lineage_evidence",
            "Use 1 to 16 registered evidence references",
            400,
        )?;
        for proof in evidence {
            let selector: Target = crate::contracts::decode(proof["target"].clone())?;
            research_metadata::source_identity(self, &selector)?;
            let manifest: Value = if selector.kind == "historical" {
                self.historical_files(&selector.identity)?.0
            } else {
                let text: String = self.db.query_row(
                    "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
                    params![
                        selector.identity["id"].as_str(),
                        selector.identity["revision"].as_i64()
                    ],
                    |r| r.get(0),
                )?;
                serde_json::from_str(&text)?
            };
            require(
                manifest["components"].as_array().is_some_and(|a| {
                    a.iter().any(|c| {
                        c["ref"] == proof["component"]
                            && c["sha256"] == proof["sha256"]
                            && c["sha256"].is_string()
                    })
                }),
                "lineage_evidence",
                "Evidence must pin a registered component and its digest",
                409,
            )?;
        }
        let annotations = stored(self)?;
        if args["primary"] == true {
            require(
                !parents(self, &target, &annotations)?
                    .iter()
                    .any(|p| p["primary"] == true && p["parent"] != json!(parent)),
                "lineage_primary",
                "A Dataset has at most one declared primary browsing parent",
                409,
            )?;
        }
        let mut pending = vec![parent.clone()];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            let key = research_metadata::snapshot_key(&node)?;
            require(
                key != target_key,
                "lineage_cycle",
                "Dataset lineage would create a cycle",
                409,
            )?;
            if !visited.insert(key) {
                continue;
            }
            require(
                visited.len() <= 4096,
                "lineage_bound",
                "Lineage traversal exceeds 4096 exact revisions",
                409,
            )?;
            for p in parents(self, &node, &annotations)? {
                pending.push(crate::contracts::decode(p["parent"].clone())?);
            }
        }
        let claim = json!({"target":target,"parent":parent,"transformation":args["transformation"],"evidence":evidence,"source_sha256":source_sha,"parent_sha256":parent_sha,"primary":args["primary"].as_bool().unwrap_or(false)});
        let digest = hash(encode(&claim)?);
        let dry_run = args["dry_run"].as_bool().unwrap_or(true);
        let mut result = json!({"digest":digest,"dry_run":dry_run,"stored":false,"claim":claim});
        if dry_run {
            return Ok(result);
        }
        require(
            args["expected_digest"] == digest,
            "source_changed",
            "Preview digest required; source or lineage claim changed",
            409,
        )?;
        let key = args["key"]
            .as_str()
            .ok_or_else(|| Error::input("Request key required"))?;
        if let Some(old) = replay(&self.db, actor, key, "dataset.lineage", &args)? {
            return Ok(old);
        }
        let id = hash(encode(&json!([
            target_key,
            parent_key,
            args["transformation"]
        ]))?);
        if let Some(old) = annotations.iter().find(|v| v["id"] == id) {
            require(old["primary"].as_bool().unwrap_or(false)==args["primary"].as_bool().unwrap_or(false),"lineage_conflict","Existing lineage attribution is immutable; primary must be selected on the initial claim",409)?;
        }
        let mut body = claim;
        body["id"] = json!(id);
        body["authority"] = json!("operator");
        body["actor"] = json!(actor.id);
        body["reason"] = args["reason"].clone();
        body["created_at"] = json!(now());
        let tx = self.transaction()?;
        active(&tx)?;
        // Duplicate requests do not replace previous attribution or evidence.
        tx.execute("INSERT INTO rust_annotations VALUES('dataset_lineage',?,?,?) ON CONFLICT(kind,id) DO NOTHING",params![id,encode(&body)?,now()])?;
        result["stored"] = json!(true);
        receipt(&tx, actor, key, "dataset.lineage", &args, &result)?;
        tx.commit()?;
        Ok(result)
    }
}
