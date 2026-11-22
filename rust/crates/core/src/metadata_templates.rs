//! Versioned metadata structure and display contracts; never scientific acceptance.
use crate::{
    auth::Actor, contracts, error::require, research_metadata, store::*, Application, Error, Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const POLICY: &str = "loom-metadata-templates/v1";
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateRef {
    pub id: String,
    pub revision: u64,
    pub sha256: String,
}
fn builtins() -> Vec<Value> {
    let source: Value =
        serde_json::from_str(include_str!("../../../contracts/metadata-templates.json"))
            .expect("metadata templates");
    source["templates"].as_array().unwrap().iter().map(|entry| {
        let mut schema=source["schema"].clone();schema["required"]=entry["required"].clone();
        schema["properties"].as_object_mut().unwrap().retain(|key,_|entry["modules"].as_array().unwrap().iter().any(|module|module==key));
        if let Some(role)=entry.get("model_role") {schema["properties"]["model"]["properties"]["role"]=json!({"const":role});}
        let fields=source["fields"].as_array().unwrap().iter().filter(|f|entry["fields"].as_array().unwrap().contains(&f["id"])).cloned().collect::<Vec<_>>();
        let definition=json!({"label":entry["label"],"kind":entry["kind"],"schema":schema,"fields":fields});
        record(entry["id"].as_str().unwrap(),1,definition,"builtin")
    }).collect()
}
fn record(id: &str, revision: u64, definition: Value, owner: &str) -> Value {
    let sha = hash(crate::compat::seal(&definition));
    json!({"id":id,"revision":revision,"sha256":sha,"kind":"metadata_template","label":definition["label"],"output_kind":definition["kind"],"definition":definition,"owner":owner,"origin":if owner=="builtin"{"builtin"}else{"registered"},"scope":"research","role":"main","origin_type":if owner=="builtin"{"builtin"}else{"registered"}})
}
pub fn reference(template: &Value) -> Value {
    json!({"id":template["id"],"revision":template["revision"],"sha256":template["sha256"]})
}
pub fn get(db: &Connection, id: &str, revision: Option<u64>) -> Result<Value> {
    if let Some(v) = builtins()
        .into_iter()
        .find(|v| v["id"] == id && revision.is_none_or(|r| v["revision"] == r))
    {
        return Ok(v);
    }
    let body:Option<String>=db.query_row("SELECT body FROM catalog_sources WHERE kind='metadata_template' AND json_extract(identity,'$.id')=? AND (? IS NULL OR json_extract(identity,'$.revision')=?) ORDER BY json_extract(identity,'$.revision') DESC LIMIT 1",params![id,revision.map(|r|r as i64),revision.map(|r|r as i64)],|r|r.get(0)).optional()?;
    body.map(|s| serde_json::from_str(&s).map_err(Error::from))
        .transpose()?
        .ok_or_else(|| Error::new("metadata_template", "Template revision not found", 404))
}
pub fn resolve(db: &Connection, pin: &Value) -> Result<Value> {
    let pin: TemplateRef = contracts::decode(pin.clone())?;
    let value = get(db, &pin.id, Some(pin.revision))?;
    require(
        value["sha256"] == pin.sha256,
        "metadata_template_changed",
        "Template digest does not match the exact revision",
        409,
    )?;
    Ok(value)
}
pub(crate) fn bounded_schema(value: &Value, depth: usize) -> Result<()> {
    require(
        depth <= 24,
        "metadata_schema",
        "Schema nesting exceeds 24 levels",
        400,
    )?;
    match value {
        Value::Object(o) => {
            for (key, v) in o {
                if ["$ref", "$dynamicRef", "$recursiveRef"].contains(&key.as_str()) {
                    require(
                        v.as_str().is_some_and(|s| s.starts_with('#')),
                        "metadata_schema",
                        "Remote schema references are unsupported",
                        400,
                    )?;
                }
                bounded_schema(v, depth + 1)?;
            }
        }
        Value::Array(a) => {
            require(
                a.len() <= 256,
                "metadata_schema",
                "Schema array exceeds 256 entries",
                400,
            )?;
            for v in a {
                bounded_schema(v, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn definition(db: &Connection, value: &Value) -> Result<Value> {
    require(
        value.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| ["label", "kind", "schema", "fields", "base_ref"].contains(&k.as_str()))
        }) && value["label"].as_str().is_some_and(|s| {
            !s.trim().is_empty() && s.chars().count() <= 120 && !s.chars().any(char::is_control)
        }),
        "metadata_template",
        "A concise template label and supported definition fields are required",
        400,
    )?;
    require(
        matches!(
            value["kind"].as_str(),
            Some("dataset" | "model" | "experiment")
        ),
        "metadata_template",
        "Template kind must be dataset, model or experiment",
        400,
    )?;
    require(
        serde_json::to_vec(value)?.len() <= 64 * 1024,
        "metadata_template",
        "Template exceeds 64 KiB",
        400,
    )?;
    bounded_schema(&value["schema"], 0)?;
    contracts::check_schema(&value["schema"])?;
    // A custom format must retain the platform vocabulary and robot recording states.
    let base = resolve(db, &value["base_ref"])?;
    require(
        base["output_kind"] == value["kind"],
        "metadata_template",
        "Base template kind differs",
        400,
    )?;
    let mut fields = base["definition"]["fields"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let additions = value["fields"]
        .as_array()
        .ok_or_else(|| Error::input("Template fields must be an array"))?;
    require(
        additions.len() <= 32,
        "metadata_fields",
        "At most 32 extension fields are allowed",
        400,
    )?;
    let mut ids = fields
        .iter()
        .filter_map(|f| f["id"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    for field in additions {
        let id = field["id"].as_str().unwrap_or("");
        require(
            field.as_object().is_some_and(|o| {
                o.keys().all(|k| {
                    [
                        "id",
                        "label",
                        "pointer",
                        "section",
                        "format",
                        "unit",
                        "unit_pointer",
                    ]
                    .contains(&k.as_str())
                })
            }) && !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
                && ids.insert(id.into()),
            "metadata_fields",
            "Display field IDs must be unique; base fields cannot be replaced",
            400,
        )?;
        require(
            field["pointer"]
                .as_str()
                .is_some_and(|p| p.starts_with("/extensions/") && p.len() <= 256)
                && field["label"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.chars().count() <= 80)
                && field["section"].as_str().is_some_and(|s| {
                    [
                        "context",
                        "collection",
                        "quality",
                        "model",
                        "training",
                        "validation",
                        "evaluation",
                        "population",
                        "physics",
                        "observations",
                        "annotations",
                        "instructions",
                        "scoring",
                        "extensions",
                    ]
                    .contains(&s)
                })
                && matches!(
                    field["format"].as_str(),
                    Some("text" | "number" | "count" | "boolean" | "list" | "split" | "reference")
                ),
            "metadata_fields",
            "Extension display fields require typed extensions pointers",
            400,
        )?;
        require(
            field.get("unit").is_none_or(|v| {
                v.as_str()
                    .is_some_and(|s| s.chars().count() <= 32 && !s.chars().any(char::is_control))
            }) && field.get("unit_pointer").is_none_or(|v| {
                v.as_str()
                    .is_some_and(|s| s.starts_with("/extensions/") && s.len() <= 256)
            }),
            "metadata_fields",
            "Field units and unit pointers must be bounded strings",
            400,
        )?;
        fields.push(field.clone());
    }
    let mut extension = value["schema"].clone();
    let resource = format!(
        "urn:loom:metadata-schema:{}",
        hash(crate::compat::seal(&extension))
    );
    if let Some(schema) = extension.as_object_mut() {
        schema.insert("$id".into(), json!(resource));
    }
    let definition = json!({"label":value["label"],"kind":value["kind"],"base_ref":value["base_ref"],"schema":{"allOf":[base["definition"]["schema"],extension]},"fields":fields});
    require(
        serde_json::to_vec(&definition)?.len() <= 128 * 1024,
        "metadata_template",
        "Expanded template exceeds 128 KiB",
        400,
    )?;
    bounded_schema(&definition["schema"], 0)?;
    Ok(definition)
}
pub(crate) fn field_value(value: &Value, format: &str) -> bool {
    match format {
        "text" => value.as_str().is_some_and(|s| {
            !s.is_empty() && s.chars().count() <= 500 && !s.chars().any(char::is_control)
        }),
        "reference" => {
            value.as_str().is_some_and(|s| {
                !s.is_empty() && s.chars().count() <= 200 && !s.chars().any(char::is_control)
            }) || value.as_object().is_some_and(|o| {
                o.keys()
                    .all(|k| ["id", "revision", "kind", "owner", "sha256"].contains(&k.as_str()))
                    && value["id"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty() && s.chars().count() <= 200)
                    && value["revision"].as_u64().is_some_and(|r| r > 0)
                    && ["kind", "owner", "sha256"].iter().all(|k| {
                        value.get(k).is_none_or(|v| {
                            v.as_str().is_some_and(|s| {
                                s.chars().count() <= 200 && !s.chars().any(char::is_control)
                            })
                        })
                    })
            })
        }
        "number" => value.as_f64().is_some_and(f64::is_finite),
        "count" => value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "list" => value.as_array().is_some_and(|a| {
            a.len() <= 32
                && a.iter().all(|v| {
                    v.as_str().is_some_and(|s| s.chars().count() <= 200)
                        || v.as_f64().is_some_and(f64::is_finite)
                        || v.is_boolean()
                })
        }),
        "split" => value.as_object().is_some_and(|o| {
            o.len() <= 24
                && o.keys().all(|k| k.chars().count() <= 64)
                && o.values().all(|v| {
                    v.as_u64().is_some()
                        || matches!(v.as_str(), Some("unrecorded" | "not_applicable"))
                })
        }),
        _ => false,
    }
}
pub fn validate(template: &Value, research: &Value, kind: &str) -> Result<()> {
    require(
        template["output_kind"] == kind,
        "metadata_kind",
        "Template does not cover this output kind",
        400,
    )?;
    require(
        serde_json::to_vec(research)?.len() <= 128 * 1024,
        "metadata_size",
        "Research metadata exceeds 128 KiB",
        400,
    )?;
    contracts::validate(&template["definition"]["schema"], research)?;
    if let Some(organization) = research.get("organization") {
        require(
            kind == "dataset",
            "metadata_kind",
            "Only Dataset metadata can declare dataset organization",
            400,
        )?;
        crate::dataset_organization::validate_declaration(organization)?;
    }
    for field in template["definition"]["fields"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(v) = research.pointer(field["pointer"].as_str().unwrap_or("")) {
            require(
                field_value(v, field["format"].as_str().unwrap_or("")),
                "metadata_field",
                "Display field value does not match its declared scalar/list format",
                400,
            )?;
            if let Some(unit) = field["unit_pointer"]
                .as_str()
                .and_then(|p| research.pointer(p))
            {
                require(
                    unit.as_str().is_some_and(|s| {
                        !s.is_empty() && s.chars().count() <= 64 && !s.chars().any(char::is_control)
                    }),
                    "metadata_unit",
                    "A display unit must be a bounded string",
                    400,
                )?;
            }
        }
    }
    if let Some(extra) = research["extensions"].as_object() {
        for key in extra.keys() {
            let pointer = format!("/extensions/{}", key.replace('~', "~0").replace('/', "~1"));
            require(
                template["definition"]["fields"]
                    .as_array()
                    .is_some_and(|fields| {
                        fields.iter().any(|f| {
                            f["pointer"].as_str().is_some_and(|p| {
                                p == pointer || p.starts_with(&(pointer.clone() + "/"))
                            })
                        })
                    }),
                "metadata_extension",
                "Extension fields require a registered display descriptor",
                400,
            )?;
        }
    }
    if let Some(e) = research.get("evaluation") {
        if let (Some(successes), Some(valid)) = (e["successes"].as_u64(), e["valid"].as_u64()) {
            require(
                successes <= valid,
                "metadata_population",
                "Successes exceed the valid population",
                400,
            )?;
        }
        if let (Some(valid), Some(planned)) = (e["valid"].as_u64(), e["planned"].as_u64()) {
            require(
                valid <= planned,
                "metadata_population",
                "Valid population exceeds planned population",
                400,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn require_dataset_split(research: &Value) -> Result<()> {
    require(
        research.pointer("/collection/split").is_some_and(|split| {
            split.as_object().is_some_and(|counts| !counts.is_empty()) && field_value(split, "split")
        }),
        "dataset_split_required",
        "Declare metadata.research.collection.split with episode counts or explicit unrecorded/not_applicable entries",
        400,
    )
}
pub fn bind_profile(db: &Connection, profile: &mut Value) -> Result<()> {
    let requested = profile
        .get("metadata_templates")
        .cloned()
        .unwrap_or(json!({}));
    require(
        requested.is_object(),
        "metadata_template",
        "Profile metadata_templates must be a selector map",
        400,
    )?;
    let mut pins = serde_json::Map::new();
    for (selector, id) in [
        ("dataset", "loom.dataset"),
        ("model", "loom.model"),
        ("experiment", "loom.experiment.evaluation"),
    ] {
        let template = if let Some(pin) = requested.get(selector) {
            resolve(db, pin)?
        } else {
            get(db, id, Some(1))?
        };
        require(
            template["output_kind"] == selector,
            "metadata_kind",
            "Profile template selector has a different output kind",
            400,
        )?;
        pins.insert(selector.into(), template);
    }
    for (selector, pin) in requested.as_object().unwrap() {
        if pins.contains_key(selector) {
            continue;
        }
        require(
            selector
                .strip_prefix("output:")
                .is_some_and(|s| !s.is_empty() && s.len() <= 128),
            "metadata_template",
            "Template selectors are output kinds or output:<local-id>",
            400,
        )?;
        pins.insert(selector.clone(), resolve(db, pin)?);
    }
    profile["metadata_templates"] = requested;
    profile["metadata_template_pins"] = json!(pins);
    profile["metadata_policy"] = json!(POLICY);
    Ok(())
}
pub fn validate_output(profile: &Value, entity: &Value) -> Result<Option<Value>> {
    if profile["metadata_policy"] != POLICY {
        return Ok(None);
    }
    let kind = entity["kind"].as_str().unwrap_or("");
    if !["dataset", "model", "experiment"].contains(&kind) {
        return Ok(None);
    }
    require(
        entity["label"].as_str().is_some_and(|s| {
            !s.trim().is_empty()
                && !s.chars().any(char::is_control)
                && robot_prefix(&entity["metadata"]["research"], s)
                    .chars()
                    .count()
                    <= 200
        }),
        "output_label",
        "Template-backed scientific outputs require a concise printable label",
        400,
    )?;
    let selector = format!("output:{}", entity["id"].as_str().unwrap_or(""));
    let template = profile["metadata_template_pins"]
        .get(&selector)
        .or_else(|| profile["metadata_template_pins"].get(kind))
        .ok_or_else(|| Error::input("Frozen Profile has no metadata template for the output"))?;
    if let Some(pin) = entity["metadata"].get("template_ref") {
        require(
            *pin == reference(template),
            "metadata_template_changed",
            "Output template differs from its frozen Profile pin",
            409,
        )?;
    }
    validate(template, &entity["metadata"]["research"], kind)?;
    Ok(Some(reference(template)))
}
pub fn summary(template: &Value, research: &Value, source: Value) -> Value {
    let prefix = if source["kind"] == "record" {
        "/metadata/research"
    } else {
        "/research"
    };
    let mut facts=template["definition"]["fields"].as_array().into_iter().flatten().filter_map(|field| {
        let value=research.pointer(field["pointer"].as_str()?)?;
        if !field_value(value,field["format"].as_str().unwrap_or("")){return None;}
        let unit=field.get("unit").cloned().or_else(||field["unit_pointer"].as_str().and_then(|p|research.pointer(p).cloned()));
        let display=if field["format"]=="reference"&&value.is_object(){json!(format!("{}{} · r{}",value["owner"].as_str().map(|s|format!("{s}/")).unwrap_or_default(),value["id"].as_str().unwrap_or(""),value["revision"]))}else{value.clone()};
        Some(json!({"id":field["id"],"section":field["section"],"label":field["label"],"value":display,"unit":unit,"status":"recorded","evidence":[{"source":source,"pointer":format!("{prefix}{}",field["pointer"].as_str().unwrap())}]}))
    }).collect::<Vec<_>>();
    if !facts.iter().any(|f| f["id"] == "robot") {
        let status = research["robot"]["status"].as_str().unwrap_or("unrecorded");
        let value = match status {
            "multiple" => research["robot"]["models"].clone(),
            "not_applicable" => json!("N/A"),
            _ => json!("Unrecorded"),
        };
        facts.insert(0,json!({"id":"robot","section":"context","label":"Robot","value":value,"unit":null,"status":status,"evidence":[{"source":source,"pointer":format!("{prefix}/robot")}]}));
    }
    json!({"schema":"loom-research-summary/v1","kind":template["output_kind"],"facts":facts,"inputs":research.get("inputs").cloned().unwrap_or(json!([])),"sources":[],"template_ref":reference(template),"robot":research["robot"]})
}
pub fn robot_prefix(research: &Value, label: &str) -> String {
    let Some(model) = research["robot"]["model"]
        .as_str()
        .filter(|_| research["robot"]["status"] == "recorded")
    else {
        return label.into();
    };
    if label == model || label.starts_with(&format!("{model} · ")) {
        label.into()
    } else {
        format!("{model} · {label}")
    }
}
pub fn cache_builtins(db: &Connection) -> Result<()> {
    for value in builtins() {
        db.execute(
            "INSERT OR IGNORE INTO catalog_sources VALUES('metadata_template',?,?)",
            params![
                encode(&json!({"id":value["id"],"revision":value["revision"]}))?,
                encode(&value)?
            ],
        )?;
    }
    Ok(())
}
pub fn source_record(db: &Connection, target: &contracts::Target) -> Result<(String, String)> {
    let i = &target.identity;
    Ok(match target.kind.as_str(){
        "entity"=>db.query_row("SELECT manifest,kind FROM all_entity_revisions WHERE id=? AND revision=?",params![i["id"].as_str(),i["revision"].as_i64()],|r|Ok((r.get(0)?,r.get(1)?)))?,
        "historical"=>db.query_row("SELECT body,kind FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",params![i["source"].as_str(),i["kind"].as_str(),i["id"].as_str(),i["revision"].as_str(),i["snapshot"].as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?,
        _=>return Err(Error::input("Exact entity or historical metadata target required"))
    })
}
pub fn annotation(db: &Connection, target: &contracts::Target) -> Result<Option<Value>> {
    let text: Option<String> = db
        .query_row(
            "SELECT body FROM rust_annotations WHERE kind='output_metadata' AND id=?",
            [research_metadata::snapshot_key(target)?],
            |r| r.get(0),
        )
        .optional()?;
    let Some(text) = text else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(&text)?;
    let (source, _) = source_record(db, target)?;
    Ok(
        (value["target"] == json!(target) && value["source_sha256"] == hash(source))
            .then_some(value),
    )
}
pub fn attach(app: &Application, entity: &mut Value) -> Result<()> {
    let Some(target) = research_metadata::snapshot_target(entity) else {
        return Ok(());
    };
    let kind = entity["kind"].as_str().unwrap_or("");
    let kind = if kind == "run" { "experiment" } else { kind };
    if !["dataset", "model", "experiment"].contains(&kind) {
        return Ok(());
    }
    let (source, _) = source_record(&app.db, &target)?;
    let raw: Value = serde_json::from_str(&source)?;
    let adopted = annotation(&app.db, &target)?;
    let (template, research, status) = if let Some(v) = adopted.as_ref() {
        (v["template"].clone(), v["research"].clone(), "adopted")
    } else if raw["metadata_template"].is_object() {
        (
            resolve(&app.db, &raw["metadata_template"])?,
            raw["metadata"]["research"].clone(),
            "validated",
        )
    } else {
        entity["metadata_status"] = json!({"state":"legacy"});
        return Ok(());
    };
    validate(&template, &research, kind)?;
    entity["research_summary"] = summary(
        &template,
        &research,
        json!({"kind":if status=="adopted"{"operator_annotation"}else{"record"},"target":target,"source_sha256":hash(source)}),
    );
    entity["metadata_status"] = json!({"state":status,"template_ref":reference(&template),"scientific_acceptance":"unverified"});
    if let Some(v) = adopted {
        entity["metadata_provenance"] = json!({"target":v["target"],"source_sha256":v["source_sha256"],"declaration_level":v["declaration_level"],"actor":v["actor"],"reason":v["reason"],"format_reference":v["format_reference"],"evidence":v["evidence"],"updated":v["updated"]});
    }
    entity["robot"] = research["robot"].clone();
    if let Some(label) = entity["label"].as_str() {
        entity["label"] = json!(robot_prefix(&research, label));
    }
    Ok(())
}
fn listing(db: &Connection, args: &Value) -> Result<Value> {
    let kind = args["kind"].as_str();
    let history = args["include_revisions"] == true;
    let offset = args["offset"].as_u64().unwrap_or(0).min(i64::MAX as u64) as i64;
    let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 100) as i64;
    let mut builtin = builtins();
    builtin.retain(|v| kind.is_none_or(|k| v["output_kind"] == k));
    builtin.retain(|_| {
        args["lifecycle_state"]
            .as_str()
            .is_none_or(|s| s == "active")
    });
    builtin.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let prefix=format!("WITH ranked AS (SELECT body,json_extract(identity,'$.id') id,json_extract(identity,'$.revision') revision,row_number() OVER(PARTITION BY json_extract(identity,'$.id') ORDER BY json_extract(identity,'$.revision') DESC) rank FROM catalog_sources WHERE kind='metadata_template' AND json_extract(identity,'$.id') NOT LIKE 'loom.%'), selected AS (SELECT * FROM ranked WHERE (? OR rank=1) AND (? IS NULL OR json_extract(body,'$.output_kind')=? ) AND ({}) ) ",crate::registry_lifecycle::listing_clause(args,&crate::registry_lifecycle::record_state_sql("metadata_template","ranked.id","ranked.revision")));
    let count: i64 = db.query_row(
        &(prefix.clone() + "SELECT count(*) FROM selected"),
        params![history, kind, kind],
        |r| r.get(0),
    )?;
    let total = builtin.len() as i64 + count;
    let mut items = builtin
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .cloned()
        .collect::<Vec<_>>();
    let remaining = limit - items.len() as i64;
    if remaining > 0 {
        let skip = (offset - builtin.len() as i64).max(0);
        for row in rows(
            db,
            &(prefix.clone()
                + "SELECT body FROM selected ORDER BY id,revision DESC LIMIT ? OFFSET ?"),
            &[&history, &kind, &kind, &remaining, &skip],
        )? {
            items.push(serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?);
        }
    }
    for item in &mut items {
        item.as_object_mut().unwrap().remove("definition");
    }
    Ok(
        json!({"items":items,"total":total,"next_offset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit))}),
    )
}
impl Application {
    pub fn metadata_template_action(
        &mut self,
        actor: &Actor,
        action: &str,
        args: Value,
    ) -> Result<Value> {
        match action {
            "metadata_templates" => listing(&self.db, &args),
            "get_metadata_template" => get(
                &self.db,
                args["id"].as_str().unwrap_or(""),
                args["revision"].as_u64(),
            ),
            "validate_metadata" => {
                let template = resolve(&self.db, &args["template_ref"])?;
                validate(
                    &template,
                    &args["research"],
                    args["kind"].as_str().unwrap_or(""),
                )?;
                if args["kind"] == "dataset" {
                    require_dataset_split(&args["research"])?;
                }
                Ok(
                    json!({"valid":true,"template_ref":reference(&template),"summary":summary(&template,&args["research"],json!({"kind":"draft"})),"scientific_acceptance":"unverified"}),
                )
            }
            "register_metadata_template" => {
                let id = args["id"].as_str().unwrap_or("");
                require(
                    !id.starts_with("loom.")
                        && !id.is_empty()
                        && id.len() <= 96
                        && id
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)),
                    "metadata_template",
                    "Choose a non-reserved template ID",
                    400,
                )?;
                let definition = definition(&self.db, &args["definition"])?;
                let tx = self.transaction()?;
                let key = args["key"]
                    .as_str()
                    .ok_or_else(|| Error::input("Request key required"))?;
                if let Some(v) = replay(&tx, actor, key, "metadata.template.register", &args)? {
                    return Ok(v);
                }
                active(&tx)?;
                let old:Option<String>=tx.query_row("SELECT body FROM catalog_sources WHERE kind='metadata_template' AND json_extract(identity,'$.id')=? ORDER BY json_extract(identity,'$.revision') DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
                let previous = old
                    .as_ref()
                    .map(|s| serde_json::from_str::<Value>(s))
                    .transpose()?;
                let revision = previous
                    .as_ref()
                    .and_then(|v| v["revision"].as_u64())
                    .unwrap_or(0);
                require(
                    args["expected_revision"].as_u64().unwrap_or(0) == revision,
                    "metadata_template_revision",
                    "Template revision changed; inspect it before revising",
                    409,
                )?;
                if let Some(old) = &previous {
                    require(
                        old["output_kind"] == args["definition"]["kind"],
                        "metadata_kind",
                        "A template family cannot change output kind",
                        409,
                    )?;
                    require(
                        old["owner"] == actor.id || actor.scopes.iter().any(|s| s == "admin"),
                        "metadata_template_owner",
                        "Only the template author or an administrator may revise its definition",
                        403,
                    )?;
                }
                let owner = previous
                    .as_ref()
                    .and_then(|v| v["owner"].as_str())
                    .unwrap_or(&actor.id);
                let value = record(id, revision + 1, definition, owner);
                tx.execute(
                    "INSERT INTO catalog_sources VALUES('metadata_template',?,?)",
                    params![
                        encode(&json!({"id":id,"revision":revision+1}))?,
                        encode(&value)?
                    ],
                )?;
                receipt(&tx, actor, key, "metadata.template.register", &args, &value)?;
                tx.commit()?;
                Ok(value)
            }
            "adopt_metadata" => self.adopt_output_metadata(actor, args),
            _ => Err(Error::input("Unknown metadata template action")),
        }
    }
    fn adopt_output_metadata(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let target: contracts::Target = contracts::decode(args["target"].clone())?;
        let (sha, kind) = research_metadata::source_identity(self, &target)?;
        let kind = if kind == "run" { "experiment" } else { &kind };
        let template = resolve(&self.db, &args["template_ref"])?;
        validate(&template, &args["research"], kind)?;
        require(
            serde_json::to_vec(&args["evidence"])?.len() <= 64 * 1024,
            "metadata_evidence",
            "Field evidence exceeds 64 KiB",
            400,
        )?;
        if let Some(reference) = args.get("format_reference") {
            let reference: contracts::Target = contracts::decode(reference.clone())?;
            let (_, reference_kind) = research_metadata::source_identity(self, &reference)?;
            let reference_kind = if reference_kind == "run" {
                "experiment"
            } else {
                &reference_kind
            };
            require(
                reference_kind == kind,
                "metadata_kind",
                "Comparable format reference must have the same output kind",
                400,
            )?;
        }
        let key = research_metadata::snapshot_key(&target)?;
        let previous: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM rust_annotations WHERE kind='output_metadata' AND id=?",
                [&key],
                |r| r.get(0),
            )
            .optional()?;
        let expected = hash(encode(&json!([sha, previous]))?);
        let mut result = json!({"target":target,"source_sha256":sha,"expected_digest":expected,"template_ref":reference(&template),"summary":summary(&template,&args["research"],json!({"kind":"operator_annotation","source_sha256":sha})),"dry_run":args["dry_run"]!=false,"stored":false,"scientific_acceptance":"unverified"});
        if args["dry_run"] != false {
            return Ok(result);
        }
        let tx = self.transaction()?;
        let request = args["key"]
            .as_str()
            .ok_or_else(|| Error::input("Request key required"))?;
        if let Some(v) = replay(&tx, actor, request, "metadata.adopt", &args)? {
            return Ok(v);
        }
        active(&tx)?;
        require(
            args["expected_digest"] == expected,
            "metadata_source_changed",
            "Source or previous annotation changed since preview",
            409,
        )?;
        let current: Option<String> = tx
            .query_row(
                "SELECT body FROM rust_annotations WHERE kind='output_metadata' AND id=?",
                [&key],
                |r| r.get(0),
            )
            .optional()?;
        require(
            current == previous && hash(source_record(&tx, &target)?.0) == sha,
            "metadata_source_changed",
            "Source or annotation changed before commit",
            409,
        )?;
        let annotation = json!({"schema":POLICY,"target":target,"source_sha256":sha,"template":template,"research":args["research"],"reason":args["reason"],"format_reference":args["format_reference"],"evidence":args["evidence"],"declaration_level":"operator_asserted","actor":actor.id,"updated":now()});
        tx.execute("INSERT INTO rust_annotations VALUES('output_metadata',?,?,?) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body,updated=excluded.updated",params![key,encode(&annotation)?,now()])?;
        tx.execute(
            "UPDATE rust_projection_version SET version=version+1 WHERE id=1",
            [],
        )?;
        let topic = if target.kind == "historical" {
            "history_records"
        } else if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM asset_revisions WHERE id=? AND revision=?)",
            params![
                target.identity["id"].as_str(),
                target.identity["revision"].as_i64()
            ],
            |r| r.get::<_, bool>(0),
        )? {
            "asset_revisions"
        } else {
            "entity_revisions"
        };
        tx.execute(
            "INSERT INTO catalog_dirty(topic,identity) VALUES(?,?)",
            params![topic, encode(&target.identity)?],
        )?;
        // Preserve the complete prior annotation in the idempotent operation receipt.
        result["stored"] = json!(true);
        result["previous_annotation"] = previous
            .as_ref()
            .map(|s| serde_json::from_str::<Value>(s))
            .transpose()?
            .unwrap_or(Value::Null);
        receipt(&tx, actor, request, "metadata.adopt", &args, &result)?;
        tx.commit()?;
        Ok(result)
    }
}
