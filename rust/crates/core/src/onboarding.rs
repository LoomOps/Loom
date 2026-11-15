//! Live instructions and exact manifest hashes are served from the selected binary.
use crate::{auth::Actor, contracts, error::require, store::hash, Error, Result};
use serde_json::{json, Value};
const DOCUMENTS: &[(&str, &str, &str)] = &[
    (
        "items",
        "Items, files and logs",
        include_str!("../../../contracts/items.md"),
    ),
    (
        "entry",
        "Loom Use",
        include_str!("../../../../loom_use/entry.md"),
    ),
    (
        "quickstart",
        "First use",
        include_str!("../../../../loom_use/quickstart.md"),
    ),
    (
        "operator",
        "Administrator operations",
        include_str!("../../../../loom_use/operator.md"),
    ),
    (
        "inventory",
        "Package inventory",
        include_str!("../../../../loom_use/inventory.md"),
    ),
    ("jobs", "Jobs", include_str!("../../../../loom_use/jobs.md")),
    (
        "components",
        "Components and Tools",
        include_str!("../../../../loom_use/components.md"),
    ),
    (
        "runtimes",
        "Runtimes",
        include_str!("../../../../loom_use/runtimes.md"),
    ),
    (
        "recovery",
        "Recovery",
        include_str!("../../../../loom_use/recovery.md"),
    ),
    (
        "updates",
        "Updates",
        include_str!("../../../../loom_use/updates.md"),
    ),
    (
        "scenarios",
        "Scenarios",
        include_str!("../../../contracts/scenarios.md"),
    ),
    (
        "authoring",
        "Rust authoring",
        include_str!("../../../contracts/authoring.md"),
    ),
];
pub fn identity() -> Value {
    let guide = hash(
        DOCUMENTS
            .iter()
            .map(|(id, _, text)| format!("{id}\0{text}"))
            .collect::<String>(),
    );
    let contract = hash(serde_json::to_vec(&contracts::actions()).expect("action registry"));
    json!({"build_id":env!("LOOM_BUILD_ID"),"api_revision":"loom-ops/v47","contract_revision":contract,"guide_revision":guide,"revision":hash(format!("{contract}{guide}")),"client_release":serde_json::from_str::<Value>(include_str!("../../../../loom_use/client-release.json")).expect("client release"),"engine":"rust","source_sha256":env!("LOOM_RUST_SOURCE")})
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
        json!({"manifest":manifest,"documents":DOCUMENTS.iter().map(|(id,title,_)|json!({"id":id,"title":title,"path":format!("{id}.md")})).collect::<Vec<_>>()}),
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
    for (name, content) in [
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
    ] {
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
    let mut description=documents.get(format!("{group}.{name}")).cloned().unwrap_or_else(||json!({"title":name.replace('_'," "),"description":match name{"scaffold"=>"Return a Rust SDK package draft. No build or installation.","validate_package"=>"Validate source paths and declared Python integrations without executing code.","bind_historical_location"=>"Verify a current copy and add a binding without replacing historical evidence.",_=>"Read or change versioned Scenario context using exact identities and explicit receipts."},"returns":"Result or an explicit conflict","example":{},"errors":["Unknown identity, stale revision, key conflict or unavailable installation grant."]}));
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
        json!({"revision":"loom-ops/v47","guidance":{"group":"system","action":"guide"},"groups":groups,"transport":{"mcp":"POST /mcp","http":"POST /api/actions","result":"JSON is in result.content[0].text. Version identity is in _meta.loom; do not concatenate text blocks."},"workflow":[{"step":"Inspect profiles","group":"components","action":"profiles","note":"Reuse exact compatible Profiles before authoring."},{"step":"Submit","group":"jobs","action":"submit","note":"One requested outcome; internal Steps or pinned DAG nodes."},{"step":"Inspect","group":"jobs","action":"get","note":"Read current state, independent scheduling facts and actual execution evidence."},{"step":"Consume events","group":"events","action":"list","note":"Use kind=all; ACK only acknowledges receipt."},{"step":"Repair","group":"recovery","action":"claim","note":"Use the current lease and shared three-round budget."}]}),
    )
}
