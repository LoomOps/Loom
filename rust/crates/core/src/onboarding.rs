//! Live instructions and exact manifest hashes are served from the selected binary.
use crate::{auth::Actor, contracts, error::require, store::hash, Error, Result};
use serde_json::{json, Value};
include!(concat!(env!("OUT_DIR"), "/guides.rs"));
pub const MCP_INSTRUCTIONS: &str = "Call system.status to confirm the target and system.guide for task routes. Load the relevant guide and system.describe action schema. Keep accepted Job IDs and pins.";
const PACKAGED: &[(&str, &str)] = &[
    ("mcp-instructions.txt", MCP_INSTRUCTIONS),
    ("LICENSE", include_str!("../../../../loom_use/LICENSE")),
    (
        "THIRD_PARTY_NOTICES.md",
        include_str!("../../../../loom_use/THIRD_PARTY_NOTICES.md"),
    ),
    ("README.md", include_str!("../../../../loom_use/README.md")),
    (
        "client-release.json",
        include_str!("../../../../loom_use/client-release.json"),
    ),
    (
        "skills/loom-use/SKILL.md",
        include_str!("../../../../loom_use/skills/loom-use/SKILL.md"),
    ),
];
const CATALOG: &str = include_str!("../../../../loom_use/catalog.json");
const ACTION_DOCS: &str = include_str!("../../../contracts/action-docs.json");
const JOB_CONTRACT: &str = include_str!("../../../contracts/job-contract.json");

fn guide_revision(
    documents: &[(&str, &str, &str)],
    packaged: &[(&str, &str)],
    catalog: &str,
    action_docs: &str,
    job_contract: &str,
) -> String {
    // Include the complete delivered guidance, not only the Markdown guide bodies.
    hash(
        serde_json::to_vec(&(documents, packaged, catalog, action_docs, job_contract))
            .expect("guide identity"),
    )
}
fn contract_revision(actions: &Value, job_contract: &str) -> String {
    let job: Value = serde_json::from_str(job_contract).expect("Job contract");
    hash(serde_json::to_vec(&(actions, job)).expect("contract identity"))
}
pub fn identity() -> Value {
    static IDENTITY: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(|| {
    let guide = guide_revision(DOCUMENTS, PACKAGED, CATALOG, ACTION_DOCS, JOB_CONTRACT);
    let contract = contract_revision(&json!(contracts::actions()), JOB_CONTRACT);
    json!({"build_id":env!("LOOM_BUILD_ID"),"api_revision":"loom-ops/v55","contract_revision":contract,"guide_revision":guide,"revision":hash(format!("{contract}{guide}")),"client_release":serde_json::from_str::<Value>(include_str!("../../../../loom_use/client-release.json")).expect("client release"),"engine":"rust","source_sha256":env!("LOOM_RUST_SOURCE")})
    }).clone()
}
pub fn manifest() -> Value {
    let mut v = identity();
    let o = v.as_object_mut().unwrap();
    o.insert("schema".into(), json!("loom-onboarding/v1"));
    o.insert("package".into(), json!("loom_use"));
    o.insert("entry".into(), json!("entry"));
    o.insert(
        "mcp".into(),
        json!({"endpoint":"/mcp","tool":"system","action":"guide"}),
    );
    o.insert(
        "http".into(),
        json!({"index":"/api/guide","bundle":"/api/guide/bundle"}),
    );
    v
}
pub fn read(args: &Value) -> Result<Value> {
    let manifest = manifest();
    if args["known_revision"] == manifest["revision"] {
        return Ok(json!({"manifest":manifest,"not_modified":true}));
    }
    if let Some(id) = args["id"].as_str() {
        let (_, title, text) = DOCUMENTS
            .iter()
            .find(|(name, _, _)| *name == id)
            .ok_or_else(|| Error::new("guide", "Unknown guide", 404))?;
        return Ok(json!({"manifest":manifest,"id":id,"title":title,"content":text}));
    }
    Ok(
        json!({"manifest":manifest,"documents":serde_json::from_str::<Value>(CATALOG)?["documents"].as_array().unwrap().iter().map(|d|json!({"id":d["id"],"title":d["title"],"path":format!("{}.md",d["id"].as_str().unwrap()),"type":d["type"],"summary":d["summary"]})).collect::<Vec<_>>()}),
    )
}
pub fn tools(actor: &Actor) -> Value {
    let groups = [
        "jobs",
        "recovery",
        "events",
        "components",
        "results",
        "system",
    ];
    json!(groups.iter().filter_map(|group|{let actions:Vec<_>=contracts::actions().into_iter().filter(|a|a.group==*group&&actor.scopes.contains(&a.scope)).map(|a|a.action).collect();if actions.is_empty(){return None}Some(json!({"name":group,"description":"Discover exact action arguments with system.describe.","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":actions},"arguments":{"type":"object"}},"required":["action"],"additionalProperties":false}}))}).collect::<Vec<_>>())
}
pub fn bundle(actor: &Actor) -> Result<Vec<u8>> {
    use std::io::{Cursor, Write};
    let buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buffer);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (id, _, text) in DOCUMENTS {
        zip.start_file(format!("{id}.md"), options)
            .map_err(|_| Error::new("bundle", "Cannot encode guides", 503))?;
        zip.write_all(text.as_bytes())?;
    }
    for &(name, content) in PACKAGED {
        zip.start_file(name, options)
            .map_err(|_| Error::new("bundle", "Cannot encode packaged guide", 503))?;
        zip.write_all(content.as_bytes())?;
    }
    for (name, value) in [
        ("manifest.json", manifest()),
        ("mcp-tools.json", json!({"tools":tools(actor)})),
        ("mcp-reference.json", reference(actor)?),
        (
            "job-contract.json",
            serde_json::from_str(include_str!("../../../contracts/job-contract.json"))?,
        ),
    ] {
        zip.start_file(name, options)
            .map_err(|_| Error::new("bundle", "Cannot encode guide manifest", 503))?;
        zip.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
    }
    let result = zip
        .finish()
        .map_err(|_| Error::new("bundle", "Cannot finalize guide bundle", 503))?
        .into_inner();
    require(
        result.len() <= 2 * 1024 * 1024,
        "bundle",
        "Guide bundle exceeds client limit",
        503,
    )?;
    Ok(result)
}
pub fn describe(actor: &Actor, group: &str, name: &str) -> Result<Value> {
    let action = contracts::action(group, name)?;
    actor.require(&action.scope)?;
    let documents: Value =
        serde_json::from_str(include_str!("../../../contracts/action-docs.json"))?;
    let mut description = documents
        .get(format!("{group}.{name}"))
        .cloned()
        .ok_or_else(|| {
            Error::new(
                "reference",
                "Action reference is missing from this build",
                503,
            )
        })?;
    let obj = description.as_object_mut().unwrap();
    obj.insert("group".into(), json!(group));
    obj.insert("action".into(), json!(name));
    obj.insert("scope".into(), json!(action.scope));
    obj.insert("schema".into(), action.schema);
    obj.insert(
        "access".into(),
        json!(
            "Shared workspace; operation scopes, recipients and exact installation grants apply."
        ),
    );
    Ok(description)
}
pub fn reference(actor: &Actor) -> Result<Value> {
    let mut groups = vec![];
    for group in [
        "jobs",
        "recovery",
        "events",
        "components",
        "results",
        "system",
    ] {
        let mut actions = vec![];
        for action in contracts::actions()
            .into_iter()
            .filter(|a| a.group == group && actor.scopes.contains(&a.scope))
        {
            actions.push(describe(actor, group, &action.action)?);
        }
        if !actions.is_empty() {
            groups.push(json!({"name":group,"description":match group{"jobs"=>"Job execution and shared queue","recovery"=>"Bounded incident repair","events"=>"Recipient lifecycle events","components"=>"Profiles, Components, Tools and Runtimes","results"=>"Results and versioned Scenario context",_=>"Installation status, contracts and guides"},"actions":actions}));
        }
    }
    Ok(
        json!({"revision":"loom-ops/v55","guidance":{"group":"system","action":"guide"},"groups":groups,"transport":{"mcp":"POST /mcp","http":"POST /api/actions","result":"JSON is in result.content[0].text. Version identity is in _meta.loom; do not concatenate text blocks."},"workflow":[{"step":"Inspect profiles","group":"components","action":"profiles","note":"Reuse exact compatible Profiles before authoring."},{"step":"Submit","group":"jobs","action":"submit","note":"One requested outcome; internal Steps or pinned DAG nodes."},{"step":"Inspect","group":"jobs","action":"get","note":"Read current state, independent scheduling facts and actual execution evidence."},{"step":"Consume events","group":"events","action":"list","note":"Use kind=all; ACK only acknowledges receipt."},{"step":"Repair","group":"recovery","action":"claim","note":"Use the current lease and shared three-round budget."}]}),
    )
}

#[cfg(test)]
#[path = "../../../../tests/rust/documentation.rs"]
mod tests;
