//! Compact Item authority and transactional change journal. No filesystem traversal or index writes.
use crate::{error::require, store::*, Error, Result};
use loom_domain::item::ItemId;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
pub fn allocate() -> Vec<u8> {
    uuid::Uuid::now_v7().into_bytes().to_vec()
}
pub fn decimal(bytes: &[u8]) -> Result<String> {
    Ok(ItemId::from_bytes(
        bytes
            .try_into()
            .map_err(|_| Error::input("Invalid stored Item ID"))?,
    )
    .to_string())
}
pub fn parse(value: &Value) -> Result<Vec<u8>> {
    Ok(ItemId::parse(
        value
            .as_str()
            .ok_or_else(|| Error::input("Item ID must be a decimal string"))?,
    )?
    .bytes()
    .to_vec())
}
pub fn installed(db: &Connection) -> Result<bool> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='catalog_meta')",
        [],
        |r| r.get(0),
    )?)
}
pub fn initialize(db: &Connection) -> Result<()> {
    db.execute_batch(include_str!("../../../contracts/item-schema.sql"))?;
    db.execute(
        "INSERT OR IGNORE INTO catalog_meta VALUES(1,?,?,1)",
        params![
            allocate(),
            [
                uuid::Uuid::new_v4().into_bytes(),
                uuid::Uuid::new_v4().into_bytes()
            ]
            .concat()
        ],
    )?;
    for (table, keys) in sources() {
        if !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
            [table],
            |r| r.get::<_, bool>(0),
        )? {
            continue;
        }
        let identity = sql_identity(&keys, "NEW");
        let old = sql_identity(&keys, "OLD");
        for (verb, key) in [
            ("INSERT", identity.clone()),
            ("UPDATE", identity.clone()),
            ("DELETE", old),
        ] {
            let name = format!("catalog_{}_{}", table, verb.to_lowercase());
            let condition = if table == "rust_runtime_observation" && verb == "UPDATE" {
                " WHEN NEW.value!=OLD.value"
            } else {
                ""
            };
            db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS {name} AFTER {verb} ON {table}{condition} BEGIN INSERT INTO catalog_dirty(topic,identity) VALUES('{table}',{key}); END;"))?;
        }
        db.execute(
            "INSERT OR IGNORE INTO catalog_backfill(topic) VALUES(?)",
            [table],
        )?;
    }
    for table in ["resident_tokens", "principals", "credentials"] {
        for verb in ["INSERT", "UPDATE", "DELETE"] {
            db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS catalog_access_{table}_{verb} AFTER {verb} ON {table} BEGIN UPDATE catalog_meta SET access_epoch=access_epoch+1 WHERE id=1; END;"))?;
        }
    }
    Ok(())
}
fn sql_identity(keys: &[&str], prefix: &str) -> String {
    format!(
        "json_object({})",
        keys.iter()
            .map(|k| format!("'{k}',{prefix}.{k}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
pub fn sources() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("jobs", vec!["id"]),
        ("job_workflows", vec!["id"]),
        ("entity_revisions", vec!["id", "revision"]),
        ("asset_revisions", vec!["id", "revision"]),
        ("asset_locations", vec!["id", "revision"]),
        (
            "history_records",
            vec!["source", "kind", "id", "revision", "snapshot"],
        ),
        ("components", vec!["owner", "id", "revision"]),
        ("tools", vec!["owner", "id", "revision"]),
        ("execution_profiles", vec!["owner", "id", "revision"]),
        ("scenario_revisions", vec!["scenario_id", "revision"]),
        ("scenarios", vec!["id"]),
        ("catalog_sources", vec!["kind", "identity"]),
        ("rust_runtime_observation", vec!["id"]),
        ("rust_effects", vec!["id"]),
        ("entity_locations", vec!["id", "revision"]),
        ("historical_bindings", vec!["identity"]),
        ("research_overlays", vec!["kind", "id"]),
        ("scenario_links", vec!["scenario_id", "revision"]),
    ]
}
fn source(db: &Connection, topic: &str, key: &Value) -> Result<Option<Value>> {
    let (table, fields) = sources()
        .into_iter()
        .find(|(t, _)| *t == topic)
        .ok_or_else(|| Error::input("Unknown catalog source"))?;
    let sql = format!(
        "SELECT {}* FROM {table} WHERE {} LIMIT 1",
        if topic == "history_records" {
            "rowid AS source_order,"
        } else {
            ""
        },
        fields
            .iter()
            .map(|k| format!("{k} IS ?"))
            .collect::<Vec<_>>()
            .join(" AND ")
    );
    let values = fields
        .iter()
        .map(|k| match &key[*k] {
            Value::Number(n) => rusqlite::types::Value::Integer(n.as_i64().unwrap_or(0)),
            Value::String(s) => rusqlite::types::Value::Text(s.clone()),
            _ => rusqlite::types::Value::Null,
        })
        .collect::<Vec<_>>();
    let mut q = db.prepare(&sql)?;
    let mut rows = q.query(rusqlite::params_from_iter(values))?;
    Ok(rows.next()?.map(json_row).transpose()?)
}
fn decode(v: &Value) -> Value {
    v.as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null)
}
fn doc(db: &Connection, topic: &str, key: &Value) -> Result<Option<(String, Value, Value, Value)>> {
    let row = source(db, topic, key)?;
    if topic == "rust_runtime_observation" {
        return Ok(None);
    }
    if matches!(
        topic,
        "entity_locations"
            | "asset_locations"
            | "historical_bindings"
            | "research_overlays"
            | "scenario_links"
    ) {
        return Ok(None);
    }
    let Some(row) = row else { return Ok(None) };
    let (native_kind, logical, exact, definition, mut body) = match topic {
        "jobs" | "job_workflows" => {
            let spec = decode(if topic == "jobs" {
                &row["spec"]
            } else {
                &row["request"]
            });
            let root: Option<String> = if topic == "jobs" {
                db.query_row(
                    "SELECT root FROM execution_memberships WHERE job=?",
                    [row["id"].as_str()],
                    |r| r.get(0),
                )
                .optional()?
            } else {
                None
            };
            (
                "job",
                json!({"id":row["id"]}),
                json!({"id":row["id"]}),
                spec.clone(),
                json!({"kind":"job","label":spec["presentation"].get("label").or_else(||spec.get("label")).or_else(||spec.get("kind")).unwrap_or(&row["id"]),"recorded_at":row["created"],"state":row["state"],"owner":row["owner"],"category":spec["category"],"scope":if spec["presentation"]["scope"].is_string(){spec["presentation"]["scope"].clone()}else{json!("research")},"role":if root.is_some(){json!("support")}else{spec["presentation"].get("role").cloned().unwrap_or(json!("main"))},"workflow_root":root,"scenario_ref":spec["scenario_ref"],"workflow":topic=="job_workflows"}),
            )
        }
        "entity_revisions" | "asset_revisions" => {
            let m = decode(&row["manifest"]);
            let p: Value = db
                .query_row(
                    "SELECT spec FROM jobs WHERE id=?",
                    [row["job"].as_str()],
                    |r| r.get::<_, String>(0),
                )
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            (
                "entity",
                json!({"id":row["id"]}),
                json!({"id":row["id"],"revision":row["revision"]}),
                m.clone(),
                json!({"kind":row["kind"],"label":m.get("label").unwrap_or(&row["id"]),"owner":row["owner"],"recorded_at":null,"job":row["job"],"scope":p["presentation"].get("scope").or_else(||m["presentation"].get("scope")).cloned().unwrap_or(json!("research")),"role":p["presentation"].get("role").or_else(||m["presentation"].get("role")).cloned().unwrap_or(json!("main")),"scenario_ref":p.get("scenario_ref").or_else(||m.get("scenario_ref")),"summary":m.get("description").and_then(Value::as_str).unwrap_or("")}),
            )
        }
        "history_records" => {
            let m = decode(&row["body"]);
            let presentation = [
                &m["presentation"],
                &m["original"]["presentation"],
                &m["entity"]["presentation"],
                &m["record"]["spec"]["presentation"],
            ]
            .into_iter()
            .find(|p| p.is_object())
            .cloned()
            .unwrap_or(json!({}));
            let kind = if row["kind"] == "job_view" {
                json!("job")
            } else if row["kind"] == "run" {
                json!("experiment")
            } else {
                row["kind"].clone()
            };
            let mut logical = key.clone();
            let mut summary = m["entity"]["summary"].clone();
            if row["source"] == crate::diagnostics::EXECUTION_ISSUE_SOURCE && kind == "issue" {
                logical.as_object_mut().unwrap().remove("revision");
                summary = json!(std::iter::once(summary.as_str().unwrap_or(""))
                    .chain(
                        m["entity"]["properties"]["findings"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                    )
                    .collect::<Vec<_>>()
                    .join(" "));
            }
            (
                "historical",
                logical,
                key.clone(),
                m.clone(),
                json!({"kind":kind,"label":m["original"].get("label").or_else(||m["entity"].get("label")).or_else(||m["record"]["spec"]["presentation"].get("label")).unwrap_or(&row["id"]),"scope":presentation.get("scope").cloned().unwrap_or(json!("research")),"role":presentation.get("role").cloned().unwrap_or(json!("main")),"summary":summary,"historical":true,"source_order":row["source_order"],"recorded_at":m["record"]["created"].as_f64(),"snapshot":row["snapshot"],"state":m["record"].get("state").or_else(||m["entity"].get("state")).cloned()}),
            )
        }
        "components" | "tools" | "execution_profiles" => {
            let m = decode(if topic == "execution_profiles" {
                &row["profile"]
            } else {
                &row["manifest"]
            });
            let k = match topic {
                "components" => "component",
                "tools" => "tool",
                _ => "profile",
            };
            (
                k,
                json!({"owner":row["owner"],"id":row["id"]}),
                key.clone(),
                m.clone(),
                json!({"kind":k,"label":m.get("label").unwrap_or(&row["id"]),"owner":row["owner"],"scope":"research","role":"main","recorded_at":null,"summary":m["description"].as_str().unwrap_or("")}),
            )
        }
        "scenario_revisions" => {
            let m = decode(&row["definition"]);
            (
                "scenario",
                json!({"id":row["scenario_id"]}),
                json!({"id":row["scenario_id"],"revision":row["revision"]}),
                m.clone(),
                json!({"kind":"scenario","label":m["label"],"summary":m["description"].as_str().unwrap_or(""),"scope":"research","role":"main","recorded_at":row["created"],"archived":db.query_row("SELECT archived FROM scenarios WHERE id=?",[row["scenario_id"].as_str()],|r|r.get::<_,bool>(0))?}),
            )
        }
        "catalog_sources" => {
            let mut m = decode(&row["body"]);
            let identity = decode(&row["identity"]);
            let k = row["kind"].as_str().unwrap_or("profile");
            if matches!(
                k,
                "metadata_template" | "report_contract" | "report_definition"
            ) {
                let definition = m.clone();
                m.as_object_mut().unwrap().remove("definition");
                m["native_ref"] = json!({"kind":k,"identity":identity});
                return Ok(Some((
                    k.into(),
                    json!({"id":identity["id"]}),
                    definition,
                    m,
                )));
            }
            return Ok(Some((k.into(), identity.clone(), identity, m)));
        }
        "rust_effects" => {
            let w = decode(&row["request"]);
            if row["stage"] == "publish" {
                return Ok(None);
            };
            (
                "log_stream",
                json!({"effect":row["id"]}),
                json!({"effect":row["id"]}),
                json!({"job":row["job"],"attempt":row["attempt"],"stage":row["stage"],"generation":w["generation"]}),
                json!({"kind":"log_stream","label":format!("{} · {}",row["job"].as_str().unwrap_or(""),row["stage"].as_str().unwrap_or("")),"job":row["job"],"attempt":row["attempt"],"stage":row["stage"],"effect":row["id"],"generation":w["generation"],"channel":"combined","source_state":row["state"],"scope":"operations","role":"support","recorded_at":row["created"]}),
            )
        }
        _ => return Ok(None),
    };
    if topic == "execution_profiles" {
        body["profile_kind"] = definition["revision"].clone();
        body["profile_sha256"] = definition["sha256"].clone();
    }
    if matches!(
        body["kind"].as_str(),
        Some("dataset" | "model" | "experiment" | "run")
    ) {
        let explicit = [
            definition["catalog_class"].as_str(),
            definition["original"]["catalog_class"].as_str(),
        ]
        .into_iter()
        .flatten()
        .find(|class| matches!(*class, "regular" | "reference"));
        if let Some(class) = explicit {
            body["catalog_class"] = json!(class);
        } else if let Some(job) = body["job"].as_str() {
            let is_probe: Option<bool> = db.query_row(
                "SELECT COALESCE(json_extract(spec,'$.execution_mode') IN('pilot','smoke'),0) FROM jobs WHERE id=?",
                [job], |r| r.get(0)
            ).optional()?;
            if is_probe == Some(true) {
                body["catalog_class"] = json!("reference");
            }
        }
    }
    if body.get("origin_type").is_none() {
        body["origin_type"] = json!(if native_kind == "historical" {
            "preserved"
        } else if topic == "entity_revisions" || topic == "asset_revisions" {
            definition["origin"]["type"].as_str().unwrap_or("produced")
        } else {
            "registered"
        });
    }
    if matches!(native_kind, "entity" | "historical") {
        if definition["metadata_template"].is_object() {
            body["robot"] = definition["metadata"]["research"]["robot"].clone();
            body["metadata_template_ref"] = definition["metadata_template"].clone();
        }
        let target = crate::contracts::Target {
            kind: native_kind.into(),
            identity: exact.clone(),
        };
        if let Some(metadata) = crate::metadata_templates::annotation(db, &target)? {
            if let Some(label) = body["label"].as_str() {
                body["label"] = json!(crate::metadata_templates::robot_prefix(
                    &metadata["research"],
                    label
                ));
            }
            body["robot"] = metadata["research"]["robot"].clone();
            body["metadata_template_ref"] =
                crate::metadata_templates::reference(&metadata["template"]);
        }
    }
    let id = exact["id"].as_str().unwrap_or("");
    let overlay: Option<String> = db
        .query_row(
            "SELECT presentation FROM research_overlays WHERE kind=? AND id=?",
            params![
                if native_kind == "entity" {
                    "entity"
                } else {
                    body["kind"].as_str().unwrap_or(native_kind)
                },
                id
            ],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(overlay) = overlay {
        let p: Value = serde_json::from_str(&overlay)?;
        for k in ["scope", "role", "label"] {
            if p[k].is_string() {
                body[k] = p[k].clone();
            }
        }
    }
    if body["robot"].is_object() {
        if let Some(label) = body["label"].as_str() {
            body["label"] = json!(crate::metadata_templates::robot_prefix(
                &json!({"robot":body["robot"]}),
                label
            ));
        }
    }
    let kind = body["kind"].as_str().unwrap_or(native_kind).to_string();
    body["kind"] = json!(kind);
    body["native_ref"] = json!({"kind":native_kind,"identity":exact});
    body["source_key"] = json!(hash(encode(&json!([body["native_ref"], definition]))?));
    Ok(Some((native_kind.into(), logical, definition, body)))
}
pub fn adopt(db: &Connection, topic: &str, key: &Value) -> Result<()> {
    if topic == "components" {
        if let Some(row) = source(db, topic, key)? {
            let profiles = decode(&row["profiles"]);
            for (kind, p) in profiles.as_object().into_iter().flatten() {
                let identity = encode(&json!({"kind":kind}))?;
                let body = json!({"kind":"profile","profile_kind":kind,"profile_sha256":p["sha256"],"label":p["usage"]["label"].as_str().unwrap_or(kind),"summary":p["usage"]["description"].as_str().unwrap_or(""),"scope":"research","role":"main","origin_type":"registered","source_key":p["sha256"].as_str().map(String::from).unwrap_or(hash(encode(p)?)),"native_ref":{"kind":"profile","identity":{"kind":kind,"sha256":p["sha256"]}},"component_ref":{"owner":row["owner"],"id":row["id"],"revision":row["revision"]}});
                db.execute("INSERT INTO catalog_sources VALUES('profile',?,?) ON CONFLICT(kind,identity) DO UPDATE SET body=excluded.body WHERE body!=excluded.body",params![identity,encode(&body)?])?;
                adopt(
                    db,
                    "catalog_sources",
                    &json!({"kind":"profile","identity":identity}),
                )?;
            }
        }
    }
    if topic == "rust_runtime_observation" {
        return adopt_runtimes(db);
    }
    if topic == "scenarios" {
        if let Some(row) = source(db, topic, key)? {
            return adopt(
                db,
                "scenario_revisions",
                &json!({"scenario_id":row["id"],"revision":row["current_revision"]}),
            );
        }
        return Ok(());
    }
    if matches!(topic, "entity_locations" | "asset_locations") {
        return emit_parent_refresh(db, "entity", key);
    }
    if topic == "historical_bindings" {
        return emit_parent_refresh(db, "historical", &decode(&key["identity"]));
    }
    if topic == "scenario_links" {
        return emit_parent_refresh(db, "scenario", &json!({"id":key["scenario_id"]}));
    }
    if topic == "research_overlays" {
        let kind = key["kind"].as_str().unwrap_or("job");
        if matches!(kind, "model" | "dataset" | "experiment") {
            let refs=crate::store::rows(db,"SELECT r.native_ref FROM catalog_items i JOIN catalog_revisions r ON i.current_revision=r.id WHERE i.native_kind='historical' AND i.kind=? AND json_extract(r.native_ref,'$.identity.id')=?",&[&kind,&key["id"].as_str()])?;
            for r in refs {
                let native: Value = serde_json::from_str(r["native_ref"].as_str().unwrap_or("{}"))?;
                adopt(db, "history_records", &native["identity"])?;
            }
            return Ok(());
        }
        let table = match kind {
            "job"
                if db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM job_workflows WHERE id=?)",
                    [key["id"].as_str()],
                    |r| r.get::<_, bool>(0),
                )? =>
            {
                "job_workflows"
            }
            "job" => "jobs",
            "component" => "components",
            "tool" => "tools",
            _ => "entity_revisions",
        };
        let (native_kind, _) = if table == "entity_revisions" {
            ("entity", 0)
        } else {
            (kind, 0)
        };
        let refs=crate::store::rows(db,"SELECT r.native_ref FROM catalog_items i JOIN catalog_revisions r ON i.current_revision=r.id WHERE i.native_kind=? AND json_extract(r.native_ref,'$.identity.id')=?",&[&native_kind,&key["id"].as_str()])?;
        for r in refs {
            let native: Value = serde_json::from_str(r["native_ref"].as_str().unwrap_or("{}"))?;
            let actual = if table == "entity_revisions"
                && db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM asset_revisions WHERE id=? AND revision=?)",
                    params![
                        native["identity"]["id"].as_str(),
                        native["identity"]["revision"].as_i64()
                    ],
                    |r| r.get::<_, bool>(0),
                )? {
                "asset_revisions"
            } else {
                table
            };
            adopt(db, actual, &native["identity"])?;
        }

        return emit_parent_refresh(
            db,
            key["kind"].as_str().unwrap_or("job"),
            &json!({"id":key["id"]}),
        );
    }
    let Some((native_kind, logical, definition, body)) = doc(db, topic, key)? else {
        let kind = match topic {
            "jobs" | "job_workflows" => "job",
            "entity_revisions" => "entity",
            "history_records" => "historical",
            "components" => "component",
            "tools" => "tool",
            "execution_profiles" => "profile",
            "scenario_revisions" => "scenario",
            _ => return Ok(()),
        };
        if source(db, topic, key)?.is_none() {
            let row: Option<Vec<u8>> = db
                .query_row(
                    "SELECT item FROM catalog_aliases WHERE kind=? AND identity=?",
                    params![kind, encode(key)?],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(item) = row {
                tombstone(db, &item)?;
            }
        }
        return Ok(());
    };
    adopt_descriptor(db, native_kind, logical, definition, body, true)
}

fn adopt_descriptor(
    db: &Connection,
    native_kind: String,
    logical: Value,
    definition: Value,
    mut body: Value,
    publish_current: bool,
) -> Result<()> {
    let logical_key = encode(&logical)?;
    let item: Vec<u8> = db
        .query_row(
            "SELECT id FROM catalog_items WHERE native_kind=? AND native_key=?",
            params![native_kind, logical_key],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_else(allocate);
    let source_key = body["source_key"]
        .as_str()
        .map(String::from)
        .unwrap_or(hash(encode(&definition)?));
    let revision: Vec<u8> = db
        .query_row(
            "SELECT id FROM catalog_revisions WHERE item=? AND source_key=?",
            params![item, source_key],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_else(allocate);
    let native_ref = body
        .get("native_ref")
        .cloned()
        .unwrap_or(json!({"kind":native_kind,"identity":logical}));
    body["native_ref"] = native_ref.clone();
    body["item_id"] = json!(decimal(&item)?);
    body["revision_id"] = json!(decimal(&revision)?);
    let origin: Vec<u8> = db.query_row("SELECT origin FROM catalog_meta WHERE id=1", [], |r| {
        r.get(0)
    })?;
    body["origin"] = json!(decimal(&origin)?);
    if body.get("label").is_none() {
        body["label"] = native_ref["identity"]
            .get("id")
            .cloned()
            .unwrap_or(json!(native_kind));
    }
    if body.get("scope").is_none() {
        body["scope"] = json!("research");
        body["role"] = json!("main");
    }
    for k in ["label", "summary"] {
        if let Some(s) = body[k].as_str() {
            body[k] = json!(s
                .chars()
                .take(if k == "label" { 512 } else { 4096 })
                .collect::<String>());
        }
    }
    let encoded = encode(&body)?;
    require(
        encoded.len() <= 65536,
        "catalog_metadata",
        "Catalog descriptor exceeds limit",
        400,
    )?;
    let previous: Option<String> = db.query_row("SELECT r.native_ref FROM catalog_items i JOIN catalog_revisions r ON i.current_revision=r.id WHERE i.id=?",[&item],|r|r.get(0)).optional()?;
    let old_native: Value = previous
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null);
    let revision_number = |identity: &Value| {
        identity["revision"].as_i64().or_else(|| {
            (identity["source"] == crate::diagnostics::EXECUTION_ISSUE_SOURCE
                && identity["kind"] == "issue")
                .then(|| identity["revision"].as_str()?.parse().ok())
                .flatten()
        })
    };
    let current = publish_current
        && match (
            revision_number(&old_native["identity"]),
            revision_number(&native_ref["identity"]),
        ) {
            (Some(a), Some(b)) => b >= a,
            _ => true,
        };
    db.execute("INSERT INTO catalog_items VALUES(?,?,?,?,?,?,0) ON CONFLICT(native_kind,native_key) DO UPDATE SET current_revision=CASE WHEN ? THEN excluded.current_revision ELSE current_revision END,deleted=0",params![item,body["kind"].as_str(),native_kind,logical_key,revision,now(),current])?;
    db.execute(
        "INSERT OR IGNORE INTO catalog_revisions VALUES(?,?,?,?,?)",
        params![revision, item, source_key, encode(&native_ref)?, encoded],
    )?;
    let secondary = body["profile_kind"]
        .as_str()
        .map(|kind| json!({"kind":kind,"sha256":body["profile_sha256"]}));
    let mut identities = vec![logical.clone(), native_ref["identity"].clone()];
    if let Some(secondary) = secondary {
        identities.push(secondary)
    }
    for identity in &identities {
        if !current && identity == &logical {
            continue;
        }
        let exact = identity == &native_ref["identity"]
            && (identity.get("revision").is_some()
                || identity.get("snapshot").is_some()
                || identity["sha256"].is_string());
        if exact {
            let existing: Option<(Vec<u8>, Vec<u8>)> = db
                .query_row(
                    "SELECT item,revision FROM catalog_aliases WHERE kind=? AND identity=?",
                    params![native_kind, encode(identity)?],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            require(
                existing.is_none_or(|(i, r)| i == item && r == revision),
                "conflicting_identity",
                "An immutable native reference already identifies another definition",
                409,
            )?;
        }
        db.execute("INSERT INTO catalog_aliases VALUES(?,?,?,?) ON CONFLICT(kind,identity) DO UPDATE SET item=excluded.item,revision=excluded.revision",params![native_kind,encode(identity)?,item,revision])?;
    }
    if !current {
        return Ok(());
    }
    let old:Option<String>=db.query_row("SELECT body FROM catalog_changes WHERE item=? AND operation='upsert' ORDER BY seq DESC LIMIT 1",[&item],|r|r.get(0)).optional()?;
    if old.as_deref() != Some(&encoded) {
        db.execute("INSERT INTO catalog_changes(item,revision,operation,body,created) VALUES(?,?,'upsert',?,?)",params![item,revision,encoded,now()])?;
    }
    Ok(())
}
fn emit_parent_refresh(db: &Connection, kind: &str, key: &Value) -> Result<()> {
    let Some((item, revision)) = db
        .query_row(
            "SELECT item,revision FROM catalog_aliases WHERE kind=? AND identity=?",
            params![kind, encode(key)?],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?)),
        )
        .optional()?
    else {
        return Ok(());
    };
    db.execute("INSERT INTO catalog_changes(item,revision,operation,body,created) VALUES(?,?,'refresh',?,?)",params![item,revision,encode(key)?,now()])?;
    Ok(())
}
pub fn flush(db: &Connection, since: i64, limit: usize) -> Result<usize> {
    let mut q = db
        .prepare("SELECT seq,topic,identity FROM catalog_dirty WHERE seq>? ORDER BY seq LIMIT ?")?;
    let batch = q
        .query_map(params![since, limit as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (seq, topic, key) in &batch {
        adopt(db, topic, &serde_json::from_str(key)?)?;
        db.execute("DELETE FROM catalog_dirty WHERE seq=?", [seq])?;
    }
    Ok(batch.len())
}
impl Application {
    pub fn catalog_head(&self) -> Result<i64> {
        Ok(self.db.query_row(
            "SELECT COALESCE(max(seq),0) FROM catalog_changes",
            [],
            |r| r.get(0),
        )?)
    }
}
impl Application {
    pub fn get_item(&self, args: &Value) -> Result<Value> {
        let item = parse(&args["item_id"])?;
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalog_items WHERE id=?)",
            [&item],
            |r| r.get(0),
        )?;
        if !exists {
            return self.get_file(&json!({"file_item_id":args["item_id"]}));
        }
        self.require_catalog_item(&item)?;
        let body: Option<String> = if args["revision_id"].is_string() {
            let rev = parse(&args["revision_id"])?;
            self.db
                .query_row(
                    "SELECT body FROM catalog_revisions WHERE item=? AND id=?",
                    params![item, rev],
                    |r| r.get(0),
                )
                .optional()?
        } else {
            self.db.query_row("SELECT body FROM catalog_changes WHERE item=? AND operation='upsert' ORDER BY seq DESC LIMIT 1",[&item],|r|r.get(0)).optional()?
        };
        let text = body.ok_or_else(|| Error::new("unknown_item", "Item not found", 404))?;
        let mut value: Value = serde_json::from_str(&text)?;
        if crate::store::review_tables(&self.db)?
            && matches!(
                value["kind"].as_str(),
                Some("dataset" | "model" | "experiment" | "run")
            )
        {
            let identity = &value["native_ref"]["identity"];
            let source = identity["source"].as_str().unwrap_or("entity");
            let kind = identity["kind"].as_str().or_else(|| value["kind"].as_str());
            let revision = identity["revision"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| identity["revision"].to_string());
            let reviewed: Option<(String, String)> = self.db.query_row(
                "SELECT catalog_class,reason FROM reviewed_outputs WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",
                params![source,kind,identity["id"].as_str(),revision,identity["snapshot"].as_str().unwrap_or("")],
                |row| Ok((row.get(0)?,row.get(1)?)),
            ).optional()?;
            if let Some((class, reason)) = reviewed {
                value["catalog_class"] = json!(class);
                value["classification_reason"] = json!(reason);
            }
        }
        let native = &value["native_ref"];
        let disposition =
            self.output_disposition(&native["identity"], value["kind"].as_str().unwrap_or(""))?;
        if matches!(native["kind"].as_str(), Some("entity" | "historical")) {
            let kind = if native["kind"] == "entity" {
                "entity"
            } else {
                value["kind"].as_str().unwrap_or("")
            };
            value["label"] = self.output_label(
                kind,
                native["identity"]["id"].as_str().unwrap_or(""),
                &value["label"],
            )?;
            if let Some(label) = value["label"].as_str() {
                value["label"] = json!(crate::metadata_templates::robot_prefix(
                    &json!({"robot":value["robot"]}),
                    label
                ));
            }
        }
        if let Some(disposition) = disposition {
            value["disposition"] = disposition;
        }
        value["files"] = json!({"action":"results.item_files","item_id":args["item_id"],"revision_id":value["revision_id"]});
        Ok(value)
    }
    pub fn resolve_item(&self, args: &Value) -> Result<Value> {
        let r = &args["native_ref"];
        let mut row: Option<(Vec<u8>, Vec<u8>)> = self
            .db
            .query_row(
                "SELECT item,revision FROM catalog_aliases WHERE kind=? AND identity=?",
                params![r["kind"].as_str(), encode(&r["identity"])?],
                |v| Ok((v.get(0)?, v.get(1)?)),
            )
            .optional()?;
        // A Runtime URL contains its immutable id/revision but not a content hash.
        // Resolve only one exact revision; an explicit hash never falls back.
        if row.is_none()
            && r["kind"] == "runtime"
            && r["identity"]
                .as_object()
                .is_some_and(|v| v.len() == 2 && v.contains_key("id") && v.contains_key("revision"))
        {
            let mut q=self.db.prepare("SELECT DISTINCT item,revision FROM catalog_aliases WHERE kind='runtime' AND json_extract(identity,'$.id')=? AND json_extract(identity,'$.revision')=? LIMIT 2")?;
            let found = q
                .query_map(
                    params![
                        r["identity"]["id"].as_str(),
                        r["identity"]["revision"].as_i64()
                    ],
                    |v| Ok((v.get::<_, Vec<u8>>(0)?, v.get::<_, Vec<u8>>(1)?)),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            require(
                found.len() <= 1,
                "ambiguous_reference",
                "Runtime revision has conflicting native aliases",
                409,
            )?;
            row = found.into_iter().next();
        }
        let (item, revision) = row
            .ok_or_else(|| Error::new("unknown_item", "Exact native reference not adopted", 404))?;
        self.require_catalog_item(&item)?;
        Ok(json!({"item_id":decimal(&item)?,"revision_id":decimal(&revision)?,"native_ref":r}))
    }
    pub fn catalog_tick(&mut self) -> Result<Value> {
        let tx = self.transaction()?;
        let mut left = 128;
        for (topic, keys) in sources() {
            if !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                [topic],
                |r| r.get::<_, bool>(0),
            )? {
                continue;
            }
            if matches!(
                topic,
                "entity_locations"
                    | "asset_locations"
                    | "historical_bindings"
                    | "research_overlays"
                    | "scenario_links"
            ) {
                tx.execute(
                    "UPDATE catalog_backfill SET complete=1 WHERE topic=?",
                    [topic],
                )?;
                continue;
            }
            let (position, complete): (i64, bool) = tx.query_row(
                "SELECT position,complete FROM catalog_backfill WHERE topic=?",
                [topic],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if complete {
                continue;
            }
            let sql = format!(
                "SELECT rowid,json_object({}) FROM {topic} WHERE rowid>? ORDER BY rowid LIMIT ?",
                keys.iter()
                    .map(|k| format!("'{k}',{k}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let batch = tx
                .prepare(&sql)?
                .query_map(params![position, left], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (_, key) in &batch {
                adopt(&tx, topic, &serde_json::from_str(key)?)?;
            }
            tx.execute(
                "UPDATE catalog_backfill SET position=?,complete=? WHERE topic=?",
                params![
                    batch.last().map(|x| x.0).unwrap_or(position),
                    batch.len() < (left as usize),
                    topic
                ],
            )?;
            left -= batch.len() as i64;
            if left <= 0 {
                break;
            }
        }
        let dirty = flush(&tx, 0, 128)?;
        tx.commit()?;
        Ok(json!({"adopted":128-left,"dirty":dirty}))
    }
}

impl Application {
    pub fn index_once(&mut self) -> Result<Value> {
        self.catalog_tick()?;
        let tasks = self.pending_inventories()?;
        for (item, revision) in tasks {
            let prepared = match crate::inventory::prepare(self, &item, &revision) {
                Ok(p) => p,
                Err(e) if e.code == "runtime_provider_required" => continue,
                Err(e) => return Err(e),
            };
            self.commit_inventory(prepared)?;
        }
        let mut index = crate::index_store::IndexWriter::open(self)?;
        let n = index.sync(self, 128)?;
        crate::log_index::project(&mut index, self)?;
        index.observe_files(self)?;
        Ok(serde_json::json!({"projected":n,"status":self.search_status()?}))
    }
}
impl Application {
    pub fn cache_catalog_sources(&mut self) -> Result<()> {
        let profiles = self.config.profiles.clone();
        let tx = self.transaction()?;
        crate::metadata_templates::cache_builtins(&tx)?;
        crate::reporting::cache_builtins(&tx)?;
        for (kind, p) in profiles {
            let identity = json!({"kind":kind});
            let body = json!({"kind":"profile","label":p.get("label").and_then(Value::as_str).unwrap_or(&kind),"summary":p.get("description").and_then(Value::as_str).unwrap_or(""),"source_key":p["sha256"].as_str().map(String::from).unwrap_or(hash(encode(&p)?)),"native_ref":{"kind":"profile","identity":{"kind":kind,"sha256":p["sha256"]}},"scope":"research","role":"main","origin_type":"installation"});
            tx.execute("INSERT INTO catalog_sources VALUES('profile',?,?) ON CONFLICT(kind,identity) DO UPDATE SET body=excluded.body WHERE body!=excluded.body",params![encode(&identity)?,encode(&body)?])?;
        }
        let body = json!({"kind":"profile","label":"cpu.smoke","native_ref":{"kind":"profile","identity":{"kind":"cpu.smoke"}},"origin_type":"builtin","scope":"research","role":"main","source_key":hash(env!("CARGO_PKG_VERSION"))});
        tx.execute(
            "INSERT OR IGNORE INTO catalog_sources VALUES('profile',?,?)",
            params![encode(&json!({"kind":"cpu.smoke"}))?, encode(&body)?],
        )?;
        tx.commit()?;
        Ok(())
    }
}

pub use loom_primitives::items::dependency_ref;
// One current source per logical Item, plus independently resolvable revision pins.
fn observe_item(
    db: &Connection,
    kind: &str,
    logical: Value,
    body: Value,
    current: bool,
) -> Result<()> {
    if current {
        db.execute("INSERT INTO catalog_sources VALUES(?,?,?) ON CONFLICT(kind,identity) DO UPDATE SET body=excluded.body WHERE body!=excluded.body", params![kind,encode(&logical)?,encode(&body)?])?;
    } else {
        let known: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalog_aliases WHERE kind=? AND identity=?)",
            params![kind, encode(&body["native_ref"]["identity"])?],
            |r| r.get(0),
        )?;
        if known {
            return Ok(());
        }
    }
    adopt_descriptor(db, kind.into(), logical.clone(), logical, body, current)
}

fn adopt_runtimes(db: &Connection) -> Result<()> {
    let encoded: Option<String> = db
        .query_row(
            "SELECT value FROM rust_runtime_observation WHERE id=1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(encoded) = encoded else {
        return Ok(());
    };
    let value: Value = serde_json::from_str(&encoded)?;
    if !value["runtimes"].is_array() || value["status"] == "unavailable" {
        return Ok(());
    }
    let mut dependencies_present = std::collections::BTreeSet::new();
    let mut present = std::collections::BTreeSet::new();
    let mut runtimes: Vec<_> = value["runtimes"].as_array().into_iter().flatten().collect();
    // Select the highest observed revision within a Runtime. A shared origin
    // reported by independent Runtimes has a stable Runtime-ID tie breaker.
    runtimes.sort_by_key(|r| (r["id"].as_str(), std::cmp::Reverse(r["revision"].as_u64())));
    for runtime in runtimes {
        if runtime["legacy"] == true {
            continue;
        }
        let primary = crate::runtime::primary(&runtime["manifest"], None)?;
        for dep in runtime["manifest"]["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if !dep["name"].is_string() || !dep["version"].is_string() {
                continue;
            }
            let exact = dependency_ref(runtime, dep);
            let logical =
                json!({"name":exact["name"],"scope":exact["scope"],"origin":exact["origin"]});
            let key = encode(&logical)?;
            let current = dependencies_present.insert(key);
            let main = primary
                .iter()
                .any(|d| d["name"] == dep["name"] && d["version"] == dep["version"]);
            let body = json!({"kind":"dependency","label":dep["name"],"version":dep["version"],"category":dep["category"],"scope":if main{"research"}else{"operations"},"role":if main{"main"}else{"support"},"origin_type":"host","source_key":hash(encode(&exact)?),"native_ref":{"kind":"dependency","identity":exact},"source_status":"observed","recorded_at":null});
            observe_item(db, "dependency", logical, body, current)?;
        }
        let identity = json!({"id":runtime["id"]});
        let exact =
            json!({"id":runtime["id"],"revision":runtime["revision"],"sha256":runtime["sha256"]});
        let body = json!({"kind":"runtime","label":runtime["manifest"].get("label").unwrap_or(&runtime["id"]),"scope":"research","role":"main","source_key":runtime["sha256"],"native_ref":{"kind":"runtime","identity":exact},"dependencies":runtime["manifest"].get("primary_dependencies").cloned().unwrap_or(json!([])),"source_status":"observed","file_inventory":runtime["manifest"]["file_inventory"]});
        let key = encode(&identity)?;
        observe_item(db, "runtime", identity, body, present.insert(key))?;
    }
    let mut q = db.prepare(
        "SELECT id,native_key FROM catalog_items WHERE native_kind='runtime' AND deleted=0",
    )?;
    let old = q
        .query_map([], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, key) in old {
        if value["complete"] != false && !present.contains(&key) {
            tombstone(db, &id)?;
        }
    }
    if value["complete"] != false {
        let mut q = db.prepare(
            "SELECT id,native_key FROM catalog_items WHERE native_kind='dependency' AND deleted=0",
        )?;
        let old = q
            .query_map([], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, key) in old {
            if !dependencies_present.contains(&key) {
                tombstone(db, &id)?;
            }
        }
    }
    Ok(())
}
fn tombstone(db: &Connection, item: &[u8]) -> Result<()> {
    db.execute("UPDATE catalog_items SET deleted=1 WHERE id=?", [item])?;
    db.execute(
        "UPDATE catalog_meta SET access_epoch=access_epoch+1 WHERE id=1",
        [],
    )?;
    db.execute(
        "INSERT INTO catalog_changes(item,operation,body,created) VALUES(?,'tombstone','{}',?)",
        params![item, now()],
    )?;
    Ok(())
}
impl Application {
    pub fn require_catalog_item(&self, id: &[u8]) -> Result<()> {
        require(
            self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM catalog_items WHERE id=? AND deleted=0)",
                [id],
                |r| r.get(0),
            )?,
            "unknown_item",
            "Item not found",
            404,
        )
    }
    pub fn dashboard_summary(&self) -> Result<Value> {
        let visible =
            crate::system_ownership::native_sql("json_extract(r.body,'$.native_ref')", "");
        let counts = rows(&self.db,&format!("SELECT i.kind,count(*) AS count FROM catalog_items i JOIN catalog_revisions r ON r.id=i.current_revision WHERE i.deleted=0 AND NOT COALESCE(({visible}),0) AND (i.kind!='job' OR NOT EXISTS(SELECT 1 FROM execution_memberships n WHERE n.root=json_extract(r.body,'$.native_ref.identity.id'))) GROUP BY i.kind"),&[])?;
        Ok(
            json!({"schema":"loom-dashboard-summary/v1","counts":counts,"search":self.search_status()?,"capabilities":{"search_items":true,"item_files":true,"logs":true},"observed_at":now()}),
        )
    }
}
