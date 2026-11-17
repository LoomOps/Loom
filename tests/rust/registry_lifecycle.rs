use super::*;
fn component(f: &mut Fixture, id: &str) -> Value {
    f.call(
        "components",
        "register",
        json!({"id":id,"manifest":{"label":id},"files":{"main.rs":"fn main() {}"}}),
    )
    .unwrap();
    json!({"kind":"component","owner":"owner","id":id,"revision":1})
}
fn preview(f: &mut Fixture, targets: Value, state: &str) -> Value {
    f.call(
        "components",
        "inspect_lifecycle",
        json!({"targets":targets,"state":state}),
    )
    .unwrap()
}
fn apply(f: &mut Fixture, targets: Value, state: &str, key: &str) -> Value {
    let p = preview(f, targets.clone(), state);
    json!({"targets":targets,"state":state,"key":key,"reason":"Remove mistaken registration from discovery","expected_digest":p["digest"]})
}
#[test]
fn registry_archive_is_atomic_reversible_and_does_not_rewrite_sealed_sources() {
    let mut f = Fixture::new();
    let a = component(&mut f, "retired");
    let b = component(&mut f, "related");
    let original = f
        .call(
            "components",
            "get",
            json!({"owner":"owner","id":"retired","revision":1}),
        )
        .unwrap();
    let args = apply(&mut f, json!([a]), "archived", "archive");
    let r = f.call("components", "set_lifecycle", args.clone()).unwrap();
    assert_eq!(r["changed"], json!([a]));
    assert_eq!(r, f.call("components", "set_lifecycle", args).unwrap());
    let archived = f
        .call(
            "components",
            "get",
            json!({"owner":"owner","id":"retired","revision":1}),
        )
        .unwrap();
    assert_eq!(archived["manifest"], original["manifest"]);
    assert_eq!(archived["profiles"], original["profiles"]);
    assert_eq!(archived["lifecycle"]["state"], "archived");
    let list = f
        .call("components", "list", json!({"include_archived":false}))
        .unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], b["id"]);
    let p = preview(&mut f, json!([a]), "active");
    assert_eq!(p["targets"][0]["history"][0]["state"], "archived");
    let args = apply(&mut f, json!([a]), "active", "restore");
    f.call("components", "set_lifecycle", args).unwrap();
    assert_eq!(
        f.call("components", "list", json!({"include_archived":false}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn registry_mixed_ownership_builtin_and_stale_preview_leave_every_target_unchanged() {
    let mut f = Fixture::new();
    let a = component(&mut f, "own");
    let mut outsider = component(&mut f, "foreign");
    f.app
        .db
        .execute("UPDATE components SET owner='other' WHERE id='foreign'", [])
        .unwrap();
    outsider["owner"] = json!("other");
    let args = apply(&mut f, json!([a, outsider]), "archived", "unauthorized");
    assert_eq!(
        f.call("components", "set_lifecycle", args)
            .unwrap_err()
            .code,
        "registry_owner"
    );
    assert_eq!(f.count("registry_lifecycle_events"), 0);
    let builtin = json!({"kind":"metadata_template","id":"loom.dataset","revision":1});
    let args = apply(&mut f, json!([a, builtin]), "archived", "protected");
    assert_eq!(
        f.call("components", "set_lifecycle", args)
            .unwrap_err()
            .code,
        "registry_builtin"
    );
    assert_eq!(f.count("registry_lifecycle_events"), 0);
    let stale = apply(&mut f, json!([a]), "archived", "stale");
    let args = apply(&mut f, json!([a]), "deprecated", "deprecate");
    f.call("components", "set_lifecycle", args).unwrap();
    assert_eq!(
        f.call("components", "set_lifecycle", stale)
            .unwrap_err()
            .code,
        "registry_preview_changed"
    );
    assert_eq!(f.count("registry_lifecycle_events"), 1);
    let missing = json!({"kind":"tool","owner":"owner","id":"absent","revision":1});
    assert!(f
        .call(
            "components",
            "inspect_lifecycle",
            json!({"targets":[a,missing],"state":"archived","evidence":"fixture protection"})
        )
        .is_err());
}
#[test]
fn registry_shared_refs_and_running_job_pins_survive_archive_and_restart() {
    let mut f = Fixture::new();
    let c = component(&mut f, "shared");
    let p = json!({"schema":"loom-execution/v2","revision":"user.run.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"executor":"container","runtime_id":"cpu","command":["/bin/true"]},"inputs":{},"outputs":null,"limits":{"timeout_seconds":30,"max_attempts":1},"component_path":f.root.join("components/owner/shared/revisions/1").to_string_lossy(),"component_ref":{"owner":"owner","id":"shared","revision":1}});
    f.app.config.profiles.insert("user.run.r1".into(), p);
    let job=f.call("jobs","submit",json!({"key":"accepted","kind":"user.run.r1","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}})).unwrap()["id"].as_str().unwrap().to_owned();
    let before = jobs::load(&f.app.db, &job).unwrap();
    let p = preview(&mut f, json!([c]), "archived");
    assert!(p["related"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["target"]["kind"] == "profile" && r["target"]["id"] == "user.run.r1"));
    assert_eq!(p["job_count"], 1);
    let args = apply(&mut f, json!([c]), "archived", "used-archive");
    f.call("components", "set_lifecycle", args.clone()).unwrap();
    assert_eq!(jobs::load(&f.app.db, &job).unwrap().spec, before.spec);
    assert_eq!(
        f.app.next_work(&Default::default()).unwrap().unwrap().job,
        job
    );
    let config = f.app.config.clone();
    f.app = Application::open(
        &f.root.join("runtime"),
        &f.root.join("data"),
        &f.root.join("components"),
        config,
    )
    .unwrap();
    assert_eq!(
        f.call(
            "components",
            "get",
            json!({"owner":"owner","id":"shared","revision":1})
        )
        .unwrap()["lifecycle"]["state"],
        "archived"
    );
    assert_eq!(
        f.call("components", "set_lifecycle", args).unwrap()["changed"],
        json!([c])
    );
}
#[test]
fn registry_template_filters_apply_before_pagination_and_builtin_prefix() {
    let mut f = Fixture::new();
    let base = f
        .call(
            "results",
            "get_metadata_template",
            json!({"id":"loom.dataset"}),
        )
        .unwrap();
    for id in ["a.dataset", "b.dataset"] {
        let d = json!({"label":id,"kind":"dataset","base_ref":metadata_templates::reference(&base),"schema":{"type":"object"},"fields":[]});
        f.call(
            "results",
            "register_metadata_template",
            json!({"id":id,"definition":d,"key":id,"reason":"fixture"}),
        )
        .unwrap();
    }
    let t = json!({"kind":"metadata_template","id":"a.dataset","revision":1});
    let args = apply(&mut f, json!([t]), "archived", "hide-template");
    f.call("components", "set_lifecycle", args).unwrap();
    let all = f
        .call("results", "metadata_templates", json!({"kind":"dataset"}))
        .unwrap();
    let visible = f
        .call(
            "results",
            "metadata_templates",
            json!({"kind":"dataset","include_archived":false}),
        )
        .unwrap();
    assert_eq!(
        all["total"].as_i64().unwrap() - visible["total"].as_i64().unwrap(),
        1
    );
    let archived = f
        .call(
            "results",
            "metadata_templates",
            json!({"lifecycle_state":"archived","limit":1}),
        )
        .unwrap();
    assert_eq!(archived["total"], 1);
    assert_eq!(archived["items"][0]["id"], "a.dataset");
    assert_eq!(archived["next_offset"], Value::Null);
    assert!(metadata_templates::get(&f.app.db, "a.dataset", Some(1))
        .unwrap()
        .get("lifecycle")
        .is_none());
}
#[test]
fn registry_visibility_is_authoritative_before_index_refresh_and_expires_cursors() {
    let mut f = Fixture::new();
    let a = component(&mut f, "a");
    component(&mut f, "b");
    component(&mut f, "c");
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..5 {
        f.app.index_once().unwrap();
    }
    let query =
        json!({"kinds":["component"],"view":"all","role":"all","include_archived":false,"limit":1});
    let page = f.call("results", "search_items", query.clone()).unwrap();
    assert!(page["next_cursor"].is_string());
    let args = apply(&mut f, json!([a]), "archived", "hide-before-index");
    f.call("components", "set_lifecycle", args).unwrap();
    let mut next = query.clone();
    next["cursor"] = page["next_cursor"].clone();
    assert_eq!(
        f.call("results", "search_items", next).unwrap_err().code,
        "cursor_expired"
    );
    let p = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["component"],"view":"all","role":"all","lifecycle_state":"archived"}),
        )
        .unwrap();
    assert_eq!(p["items"].as_array().unwrap().len(), 1);
    assert_eq!(p["items"][0]["native_ref"]["identity"]["id"], "a");
    let p = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["component"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(p["entities"].as_array().unwrap().len(), 2);
    // A new active revision does not erase access to an older archived revision.
    component(&mut f, "a");
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..5 {
        f.app.index_once().unwrap();
    }
    let p=f.call("system","dashboard_page",json!({"kinds":["component"],"view":"all","role":"all","history":"all","lifecycle_state":"archived"})).unwrap();
    assert_eq!(p["entities"].as_array().unwrap().len(), 1);
    assert_eq!(p["entities"][0]["loom_extension"]["revision"], 1);
    assert_eq!(p["entities"][0]["lifecycle"]["state"], "archived");
    assert_eq!(
        p["entities"][0]["loom_extension"]["revisions"][0]["lifecycle"]["state"],
        "archived"
    );
}
#[test]
fn registry_all_kinds_share_overlay_legacy_authorization_and_explicit_batch() {
    let mut f = Fixture::new();
    f.call(
        "components",
        "register_tool",
        json!({"id":"helper","manifest":{"label":"Helper"},"files":{"main.rs":"fn main() {}"}}),
    )
    .unwrap();
    let base = f
        .call(
            "results",
            "get_report_contract",
            json!({"id":"loom.report.dataset"}),
        )
        .unwrap();
    f.call("results","register_report_contract",json!({"id":"user.report","definition":base["definition"],"key":"report","reason":"fixture"})).unwrap();
    let base = f
        .call(
            "results",
            "get_report_definition",
            json!({"id":"loom.metric.success"}),
        )
        .unwrap();
    f.call("results","register_report_definition",json!({"id":"user.metric","definition":base["definition"],"key":"metric","reason":"fixture"})).unwrap();
    let manifest = json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[{"name":"Python","version":"3.11"}]});
    f.app.observe_runtimes(json!({"runtimes":[{"id":"python","revision":1,"sha256":store::hash(compat::seal(&manifest)),"manifest":manifest}]})).unwrap();
    f.app.config.profiles.insert("configured.r1".into(),json!({"schema":"loom-execution/v2","revision":"configured.r1","sha256":"a".repeat(64),"runtime":{"runtime_id":"cpu","command":["/bin/true"]},"inputs":{},"outputs":null,"params_schema":{"type":"object"},"limits":{"max_attempts":1,"timeout_seconds":30}}));
    let targets = json!([{"kind":"tool","owner":"owner","id":"helper","revision":1},{"kind":"runtime","id":"python","revision":1},{"kind":"profile","id":"configured.r1","revision":0},{"kind":"report_contract","id":"user.report","revision":1},{"kind":"report_definition","id":"user.metric","revision":1}]);
    let args = apply(&mut f, targets.clone(), "archived", "mixed-kinds");
    assert_eq!(
        f.call("components", "set_lifecycle", args)
            .unwrap_err()
            .code,
        "registry_owner"
    );
    let digest = store::hash("fixture admin");
    f.actor.token_sha256 = Some(digest.clone());
    f.app.config.actors[0].token_sha256 = Some(digest);
    let args = apply(&mut f, targets.clone(), "archived", "mixed-kinds");
    assert_eq!(
        f.call("components", "set_lifecycle", args).unwrap()["changed"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        f.call(
            "components",
            "list_tools",
            json!({"lifecycle_state":"archived"})
        )
        .unwrap()
        .as_array()
        .unwrap()
        .len(),
        1
    );
    for action in ["report_contracts", "report_definitions"] {
        let result = f
            .call(
                "results",
                action,
                json!({"lifecycle_state":"archived","limit":1}),
            )
            .unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["items"][0]["lifecycle"]["state"], "archived");
    }
    assert_eq!(
        f.call("components", "runtimes", json!({"include_archived":false}))
            .unwrap()["runtimes"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let p = f
        .call(
            "components",
            "profiles",
            json!({"lifecycle_state":"archived"}),
        )
        .unwrap();
    assert_eq!(p.as_array().unwrap().len(), 1);
    f.call(
        "components",
        "curate",
        json!({"type":"profile","id":"configured.r1","state":"active","evidence":"fixture restoration"}),
    )
    .unwrap();
    assert_eq!(f.count("registry_lifecycle_events"), 6);
    f.call("components","curate",json!({"type":"profile","id":"cpu.smoke","label":"Smoke display label","evidence":"Display edit only"})).unwrap();
    assert_eq!(
        f.count("registry_lifecycle_events"),
        6,
        "Display edits must not create lifecycle transitions"
    );

    assert_eq!(
        f.call(
            "components",
            "curate",
            json!({"type":"profile","id":"cpu.smoke","state":"archived","evidence":"fixture protection"})
        )
        .unwrap_err()
        .code,
        "registry_builtin"
    );
    // A scope label alone cannot confer installation administration.
    f.actor.token_sha256 = None;
    f.actor.scopes.push("admin".into());
    assert_eq!(
        f.call(
            "components",
            "curate",
            json!({"type":"runtime","id":"python","revision":1,"state":"active","evidence":"fixture restoration"})
        )
        .unwrap_err()
        .code,
        "registry_owner"
    );
}
