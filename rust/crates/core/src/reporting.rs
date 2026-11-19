//! Immutable analysis coverage, prospective adaptations and verified report publication.
use crate::{
    auth::Actor, contracts, error::require, storage, store::*, Application, Error, Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
    sync::{Mutex, OnceLock},
};

pub const POLICY: &str = "loom-reporting/v1";
const MAX_REPORT: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub pointer: String,
    pub format: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub x_pointer: Option<String>,
    #[serde(default)]
    pub y_pointer: Option<String>,
    #[serde(default)]
    pub x_label: Option<String>,
    #[serde(default)]
    pub y_label: Option<String>,
    #[serde(default)]
    pub columns: Vec<Column>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub label: String,
    pub pointer: String,
    pub format: String,
    #[serde(default)]
    pub unit: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub label: String,
    pub role: String,
    #[serde(default = "yes")]
    pub required: bool,
    pub schema: Value,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub definitions: Vec<Value>,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adaptation {
    pub requirement_id: String,
    pub disposition: String,
    pub reason: String,
    pub analysis_impact: String,
    pub evidence: Vec<Value>,
    pub when: Value,
    #[serde(default)]
    pub replacement_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub label: String,
    pub output_kind: String,
    pub activity: String,
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub base_ref: Option<Value>,
    #[serde(default)]
    pub adaptations: Vec<Adaptation>,
}
fn check(ok: bool, message: impl Into<String>) -> Result<()> {
    require(ok, "report_contract", &message.into(), 400)
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn printable(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
pub fn reference(record: &Value) -> Value {
    json!({"id":record["id"],"revision":record["revision"],"sha256":record["sha256"]})
}
fn record(id: &str, revision: u64, definition: Value, owner: &str, kind: &str) -> Value {
    json!({"id":id,"revision":revision,"sha256":hash(crate::compat::seal(&definition)),"kind":kind,"label":definition["label"],"output_kind":definition["output_kind"],"activity":definition["activity"],"owner":owner,"origin":if owner=="builtin"{"builtin"}else{"registered"},"definition":definition,"scope":"research","role":"main","origin_type":if owner=="builtin"{"builtin"}else{"registered"}})
}
fn builtins(kind: &str) -> Vec<Value> {
    let source: Value = serde_json::from_str(include_str!("../../../contracts/reporting.json"))
        .expect("reporting contracts");
    let mut records: Vec<_> = source[if kind == "report_contract" {
        "contracts"
    } else {
        "definitions"
    }]
    .as_array()
    .unwrap()
    .iter()
    .map(|v| {
        let mut d = v["definition"].clone();
        if kind == "report_contract" {
            for r in d["requirements"].as_array_mut().unwrap() {
                for pin in r["definitions"].as_array_mut().unwrap() {
                    let definition = source["definitions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|d| d["id"] == *pin)
                        .unwrap();
                    *pin = reference(&record(
                        definition["id"].as_str().unwrap(),
                        1,
                        definition["definition"].clone(),
                        "builtin",
                        "report_definition",
                    ));
                }
            }
        }
        record(v["id"].as_str().unwrap(), 1, d, "builtin", kind)
    })
    .collect();
    if kind == "report_contract" {
        // Preserve the existing single-checkpoint contract and its exact pins.
        let mut definition = records
            .iter()
            .find(|r| r["id"] == "loom.report.evaluation")
            .expect("base evaluation report")["definition"]
            .clone();
        definition["label"] = json!("Checkpoint evaluation");
        let schema = &mut definition["requirements"][0]["schema"];
        schema["properties"]["checkpoint_selection"] = source["checkpoint_selection"].clone();
        schema["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("checkpoint_selection"));
        records.push(record(
            "loom.report.checkpoint_evaluation",
            1,
            definition,
            "builtin",
            kind,
        ));
    }
    records
}
pub fn get(db: &Connection, kind: &str, id: &str, revision: Option<u64>) -> Result<Value> {
    if let Some(v) = builtins(kind)
        .into_iter()
        .find(|v| v["id"] == id && revision.is_none_or(|r| v["revision"] == r))
    {
        return Ok(v);
    }
    let body: Option<String> = db.query_row("SELECT body FROM catalog_sources WHERE kind=? AND json_extract(identity,'$.id')=? AND (? IS NULL OR json_extract(identity,'$.revision')=?) ORDER BY json_extract(identity,'$.revision') DESC LIMIT 1",params![kind,id,revision.map(|r|r as i64),revision.map(|r|r as i64)],|r|r.get(0)).optional()?;
    body.map(|b| serde_json::from_str(&b).map_err(Error::from))
        .transpose()?
        .ok_or_else(|| {
            Error::new(
                "report_contract_missing",
                "Exact reporting resource not found",
                404,
            )
        })
}
pub fn resolve(db: &Connection, kind: &str, pin: &Value) -> Result<Value> {
    let p: crate::metadata_templates::TemplateRef = contracts::decode(pin.clone())?;
    let value = get(db, kind, &p.id, Some(p.revision))?;
    require(
        value["sha256"] == p.sha256,
        "report_contract_changed",
        "Reporting revision digest differs",
        409,
    )?;
    Ok(value)
}
pub fn cache_builtins(db: &Connection) -> Result<()> {
    for kind in ["report_contract", "report_definition"] {
        for v in builtins(kind) {
            db.execute(
                "INSERT OR IGNORE INTO catalog_sources VALUES(?,?,?)",
                params![
                    kind,
                    encode(&json!({"id":v["id"],"revision":v["revision"]}))?,
                    encode(&v)?
                ],
            )?;
        }
    }
    Ok(())
}
fn source_body(db: &Connection, target: &contracts::Target) -> Result<String> {
    let keys: &[&str] = match target.kind.as_str() {
        "entity" => &["id", "revision"],
        "historical" => &["source", "kind", "id", "revision", "snapshot"],
        "component" | "tool" | "profile" => &["owner", "id", "revision"],
        _ => {
            return Err(Error::input(
                "Use exact record or registered software adaptation evidence",
            ))
        }
    };
    check(
        target
            .identity
            .as_object()
            .is_some_and(|o| o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k))),
        "Adaptation evidence needs an exact source identity",
    )?;
    Ok(
        if matches!(target.kind.as_str(), "component" | "tool" | "profile") {
            let (table, column) = match target.kind.as_str() {
                "component" => ("components", "manifest"),
                "tool" => ("tools", "manifest"),
                _ => ("execution_profiles", "profile"),
            };
            db.query_row(
                &format!("SELECT {column} FROM {table} WHERE owner=? AND id=? AND revision=?"),
                params![
                    target.identity["owner"].as_str(),
                    target.identity["id"].as_str(),
                    target.identity["revision"].as_i64()
                ],
                |r| r.get::<_, String>(0),
            )?
        } else {
            crate::metadata_templates::source_record(db, target)?.0
        },
    )
}
fn source(db: &Connection, evidence: &Value) -> Result<Value> {
    let target: contracts::Target = contracts::decode(evidence["target"].clone())?;
    let body = source_body(db, &target)?;
    require(
        evidence["sha256"] == hash(&body),
        "report_evidence_changed",
        "Adaptation evidence source digest differs",
        409,
    )?;
    let body: Value = serde_json::from_str(&body)?;
    let pointer = evidence["pointer"]
        .as_str()
        .ok_or_else(|| Error::input("Evidence JSON pointer required"))?;
    let value = body
        .pointer(pointer)
        .ok_or_else(|| Error::input("Adaptation evidence pointer not found"))?;
    check(
        evidence.get("value") == Some(value),
        "Adaptation evidence value differs from source",
    )?;
    Ok(value.clone())
}
fn predicate(value: &Value) -> Result<()> {
    check(
        value
            .as_object()
            .is_some_and(|v| v.len() == 2 && v.contains_key("pointer") && v.contains_key("equals"))
            && value["pointer"].as_str().is_some_and(|p| {
                p.len() <= 256 && (p.starts_with("/params/") || p.starts_with("/profile/"))
            }),
        "Adaptation requires a bounded params/profile equality predicate",
    )
}
fn verify_evidence(db: &Connection, a: &Adaptation) -> Result<()> {
    check(
        printable(&a.reason, 2000)
            && printable(&a.analysis_impact, 2000)
            && !a.evidence.is_empty()
            && a.evidence.len() <= 16,
        "Adaptation needs reason, analysis impact and bound evidence",
    )?;
    predicate(&a.when)?;
    for e in &a.evidence {
        source(db, e)?;
    }
    Ok(())
}
fn validate_definition(db: &Connection, value: &Value) -> Result<Value> {
    check(
        serde_json::to_vec(value)?.len() <= 128 * 1024,
        "Contract exceeds 128 KiB",
    )?;
    let mut d: Definition = contracts::decode(value.clone())?;
    check(printable(&d.label, 160), "Contract label required")?;
    check(matches!((d.output_kind.as_str(),d.activity.as_str()),("dataset","sdg"|"dataset_import")|("model","training"|"policy_import")|("experiment","evaluation")),"Choose a Dataset, Policy or evaluation activity; training reports/comparisons are not Experiments")?;
    check(
        d.requirements.len() <= 48 && d.adaptations.len() <= 32,
        "Coverage exceeds contract bounds",
    )?;
    let mut requirements = BTreeMap::new();
    let mut lineage = vec![];
    if let Some(base) = &d.base_ref {
        let base = resolve(db, "report_contract", base)?;
        check(
            base["output_kind"] == d.output_kind && base["activity"] == d.activity,
            "Base activity/output kind differs",
        )?;
        let chain = base["definition"]["lineage"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        check(chain.len() < 12, "Contract ancestry exceeds 12 revisions")?;
        lineage.extend(chain);
        lineage.push(reference(&base));
        for r in effective(&base) {
            requirements.insert(r.id.clone(), r);
        }
    } else {
        check(
            d.adaptations.is_empty(),
            "Adaptations require an exact base contract",
        )?;
    }
    if d.base_ref.is_none() {
        let minimum: &[&str] = match d.activity.as_str() {
            "sdg" => &["dataset_statistics", "sdg_conditions", "sdg_readback"],
            "dataset_import" => &["dataset_statistics"],
            "training" => &["instructions", "policy_inputs", "learning_series"],
            "policy_import" => &["instructions", "policy_inputs"],
            _ => &["evaluation_cases"],
        };
        for role in minimum {
            check(d.requirements.iter().any(|r|r.role==*role&&r.required),format!("Root contract lacks minimum coverage {role}; derive with evidence to exclude it"))?;
        }
    }
    let mut added = BTreeSet::new();
    for r in &d.requirements {
        check(
            identifier(&r.id) && added.insert(r.id.clone()) && !requirements.contains_key(&r.id),
            "Requirement ID is invalid, duplicated or shadows inherited coverage",
        )?;
        check(
            printable(&r.label, 160)
                && printable(&r.role, 80)
                && r.fields.len() <= 32
                && r.definitions.len() <= 16,
            "Invalid report descriptor",
        )?;
        crate::metadata_templates::bounded_schema(&r.schema, 0)?;
        contracts::check_schema(&r.schema)?;
        check(
            r.schema.get("type") == Some(&json!("object")),
            "Report payload schema must describe an object",
        )?;
        let mut fields = BTreeSet::new();
        for f in &r.fields {
            check(
                identifier(&f.id)
                    && fields.insert(&f.id)
                    && printable(&f.label, 100)
                    && f.pointer.starts_with('/')
                    && f.pointer.len() <= 256
                    && f.unit.len() <= 32
                    && matches!(
                        f.format.as_str(),
                        "text"
                            | "number"
                            | "count"
                            | "boolean"
                            | "table"
                            | "series"
                            | "scatter"
                            | "bar"
                            | "coverage_matrix"
                            | "reference"
                            | "instructions"
                            | "properties"
                    ),
                "Invalid typed report display field",
            )?;
            check(
                [&f.x_label, &f.y_label]
                    .iter()
                    .all(|s| s.as_ref().is_none_or(|s| printable(s, 100))),
                "Invalid chart axis label",
            )?;
            if matches!(f.format.as_str(), "series" | "scatter" | "bar") {
                check(
                    [&f.x_pointer, &f.y_pointer].iter().all(|p| {
                        p.as_ref()
                            .is_some_and(|p| p.starts_with('/') && p.len() <= 256)
                    }),
                    "Chart fields require explicit row-relative X/Y pointers",
                )?;
            }
            check(
                f.columns.len() <= 16
                    && f.columns.iter().all(|c| {
                        printable(&c.label, 100)
                            && c.pointer.starts_with('/')
                            && c.pointer.len() <= 256
                            && c.unit.len() <= 32
                            && matches!(
                                c.format.as_str(),
                                "text" | "number" | "count" | "boolean" | "reference"
                            )
                    }),
                "Invalid report table columns",
            )?;
        }
        for pin in &r.definitions {
            resolve(db, "report_definition", pin)?;
        }
        requirements.insert(r.id.clone(), r.clone());
    }
    let mut changed = BTreeSet::new();
    let mut exclusions = if let Some(base) = &d.base_ref {
        resolve(db, "report_contract", base)?["definition"]["excluded_requirements"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        vec![]
    };
    for a in &d.adaptations {
        check(
            changed.insert(a.requirement_id.clone()) && !added.contains(&a.requirement_id),
            "Repeated adaptation or non-inherited requirement",
        )?;
        verify_evidence(db, a)?;
        let removed = requirements
            .remove(&a.requirement_id)
            .ok_or_else(|| Error::input("Adapted requirement not found in base coverage"))?;
        if a.disposition == "excluded" {
            exclusions.push(json!({"id":removed.id,"label":removed.label,"role":removed.role,"reason":a.reason,"analysis_impact":a.analysis_impact,"source_contract_ref":d.base_ref}));
        }
        match a.disposition.as_str() {
            "excluded" => check(
                a.replacement_id.is_none(),
                "Excluded requirement cannot name a replacement",
            )?,
            "replaced" => check(
                a.replacement_id
                    .as_ref()
                    .is_some_and(|id| added.contains(id)),
                "Replacement must name a newly declared requirement",
            )?,
            _ => {
                return Err(Error::input(
                    "Adaptation disposition must be excluded or replaced",
                ))
            }
        }
    }
    check(
        !requirements.is_empty(),
        "An analysis contract needs effective coverage",
    )?;
    // Retain inherited exclusion proofs: a derived revision cannot launder its base's decisions.
    let mut inherited = if let Some(base) = &d.base_ref {
        resolve(db, "report_contract", base)?["definition"]["resolved_adaptations"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        vec![]
    };
    inherited.extend(d.adaptations.iter().map(|a| json!(a)));
    d.requirements.shrink_to_fit();
    let mut value = serde_json::to_value(d)?;
    value["effective_requirements"] = json!(requirements.values().collect::<Vec<_>>());
    let mut pins = vec![];
    for r in requirements.values() {
        for pin in &r.definitions {
            let definition = resolve(db, "report_definition", pin)?;
            if !pins.contains(&definition) {
                pins.push(definition);
            }
        }
    }
    value["definition_pins"] = json!(pins);
    value["resolved_adaptations"] = json!(inherited);
    value["excluded_requirements"] = json!(exclusions);
    value["lineage"] = json!(lineage);
    check(
        requirements.len() <= 48 && serde_json::to_vec(&value)?.len() <= 512 * 1024,
        "Resolved reporting coverage exceeds bounds",
    )?;
    Ok(value)
}

fn definition_resource(db: &Connection, value: &Value) -> Result<Value> {
    check(
        value.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "kind",
                    "label",
                    "quantity",
                    "unit",
                    "population",
                    "reduction",
                    "direction",
                    "dimensions",
                    "mode",
                    "environment",
                    "controller",
                    "primary_metric_ref",
                    "predicate",
                    "frame",
                    "scope",
                ]
                .contains(&k.as_str())
            })
        }) && serde_json::to_vec(value)?.len() <= 32 * 1024,
        "Invalid or oversized semantic definition",
    )?;
    for k in ["kind", "label"] {
        check(
            value[k].as_str().is_some_and(|s| printable(s, 160)),
            format!("Definition {k} required"),
        )?;
    }
    match value["kind"].as_str().unwrap_or("") {
        "metric" => {
            for k in ["quantity", "unit", "population", "reduction", "direction"] {
                check(
                    value[k].as_str().is_some_and(|s| printable(s, 256)),
                    format!("Metric {k} required"),
                )?;
            }
            check(
                matches!(
                    value["direction"].as_str(),
                    Some("minimize" | "maximize" | "neutral")
                ),
                "Invalid metric direction",
            )?;
        }
        "factor" => {
            for k in ["quantity", "unit", "frame"] {
                check(
                    value[k].as_str().is_some_and(|s| printable(s, 256)),
                    format!("Factor {k} required"),
                )?;
            }
        }
        "protocol" => {
            contracts::validate(
                &json!({"type":"object","required":["evaluator","phases","gt_access","policy_active_during_hold","physics_hz","control_hz","goal_frame"],"additionalProperties":false,"properties":{"evaluator":{"oneOf":[{"const":{"kind":"producer"}},{"type":"object","required":["kind","alias","ref"],"properties":{"kind":{"const":"tool"},"alias":{"type":"string","minLength":1,"maxLength":96},"ref":{"type":"object","required":["owner","id","revision","sha256"]}},"additionalProperties":false}]},"phases":{"type":"array","maxItems":64,"items":{"type":"object","required":["id","controller"],"properties":{"id":{"type":"string","minLength":1,"maxLength":96},"controller":{"enum":["policy","planner","hybrid"]}},"additionalProperties":false}},"gt_access":{"type":"object","required":["policy","planner","scoring"],"properties":{"policy":{"type":"boolean"},"planner":{"type":"boolean"},"scoring":{"type":"boolean"}},"additionalProperties":false},"policy_active_during_hold":{"type":["boolean","null"]},"physics_hz":{"type":["number","null"],"exclusiveMinimum":0},"control_hz":{"type":["number","null"],"exclusiveMinimum":0},"goal_frame":{"type":"string","minLength":1,"maxLength":160}}}),
                &value["scope"],
            )?;
            check(
                matches!(value["mode"].as_str(), Some("offline" | "closed_loop"))
                    && matches!(value["environment"].as_str(), Some("sim" | "real"))
                    && matches!(
                        value["controller"].as_str(),
                        Some("policy" | "hybrid" | "planner")
                    ),
                "Protocol needs typed mode, environment and controller",
            )?;
            let primary = resolve(db, "report_definition", &value["primary_metric_ref"])?;
            check(
                primary["definition"]["kind"] == "metric"
                    && value["predicate"]
                        .as_object()
                        .is_some_and(|o| !o.is_empty()),
                "Protocol needs registered primary metric and explicit predicate",
            )?;
            let rules = value["predicate"]["all"].as_array().ok_or_else(|| {
                Error::input("Protocol predicate requires all[] measurement rules")
            })?;
            check(
                !rules.is_empty() && rules.len() <= 32,
                "Protocol needs 1..32 success rules",
            )?;
            for rule in rules {
                check(
                    rule["pointer"]
                        .as_str()
                        .is_some_and(|p| p.starts_with("/measurements/") && p.len() <= 256)
                        && matches!(rule["op"].as_str(), Some("lte" | "gte" | "between"))
                        && rule["unit"].as_str().is_some_and(|s| printable(s, 32)),
                    "Protocol success rule needs a measurement pointer, comparison and unit",
                )?;
                check(
                    if rule["op"] == "between" {
                        rule["min"].as_f64().is_some_and(f64::is_finite)
                            && rule["max"].as_f64().is_some_and(|max| {
                                max.is_finite() && rule["min"].as_f64().unwrap() <= max
                            })
                    } else {
                        rule["value"].as_f64().is_some_and(f64::is_finite)
                    },
                    "Protocol threshold must be finite and ordered",
                )?;
            }
        }
        _ => {
            return Err(Error::input(
                "Definition kind must be metric, factor or protocol",
            ))
        }
    }
    Ok(value.clone())
}

pub(crate) fn effective(contract: &Value) -> Vec<Requirement> {
    contract["definition"]["effective_requirements"]
        .as_array()
        .or_else(|| contract["definition"]["requirements"].as_array())
        .into_iter()
        .flatten()
        .map(|r| serde_json::from_value(r.clone()).expect("validated report requirement"))
        .collect()
}
fn extensible_shape(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if k == "additionalProperties" && v == false {
                            json!(true)
                        } else {
                            extensible_shape(v)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(rows) => Value::Array(rows.iter().map(extensible_shape).collect()),
        _ => value.clone(),
    }
}
fn resolve_from_pins(contract: &Value, requirement: &Requirement, pin: &Value) -> Result<Value> {
    check(
        requirement.definitions.contains(pin),
        "Semantic definition not selected by this report requirement",
    )?;
    contract["definition"]["definition_pins"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .chain(builtins("report_definition"))
        .find(|value| reference(value) == *pin)
        .ok_or_else(|| Error::input("Frozen semantic definition not found"))
}
pub fn bind_profile(db: &Connection, profile: &mut Value) -> Result<()> {
    let outputs = profile["outputs"].as_object().cloned().unwrap_or_default();
    let scientific = outputs
        .values()
        .any(|v| matches!(v.as_str(), Some("dataset" | "model" | "experiment")));
    if !scientific && profile.get("reporting").is_none() {
        profile["reporting_pins"] = json!({});
        profile["reporting_policy"] = json!(POLICY);
        return Ok(());
    }
    let reporting = &profile["reporting"];
    check(
        reporting.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "contracts",
                    "capabilities",
                    "input_kinds",
                    "conditioning",
                    "expectations",
                ]
                .contains(&k.as_str())
            })
        }) && reporting["contracts"].is_object()
            && reporting["capabilities"].is_array(),
        "New scientific Profiles require reporting.contracts and reporter capabilities",
    )?;
    let mut pins = serde_json::Map::new();
    for (selector, pin) in reporting["contracts"].as_object().unwrap() {
        check(
            matches!(selector.as_str(), "dataset" | "model" | "experiment")
                || selector.strip_prefix("output:").is_some_and(identifier),
            "Invalid reporting selector",
        )?;
        let contract = resolve(db, "report_contract", pin)?;
        let kind = contract["definition"]["output_kind"].as_str().unwrap_or("");
        if kind == "model" {
            check(
                matches!(
                    reporting["conditioning"].as_str(),
                    Some("none" | "language")
                ),
                "Policy Profiles declare reporting.conditioning as none or language",
            )?;
        }
        let input_kinds = reporting["input_kinds"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let needed: &[&str] = match contract["definition"]["activity"].as_str() {
            Some("training") => &["dataset"],
            Some("evaluation") => &["model", "dataset"],
            _ => &[],
        };
        for needed in needed {
            check(
                input_kinds.values().any(|v| v == needed),
                format!("Reporting activity requires an exact {needed} input"),
            )?;
        }
        for (name, kind) in &input_kinds {
            check(
                profile["inputs"].get(name).is_some()
                    && matches!(kind.as_str(), Some("dataset" | "model")),
                "Reporting input_kinds must select declared Dataset/Policy inputs",
            )?;
        }
        check(
            selector == kind
                || selector
                    .strip_prefix("output:")
                    .is_some_and(|id| outputs.get(id) == Some(&json!(kind))),
            "Reporting selector differs from declared output",
        )?;
        for r in effective(&contract).iter().filter(|r| r.required) {
            check(
                reporting["capabilities"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(r.role)),
                format!("Reporter capability missing: {}", r.role),
            )?;
            if r.role == "evaluation_cases" {
                check(r.definitions.iter().any(|pin|resolve(db,"report_definition",pin).is_ok_and(|d|d["definition"]["kind"]=="protocol")),"Evaluation contract requires a registered protocol; author a task-specific revision from the evaluation template")?;
                for definition in &r.definitions {
                    let protocol = resolve(db, "report_definition", definition)?;
                    if protocol["definition"]["kind"] == "protocol" {
                        let metric = resolve(
                            db,
                            "report_definition",
                            &protocol["definition"]["primary_metric_ref"],
                        )?;
                        check(metric["definition"]["quantity"]=="success_rate"&&metric["definition"]["unit"]=="ratio","The evaluation_cases template uses a valid-case success-rate primary metric; publish continuous diagnostics as separate registered metric reports")?;
                        let evaluator = &protocol["definition"]["scope"]["evaluator"];
                        if evaluator["kind"] == "tool" {
                            let pin = &profile["tool_pins"][evaluator["alias"].as_str().unwrap()];
                            for field in ["owner", "id", "revision", "sha256"] {
                                check(
                                    pin[field] == evaluator["ref"][field],
                                    "Protocol evaluator differs from the accepted Tool pin",
                                )?;
                            }
                        }
                    }
                }
            }
        }
        pins.insert(selector.clone(), contract);
    }
    for (id, kind) in &outputs {
        if matches!(kind.as_str(), Some("dataset" | "model" | "experiment")) {
            check(
                pins.contains_key(&format!("output:{id}"))
                    || pins.contains_key(kind.as_str().unwrap()),
                format!("No report contract for scientific output {id}"),
            )?;
        }
    }
    if let Some(expectations) = reporting.get("expectations") {
        check(
            expectations.is_object() && serde_json::to_vec(expectations)?.len() <= 64 * 1024,
            "Reporting expectations must be a bounded selector map",
        )?;
        for (selector, requirements) in expectations.as_object().unwrap() {
            let contract = pins
                .get(selector)
                .ok_or_else(|| Error::input("Expectation selector has no report contract"))?;
            check(
                requirements.is_object(),
                "Expectations map requirement IDs to pointer bindings",
            )?;
            for (id, bindings) in requirements.as_object().unwrap() {
                check(
                    effective(contract).iter().any(|r| r.id == *id),
                    "Expectation targets an unknown or excluded requirement",
                )?;
                let bindings = bindings.as_array().ok_or_else(|| {
                    Error::input("Expected report pointer bindings must be an array")
                })?;
                check(bindings.len() <= 32, "Too many report pointer bindings")?;
                let mut seen = BTreeSet::new();
                for binding in bindings {
                    check(binding.as_object().is_some_and(|o|o.len()==2&&o.contains_key("pointer")&&(o.contains_key("value")!=o.contains_key("context_pointer")))&&binding["pointer"].as_str().is_some_and(|p|p.starts_with('/')&&p.len()<=256&&seen.insert(p)),"Expected report value needs a unique pointer and literal value or context_pointer")?;
                    if let Some(pointer) = binding.get("context_pointer") {
                        check(
                            pointer.as_str().is_some_and(|p| {
                                p.len() <= 256
                                    && (p.starts_with("/params/") || p.starts_with("/profile/"))
                            }),
                            "Expected report context must be params or the frozen Profile",
                        )?;
                    }
                }
            }
        }
    }
    for (selector, contract) in &pins {
        for r in effective(contract).iter().filter(|r| r.required) {
            let binding = match r.role.as_str() {
                "learning_series" if contract["definition"]["activity"] == "training" => {
                    Some("/schedule")
                }
                "evaluation_cases"
                    if r.schema["required"]
                        .as_array()
                        .is_some_and(|a| a.contains(&json!("checkpoint_selection"))) =>
                {
                    Some("/checkpoint_selection")
                }
                _ => None,
            };
            if let Some(pointer) = binding {
                check(
                    reporting["expectations"][selector][&r.id]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|v| v["pointer"] == pointer)),
                    format!("Declare {pointer} before execution: {}", r.id),
                )?;
            }
        }
    }
    profile["reporting_pins"] = json!(pins);
    profile["reporting_policy"] = json!(POLICY);
    Ok(())
}
pub fn validate_plan(db: &Connection, spec: &Value) -> Result<()> {
    let p = &spec["profile"];
    if p["reporting_policy"] != POLICY {
        return Ok(());
    }
    for (_, requirements) in p["reporting"]["expectations"]
        .as_object()
        .into_iter()
        .flatten()
    {
        for (_, bindings) in requirements.as_object().into_iter().flatten() {
            for b in bindings.as_array().into_iter().flatten() {
                let value = if let Some(pointer) = b["context_pointer"].as_str() {
                    spec.pointer(pointer).ok_or_else(|| {
                        Error::new(
                            "report_contract",
                            "Expected report context pointer is absent from this Job",
                            400,
                        )
                    })?
                } else {
                    &b["value"]
                };
                if b["pointer"] == "/checkpoint_selection" {
                    check(matches!(value["split"].as_str(), Some("development" | "validation" | "test")) && match value["rule"].as_str() {
                        Some("best_success_rate") => value["split"] != "test" && value.get("selected_step").is_some_and(Value::is_null),
                        Some("prespecified") => value["selected_step"].as_u64().is_some(),
                        _ => false,
                    }, "Freeze a prespecified checkpoint or a development/validation success-rate rule before execution")?;
                }
            }
        }
    }
    for (name, kind) in p["reporting"]["input_kinds"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let value = &spec["params"][name];
        let found = if value["artifact"].is_object() {
            db.query_row(
                "SELECT kind FROM all_entity_revisions WHERE id=? AND revision=?",
                params![
                    value["artifact"]["id"].as_str(),
                    value["artifact"]["revision"].as_i64()
                ],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        } else if let (Some(job), Some(output)) = (value["job"].as_str(), value["entity"].as_str())
        {
            let mut upstream = job.to_owned();
            let mut output = output.to_owned();
            if let Some(body) = db
                .query_row("SELECT request FROM job_workflows WHERE id=?", [job], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?
            {
                let request: Value = serde_json::from_str(&body)?;
                let alias = &request["outputs"][&output];
                upstream = db.query_row(
                    "SELECT job FROM workflow_nodes WHERE root=? AND node=?",
                    params![job, alias["node"].as_str()],
                    |r| r.get(0),
                )?;
                output = alias["entity"].as_str().unwrap_or("").into();
            }
            crate::jobs::load(db, &upstream)?.spec["profile"]["outputs"][&output]
                .as_str()
                .map(str::to_owned)
        } else {
            None
        };
        check(
            found.as_ref().is_some_and(|k| json!(k) == *kind),
            format!("Reporting input {name} must pin an exact {kind} revision or upstream output"),
        )?;
    }
    for (_, contract) in p["reporting_pins"].as_object().into_iter().flatten() {
        for a in contract["definition"]["resolved_adaptations"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let a: Adaptation = contracts::decode(a.clone())?;
            verify_evidence(db, &a)?;
            check(
                spec.pointer(a.when["pointer"].as_str().unwrap()) == Some(&a.when["equals"]),
                format!(
                    "Exclusion/replacement does not apply to this Job: {}",
                    a.requirement_id
                ),
            )?;
        }
    }
    Ok(())
}
pub fn validate_new_admission(spec: &Value) -> Result<()> {
    let p = &spec["profile"];
    let scientific = p["outputs"].as_object().is_some_and(|o| {
        o.values()
            .any(|v| matches!(v.as_str(), Some("dataset" | "model" | "experiment")))
    });
    require(!scientific||p["reporting_policy"]==POLICY,"report_profile_legacy","New scientific Jobs require a reporting-ready Profile revision; accepted Jobs retain their contract",409)
}
fn selected<'a>(profile: &'a Value, entity: &Value) -> Option<&'a Value> {
    profile["reporting_pins"]
        .get(format!("output:{}", entity["id"].as_str().unwrap_or("")).as_str())
        .or_else(|| profile["reporting_pins"].get(entity["kind"].as_str().unwrap_or("")))
}
pub fn validate_declaration(profile: &Value, entity: &Value) -> Result<()> {
    if profile["reporting_policy"] != POLICY {
        return Ok(());
    }
    if entity["kind"] == "model" && profile["metadata_policy"] == crate::metadata_templates::POLICY
    {
        require(entity.pointer("/metadata/research/model/role")==Some(&json!("policy")),"output_kind_mismatch","Policy outputs require model.role=policy; publish Observer weights with their Tool/Component evidence",400)?;
    }
    let Some(c) = selected(profile, entity) else {
        return check(
            !matches!(
                entity["kind"].as_str(),
                Some("model" | "dataset" | "experiment")
            ),
            "Scientific output lacks frozen report contract",
        );
    };
    check(
        entity["reporting"]["contract_ref"] == reference(c),
        "Output reporting contract differs from frozen pin",
    )?;
    let bindings = entity["reporting"]["reports"]
        .as_array()
        .ok_or_else(|| Error::input("Report bindings required"))?;
    check(bindings.len() <= 48, "Too many report bindings")?;
    let requirements = effective(c);
    let mut seen = BTreeSet::new();
    for b in bindings {
        let id = b["requirement_id"].as_str().unwrap_or("");
        check(
            b.as_object().is_some_and(|o| {
                o.len() == 3
                    && o.contains_key("requirement_id")
                    && o.contains_key("component")
                    && o.contains_key("sha256")
            }) && seen.insert(id)
                && requirements.iter().any(|r| r.id == id),
            "Unknown, excluded, repeated or malformed report binding",
        )?;
        check(
            entity["components"]
                .as_array()
                .is_some_and(|a| a.contains(&b["component"])),
            "Report must be a registered inline output component",
        )?;
    }
    for r in requirements.iter().filter(|r| r.required) {
        check(
            seen.contains(r.id.as_str()),
            format!("Required report missing: {}", r.id),
        )?;
    }
    Ok(())
}
pub(crate) fn read_json(path: &Path, proof: &storage::VerifiedFile) -> Result<Value> {
    check(
        proof.size <= MAX_REPORT as u64,
        "Report exceeds 8 MiB; use bounded report shards",
    )?;
    let mut file = storage::open_regular(path)?;
    proof.revalidate_open(&file)?;
    let mut bytes = vec![];
    file.by_ref()
        .take(MAX_REPORT as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= MAX_REPORT && hash(&bytes) == proof.sha256,
        "report_changed",
        "Report content differs from verified bytes",
        409,
    )?;
    proof.revalidate_open(&file)?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn validate_reports(
    spec: &Value,
    entity: &Value,
    producer: (&str, &str),
    folder: &Path,
    components: &[Value],
    proofs: &[storage::VerifiedFile],
    relations: &[Value],
) -> Result<Option<Value>> {
    let (job, attempt) = producer;
    let profile = &spec["profile"];
    if profile["reporting_policy"] != POLICY {
        return Ok(None);
    }
    let Some(contract) = selected(profile, entity) else {
        return Ok(None);
    };
    validate_declaration(profile, entity)?;
    let expected_inputs = relations
        .iter()
        .filter(|r| r["kind"] == "consumes")
        .cloned()
        .collect::<Vec<_>>();
    let mut reports = vec![];
    let mut report_data = BTreeMap::new();
    for b in entity["reporting"]["reports"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let r = effective(contract)
            .into_iter()
            .find(|r| r.id == b["requirement_id"])
            .unwrap();
        let component = components
            .iter()
            .find(|c| c["ref"] == b["component"])
            .ok_or_else(|| Error::input("Registered report component missing"))?;
        check(
            b["sha256"] == component["sha256"],
            format!("Report binding hash differs: {}", r.id),
        )?;
        let path = storage::relative(folder, b["component"].as_str().unwrap_or(""))?;
        let proof = proofs
            .iter()
            .find(|p| p.path == path)
            .ok_or_else(|| Error::input("Verified report proof missing"))?;
        let report = read_json(&path, proof)?;
        let selector = format!("output:{}", entity["id"].as_str().unwrap());
        let expectations = &profile["reporting"]["expectations"];
        let binding_set = expectations
            .get(&selector)
            .or_else(|| expectations.get(entity["kind"].as_str().unwrap()));
        for binding in binding_set
            .and_then(|b| b.get(&r.id))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let expected = if let Some(pointer) = binding["context_pointer"].as_str() {
                spec.pointer(pointer)
            } else {
                binding.get("value")
            };
            check(
                expected.is_some()
                    && report["data"].pointer(binding["pointer"].as_str().unwrap()) == expected,
                format!(
                    "Report differs from the pre-execution plan: {} {}",
                    r.id, binding["pointer"]
                ),
            )?;
        }
        check(
            report["schema"] == "loom-report/v1"
                && report["contract_ref"] == reference(contract)
                && report["requirement_id"] == r.id
                && report["producer"]
                    == json!({"job":job,"attempt":attempt,"profile_sha256":profile["sha256"]}),
            format!(
                "Report belongs to another output, Job, attempt or contract: {}",
                r.id
            ),
        )?;
        contracts::validate(&r.schema, &report["data"])
            .map_err(|e| Error::new("report_schema", format!("{}: {}", r.id, e.message), 400))?;
        if let Some(base) = builtins("report_contract")
            .iter()
            .flat_map(effective)
            .find(|b| b.role == r.role)
        {
            contracts::validate(&extensible_shape(&base.schema), &report["data"]).map_err(|e| {
                Error::new("report_schema", format!("{}: {}", r.id, e.message), 400)
            })?;
        }
        semantic(&r, &report["data"], contract, &expected_inputs)?;
        if r.role == "evaluation_cases" {
            let protocol = resolve_from_pins(contract, &r, &report["data"]["protocol_ref"])?;
            for case in report["data"]["cases"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["valid"] == true)
            {
                let passed = protocol["definition"]["predicate"]["all"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|rule| {
                        let measured = case
                            .pointer(rule["pointer"].as_str().unwrap())
                            .unwrap_or(&Value::Null);
                        if measured["unit"] != rule["unit"] {
                            return false;
                        }
                        let Some(value) = measured["value"].as_f64().filter(|v| v.is_finite())
                        else {
                            return false;
                        };
                        match rule["op"].as_str().unwrap() {
                            "lte" => value <= rule["value"].as_f64().unwrap(),
                            "gte" => value >= rule["value"].as_f64().unwrap(),
                            _ => {
                                value >= rule["min"].as_f64().unwrap()
                                    && value <= rule["max"].as_f64().unwrap()
                            }
                        }
                    });
                check(
                    passed == (case["outcome"] == "success"),
                    "Evaluation success disagrees with the frozen measurement predicate",
                )?;
            }
            if entity.pointer("/metadata/research/evaluation").is_some() {
                for (field, count) in [
                    ("planned", "planned"),
                    ("valid", "valid"),
                    ("successes", "successes"),
                ] {
                    if let Some(value) = entity["metadata"]["research"]["evaluation"].get(field) {
                        check(
                            *value == report["data"]["counts"][count],
                            "Catalog evaluation summary differs from verified case counts",
                        )?;
                    }
                }
            }
        }
        if r.role == "instructions" {
            check(
                (profile["reporting"]["conditioning"] == "none")
                    == (report["data"]["mode"] == "none"),
                "Instruction report contradicts the accepted Policy conditioning",
            )?;
        }
        if r.role == "policy_inputs" {
            check(
                report["data"]["checkpoint"]["sha256"] == components[0]["sha256"],
                "Policy report identifies another checkpoint payload",
            )?;
            for (pointer, field) in [
                ("/metadata/research/robot/mechanical_dof", "robot_dof"),
                (
                    "/metadata/research/model/action_dimension",
                    "action_dimension",
                ),
            ] {
                if let Some(value) = entity.pointer(pointer) {
                    check(
                        *value == report["data"][field],
                        "Catalog DoF/dimension differs from the Policy interface report",
                    )?;
                }
            }
        }
        if r.role == "dataset_statistics"
            && entity.pointer("/metadata/research/collection/status") == Some(&json!("recorded"))
        {
            check(
                entity["metadata"]["research"]["collection"]["count"] == report["data"]["count"],
                "Catalog Dataset count differs from membership",
            )?;
        }
        if r.role == "action_metrics" {
            for m in report["data"]["measurements"].as_array().unwrap() {
                let d = resolve_from_pins(contract, &r, &m["definition_ref"])?;
                check(
                    m["unit"] == d["definition"]["unit"],
                    "Action metric unit differs from its registered definition",
                )?;
            }
        }
        report_data
            .entry(r.role.clone())
            .or_insert_with(Vec::new)
            .push(report["data"].clone());
        let mut fields = vec![];
        for f in &r.fields {
            if let Some(v) = report["data"].pointer(&f.pointer) {
                if v.is_null() {
                    fields.push(json!({"id":f.id,"label":f.label,"format":f.format,"pointer":f.pointer,"state":"missing","value":null}));
                    continue;
                }
                if [
                    "table",
                    "series",
                    "scatter",
                    "instructions",
                    "bar",
                    "coverage_matrix",
                ]
                .contains(&f.format.as_str())
                {
                    check(
                        v.is_array(),
                        "Collection display field must point to an array",
                    )?;
                } else if f.format == "properties" {
                    check(
                        v.as_object().is_some_and(|o| o.len() <= 32)
                            && serde_json::to_vec(v)?.len() <= 8192,
                        "Properties must be a bounded field group",
                    )?;
                } else {
                    check(
                        crate::metadata_templates::field_value(v, &f.format),
                        "Display value differs from its registered type",
                    )?;
                }
                if matches!(f.format.as_str(), "series" | "scatter") {
                    for row in v.as_array().unwrap() {
                        check([&f.x_pointer,&f.y_pointer].iter().all(|p|row.pointer(p.as_ref().unwrap()).and_then(Value::as_f64).is_some_and(f64::is_finite)),"Chart coordinates must be measured finite values at registered pointers")?;
                    }
                }
                if f.format == "bar" {
                    for row in v.as_array().unwrap() {
                        check(
                            row.pointer(f.x_pointer.as_ref().unwrap())
                                .and_then(Value::as_str)
                                .is_some_and(|label| printable(label, 100))
                                && row
                                    .pointer(f.y_pointer.as_ref().unwrap())
                                    .and_then(Value::as_f64)
                                    .is_some_and(|value| value.is_finite() && value >= 0.0),
                            "Bar charts require labeled nonnegative observations or bins",
                        )?;
                    }
                }
                if f.format == "coverage_matrix" {
                    let mut pairs = BTreeSet::new();
                    for cell in v.as_array().unwrap() {
                        let row = cell["row"].as_str().filter(|label| printable(label, 100));
                        let column = cell["column"]
                            .as_str()
                            .filter(|label| printable(label, 100));
                        let counts = cell["assigned"].as_u64().zip(cell["admitted"].as_u64());
                        check(
                            row.zip(column).is_some_and(|pair| pairs.insert(pair))
                                && counts.is_some_and(|(assigned, admitted)| admitted <= assigned),
                            "Coverage cells need unique row/column labels and consistent assigned/admitted counts",
                        )?;
                    }
                }
                fields.push(json!({"id":f.id,"label":f.label,"format":f.format,"unit":f.unit,"pointer":f.pointer,"x_pointer":f.x_pointer,"y_pointer":f.y_pointer,"x_label":f.x_label,"y_label":f.y_label,"columns":f.columns,"value":if v.is_array(){json!({"rows":v.as_array().unwrap().len()})}else{v.clone()}}));
            }
        }
        reports.push(json!({"requirement_id":r.id,"label":r.label,"role":r.role,"required":r.required,"component":b["component"],"sha256":b["sha256"],"size":component["size"],"state":"recorded","fields":fields}));
    }
    for r in effective(contract) {
        if !reports.iter().any(|b| b["requirement_id"] == r.id) {
            reports.push(json!({"requirement_id":r.id,"label":r.label,"role":r.role,"required":r.required,"state":"missing","fields":[]}));
        }
    }
    if let (Some(plans), Some(readbacks)) = (
        report_data.get("sdg_conditions"),
        report_data.get("sdg_readback"),
    ) {
        let planned = plans
            .iter()
            .flat_map(|p| p["conditions"].as_array().unwrap())
            .map(|v| v["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        let rows = readbacks
            .iter()
            .flat_map(|p| p["observations"].as_array().unwrap())
            .collect::<Vec<_>>();
        let observed = rows
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        check(
            planned == observed && observed.len() == rows.len(),
            "SDG readback must account for each planned condition exactly once",
        )?;
        if let Some(stats) = report_data.get("dataset_statistics") {
            let members = stats
                .iter()
                .flat_map(|p| p["members"].as_array().unwrap())
                .map(|v| v["id"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            let accepted = rows
                .iter()
                .filter(|v| v["state"] == "accepted")
                .map(|v| v["member_id"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            check(
                members == accepted,
                "Published Dataset membership differs from accepted SDG readbacks",
            )?;
            let splits = stats
                .iter()
                .flat_map(|p| p["members"].as_array().unwrap())
                .map(|v| (v["id"].as_str().unwrap(), v["split"].clone()))
                .collect::<BTreeMap<_, _>>();
            let plan_splits = plans
                .iter()
                .flat_map(|p| p["conditions"].as_array().unwrap())
                .map(|v| (v["id"].as_str().unwrap(), v["split"].clone()))
                .collect::<BTreeMap<_, _>>();
            for row in rows.iter().filter(|v| v["state"] == "accepted") {
                check(
                    splits.get(row["member_id"].as_str().unwrap())
                        == plan_splits.get(row["id"].as_str().unwrap()),
                    "Generated Dataset split differs from its frozen condition split",
                )?;
            }
        }
    }
    if let (Some(inputs), Some(series)) = (
        report_data.get("policy_inputs"),
        report_data.get("learning_series"),
    ) {
        for series in series.iter().filter(|v| v["selection"].is_object()) {
            check(
                inputs
                    .iter()
                    .all(|p| p["checkpoint"]["step"] == series["selection"]["step"]),
                "Selected step differs from the published Policy checkpoint",
            )?;
        }
    }
    if let (Some(inputs), Some(metrics)) = (
        report_data.get("policy_inputs"),
        report_data.get("action_metrics"),
    ) {
        for metric in metrics {
            check(
                inputs.iter().all(|p| {
                    p["action_channels"] == metric["channels"]
                        && p["action_representation"] == metric["representation"]
                }),
                "Action-error channels differ from the accepted Policy interface",
            )?;
        }
    }
    let cases = report_data.get("evaluation_cases").and_then(|a| a.first());
    let checkpoint_sweep =
        cases.and_then(|data| checkpoint_evaluation(data, false).filter(|v| v["rule"].is_string()));
    let primary=cases.filter(|_|checkpoint_sweep.is_none()).map(|d|json!({"label":"Whole-task success","numerator":d["counts"]["successes"],"denominator":d["counts"]["valid"],"protocol_ref":d["protocol_ref"]}));
    let evaluation_summary = report_data.get("evaluation_cases").and_then(|data| data.first()).map(|data| {
        let mut failure_counts = BTreeMap::new();
        for case in data["cases"].as_array().into_iter().flatten() {
            if case["valid"] == true && case["outcome"] != "success" {
                if let Some(stage) = case["failure_stage"].as_str() {
                    *failure_counts.entry(stage).or_insert(0usize) += 1;
                }
            }
        }
        let criteria = effective(contract).into_iter().filter(|r| r.role == "evaluation_cases")
            .find_map(|r| resolve_from_pins(contract, &r, &data["protocol_ref"]).ok())
            .map(|protocol| protocol["definition"]["predicate"]["all"].clone());
        json!({"counts":data["counts"],"recorded_failure_subtasks":failure_counts,"criteria":criteria})
    });
    Ok(Some(
        json!({"state":"validated","contract_ref":reference(contract),"activity":contract["definition"]["activity"],"reports":reports,"adaptations":contract["definition"]["resolved_adaptations"],"exclusions":contract["definition"]["excluded_requirements"],"inputs":expected_inputs,"primary_outcome":primary,"evaluation_summary":evaluation_summary,"checkpoint_sweep":checkpoint_sweep}),
    ))
}

#[derive(Default)]
struct CheckpointCases {
    sha256: String,
    resets: BTreeMap<i64, String>,
    valid: u64,
    successes: u64,
}

fn checkpoint_evaluation(data: &Value, legacy_non_test: bool) -> Option<Value> {
    let mut groups = BTreeMap::<u64, CheckpointCases>::new();
    for row in data["cases"].as_array()? {
        let factors = &row["factors"];
        let step = factors["update"].as_u64()?;
        let digest = digest_field(&factors["checkpoint_sha256"])?;
        let reset = digest_field(&factors["reset_state_sha256"])?;
        let seed = row["seed"].as_i64()?;
        let group = groups.entry(step).or_insert_with(|| CheckpointCases {
            sha256: digest.into(),
            ..Default::default()
        });
        if group.sha256 != digest
            || group.resets.insert(seed, reset.into()).is_some()
            || groups.len() > 64
        {
            return None;
        }
        let group = groups.get_mut(&step).unwrap();
        group.valid += u64::from(row["valid"] == true);
        group.successes += u64::from(row["valid"] == true && row["outcome"] == "success");
    }
    if groups.len() < 2
        || groups
            .values()
            .map(|g| &g.sha256)
            .collect::<BTreeSet<_>>()
            .len()
            != groups.len()
    {
        return None;
    }
    let selection = &data["checkpoint_selection"];
    let first = groups.values().next()?;
    let comparable = data["counts"]["unexecuted"] == 0
        && groups
            .values()
            .all(|g| g.valid as usize == g.resets.len() && g.resets == first.resets);
    let can_rank = comparable
        && (legacy_non_test
            || matches!(
                selection["split"].as_str(),
                Some("development" | "validation")
            ));
    let max = can_rank
        .then(|| groups.values().map(|g| g.successes).max())
        .flatten();
    let best_steps: Vec<_> = groups
        .iter()
        .filter_map(|(step, g)| (Some(g.successes) == max).then_some(*step))
        .collect();
    let selected = match selection["rule"].as_str() {
        Some("prespecified") => {
            let step = selection["selected_step"].as_u64()?;
            if groups.get(&step)?.valid == 0 {
                return None;
            }
            Some(step)
        }
        Some("best_success_rate") if can_rank && selection["selected_step"].is_null() => {
            if best_steps.len() == 1 {
                best_steps.first().copied()
            } else {
                None
            }
        }
        None => None,
        _ => return None,
    };
    Some(
        json!({"split":selection["split"],"rule":selection["rule"],"selected_step":selected,"best_steps":best_steps,"comparable":comparable,"checkpoints":groups.iter().map(|(step,g)|json!({"step":step,"sha256":g.sha256,"valid":g.valid,"successes":g.successes,"executed":g.resets.len()})).collect::<Vec<_>>()}),
    )
}

fn digest_field(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

// Built-in semantics are validated in addition to JSON Schema. Custom roles carry
// their registered schema/definitions; validation never pretends to prove physics.
fn semantic(r: &Requirement, data: &Value, contract: &Value, inputs: &[Value]) -> Result<()> {
    if let Some(refs) = data.get("inputs") {
        check(
            refs == &json!(inputs),
            "Report inputs differ from accepted exact Dataset/Policy lineage",
        )?;
    }
    if r.role == "instructions" {
        let mode = data["mode"].as_str().unwrap_or("");
        let texts = data["texts"].as_array().unwrap();
        check(
            (mode == "none" && texts.is_empty()) || (mode != "none" && !texts.is_empty()),
            "Instruction mode contradicts exact texts",
        )?;
        let ids = texts
            .iter()
            .filter_map(|t| t["id"].as_str())
            .collect::<BTreeSet<_>>();
        check(ids.len() == texts.len(), "Duplicate instruction IDs")?;
        for assignment in data["assignments"].as_array().into_iter().flatten() {
            check(
                ids.contains(assignment["instruction_id"].as_str().unwrap_or("")),
                "Instruction assignment has unknown text",
            )?;
        }
        check(
            data["coverage"]["assigned"].as_u64() <= data["coverage"]["total"].as_u64(),
            "Instruction coverage exceeds population",
        )?;
        check(
            mode == "none" || data["coverage"]["assigned"] == data["coverage"]["total"],
            "Required instruction coverage incomplete",
        )?;
    }
    if r.role == "learning_series" {
        check(
            r.definitions.contains(&data["definition_ref"]),
            "Learning metric differs from frozen semantic definitions",
        )?;
        let points = data["points"].as_array().unwrap();
        check(!points.is_empty(), "Required learning series is empty")?;
        let mut seen = BTreeSet::new();
        for p in points {
            check(
                p["value"].as_f64().is_some_and(f64::is_finite)
                    && p["population"].as_u64().is_some_and(|n| n > 0),
                "Learning metric needs a finite value and measured population",
            )?;
            check(
                seen.insert((
                    p["step"].to_string(),
                    p["split"].to_string(),
                    p["phase"].to_string(),
                )),
                "Duplicate learning point",
            )?;
        }
        for step in data["schedule"].as_array().into_iter().flatten() {
            check(
                points.iter().any(|p| p["step"] == *step)
                    || data["interruptions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| p["step"] == *step),
                "Scheduled learning point is missing without an interruption reason",
            )?;
        }
        if data["selection"].is_object() {
            check(
                data["selection"]["split"] != "test"
                    && points.iter().any(|p| {
                        p["step"] == data["selection"]["step"]
                            && p["split"] == data["selection"]["split"]
                    }),
                "Checkpoint selection must use a recorded non-test point",
            )?;
        }
    }
    if r.role == "evaluation_cases" {
        let rows = data["cases"].as_array().unwrap();
        if data["checkpoint_selection"].is_object() {
            check(checkpoint_evaluation(data, false).is_some(), "Checkpoint evaluation needs distinct checkpoint pins and unique seed/reset cases; select by success only on identical complete development/validation cohorts")?;
        }
        let mut ids = BTreeSet::new();
        for row in rows {
            check(
                ids.insert((row["id"].to_string(), row["seed"].to_string())),
                "Duplicate evaluation case/seed",
            )?;
            check(
                row["valid"] == true || row["outcome"] != "success",
                "Invalid case cannot be a success",
            )?;
        }
        let valid = rows.iter().filter(|v| v["valid"] == true).count();
        let success = rows
            .iter()
            .filter(|v| v["valid"] == true && v["outcome"] == "success")
            .count();
        check(
            data["counts"]["valid"] == valid
                && data["counts"]["successes"] == success
                && data["counts"]["executed"] == rows.len(),
            "Evaluation aggregate does not reconcile with cases",
        )?;
        check(
            data["counts"]["planned"].as_u64()
                == (rows.len() as u64).checked_add(data["counts"]["unexecuted"].as_u64().unwrap()),
            "Evaluation planned/executed accounting differs",
        )?;
        let infra = rows.iter().filter(|v| v["outcome"] == "infra").count();
        let invalid = rows
            .iter()
            .filter(|v| v["valid"] != true && v["outcome"] != "infra")
            .count();
        check(
            data["counts"]["infra"] == infra
                && data["counts"]["invalid"] == invalid
                && valid + infra + invalid == rows.len(),
            "Evaluation validity/infra populations overlap or counts differ",
        )?;
        check(
            data["protocol_ref"].is_object() && r.definitions.contains(&data["protocol_ref"]),
            "Evaluation must use a protocol pinned in this requirement",
        )?;
    }
    if r.role == "sdg_conditions" {
        let rows = data["conditions"].as_array().unwrap();
        let ids = rows
            .iter()
            .filter_map(|r| r["id"].as_str())
            .collect::<BTreeSet<_>>();
        check(ids.len() == rows.len(), "Duplicate SDG condition IDs")?;
        check(
            !r.definitions.is_empty(),
            "SDG coverage needs a registered factor definition",
        )?;
    }
    if matches!(r.role.as_str(), "sdg_conditions" | "sdg_readback") {
        check(
            data["factor_refs"]
                .as_array()
                .is_some_and(|a| !a.is_empty() && a.iter().all(|pin| r.definitions.contains(pin))),
            "SDG factors differ from frozen definitions",
        )?;
    }
    if r.role == "dataset_statistics" {
        let members = data["members"].as_array().unwrap();
        let mut ids = BTreeSet::new();
        let mut splits = BTreeMap::<String, u64>::new();
        for m in members {
            check(
                ids.insert(m["id"].as_str().unwrap()),
                "Duplicate Dataset membership or split overlap",
            )?;
            *splits
                .entry(m["split"].as_str().unwrap().into())
                .or_default() += 1;
        }
        check(
            data["count"] == members.len() && data["split_counts"] == json!(splits),
            "Dataset counts/splits differ from membership",
        )?;
    }
    if r.role == "policy_inputs" {
        let channels = data["action_channels"].as_array().unwrap();
        check(
            data["action_dimension"] == channels.len(),
            "Action dimension differs from named channels",
        )?;
        for (index, c) in channels.iter().enumerate() {
            check(
                c["index"] == index,
                "Action channel indices must preserve order",
            )?;
        }
        if let (Some(controlled), Some(robot)) = (
            data["controlled_joint_dof"].as_u64(),
            data["robot_dof"].as_u64(),
        ) {
            check(
                controlled <= robot,
                "Controlled joint DoF exceeds mechanical Robot DoF",
            )?;
        }
    }
    if r.role == "action_metrics" {
        let channels = data["channels"].as_array().unwrap();
        let normalized = data["normalization"]["mode"] == "normalized";
        check(
            !normalized || data["normalization"]["fitted_split"] != "none",
            "Normalized errors need a declared normalization fit",
        )?;
        let mut seen = BTreeSet::new();
        for m in data["measurements"].as_array().unwrap() {
            check(
                r.definitions.contains(&m["definition_ref"]),
                "Action metric differs from frozen definition",
            )?;
            let index = m["channel_index"].as_u64().unwrap() as usize;
            let channel = channels
                .get(index)
                .ok_or_else(|| Error::input("Action error channel out of range"))?;
            let unit = if normalized {
                "1"
            } else {
                channel["unit"].as_str().unwrap()
            };
            let expected = if m["error"] == "mse" && unit != "1" {
                format!("{unit}²")
            } else {
                unit.into()
            };
            check(
                m["unit"] == expected
                    && m["value"]
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && v >= 0.0),
                "Action error needs nonnegative values and dimension-correct units",
            )?;
            check(
                seen.insert((
                    index,
                    m["split"].to_string(),
                    m["phase"].to_string(),
                    m["error"].to_string(),
                )),
                "Duplicate action error measurement",
            )?;
        }
        if r.required {
            for index in 0..channels.len() {
                check(data["measurements"].as_array().unwrap().iter().any(|m|m["channel_index"]==index&&m["error"]=="mse"&&m["split"]!="train"),"Required action report needs held-out MSE for every named channel; adapt coverage with evidence if inapplicable")?;
            }
        }
    }
    check(
        contract["definition"]["output_kind"] != "experiment"
            || contract["definition"]["activity"] == "evaluation",
        "Training/comparison is not a standalone Experiment",
    )
}

impl Application {
    pub fn reporting_action(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        let kind = if action.contains("definition") {
            "report_definition"
        } else {
            "report_contract"
        };
        if action == "read_report" {
            return self.read_report(args);
        }
        if action == "get_report_evidence" {
            let target: contracts::Target = contracts::decode(args["target"].clone())?;
            let body = source_body(&self.db, &target)?;
            let parsed: Value = serde_json::from_str(&body)?;
            let pointer = args["pointer"].as_str().unwrap_or("");
            let value = parsed
                .pointer(pointer)
                .ok_or_else(|| Error::input("Evidence JSON pointer not found"))?;
            require(
                serde_json::to_vec(value)?.len() <= 65536,
                "report_evidence_limit",
                "Select a smaller evidence subtree (maximum 64 KiB)",
                413,
            )?;
            return Ok(
                json!({"target":args["target"], "sha256":hash(&body), "pointer":pointer, "value":value}),
            );
        }
        if action.starts_with("get_") {
            return get(
                &self.db,
                kind,
                args["id"].as_str().unwrap_or(""),
                args["revision"].as_u64(),
            );
        }
        if action == "report_contracts" || action == "report_definitions" {
            let offset = args["offset"].as_u64().unwrap_or(0).min(i64::MAX as u64) as i64;
            let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 100) as i64;
            let filter = args["kind"].as_str();
            let history = args["include_revisions"] == true;
            let mut builtin = builtins(kind);
            builtin.retain(|v| {
                filter.is_none_or(|k| {
                    v["definition"]["output_kind"] == k || v["definition"]["kind"] == k
                })
            });
            builtin.retain(|_| {
                args["lifecycle_state"]
                    .as_str()
                    .is_none_or(|s| s == "active")
            });
            builtin.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
            let prefix=format!("WITH ranked AS (SELECT body,json_extract(identity,'$.id') id,json_extract(identity,'$.revision') revision,row_number() OVER(PARTITION BY json_extract(identity,'$.id') ORDER BY json_extract(identity,'$.revision') DESC) rank FROM catalog_sources WHERE kind=? AND json_extract(identity,'$.id') NOT LIKE 'loom.%'), selected AS (SELECT * FROM ranked WHERE (? OR rank=1) AND (? IS NULL OR json_extract(body,'$.definition.output_kind')=? OR json_extract(body,'$.definition.kind')=? ) AND ({}) ) ",crate::registry_lifecycle::listing_clause(&args,&crate::registry_lifecycle::record_state_sql(kind,"ranked.id","ranked.revision")));
            let count: i64 = self.db.query_row(
                &(prefix.clone() + "SELECT count(*) FROM selected"),
                params![kind, history, filter, filter, filter],
                |r| r.get(0),
            )?;
            let total = builtin.len() as i64 + count;
            let mut selected = builtin
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .cloned()
                .collect::<Vec<_>>();
            let remaining = limit - selected.len() as i64;
            if remaining > 0 {
                let skip = (offset - builtin.len() as i64).max(0);
                for row in rows(
                    &self.db,
                    &(prefix.clone()
                        + "SELECT body FROM selected ORDER BY id,revision DESC LIMIT ? OFFSET ?"),
                    &[
                        &kind, &history, &filter, &filter, &filter, &remaining, &skip,
                    ],
                )? {
                    selected.push(serde_json::from_str(row["body"].as_str().unwrap())?);
                }
            }
            for v in &mut selected {
                v.as_object_mut().unwrap().remove("definition");
            }
            return Ok(
                json!({"items":selected,"total":total,"next_offset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit))}),
            );
        }
        if action == "validate_report_contract" {
            let definition = validate_definition(&self.db, &args["definition"])?;
            return Ok(
                json!({"valid":true,"definition":definition,"scientific_acceptance":"unverified"}),
            );
        }
        check(
            action == "register_report_contract" || action == "register_report_definition",
            "Unknown reporting action",
        )?;
        let id = args["id"].as_str().unwrap_or("");
        check(
            identifier(id) && !id.starts_with("loom."),
            "Choose a non-reserved reporting resource ID",
        )?;
        let key = args["key"].as_str().unwrap_or("");
        let operation = format!("{kind}.register");
        if let Some(v) = replay(&self.db, actor, key, &operation, &args)? {
            return Ok(v);
        }
        let definition = if kind == "report_contract" {
            validate_definition(&self.db, &args["definition"])?
        } else {
            definition_resource(&self.db, &args["definition"])?
        };
        let tx = self.transaction()?;
        if let Some(v) = replay(&tx, actor, key, &operation, &args)? {
            return Ok(v);
        }
        active(&tx)?;
        let old:Option<String>=tx.query_row("SELECT body FROM catalog_sources WHERE kind=? AND json_extract(identity,'$.id')=? ORDER BY json_extract(identity,'$.revision') DESC LIMIT 1",params![kind,id],|r|r.get(0)).optional()?;
        let old = old.map(|s| serde_json::from_str::<Value>(&s)).transpose()?;
        let revision = old
            .as_ref()
            .and_then(|v| v["revision"].as_u64())
            .unwrap_or(0);
        require(
            args["expected_revision"].as_u64().unwrap_or(0) == revision,
            "report_revision_conflict",
            "Reporting resource revision changed",
            409,
        )?;
        if let Some(old) = &old {
            require(
                old["owner"] == actor.id || actor.scopes.iter().any(|s| s == "admin"),
                "report_owner",
                "Only resource author may revise it",
                403,
            )?;
            check(
                old["definition"]["output_kind"] == definition["output_kind"]
                    && old["definition"]["activity"] == definition["activity"]
                    && old["definition"]["kind"] == definition["kind"],
                "A resource family cannot change kind/activity",
            )?;
        }
        let owner = old
            .as_ref()
            .and_then(|v| v["owner"].as_str())
            .unwrap_or(&actor.id);
        let value = record(id, revision + 1, definition, owner, kind);
        tx.execute(
            "INSERT INTO catalog_sources VALUES(?,?,?)",
            params![
                kind,
                encode(&json!({"id":id,"revision":revision+1}))?,
                encode(&value)?
            ],
        )?;
        receipt(&tx, actor, key, &operation, &args, &value)?;
        tx.commit()?;
        Ok(value)
    }

    pub(crate) fn legacy_checkpoint_evaluation(
        &self,
        entity: &Value,
        params: &Value,
    ) -> Option<Value> {
        let steps = params
            .get("updates")
            .or_else(|| params.get("checkpoint_steps"))?
            .as_array()?;
        if !(2..=64).contains(&steps.len()) || entity["reporting"]["activity"] != "evaluation" {
            return None;
        }
        let planned = steps
            .iter()
            .map(Value::as_u64)
            .collect::<Option<BTreeSet<_>>>()?;
        if planned.len() != steps.len() {
            return None;
        }
        let report = entity["reporting"]["reports"]
            .as_array()?
            .iter()
            .find(|r| r["state"] == "recorded" && r["role"] == "evaluation_cases")?;
        let digest = digest_field(&report["sha256"])?;
        let split = entity
            .pointer("/metadata/research/evaluation/split")
            .and_then(Value::as_str)
            .unwrap_or("")
            .split_whitespace()
            .next();
        let non_test = matches!(split, Some("development" | "validation"));
        let key = (digest.to_string(), non_test);
        type Summaries = BTreeMap<(String, bool), Value>;
        static CACHE: OnceLock<Mutex<Summaries>> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let summary =
            if let Some(value) = cache.lock().ok().and_then(|cache| cache.get(&key).cloned()) {
                value
            } else {
                let payload = self
                    .read_bound_report(entity["id"].as_str()?, entity["revision"].as_u64()?, report)
                    .ok()?;
                let value = checkpoint_evaluation(&payload["data"], non_test)?;
                if let Ok(mut cache) = cache.lock() {
                    if cache.len() >= 128 {
                        cache.pop_first();
                    }
                    cache.insert(key, value.clone());
                }
                value
            };
        let recorded = summary["checkpoints"]
            .as_array()?
            .iter()
            .map(|c| c["step"].as_u64())
            .collect::<Option<BTreeSet<_>>>()?;
        (planned == recorded).then_some(summary)
    }

    fn read_report(&self, args: Value) -> Result<Value> {
        let id = args["id"].as_str().unwrap_or("");
        let revision = args["revision"].as_u64().unwrap_or(1);
        let body: String = self.db.query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![id, revision as i64],
            |r| r.get(0),
        )?;
        let entity: Value = serde_json::from_str(&body)?;
        let report = entity["reporting"]["reports"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["requirement_id"] == args["requirement_id"])
            .ok_or_else(|| {
                Error::new(
                    "report_missing",
                    "Report is not bound to this exact record",
                    404,
                )
            })?;
        require(
            report["state"] == "recorded",
            "report_missing",
            "Report was not recorded",
            404,
        )?;
        let payload = self.read_bound_report(id, revision, report)?;
        let pointer = args["pointer"].as_str().unwrap_or("");
        let mut value = payload["data"]
            .pointer(pointer)
            .cloned()
            .ok_or_else(|| Error::input("Report field not found"))?;
        let total = value.as_array().map(Vec::len);
        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let limit = args["limit"].as_u64().unwrap_or(100).clamp(1, 500) as usize;
        if let Some(rows) = value.as_array() {
            value = json!(rows
                .iter()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>());
        }
        require(
            serde_json::to_vec(&value)?.len() <= 256 * 1024,
            "report_page_size",
            "Select a report field or smaller row page",
            413,
        )?;
        Ok(
            json!({"id":id,"revision":revision,"report":report,"pointer":pointer,"value":value,"total":total,"next_offset":total.filter(|n|offset.saturating_add(limit)<*n).map(|_|offset+limit),"verification":"sha256_verified"}),
        )
    }

    fn read_bound_report(&self, id: &str, revision: u64, report: &Value) -> Result<Value> {
        let locations=rows(&self.db,"SELECT storage_id,ref FROM all_entity_locations WHERE id=? AND revision=? AND component=? AND status='available' ORDER BY storage_id,ref",&[&id,&(revision as i64),&report["component"].as_str()])?;
        let mut data = None;
        let mut last_error = None;
        for l in locations {
            let read = (|| -> Result<Value> {
                let path = storage::locator(
                    &self.config,
                    &self.bulk,
                    &json!({"machine_id":self.config.machine_id,"storage_id":l["storage_id"],"ref":l["ref"]}),
                )?;
                let proof = storage::VerifiedFile::read(&path)?;
                require(
                    report["sha256"] == proof.sha256,
                    "report_changed",
                    "Stored report bytes changed",
                    409,
                )?;
                read_json(&path, &proof)
            })();
            match read {
                Ok(value) => {
                    data = Some(value);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        data.ok_or_else(|| {
            last_error.unwrap_or_else(|| {
                Error::new(
                    "report_unavailable",
                    "No currently readable registered report copy",
                    404,
                )
            })
        })
    }
}
