//! Public contracts: immutable compatibility registry plus typed new application DTOs.
use crate::{error::require, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    pub group: String,
    pub action: String,
    pub operation: String,
    pub scope: String,
    pub schema: Value,
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub fn actions() -> Vec<Action> {
    let mut result: Vec<Action> =
        serde_json::from_str(include_str!("../../../contracts/legacy-actions.json"))
            .expect("embedded registry");
    let string = json!({"type":"string","minLength":1,"maxLength":2048});
    let reference = object(
        json!({"id":string,"revision":{"type":"integer","minimum":1}}),
        &["id", "revision"],
    );
    let target = object(
        json!({"kind":{"enum":["job","entity","historical","component","tool","profile","runtime","protocol_artifact"]},"identity":{"type":"object"}}),
        &["kind", "identity"],
    );
    let definitions = [
        (
            "results",
            "scenarios",
            "read",
            object(
                json!({"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200},"include_archived":{"type":"boolean"}}),
                &[],
            ),
        ),
        (
            "results",
            "get_scenario",
            "read",
            object(
                json!({"id":string,"revision":{"type":"integer","minimum":1}}),
                &["id"],
            ),
        ),
        (
            "results",
            "scenario_graph",
            "read",
            object(
                json!({"id":string,"revision":{"type":"integer","minimum":1},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
                &["id"],
            ),
        ),
        (
            "results",
            "register_scenario",
            "submit",
            object(
                json!({"key":string,"definition":{"type":"object"}}),
                &["key", "definition"],
            ),
        ),
        (
            "results",
            "update_scenario",
            "submit",
            object(
                json!({"key":string,"id":string,"expected_revision":{"type":"integer","minimum":1},"definition":{"type":"object"},"reason":string}),
                &["key", "id", "expected_revision", "definition", "reason"],
            ),
        ),
        (
            "results",
            "archive_scenario",
            "submit",
            object(
                json!({"key":string,"id":string,"expected_revision":{"type":"integer","minimum":1},"reason":string}),
                &["key", "id", "expected_revision", "reason"],
            ),
        ),
        (
            "results",
            "link_scenario",
            "submit",
            object(
                json!({"key":string,"scenario_ref":reference,"target":target,"relation":{"enum":["produced_for","used_by","demonstrates","evaluates","implements","contextual_reference"]},"provenance":{"type":"object"}}),
                &["key", "scenario_ref", "target", "relation", "provenance"],
            ),
        ),
        (
            "results",
            "supersede_scenario_link",
            "submit",
            object(
                json!({"key":string,"id":string,"reason":string,"replacement":{"type":["string","null"]}}),
                &["key", "id", "reason"],
            ),
        ),
        (
            "results",
            "bind_historical_location",
            "submit",
            object(
                json!({"key":string,"identity":{"type":"object"},"locator":{"type":"object"},"sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"},"size":{"type":"integer","minimum":0}}),
                &["key", "identity", "locator", "sha256", "size"],
            ),
        ),
        (
            "components",
            "scaffold",
            "read",
            object(
                json!({"template":{"enum":["tool","component","hybrid"]},"name":string}),
                &["template", "name"],
            ),
        ),
        (
            "components",
            "validate_package",
            "read",
            object(
                json!({"manifest":{"type":"object"},"files":{"type":"object","additionalProperties":{"type":"string"}}}),
                &["manifest", "files"],
            ),
        ),
    ];
    for (g, a, s, schema) in definitions {
        result.push(Action {
            group: g.into(),
            action: a.into(),
            operation: format!("{g}.{a}"),
            scope: s.into(),
            schema,
        });
    }
    for entry in &mut result {
        if entry.group == "jobs" && entry.action == "list" {
            entry.schema["properties"]["view"]["default"] = json!("all");
        }
        if entry.group == "jobs" && matches!(entry.action.as_str(), "submit" | "submit_workflow") {
            entry.schema["properties"]["scenario_ref"] = reference.clone();
        }
    }
    result
}
pub fn action(group: &str, name: &str) -> Result<Action> {
    actions()
        .into_iter()
        .find(|a| a.group == group && a.action == name)
        .ok_or_else(|| Error::input("Unknown action"))
}
pub fn check_schema(schema: &Value) -> Result<()> {
    fn refs(v: &Value) -> bool {
        match v {
            Value::Object(o) => o.iter().all(|(k, v)| {
                if k == "$ref" || k == "$dynamicRef" {
                    v.as_str().is_some_and(|s| s.starts_with('#'))
                } else {
                    refs(v)
                }
            }),
            Value::Array(a) => a.iter().all(refs),
            _ => true,
        }
    }
    require(
        refs(schema),
        "schema",
        "Only local schema references are supported",
        400,
    )?;
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(schema)
        .map_err(|_| Error::input("Invalid parameter schema"))?;
    Ok(())
}
pub fn validate(schema: &Value, value: &Value) -> Result<()> {
    check_schema(schema)?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(schema)
        .map_err(|_| Error::input("Invalid parameter schema"))?;
    require(
        validator.is_valid(value),
        "schema",
        "Arguments do not match action schema",
        400,
    )
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioRef {
    pub id: String,
    pub revision: i64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    #[serde(default = "research")]
    pub scope: String,
    #[serde(default = "main_role")]
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_job: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub output_roles: BTreeMap<String, String>,
}
fn research() -> String {
    "research".into()
}
fn main_role() -> String {
    "main".into()
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            scope: research(),
            role: main_role(),
            label: None,
            experiment: None,
            request_ref: None,
            parent_job: None,
            output_roles: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Submit {
    pub key: String,
    pub kind: String,
    pub owner: String,
    pub notify: String,
    pub params: Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "empty_object")]
    pub execution: Value,
    #[serde(default = "tail")]
    pub queue_position: String,
    #[serde(default = "primary")]
    pub output_storage: String,
    #[serde(default)]
    pub consolidates: Vec<String>,
    #[serde(default)]
    pub presentation: Option<Presentation>,
    #[serde(default = "other")]
    pub category: String,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub scenario_ref: Option<ScenarioRef>,
}
fn empty_object() -> Value {
    json!({})
}
fn tail() -> String {
    "tail".into()
}
fn primary() -> String {
    "data/0".into()
}
fn other() -> String {
    "other".into()
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub spec: Value,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub key: String,
    pub label: String,
    pub owner: String,
    pub notify: String,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub outputs: BTreeMap<String, OutputAlias>,
    #[serde(default)]
    pub presentation: Option<Presentation>,
    #[serde(default = "other")]
    pub category: String,
    #[serde(default)]
    pub scenario_ref: Option<ScenarioRef>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputAlias {
    pub node: String,
    pub entity: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioCreate {
    pub key: String,
    pub definition: Value,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioUpdate {
    pub key: String,
    pub id: String,
    pub expected_revision: i64,
    pub definition: Value,
    pub reason: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub kind: String,
    pub identity: Value,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioLink {
    pub key: String,
    pub scenario_ref: ScenarioRef,
    pub target: Target,
    pub relation: String,
    pub provenance: Value,
}
pub fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    Ok(serde_json::from_value(v)?)
}
