//! Real application/SQLite/filesystem tests. No installed instance or user data.
use super::*;
use serde_json::{json, Value};
use std::fs;
struct Fixture {
    app: Application,
    root: std::path::PathBuf,
    actor: auth::Actor,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("loom-rust-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let bulk = root.join("data");
        fs::create_dir(&bulk).unwrap();
        fs::write(
            bulk.join(".loom-storage.json"),
            r#"{"storage_id":"fixture-data"}"#,
        )
        .unwrap();
        let components = root.join("components");
        fs::create_dir(&components).unwrap();
        let actor = auth::person_actor("owner");
        let config: Config = serde_json::from_value(
            json!({"machine_id":"fixture","storage_id":"fixture-data","actors":[actor]}),
        )
        .unwrap();
        let app = Application::open(&root.join("runtime"), &bulk, &components, config).unwrap();
        Self { app, root, actor }
    }
    fn call(&mut self, group: &str, action: &str, args: Value) -> Result<Value> {
        self.app.execute(&self.actor.clone(), group, action, args)
    }
    fn submit(&mut self, key: &str) -> String {
        self.call("jobs","submit",json!({"key":key,"kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}})).unwrap()["id"].as_str().unwrap().into()
    }
    fn count(&self, table: &str) -> i64 {
        self.app
            .db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn schema_initialization_is_repeatable_and_preserves_rows() {
    let mut f = Fixture::new();
    f.submit("one");
    let reopened = Application::open(
        &f.app.local,
        &f.app.bulk,
        &f.app.components,
        f.app.config.clone(),
    )
    .unwrap();
    assert_eq!(
        reopened
            .db
            .query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn actual_connection_enforces_durability() {
    let f = Fixture::new();
    assert_eq!(
        f.app
            .db
            .query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        f.app
            .db
            .query_row("PRAGMA synchronous", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        f.app
            .db
            .query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
}
#[test]
fn scenario_revisions_are_immutable_and_keyed() {
    let mut f = Fixture::new();
    let args = json!({"key":"create","definition":{"label":"Fixture task","protocol":{"criterion":"recorded"}}});
    let first = f
        .call("results", "register_scenario", args.clone())
        .unwrap();
    assert_eq!(first, f.call("results", "register_scenario", args).unwrap());
    let id = first["id"].as_str().unwrap();
    let updated=f.call("results","update_scenario",json!({"key":"update","id":id,"expected_revision":1,"definition":{"label":"Updated"},"reason":"Protocol changed"})).unwrap();
    assert_eq!(updated["revision"], 2);
    let old = f
        .call("results", "get_scenario", json!({"id":id,"revision":1}))
        .unwrap();
    assert_eq!(old["revision"]["definition"]["label"], "Fixture task");
    assert_eq!(f.count("scenario_revisions"), 2);
}
#[test]
fn scenario_update_conflict_rolls_back() {
    let mut f = Fixture::new();
    let first = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"create","definition":{"label":"Fixture"}}),
        )
        .unwrap();
    let before = f.count("rust_receipts");
    assert!(f.call("results","update_scenario",json!({"key":"wrong","id":first["id"],"expected_revision":2,"definition":{"label":"Wrong"},"reason":"Conflict"})).is_err());
    assert_eq!(f.count("rust_receipts"), before);
    assert_eq!(f.count("scenario_revisions"), 1);
}
#[test]
fn accepted_job_pins_scenario_revision() {
    let mut f = Fixture::new();
    let s = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"s","definition":{"label":"Fixture"}}),
        )
        .unwrap();
    let job=f.call("jobs","submit",json!({"key":"one","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"scenario_ref":{"id":s["id"],"revision":1}})).unwrap();
    f.call("results","update_scenario",json!({"key":"s2","id":s["id"],"expected_revision":1,"definition":{"label":"Later"},"reason":"New protocol"})).unwrap();
    let detail = f.call("jobs", "get", json!({"id":job["id"]})).unwrap();
    assert_eq!(detail["scenario_ref"]["revision"], 1);
}
#[test]
fn submit_key_replays_and_changed_parameters_conflict() {
    let mut f = Fixture::new();
    let id = f.submit("one");
    assert_eq!(f.submit("one"), id);
    assert_eq!(f.count("jobs"), 1);
    assert!(f.call("jobs","submit",json!({"key":"one","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":1,"fail":false}})).is_err());
}
#[test]
fn queue_contains_operations_jobs() {
    let mut f = Fixture::new();
    let id = f.submit("operations");
    let q = f.call("jobs", "queue", json!({})).unwrap();
    assert!(q["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == id));
}
#[test]
fn presentation_parent_does_not_fold_execution() {
    let mut f = Fixture::new();
    let parent = f.submit("parent");
    let child=f.call("jobs","submit",json!({"key":"child","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"presentation":{"scope":"research","role":"support","parent_job":parent}})).unwrap();
    let list = f.call("jobs", "list", json!({"view":"all"})).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == child["id"]));
}
#[test]
fn profile_registration_never_runs_build_scripts() {
    let f = Fixture::new();
    let report=software::validate_package(&json!({"manifest":{"implementation":{"language":"rust"}},"files":{"Cargo.toml":"[package]\nname='fixture'\nversion='0.1.0'","build.rs":"panic!(\"must never execute\")","src/main.rs":"fn main(){}"}})).unwrap();
    assert_eq!(report["build_executed"], false);
    assert!(!f.root.join("ran-build").exists());
}
#[test]
fn source_traversal_is_rejected() {
    assert!(
        software::validate_package(&json!({"manifest":{},"files":{"../escape":"bad"}})).is_err()
    );
}
#[test]
fn invalid_command_has_no_partial_records() {
    let mut f = Fixture::new();
    assert!(f.call("jobs","submit",json!({"key":"invalid","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":true,"fail":false}})).is_err());
    assert_eq!(f.count("jobs"), 0);
    assert_eq!(f.count("job_revisions"), 0);
}
#[test]
fn no_write_scope_cannot_submit() {
    let mut f = Fixture::new();
    f.actor.scopes = vec!["read".into()];
    assert_eq!(f.call("jobs","submit",json!({"key":"denied","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}})).unwrap_err().status,403);
    assert_eq!(f.count("jobs"), 0);
}
fn fail_execution(f: &mut Fixture, key: &str) -> (String, execution::Work) {
    let id = f.submit(key);
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app
        .effect_accepted(&work, json!({"ticket":"fixture"}))
        .unwrap();
    f.app
        .stage_returned(&work, 1, json!({"returned":true,"exit_code":1}))
        .unwrap();
    (id, work)
}
#[test]
fn original_submission_key_replays_after_recovery_and_old_return_is_rejected() {
    let mut f = Fixture::new();
    let (id, old) = fail_execution(&mut f, "original");
    let lease = f.call("recovery", "claim", json!({"id":id})).unwrap();
    let resume = json!({"id":id,"lease":lease["lease"],"patch":{"params":{"seconds":1,"fail":false}},"reason":"Retry with changed duration"});
    let first = f.call("recovery", "resume", resume.clone()).unwrap();
    assert_eq!(first, f.call("recovery", "resume", resume).unwrap());
    assert_eq!(id, f.submit("original"));
    let next = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_ne!(old.attempt, next.attempt);
    assert_eq!(next.revision, 1);
    assert!(f
        .app
        .stage_returned(&old, 0, json!({"returned":true}))
        .is_err());
}
#[test]
fn submission_conflicts_on_execution_and_storage_change() {
    let mut f = Fixture::new();
    f.submit("same");
    assert_eq!(f.call("jobs","submit",json!({"key":"same","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"execution":{"max_attempts":2}})).unwrap_err().code,"idempotency_conflict");
    assert_eq!(f.call("jobs","submit",json!({"key":"same","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"output_storage":"data/1"})).unwrap_err().code,"idempotency_conflict");
}
#[test]
fn stale_revision_cannot_transition_even_when_attempt_and_state_match() {
    let mut f = Fixture::new();
    let id = f.submit("fence");
    let old = jobs::load(&f.app.db, &id).unwrap();
    f.app
        .db
        .execute("UPDATE rust_job_fences SET revision=1 WHERE job=?", [id])
        .unwrap();
    assert!(jobs::transition(&f.app.db, &old, loom_domain::Event::Start).is_err());
}
#[test]
fn cancellation_wins_unreadable_publication_and_no_entities_are_registered() {
    let mut f = Fixture::new();
    let id = f.submit("cancel-publish");
    let run = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app
        .stage_returned(&run, 0, json!({"returned":true}))
        .unwrap();
    f.call("jobs", "cancel", json!({"id":id})).unwrap();
    let publish = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert!(publish.publication);
    assert_eq!(
        f.app.publish_work(&publish, Value::Null).unwrap()["state"],
        "cancelled"
    );
    assert_eq!(f.count("entity_revisions"), 0);
    assert_eq!(
        f.call("jobs", "get", json!({"id":id})).unwrap()["state"],
        "cancelled"
    );
}
#[test]
fn expired_recovery_reservation_is_deferred_without_fabricating_execution() {
    let mut f = Fixture::new();
    let (id, _) = fail_execution(&mut f, "expired");
    f.app
        .db
        .execute("UPDATE recovery SET deadline=0 WHERE job=?", [&id])
        .unwrap();
    assert!(f.app.next_work(&Default::default()).unwrap().is_none());
    assert_eq!(
        f.call("jobs", "get", json!({"id":id})).unwrap()["recovery"]["state"],
        "deferred"
    );
    assert_eq!(f.count("attempts"), 1);
}
#[test]
fn publication_retry_keeps_attempt_and_execution_count() {
    let mut f = Fixture::new();
    let id = f.submit("result-only");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app
        .stage_returned(&work, 0, json!({"returned":true}))
        .unwrap();
    for _ in 0..3 {
        let publication = f.app.next_work(&Default::default()).unwrap().unwrap();
        assert!(publication.publication);
        assert!(f.app.publish_work(&publication, json!({})).is_err());
        f.app
            .db
            .execute("UPDATE result_outbox SET next_try=0", [])
            .unwrap();
    }
    let lease = f.call("recovery", "claim", json!({"id":id})).unwrap();
    f.call(
        "recovery",
        "resume",
        json!({"id":id,"lease":lease["lease"]}),
    )
    .unwrap();
    let retry = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert!(retry.publication);
    assert_eq!(retry.attempt, work.attempt);
    assert_eq!(f.count("attempts"), 1);
}
#[test]
fn immutable_package_reconciles_rename_before_database_commit() {
    let f = Fixture::new();
    let files = json!({"src/main.rs":"fn main(){}"});
    let dest = f.root.join("revision");
    let first = storage::stage(&f.root, files.as_object().unwrap()).unwrap();
    storage::publish(&first, &dest).unwrap();
    let same = storage::stage(&f.root, files.as_object().unwrap()).unwrap();
    storage::publish(&same, &dest).unwrap();
    assert!(!same.exists());
    let changed = storage::stage(
        &f.root,
        json!({"src/main.rs":"fn main(){panic!()}"})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert!(storage::publish(&changed, &dest).is_err());
}
#[test]
fn same_writer_lock_excludes_another_instance_process() {
    let f = Fixture::new();
    let _first = store::WriterLock::acquire(&f.app.local).unwrap();
    assert!(store::WriterLock::acquire(&f.app.local).is_err());
}
#[test]
fn schema_fences_legacy_writer_and_read_only_blocks_mutation() {
    let mut f = Fixture::new();
    assert_eq!(
        f.app
            .db
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
    f.app.read_only = true;
    assert_eq!(
        f.call("jobs", "submit", json!({})).unwrap_err().code,
        "read_only"
    );
    assert!(f.call("jobs", "queue", json!({})).is_ok());
}
#[test]
fn dependency_catalog_lists_software_units_and_filters_transitive_inventory() {
    let mut deps = vec![
        json!({"name":"Python","version":"3.11","scope":"image"}),
        json!({"name":"torch","version":"2.7","scope":"image"}),
    ];
    for n in 0..995 {
        deps.push(json!({"name":format!("transitive-{n}"),"version":"1"}));
    }
    let catalog = json!({"status":"ok","runtimes":[{"id":"one","revision":1,"manifest":{"dependencies":deps}}]});
    let units = runtime::dependencies(&catalog);
    assert_eq!(units["dependencies"].as_array().unwrap().len(), 2);
    assert!(units["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["category"] == "languages"));
}
#[test]
fn exact_runtime_binding_requires_grant_and_does_not_change_old_pin() {
    let mut f = Fixture::new();
    let manifest = json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[{"name":"Python","version":"3.11"}]});
    let sha = store::hash(compat::seal(&manifest));
    f.app
        .observe_runtimes(
            json!({"runtimes":[{"id":"python","revision":1,"sha256":sha,"manifest":manifest}]}),
        )
        .unwrap();
    let catalog = f.app.runtime_inventory().unwrap();
    let mut p = json!({"runtime":{"runtime_ref":{"id":"python","revision":1},"command":["/component/runner"]}});
    runtime::bind(&catalog, &mut p).unwrap();
    assert_eq!(p["runtime"]["runtime_pin"]["sha256"], sha);
    assert!(runtime::require_dependencies(&json!({"requires":{"Python":"3.11"}}), &p).is_ok());
    assert!(runtime::require_dependencies(&json!({"requires":{"Python":"3.10"}}), &p).is_err());
    let mut absent = json!({"runtime":{"runtime_ref":{"id":"other","revision":1}}});
    assert!(runtime::bind(&catalog, &mut absent).is_err());
}
#[test]
fn unicode_identity_casefold_matches_preserved_user_ids() {
    assert_eq!(auth::casefold("Straße@EXAMPLE.com"), "strasse@example.com");
    assert_eq!(auth::casefold("Σςσ"), "σσσ");
}
#[test]
fn all_legacy_casefold_identities_match_python_312_unicode_15() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/casefold.json")).unwrap();
    for (input, expected) in fixture["mappings"].as_object().unwrap() {
        assert_eq!(
            auth::casefold(input),
            expected.as_str().unwrap(),
            "casefold {input:?}"
        );
    }
}
#[test]
fn numeric_and_unicode_seals_match_independent_python_namespaces() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/json-namespaces.json")).unwrap();
    for case in fixture.as_array().unwrap() {
        let value: Value = serde_json::from_str(case["input"].as_str().unwrap()).unwrap();
        assert_eq!(compat::dump(&value), case["dump"].as_str().unwrap());
        assert_eq!(compat::seal(&value), case["seal"].as_str().unwrap());
    }
}
#[test]
fn research_curation_rejects_cycles_and_unknown_items_atomically() {
    let mut f = Fixture::new();
    let a = f.submit("a");
    let b = f.submit("b");
    assert!(f.call("jobs","curate",json!({"key":"cycle","reason":"review","dry_run":false,"changes":[{"kind":"job","id":a,"presentation":{"parent_job":b}},{"kind":"job","id":b,"presentation":{"parent_job":a}}]})).is_err());
    assert_eq!(f.count("research_overlays"), 0);
    assert!(f.call("jobs","curate",json!({"key":"unknown","reason":"review","dry_run":false,"changes":[{"kind":"tool","id":"unknown","presentation":{"role":"support"}}]})).is_err());
}
#[test]
fn delivery_versions_replay_and_recipient_grants_are_preserved() {
    let mut f = Fixture::new();
    let (id, work) = fail_execution(&mut f, "delivery");
    let events = f.call("events", "list", json!({"kind":"all"})).unwrap();
    let seq = events[0]["seq"].clone();
    let args = json!({"event":seq,"state":"sent","receipt":"fixture-receipt","version":0});
    let first = f
        .app
        .record_delivery(&f.actor.clone(), args.clone())
        .unwrap();
    assert_eq!(
        first,
        f.app.record_delivery(&f.actor.clone(), args).unwrap()
    );
    assert_eq!(f.count("delivery_history"), 1);
    assert!(f
        .app
        .record_delivery(
            &f.actor.clone(),
            json!({"event":seq,"state":"completed","receipt":"out-of-order","version":1})
        )
        .is_err());
    let _ = (id, work);
}

#[test]
fn recovery_patches_accumulate_without_changing_original_idempotency() {
    let mut f = Fixture::new();
    let (id, _) = fail_execution(&mut f, "patched");
    let claim = f.call("recovery", "claim", json!({"id":id})).unwrap();
    f.call("recovery","resume",json!({"id":id,"lease":claim["lease"],"patch":{"params":{"seconds":2,"fail":false}},"reason":"First repair"})).unwrap();
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app
        .effect_accepted(&work, json!({"ticket":"second"}))
        .unwrap();
    f.app
        .stage_returned(&work, 1, json!({"returned":true,"exit_code":1}))
        .unwrap();
    let claim = f.call("recovery", "claim", json!({"id":id})).unwrap();
    f.call("recovery","resume",json!({"id":id,"lease":claim["lease"],"patch":{"execution":{"max_attempts":1}},"reason":"Second repair"})).unwrap();
    assert_eq!(
        f.call("jobs", "get", json!({"id":id})).unwrap()["spec"]["params"]["seconds"],
        2
    );
    assert_eq!(f.submit("patched"), id);
}
#[test]
fn workflow_hold_and_resume_apply_only_to_real_waiting_members() {
    let mut f = Fixture::new();
    let spec = json!({"kind":"cpu.smoke","params":{"seconds":0,"fail":false}});
    let w=f.call("jobs","submit_workflow",json!({"key":"hold-plan","label":"Outcome","owner":"owner","notify":"owner","nodes":[{"id":"a","label":"First","spec":spec},{"id":"b","label":"Second","depends_on":["a"],"spec":spec}],"outputs":{"out":{"node":"b","entity":"observation"}}})).unwrap();
    f.call(
        "jobs",
        "hold",
        json!({"id":w["id"],"reason":"Operator hold"}),
    )
    .unwrap();
    assert!(f.app.next_work(&Default::default()).unwrap().is_none());
    f.call(
        "jobs",
        "resume",
        json!({"id":w["id"],"reason":"Operator resume"}),
    )
    .unwrap();
    assert!(f.app.next_work(&Default::default()).unwrap().is_some());
}
#[test]
fn reserved_attempt_does_not_claim_an_actual_process_start() {
    let mut f = Fixture::new();
    let id = f.submit("reserve");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    let detail = f.call("jobs", "get", json!({"id":id})).unwrap();
    assert!(detail["attempts"][0]["started"].is_null());
    assert!(detail["attempts"][0]["reserved_at"].is_number());
    f.app
        .effect_accepted(&work, json!({"ticket":"accepted"}))
        .unwrap();
    assert!(f.call("jobs", "get", json!({"id":id})).unwrap()["attempts"][0]["started"].is_number());
}
#[test]
fn expired_claim_cannot_reissue_an_expired_recovery_lease() {
    let mut f = Fixture::new();
    let (id, _) = fail_execution(&mut f, "expire-claim");
    f.call("recovery", "claim", json!({"id":id})).unwrap();
    f.app
        .db
        .execute("UPDATE recovery SET deadline=0 WHERE job=?", [&id])
        .unwrap();
    assert!(f.call("recovery", "claim", json!({"id":id})).is_err());
}
#[test]
fn exact_historical_file_binding_preserves_original_availability() {
    let mut f = Fixture::new();
    let bytes = b"original bytes";
    let sha = store::hash(bytes);
    fs::write(f.app.bulk.join("preserved.txt"), bytes).unwrap();
    let body = json!({"payload_status":"missing","locations":[{"source_ref":"old/a.txt","sha256":sha,"status":"missing"},{"source_ref":"old/b.txt","sha256":"b".repeat(64),"status":"missing"}]});
    f.app.db.execute("INSERT INTO history_records(source,kind,id,revision,snapshot,body) VALUES('catalog','dataset','old','r1','s1',?)",[body.to_string()]).unwrap();
    let identity =
        json!({"source":"catalog","kind":"dataset","id":"old","revision":"r1","snapshot":"s1"});
    let mut args = json!({"key":"bind","identity":identity,"locator":{"machine_id":"fixture","storage_id":"fixture-data","ref":"preserved.txt"},"sha256":sha,"size":bytes.len()});
    assert!(f
        .call("results", "bind_historical_location", args.clone())
        .is_err());
    args["identity"]["file"] = json!("old/b.txt");
    assert!(f
        .call("results", "bind_historical_location", args.clone())
        .is_err());
    args["identity"]["file"] = json!("old/a.txt");
    let bound = f
        .call("results", "bind_historical_location", args.clone())
        .unwrap();
    assert_eq!(
        bound,
        f.call("results", "bind_historical_location", args).unwrap()
    );
    let old = f
        .call(
            "results",
            "historical",
            json!({"source":"catalog","kind":"dataset","id":"old","revision":"r1"}),
        )
        .unwrap();
    assert_eq!(old["body"], body);
    assert_eq!(old["current_bindings"].as_array().unwrap().len(), 1);
}
#[test]
fn imported_job_views_remain_readable_without_creating_execution_rows() {
    let mut f = Fixture::new();
    let entity = json!({"id":"work:old","label":"Preserved Job","kind":"job","created":1,"properties":{"state":"paused","record_role":"work","created_at":1},"steps":[{"id":"step-a"}]});
    for snapshot in ["old", "new"] {
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES('queue','job_view','work:old',?,?,?)",
                rusqlite::params![snapshot, snapshot, json!({"entity":entity}).to_string()],
            )
            .unwrap();
    }
    let detail = f.call("jobs", "get", json!({"id":"work:old"})).unwrap();
    assert_eq!(detail["state"], "paused");
    assert_eq!(detail["migration"]["read_only"], true);
    assert_eq!(detail["loom_history"]["snapshot"], "new");
    assert_eq!(
        f.call("jobs", "list", json!({"view":"all"}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(f.count("jobs"), 0);
    assert_eq!(f.count("attempts"), 0);
    assert_eq!(
        f.app.dashboard_catalog().unwrap()["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["id"] == "work:old")
            .count(),
        1
    );
}
#[test]
fn read_only_queue_controls_do_not_advertise_write_capabilities() {
    let mut f = Fixture::new();
    f.submit("readonly-queue");
    f.actor.scopes = vec!["read".into()];
    let queue = f.call("jobs", "queue", json!({})).unwrap();
    assert_eq!(queue["entries"][0]["can_hold"], false);
    assert_eq!(queue["entries"][0]["can_reorder"], false);
}
#[test]
fn job_history_filters_keep_classification_when_a_curation_overlay_exists() {
    let mut f = Fixture::new();
    let id = f.submit("overlay-filter");
    f.call("jobs","curate",json!({"key":"curate","reason":"Reclassify","dry_run":false,"changes":[{"kind":"job","id":id,"presentation":{"scope":"research","role":"main"}}]})).unwrap();
    let list = f
        .call(
            "jobs",
            "list",
            json!({"view":"research","category":"other","limit":1}),
        )
        .unwrap();
    assert_eq!(list[0]["id"], id);
}
#[test]
fn multiple_failed_workflow_nodes_emit_one_public_failure() {
    let mut f = Fixture::new();
    let spec = json!({"kind":"cpu.smoke","params":{"seconds":0,"fail":true}});
    let root=f.call("jobs","submit_workflow",json!({"key":"failed-plan","label":"One outcome","owner":"owner","notify":"owner","nodes":[{"id":"a","label":"A","spec":spec},{"id":"b","label":"B","spec":spec}],"outputs":{"out":{"node":"b","entity":"observation"}}})).unwrap();
    for _ in 0..2 {
        let work = f.app.next_work(&Default::default()).unwrap().unwrap();
        f.app
            .effect_accepted(&work, json!({"ticket":work.job}))
            .unwrap();
        f.app
            .stage_returned(&work, 1, json!({"returned":true,"exit_code":1}))
            .unwrap();
    }
    let events = f.call("events", "list", json!({"kind":"all"})).unwrap();
    assert_eq!(
        events
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["job"] == root["id"] && e["code"] == "failed")
            .count(),
        1
    );
    assert_eq!(f.count("rust_node_events"), 2);
}
#[test]
fn observer_freshness_uses_the_recorded_clock_and_timezone_offset() {
    let clock = 1735689600.;
    assert!(catalog::observation_fresh(
        &json!({"observed_at":"2025-01-01T09:00:00+09:00"}),
        clock
    ));
    assert!(catalog::observation_fresh(
        &json!({"observed_at":"2024-12-31T23:59:59.125Z"}),
        clock
    ));
    assert!(!catalog::observation_fresh(
        &json!({"observed_at":"2024-01-01T00:00:00Z"}),
        clock
    ));
    assert!(!catalog::observation_fresh(
        &json!({"observed_at":clock+1.}),
        clock
    ));
    assert!(!catalog::observation_fresh(
        &json!({"observed_at":"invalid"}),
        clock
    ));
}
#[test]
fn workflow_queue_moves_its_remaining_nodes_as_one_outcome() {
    let mut f = Fixture::new();
    let first = f.submit("ahead");
    let spec = json!({"kind":"cpu.smoke","params":{"seconds":0,"fail":false}});
    let root=f.call("jobs","submit_workflow",json!({"key":"move-plan","label":"One outcome","owner":"owner","notify":"owner","nodes":[{"id":"a","label":"A","spec":spec},{"id":"b","label":"B","depends_on":["a"],"spec":spec}],"outputs":{"out":{"node":"b","entity":"observation"}}})).unwrap();
    f.call(
        "jobs",
        "reorder",
        json!({"id":root["id"],"position":"next","reason":"Prioritize outcome"}),
    )
    .unwrap();
    let queue = f.call("jobs", "queue", json!({})).unwrap();
    let entries = queue["entries"].as_array().unwrap();
    assert_eq!(entries[2]["id"], first);
    assert!(entries[..2].iter().all(|e| e["workflow"] == root["id"]));
}
#[test]
fn bound_historical_preview_reads_verified_copy_without_promoting_a_record() {
    let mut f = Fixture::new();
    let content = b"preserved text";
    let sha = store::hash(content);
    fs::write(f.app.bulk.join("copy.txt"), content).unwrap();
    let identity = json!({"source":"catalog","kind":"dataset","id":"preserved","revision":"r1","snapshot":"s1"});
    let body = json!({"original":{"id":"preserved","label":"Preserved","kind":"dataset","properties":{}},"locations":[{"source_ref":"old.txt","sha256":sha,"size":content.len(),"status":"missing"}],"payload_status":"missing"});
    f.app
        .db
        .execute(
            "INSERT INTO history_records VALUES('catalog','dataset','preserved','r1','s1',?)",
            [body.to_string()],
        )
        .unwrap();
    let mut selected = identity.clone();
    selected["file"] = json!("old.txt");
    f.call("results","bind_historical_location",json!({"key":"preview-bind","identity":selected,"locator":{"machine_id":"fixture","storage_id":"fixture-data","ref":"copy.txt"},"sha256":sha,"size":content.len()})).unwrap();
    let catalog = f.app.dashboard_catalog().unwrap();
    let entity = catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "preserved")
        .unwrap();
    let preview = entity["loom_media"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["status"] == "available")
        .unwrap();
    let descriptor = f
        .app
        .media_lookup(http::HttpRequest {
            method: "GET".into(),
            path: format!("/api/dashboard/media/{}", preview["id"].as_str().unwrap()),
            headers: Default::default(),
            peer: "127.0.0.1".into(),
            body: Value::Null,
            dev: true,
        })
        .unwrap();
    assert_eq!(media::read(descriptor).unwrap().body, content);
    assert_eq!(f.count("entity_revisions"), 0);
    assert_eq!(f.count("jobs"), 0);
}
#[test]
fn projection_revision_changes_for_domain_facts_but_not_profile_heartbeat() {
    let mut f = Fixture::new();
    let first = f.app.dashboard_catalog().unwrap();
    assert_eq!(first, f.app.dashboard_catalog().unwrap());
    f.submit("changed-catalog");
    let next = f.app.dashboard_catalog().unwrap();
    assert_ne!(first["revision"], next["revision"]);
    assert!(next["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "job"));
    f.app.db.execute("INSERT INTO user_profiles(id,provider,login,display_name,updated) VALUES('person','test','p@example.test','Person',1)",[]).unwrap();
    let registered = f.app.dashboard_revision().unwrap();
    f.app
        .db
        .execute("UPDATE user_profiles SET updated=2 WHERE id='person'", [])
        .unwrap();
    assert_eq!(registered, f.app.dashboard_revision().unwrap());
    f.app
        .db
        .execute(
            "UPDATE user_profiles SET display_name='Renamed' WHERE id='person'",
            [],
        )
        .unwrap();
    assert_ne!(registered, f.app.dashboard_revision().unwrap());
}
#[test]
fn annotations_are_discoverable_and_responsibility_keeps_execution_owner_frozen() {
    let mut f = Fixture::new();
    let id = f.submit("responsibility");
    f.app.db.execute("INSERT INTO user_profiles(id,provider,login,display_name,updated) VALUES('responsible','test','r@example.test','Responsible',1)",[]).unwrap();
    let payload=f.call("results","annotate",json!({"id":id,"annotation_id":"input-locator","reason":"Preserved inputs","evidence":{"locator":"registered-copy"}})).unwrap();
    assert_eq!(payload["id"], "input-locator");
    f.call(
        "jobs",
        "assign_owner",
        json!({"id":id,"owner":"responsible"}),
    )
    .unwrap();
    let details = f.call("jobs", "get", json!({"id":id})).unwrap();
    assert_eq!(details["annotations"][0], payload);
    assert_eq!(details["owner"], "responsible");
    assert_eq!(details["recorded_owner"], "owner");
    assert_eq!(details["spec"]["owner"], "owner");
    assert_eq!(f.count("ownership_history"), 1);
    assert!(f
        .call("jobs", "assign_owner", json!({"id":id,"owner":"unknown"}))
        .is_err());
}
#[test]
fn archived_scenario_hides_from_discovery_but_does_not_gate_explicit_job_context() {
    let mut f = Fixture::new();
    let scenario = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"archived-context","definition":{"label":"Preserved protocol"}}),
        )
        .unwrap();
    f.call("results","archive_scenario",json!({"key":"archive","id":scenario["id"],"expected_revision":1,"reason":"Hide from default selection"})).unwrap();
    assert!(f
        .call("results", "scenarios", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let job=f.call("jobs","submit",json!({"key":"explicit-protocol","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"scenario_ref":{"id":scenario["id"],"revision":1}})).unwrap();
    assert_eq!(
        f.call("jobs", "get", json!({"id":job["id"]})).unwrap()["scenario_ref"]["revision"],
        1
    );
}
#[test]
fn cancellation_closes_failed_recovery_and_cannot_leave_a_ghost_queue_reservation() {
    let mut f = Fixture::new();
    let (id, _) = fail_execution(&mut f, "cancel-recovery");
    let claim = f.call("recovery", "claim", json!({"id":id})).unwrap();
    assert!(claim["lease"].is_string());
    let cancelled = f.call("jobs", "cancel", json!({"id":id})).unwrap();
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["recovery"]["state"], "closed");
    assert!(!f.call("jobs", "queue", json!({})).unwrap()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == id));
    assert!(f
        .call(
            "recovery",
            "resume",
            json!({"id":id,"lease":claim["lease"],"patch":{},"reason":"Stale repair"})
        )
        .is_err());
    assert_eq!(f.count("attempts"), 1);
}
#[test]
fn a_read_snapshot_cannot_be_reused_as_a_mutation_authority() {
    let mut f = Fixture::new();
    let r = http::HttpRequest {
        method: "POST".into(),
        path: "/api/actions".into(),
        headers: Default::default(),
        peer: "127.0.0.1".into(),
        dev: true,
        body: json!({"group":"jobs","action":"submit","arguments":{"key":"forged-read","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}}}),
    };
    let context = f.app.read_context(&r).unwrap();
    assert_eq!(context.http(r).unwrap_err().code, "query_snapshot");
    assert_eq!(f.count("jobs"), 0);
}
