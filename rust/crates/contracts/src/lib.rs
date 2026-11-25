//! Public contracts: immutable compatibility registry plus typed new application DTOs.
use loom_primitives::{error::require, Error, Result};
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
    let edit_authorization = object(
        json!({"basis":{"enum":["explicit_request","confirmed"]},"evidence":string}),
        &["basis", "evidence"],
    );
    result.push(Action {
        group: "results".into(),
        action: "adopt_output_revision".into(),
        operation: "results.adopt_output_revision".into(),
        scope: "submit".into(),
        schema: object(
            json!({"base":reference,"source":reference,"mode":{"enum":["continuation","replace_unused"],"default":"continuation"},"authorization":edit_authorization,"key":string}),
            &["base","source","authorization","key"],
        ),
    });
    result.push(Action {
        group: "results".into(),
        action: "group_evaluations".into(),
        operation: "results.group_evaluations".into(),
        scope: "submit".into(),
        schema: object(
            json!({"key":string,"label":{"type":"string","minLength":1,"maxLength":160},
                "experiments":{"type":"array","minItems":2,"maxItems":16,"uniqueItems":true,"items":reference}}),
            &["key","label","experiments"],
        ),
    });
    let target = object(
        json!({"kind":{"enum":["job","entity","historical","component","tool","profile","runtime","protocol_artifact"]},"identity":{"type":"object"}}),
        &["kind", "identity"],
    );
    result.push(Action {
        group: "results".into(),
        action: "set_output_disposition".into(),
        operation: "results.set_output_disposition".into(),
        scope: "submit".into(),
        schema: object(
            json!({"targets":{"type":"array","minItems":1,"maxItems":100,"uniqueItems":true,
                   "items":object(json!({"kind":{"enum":["entity","historical"]},"identity":{"type":"object"}}), &["kind","identity"])},
                   "state":{"enum":["current","discarded"]},"reason":string,"key":string}),
            &["targets","state","reason","key"],
        ),
    });
    let template_ref = object(
        json!({"id":{"type":"string","minLength":1,"maxLength":96},"revision":{"type":"integer","minimum":1},"sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"}}),
        &["id", "revision", "sha256"],
    );
    let registry_target = object(
        json!({"kind":{"enum":["component","tool","profile","runtime","metadata_template","report_contract","report_definition"]},"id":string,"revision":{"type":"integer","minimum":0},"owner":string}),
        &["kind", "id", "revision"],
    );
    let lifecycle_properties = json!({"targets":{"type":"array","minItems":1,"maxItems":100,"items":registry_target},"state":{"enum":["active","archived","deprecated"]}});
    let inspect = object(lifecycle_properties.clone(), &["targets", "state"]);
    let mut apply_properties = lifecycle_properties;
    apply_properties["reason"] = json!({"type":"string","minLength":1,"maxLength":2000});
    apply_properties["key"] = string.clone();
    apply_properties["expected_digest"] = json!({"type":"string","pattern":"^[a-f0-9]{64}$"});
    for (name, scope, schema) in [
        ("inspect_lifecycle", "read", inspect),
        (
            "set_lifecycle",
            "submit",
            object(
                apply_properties,
                &["targets", "state", "reason", "key", "expected_digest"],
            ),
        ),
    ] {
        result.push(Action {
            group: "components".into(),
            action: name.into(),
            operation: format!("components.{name}"),
            scope: scope.into(),
            schema,
        });
    }
    let pilot = object(
        json!({"preflight_for":string,"reduced_scope":string}),
        &["preflight_for", "reduced_scope"],
    );
    result
        .iter_mut()
        .find(|a| a.group == "jobs" && a.action == "submit")
        .unwrap()
        .schema["properties"]["pilot"] = pilot;
    result
        .iter_mut()
        .find(|a| a.group == "recovery" && a.action == "resume")
        .expect("recovery resume schema")
        .schema["properties"]["preview"] = json!({"type":"boolean"});
    result
        .iter_mut()
        .find(|a| a.group == "recovery" && a.action == "progress")
        .expect("recovery progress schema")
        .schema["properties"]["finding"] = json!({"type":"string","minLength":1,"maxLength":1000});
    let workflow = &result
        .iter()
        .find(|a| a.group == "jobs" && a.action == "submit_workflow")
        .expect("workflow schema")
        .schema["properties"];
    let local = &workflow["nodes"]["items"]["properties"];
    let runtime = object(
        json!({"id":string,"revision":{"type":"integer","minimum":1,"maximum":i64::MAX},"sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"}}),
        &["id", "revision"],
    );
    let gate = object(
        json!({"stage":{"enum":["prepare","runtime","finalize","on_complete","on_error","on_cancel"]},"runtime":runtime}),
        &["stage", "runtime"],
    );
    let mut execution = result
        .iter()
        .find(|a| a.group == "jobs" && a.action == "submit")
        .expect("submission schema")
        .schema
        .clone();
    for field in ["key", "owner", "notify", "presentation", "consolidates"] {
        execution["properties"]
            .as_object_mut()
            .unwrap()
            .remove(field);
    }
    execution["required"] = json!(["kind", "params"]);
    let slot = object(
        json!({"id":local["id"],"label":local["label"],"depends_on":local["depends_on"],"requires":{"type":"array","maxItems":6,"uniqueItems":true,"items":gate},"execution":execution}),
        &["id", "label"],
    );
    let plan = object(
        json!({"sealed":{"type":"boolean"},"units":{"type":"array","maxItems":64,"items":slot}}),
        &["sealed", "units"],
    );
    let output = object(
        json!({"alias":local["id"],"kind":{"enum":["artifact","dataset","model","experiment"]},"from":workflow["outputs"]["additionalProperties"]}),
        &["alias", "kind", "from"],
    );
    let receipt = object(
        json!({"alias":local["id"],"kind":{"const":"runtime_receipt"},"runtime":runtime}),
        &["alias", "kind", "runtime"],
    );
    let pin = object(
        json!({"id":string,"revision":{"type":"integer","minimum":1,"maximum":i64::MAX},"sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"}}),
        &["id", "revision", "sha256"],
    );
    let session = object(
        json!({"id":local["id"],"domain":{"enum":["simulation","real"]},"configuration":pin}),
        &["id", "domain", "configuration"],
    );
    let expected = object(
        json!({"activity":string,"template_ref":pin,"contract_ref":pin,
            "inputs":{"type":"object","maxProperties":64,"additionalProperties":reference},
            "sessions":{"type":"array","maxItems":64,"items":session}}),
        &["activity", "template_ref", "contract_ref"],
    );
    let hypothesis = object(
        json!({"statement":string,"analysis":string,"population":string,"metric":string,
            "aliases":{"type":"array","minItems":1,"maxItems":64,"uniqueItems":true,"items":local["id"]}}),
        &["statement", "analysis", "population", "metric", "aliases"],
    );
    let science = object(
        json!({"deliverables":{"type":"object","minProperties":1,"maxProperties":64,"additionalProperties":expected},"hypothesis":hypothesis}),
        &["deliverables"],
    );
    let outcome = object(
        json!({"goal":workflow["label"],"deliverables":{"type":"array","maxItems":256,"items":{"oneOf":[output,receipt]}},"science":science}),
        &["goal"],
    );
    let mut create = object(
        json!({"key":workflow["key"],"owner":string,"notify":string,"presentation":workflow["presentation"],"category":workflow["category"],"scenario_ref":reference,"outcome":outcome,"plan":plan}),
        &["key", "owner", "notify", "outcome", "plan"],
    );
    let mut revise = object(
        json!({"id":string,"key":workflow["key"],"expected_revision":{"type":"integer","minimum":1,"maximum":i64::MAX-1},"reason":string,"plan":plan}),
        &["id", "key", "expected_revision", "reason", "plan"],
    );
    let mut direct_outcome = outcome.clone();
    direct_outcome["properties"]["deliverables"]["items"] = object(
        json!({"alias":local["id"],"entity":local["id"],"kind":output["properties"]["kind"]}),
        &["alias", "entity", "kind"],
    );
    // An unbound reservation has one optional execution; historical plans retain their wire format.
    let run = object(
        json!({"label":local["label"],"requires":slot["properties"]["requires"],"execution":execution}),
        &["label"],
    );
    let single = object(json!({"sealed":{"type":"boolean"},"run":run}), &["sealed"]);
    create["properties"]["plan"] = json!({"oneOf":[plan,single]});
    revise["properties"]["plan"] = create["properties"]["plan"].clone();
    let gpu_access = object(
        json!({"mode":{"enum":["shared","exclusive"]},"memory_mib":{"type":"integer","minimum":1},"devices":{"type":"array","maxItems":8,"uniqueItems":true,"items":string}}),
        &["mode"],
    );
    let mut research_slot = slot.clone();
    research_slot["properties"]["phase"] = json!({"enum":["prepare","run","finalize"]});
    research_slot["properties"]["execution"]["properties"]["gpu_access"] = gpu_access.clone();
    let mut research_create = create.clone();
    research_create["required"]
        .as_array_mut()
        .unwrap()
        .push(json!("scenario_ref"));
    research_create["properties"]["plan"] = object(
        json!({"sealed":{"type":"boolean"},"units":{"type":"array","minItems":1,"maxItems":64,"items":research_slot}}),
        &["sealed", "units"],
    );
    let mut research_revise = revise.clone();
    research_revise["properties"]["plan"] = research_create["properties"]["plan"].clone();
    research_create["properties"]["outcome"]["properties"]["deliverables"] = json!({
        "type":"array","minItems":1,"maxItems":1,
        "items":object(json!({"alias":local["id"],"kind":{"enum":["dataset","model","experiment"]},
            "from":object(json!({"unit":local["id"],"entity":local["id"]}),&["unit","entity"]),
            "supplements":reference,"revises":reference,"edit_authorization":edit_authorization}), &["alias","kind","from"])
    });
    research_create["properties"]["outcome"]["required"] = json!(["goal", "deliverables"]);
    let mut preview_outcome = outcome.clone();
    preview_outcome["properties"]["deliverables"]["items"]["oneOf"]
        .as_array_mut()
        .unwrap()
        .push(direct_outcome["properties"]["deliverables"]["items"].clone());
    create["properties"]["outcome"] = preview_outcome.clone();
    let preview = object(json!({"outcome":preview_outcome}), &["outcome"]);
    result
        .iter_mut()
        .find(|a| a.group == "jobs" && a.action == "submit")
        .unwrap()
        .schema["properties"]["outcome"] = direct_outcome;
    let history = object(
        json!({"ids":{"type":"array","items":string,"minItems":1,"maxItems":24,"uniqueItems":true}}),
        &["ids"],
    );
    for (name, scope, schema) in [
        ("create", "submit", create),
        ("revise_plan", "submit", revise),
        ("submit_research", "submit", research_create),
        ("revise_research", "submit", research_revise),
        ("preview_outcome", "read", preview),
        ("preview_history", "read", history),
    ] {
        result.push(Action {
            group: "jobs".into(),
            action: name.into(),
            operation: format!("jobs.{name}"),
            scope: scope.into(),
            schema,
        });
    }
    let mut unit_submit = result
        .iter()
        .find(|a| a.group == "jobs" && a.action == "submit")
        .unwrap()
        .schema
        .clone();
    unit_submit["properties"]["gpu_access"] = gpu_access;
    unit_submit["properties"]["scenario_ref"] = reference.clone();
    for field in ["consolidates", "outcome"] {
        unit_submit["properties"]
            .as_object_mut()
            .unwrap()
            .remove(field);
    }
    result.push(Action {
        group: "units".into(),
        action: "submit".into(),
        operation: "units.submit".into(),
        scope: "submit".into(),
        schema: unit_submit,
    });
    for name in ["get", "hold", "resume", "cancel", "reorder"] {
        let mut action = result
            .iter()
            .find(|a| a.group == "jobs" && a.action == name)
            .unwrap()
            .clone();
        action.group = "units".into();
        action.operation = format!("units.{name}");
        result.push(action);
    }
    for name in ["pause", "resume_checkpoint"] {
        result.push(Action {
            group: "units".into(),
            action: name.into(),
            operation: format!("units.{name}"),
            scope: "submit".into(),
            schema: object(json!({"id":{"type":"string"},"attempt":{"type":"string"},"reason":{"type":"string","minLength":1,"maxLength":1024}}), &["id","attempt","reason"]),
        });
    }
    let metadata_kind = json!({"enum":["dataset","model","experiment"]});
    let definitions = [
        (
            "results",
            "metadata_templates",
            "read",
            object(
                json!({"kind":metadata_kind,"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":100},"include_revisions":{"type":"boolean","default":false}}),
                &[],
            ),
        ),
        (
            "results",
            "get_metadata_template",
            "read",
            object(
                json!({"id":string,"revision":{"type":"integer","minimum":1}}),
                &["id"],
            ),
        ),
        (
            "results",
            "register_metadata_template",
            "submit",
            object(
                json!({"id":{"type":"string","minLength":1,"maxLength":96},"definition":{"type":"object"},"expected_revision":{"type":"integer","minimum":0},"reason":{"type":"string","minLength":1,"maxLength":2000},"key":string}),
                &["id", "definition", "reason", "key"],
            ),
        ),
        (
            "results",
            "validate_metadata",
            "read",
            object(
                json!({"kind":metadata_kind,"template_ref":template_ref,"research":{"type":"object"}}),
                &["kind", "template_ref", "research"],
            ),
        ),
        (
            "results",
            "adopt_metadata",
            "submit",
            object(
                json!({"target":object(json!({"kind":{"enum":["entity","historical"]},"identity":{"type":"object"}}), &["kind","identity"]),"template_ref":template_ref,"research":{"type":"object"},"reason":{"type":"string","minLength":1,"maxLength":2000},"format_reference":target,"evidence":{"type":"array","maxItems":16,"items":{"type":"object"}},"dry_run":{"type":"boolean","default":true},"key":string,"expected_digest":{"type":"string","pattern":"^[a-f0-9]{64}$"}}),
                &["target", "template_ref", "research", "reason"],
            ),
        ),
        (
            "results",
            "organize_dataset",
            "submit",
            object(
                json!({"target":target,"task_ref":reference,"path":{"type":"array","maxItems":12,"items":object(json!({"id":{"type":"string","pattern":"^[A-Za-z0-9_.-]{1,64}$"},"label":{"type":"string","minLength":1,"maxLength":80}}),&["id","label"])},"handle":{"anyOf":[dataset_handle_schema(),{"type":"null"}]},"tags":dataset_tags_schema(),"description":dataset_description_schema(),"properties":dataset_display_properties_schema(),"reason":{"type":"string","minLength":1,"maxLength":2000},"dry_run":{"type":"boolean","default":true},"expected_digest":{"type":"string","pattern":"^[a-f0-9]{64}$"},"key":string}),
                &["target", "task_ref", "reason"],
            ),
        ),
        (
            "results",
            "link_dataset",
            "submit",
            object(
                json!({"target":target,"parent":target,"primary":{"type":"boolean","default":false},"transformation":object(json!({"kind":{"enum":TRANSFORMS},"details":{"type":"string","minLength":1,"maxLength":2000},"tool_ref":{"type":"object"},"config_ref":{"type":"object"},"member_count":{"type":"integer","minimum":0}}),&["kind","details"]),"evidence":{"type":"array","minItems":1,"maxItems":16,"items":object(json!({"target":target,"component":string,"sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"},"pointer":{"type":"string","maxLength":2048},"archive_member":{"type":"string","maxLength":2048},"member_sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"}}),&["target","component","sha256"])},"reason":{"type":"string","minLength":1,"maxLength":2000},"dry_run":{"type":"boolean","default":true},"expected_digest":{"type":"string","pattern":"^[a-f0-9]{64}$"},"key":string}),
                &["target", "parent", "transformation", "evidence", "reason"],
            ),
        ),
        (
            "results",
            "refresh_metadata",
            "submit",
            object(
                json!({"target":object(json!({"kind":{"enum":["entity","historical"]},"identity":{"type":"object"}}), &["kind","identity"]),"dry_run":{"type":"boolean","default":true},"key":string,"expected_source_sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"},"reason":{"type":"string","minLength":1,"maxLength":2000}}),
                &["target", "reason"],
            ),
        ),
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
    for (name, scope, schema) in [
        ("reconcile_contract", "read", object(json!({}), &[])),
        (
            "reconcile_inspect",
            "read",
            object(
                json!({"kinds":{"type":"array","items":{"type":"string"},"maxItems":16},"limit":{"type":"integer","minimum":1,"maximum":200},"cursor":{"type":"string","maxLength":4096}}),
                &[],
            ),
        ),
        (
            "reconcile_plan",
            "submit",
            object(
                json!({"key":string,"proposal":{"type":"object"}}),
                &["key", "proposal"],
            ),
        ),
        (
            "reconcile_get",
            "read",
            object(json!({"plan_id":string}), &["plan_id"]),
        ),
        (
            "reconcile_validate",
            "submit",
            object(json!({"plan_id":string}), &["plan_id"]),
        ),
        (
            "reconcile_apply",
            "submit",
            object(json!({"plan_id":string}), &["plan_id"]),
        ),
    ] {
        result.push(Action {
            group: "results".into(),
            action: name.into(),
            operation: format!("results.{name}"),
            scope: scope.into(),
            schema,
        });
    }
    for (group, name, scope, schema) in [
        (
            "system",
            "prune_search",
            "submit",
            object(
                json!({"key":string,"max_generations":{"type":"integer","minimum":1,"maximum":8}}),
                &["key"],
            ),
        ),
        (
            "system",
            "retry_catalog_task",
            "submit",
            object(json!({"id":string}), &["id"]),
        ),
        (
            "system",
            "rebuild_search",
            "submit",
            object(
                json!({"key":string,"max_batches":{"type":"integer","minimum":1,"maximum":100000}}),
                &["key"],
            ),
        ),
        (
            "system",
            "catalog_task",
            "read",
            object(json!({"id":string}), &["id"]),
        ),
        (
            "jobs",
            "prune_logs",
            "submit",
            object(
                json!({"key":string,"reason":string,"streams":{"type":"array","minItems":1,"maxItems":200,"items":{"type":"object","properties":{"stream_id":{"type":"string"},"source_generation":{"type":"string"}},"required":["stream_id","source_generation"],"additionalProperties":false}}}),
                &["key", "reason", "streams"],
            ),
        ),
    ] {
        result.push(Action {
            group: group.into(),
            action: name.into(),
            operation: format!("{group}.{name}"),
            scope: scope.into(),
            schema,
        });
    }
    let item_id = json!({"type":"string","pattern":"^[1-9][0-9]{0,38}$"});
    for (name, schema) in [
        (
            "get_item",
            object(
                json!({"item_id":item_id,"revision_id":item_id}),
                &["item_id"],
            ),
        ),
        (
            "resolve_item",
            object(
                json!({"native_ref":{"type":"object","properties":{"kind":{"type":"string"},"identity":{"type":"object"}},"required":["kind","identity"],"additionalProperties":false}}),
                &["native_ref"],
            ),
        ),
    ] {
        result.push(Action {
            group: "results".into(),
            action: name.into(),
            operation: format!("results.{name}"),
            scope: "read".into(),
            schema,
        });
    }
    let mut search = object(
        json!({"text":{"type":"string","maxLength":1024},"kinds":{"type":"array","items":{"type":"string"},"maxItems":16},"view":{"enum":["all","research","operations"]},"role":{"enum":["all","main","support","test"]},"scenario_ref":reference,"task_id":string,"parent_item_id":item_id,"category":{"type":"string","maxLength":128},"availability":{"enum":["verified","source_unavailable","recorded"]},"recorded_from":{"type":"number"},"recorded_to":{"type":"number"},"cursor":{"type":"string","maxLength":4096},"limit":{"type":"integer","minimum":1,"maximum":200}}),
        &[],
    );
    for (k, v) in [
        ("history", json!({"enum":["current","all"]})),
        ("origin_type", json!({"type":"string","maxLength":128})),
        ("include_system", json!({"type":"boolean","default":true})),
        ("native_kind", json!({"type":"string","maxLength":128})),
        ("dependency_ref", json!({"type":"object"})),
        (
            "at_least",
            json!({"type":"string","pattern":"^[0-9]{1,20}$"}),
        ),
        (
            "timeout_ms",
            json!({"type":"integer","minimum":0,"maximum":1000}),
        ),
        (
            "related_to",
            json!({"type":"object","properties":{"item_id":item_id,"relation":{"type":"string","maxLength":128}},"required":["item_id"],"additionalProperties":false}),
        ),
    ] {
        search["properties"][k] = v;
    }
    search["properties"]["catalog_classes"] = json!({"type":"array","maxItems":2,"uniqueItems":true,"items":{"enum":["regular","reference"]}});
    search["properties"]["output_state"] = json!({"enum":["current","discarded","all"]});
    let mut page = search.clone();
    page["properties"]["records_only"] = json!({"type":"boolean"});
    page["properties"]["projection"] = json!({"enum":["list","history","files"]});
    page["properties"]["directory"] = json!({"type":"string","maxLength":1024});
    page["properties"]["offset"] = json!({"type":"integer","minimum":0,"maximum":1000000});
    let mut files = search.clone();
    files["properties"]
        .as_object_mut()
        .unwrap()
        .remove("output_state");
    files["properties"]["item_id"] = item_id.clone();
    files["properties"]["revision_id"] = item_id.clone();
    files["required"] = json!(["item_id"]);
    for (group, name, schema) in [
        ("system", "dashboard_page", page),
        ("results", "search_items", search),
        ("results", "item_files", files),
        (
            "results",
            "get_file",
            object(json!({"file_item_id":item_id}), &["file_item_id"]),
        ),
        (
            "results",
            "read_file",
            object(
                json!({"file_item_id":item_id,"offset":{"type":"string","pattern":"^[0-9]{1,20}$"},"limit":{"type":"integer","minimum":1,"maximum":262144}}),
                &["file_item_id"],
            ),
        ),
        ("system", "search_status", object(json!({}), &[])),
    ] {
        result.push(Action {
            group: group.into(),
            action: name.into(),
            operation: format!("{group}.{name}"),
            scope: "read".into(),
            schema,
        });
    }
    let stream_args = json!({"id":{"type":"string"},"attempt":{"type":"string"},"stage":{"type":"string"},"after":{"type":"string","pattern":"^[0-9A-F]{32}$"},"limit":{"type":"integer","minimum":1,"maximum":200}});
    let mut search_logs = stream_args.clone();
    search_logs["cursor"] = json!({"type":"string","maxLength":4096});
    search_logs["text"] = json!({"type":"string","minLength":3,"maxLength":1024});
    search_logs["stream_id"] = item_id.clone();
    for (group, name, schema) in [
        (
            "results",
            "item_relations",
            object(
                json!({"item_id":item_id,"revision_id":item_id,"direction":{"enum":["outgoing","both"]},"include_support":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":200},"cursor":{"type":"string","maxLength":4096}}),
                &["item_id"],
            ),
        ),
        (
            "system",
            "dashboard_queue",
            object(json!({"compact":{"type":"boolean"}}), &[]),
        ),
        ("system", "dashboard_summary", object(json!({}), &[])),
        (
            "system",
            "dashboard_activity",
            object(
                json!({"view":{"enum":["research","operations","all"]}}),
                &[],
            ),
        ),
        (
            "system",
            "dashboard_entity",
            object(
                json!({"item_id":item_id,"revision_id":item_id,"records_only":{"type":"boolean"},
                    "section":{"enum":["overview","runtime_usage","runtime_inventory","profile_usage","files","training"]},
                    "offset":{"type":"integer","minimum":0,"maximum":1000000},
                    "limit":{"type":"integer","minimum":1,"maximum":200},
                    "text":{"type":"string","maxLength":256},
                    "origin":{"enum":["all","managed","external","image","deprecated","unknown"]}}),
                &["item_id"],
            ),
        ),
        ("jobs", "log_streams", object(stream_args, &["id"])),
        ("jobs", "search_logs", object(search_logs, &["text"])),
        (
            "jobs",
            "read_log",
            object(
                json!({"stream_id":item_id,"source_generation":{"type":"string","maxLength":128},"offset":{"type":"string","pattern":"^[0-9]{1,20}$"},"tail":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":262144}}),
                &["stream_id"],
            ),
        ),
    ] {
        result.push(Action {
            group: group.into(),
            action: name.into(),
            operation: format!("{group}.{name}"),
            scope: "read".into(),
            schema,
        });
    }
    for (name, scope, schema) in [
        (
            "report_contracts",
            "read",
            object(
                json!({"kind":{"enum":["dataset","model","experiment"]},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":100},"include_revisions":{"type":"boolean"}}),
                &[],
            ),
        ),
        (
            "report_definitions",
            "read",
            object(
                json!({"kind":{"enum":["metric","factor","protocol"]},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":100},"include_revisions":{"type":"boolean"}}),
                &[],
            ),
        ),
        (
            "get_report_contract",
            "read",
            object(
                json!({"id":string,"revision":{"type":"integer","minimum":1,"maximum":i64::MAX}}),
                &["id"],
            ),
        ),
        (
            "get_report_evidence",
            "read",
            object(
                json!({"target":{"type":"object","properties":{"kind":{"enum":["entity","historical","component","tool","profile"]},"identity":{"type":"object"}},"required":["kind","identity"],"additionalProperties":false},"pointer":{"type":"string","maxLength":256}}),
                &["target", "pointer"],
            ),
        ),
        (
            "get_report_definition",
            "read",
            object(
                json!({"id":string,"revision":{"type":"integer","minimum":1,"maximum":i64::MAX}}),
                &["id"],
            ),
        ),
        (
            "register_report_contract",
            "submit",
            object(
                json!({"id":string,"definition":{"type":"object"},"expected_revision":{"type":"integer","minimum":0,"maximum":i64::MAX-1},"reason":{"type":"string","minLength":1,"maxLength":2000},"key":string}),
                &["id", "definition", "reason", "key"],
            ),
        ),
        (
            "register_report_definition",
            "submit",
            object(
                json!({"id":string,"definition":{"type":"object"},"expected_revision":{"type":"integer","minimum":0,"maximum":i64::MAX-1},"reason":{"type":"string","minLength":1,"maxLength":2000},"key":string}),
                &["id", "definition", "reason", "key"],
            ),
        ),
        (
            "validate_report_contract",
            "read",
            object(json!({"definition":{"type":"object"}}), &["definition"]),
        ),
        (
            "read_report",
            "read",
            object(
                json!({"id":string,"revision":{"type":"integer","minimum":1,"maximum":i64::MAX},"requirement_id":string,"pointer":{"type":"string","maxLength":256},"offset":{"type":"integer","minimum":0,"maximum":10000000},"limit":{"type":"integer","minimum":1,"maximum":500}}),
                &["id", "revision", "requirement_id"],
            ),
        ),
    ] {
        result.push(Action {
            group: "results".into(),
            action: name.into(),
            operation: format!("results.{name}"),
            scope: scope.into(),
            schema,
        });
    }
    for entry in &mut result {
        if entry.group == "results" && entry.action == "historical" {
            entry.schema["properties"]["snapshot"] = json!({"type":"string"});
        }
        if entry.group == "jobs" && entry.action == "logs" {
            entry.schema["properties"]["offset"]["minimum"] = json!(0);
            entry.schema["properties"]["limit"]["minimum"] = json!(1);
            entry.schema["properties"]["limit"]["maximum"] = json!(65536);
        }
        if entry.group == "jobs" && entry.action == "list" {
            entry.schema["properties"]["view"]["default"] = json!("all");
        }
        if entry.group == "jobs" && matches!(entry.action.as_str(), "submit" | "submit_workflow") {
            entry.schema["properties"]["scenario_ref"] = reference.clone();
        }
    }
    for action in &mut result {
        if matches!(
            format!("{}.{}", action.group, action.action).as_str(),
            "components.list"
                | "components.list_tools"
                | "components.profiles"
                | "components.runtimes"
                | "results.metadata_templates"
                | "results.report_contracts"
                | "results.report_definitions"
                | "results.search_items"
                | "system.dashboard_page"
                | "results.item_files"
        ) {
            action.schema["properties"]["include_archived"] =
                json!({"type":"boolean","default":true});
            action.schema["properties"]["lifecycle_state"] =
                json!({"enum":["active","archived","deprecated"]});
        }
    }
    let groups = json!(result
        .iter()
        .map(|action| &action.group)
        .collect::<std::collections::BTreeSet<_>>());
    result
        .iter_mut()
        .find(|action| action.group == "system" && action.action == "describe")
        .expect("action discovery schema")
        .schema["properties"]["group"]["enum"] = groups;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_access: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pilot: Option<Value>,
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

pub fn dataset_handle_schema() -> Value {
    json!({"type":"string","minLength":1,"maxLength":64,"pattern":"^\\S(?:[^\\r\\n]*\\S)?$",
        "not":{"pattern":"[\\u0000-\\u001F\\u007F-\\u009F\\u2028\\u2029]"}})
}

pub fn dataset_tags_schema() -> Value {
    json!({"type":"array","maxItems":24,"uniqueItems":true,"items":{"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_./-]{0,63}$"}})
}

pub fn dataset_description_schema() -> Value {
    json!({"type":"string","maxLength":4096})
}

pub fn dataset_display_properties_schema() -> Value {
    json!({"type":"object","maxProperties":32,"propertyNames":{"minLength":1,"maxLength":96,"pattern":"\\S"},
        "additionalProperties":{"anyOf":[{"type":"string","maxLength":1024},{"type":"number"},{"type":"boolean"}]}})
}

pub const TRANSFORMS: &[&str] = &[
    "subset",
    "condition_selection",
    "includes",
    "relabel",
    "resample",
    "sensor_rerender",
    "augment",
    "merge",
];
