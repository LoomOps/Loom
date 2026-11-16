//! Real application/SQLite/filesystem tests. No installed instance or user data.
use super::*;
use crate::store::hash;
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
        7
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
fn dashboard_host_and_inspector_preserve_observation_freshness_and_sections() {
    let mut f = Fixture::new();
    f.app.config.extra.insert(
        "inventory".into(),
        json!({"docker_engine":"old-install-value","gpu":"old-driver"}),
    );
    let missing = f.app.resources().unwrap();
    assert_eq!(missing["host"]["machine_id"], "fixture");
    assert_eq!(missing["host"]["name"], "fixture");
    assert_eq!(missing["host"]["status"], "unavailable");
    assert_eq!(missing["sections"]["memory"]["status"], "ok");
    assert!(missing["sections"]["leases"]["data"]["registered"].is_array());
    assert_eq!(missing["sections"]["leases"]["data"]["queued_jobs"], 0);
    let path = f.app.observation_path("docker-status.json");
    let mut sample = json!({"observed_at":crate::store::now(),"host":{
        "hostname":"actual-host","ip":"192.0.2.3","ip_kind":"LAN","os":"Fixture Linux",
        "kernel":"fixture-kernel","logical_cpus":8,"private":"never publish"},"engine":{"version":"29.9.3"},"containers":[]});
    fs::write(&path, sample.to_string()).unwrap();
    fs::write(f.app.observation_path("dashboard-gpus.json"),json!({"observed_at":crate::store::now(),"status":"ok","data":[{"name":"Fixture GPU","driver_version":"580.178.04"}]}).to_string()).unwrap();
    let host = f.app.dashboard_catalog().unwrap()["host"].clone();
    assert_eq!(host["hostname"], "actual-host");
    assert_eq!(host["machine_id"], "fixture");
    assert_eq!(host["status"], "ok");
    assert_eq!(host["inventory"]["docker_engine"], "29.9.3");
    assert_eq!(host["inventory"]["gpu_driver"], "580.178.04");
    assert!(host["inventory"].get("gpu").is_none());
    assert!(host.get("private").is_none());
    assert_eq!(f.app.resources().unwrap()["host"], host);
    sample["observed_at"] = json!(crate::store::now() - 3600.);
    fs::write(&path, sample.to_string()).unwrap();
    assert_eq!(f.app.resources().unwrap()["host"]["status"], "stale");
    assert!(f.app.resources().unwrap()["host"]["inventory"]
        .get("docker_engine")
        .is_none());
    fs::write(&path, b"not JSON").unwrap();
    assert_eq!(f.app.resources().unwrap()["host"]["status"], "unavailable");
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

#[test]
fn diagnostics_fail_open_without_following_a_linked_directory_or_recording_secrets() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let target = f.root.join("foreign-diagnostics");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    symlink(&target, f.app.local.join("diagnostics")).unwrap();
    assert!(f
        .app
        .incident(
            "jobs.submit",
            &Error::new(
                "input",
                "private fixture request body must not be recorded",
                400
            )
        )
        .is_none());
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    fs::remove_file(f.app.local.join("diagnostics")).unwrap();
    assert!(f
        .app
        .incident(
            "jobs.submit",
            &Error::new(
                "input",
                "private fixture request body must not be recorded",
                400
            )
        )
        .is_some());
    let text = fs::read_to_string(f.app.local.join("diagnostics/rust-incidents.jsonl")).unwrap();
    assert!(!text.contains("private fixture"));
    assert_eq!(f.count("jobs"), 0);
}

#[test]
fn common_item_ids_are_lossless_and_native_submission_identity_is_preserved() {
    let mut f = Fixture::new();
    let id = f.submit("common");
    let resolved = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"job","identity":{"id":id}}}),
        )
        .unwrap();
    let common = resolved["item_id"].as_str().unwrap();
    assert!(common.len() > 20);
    let decoded = loom_domain::item::ItemId::parse(common).unwrap();
    assert_eq!(decoded.to_string(), common);
    let item = f
        .call("results", "get_item", json!({"item_id":common}))
        .unwrap();
    assert_eq!(item["native_ref"]["identity"]["id"], id);
    assert_eq!(f.submit("common"), id);
    assert_eq!(f.count("catalog_items"), 1);
    assert_eq!(f.count("events"), 0);
    for bad in ["0", "01", "-1", "340282366920938463463374607431768211456"] {
        assert!(loom_domain::item::ItemId::parse(bad).is_err());
    }
    assert!(f
        .call("results", "get_item", json!({"item_id":123}))
        .is_err());
}
#[test]
fn failed_domain_transaction_cannot_leave_a_common_item_or_projection_event() {
    let mut f = Fixture::new();
    let before = f.count("catalog_changes");
    {
        let tx = f.app.transaction().unwrap();
        tx.execute(
            "INSERT INTO scenarios VALUES('rolled-back','X',1,0,0,'owner')",
            [],
        )
        .unwrap();
        tx.execute("INSERT INTO scenario_revisions VALUES('rolled-back',1,'{}','digest',0,'owner','fixture')",[]).unwrap();
    }
    assert_eq!(f.count("catalog_items"), 0);
    assert_eq!(f.count("catalog_changes"), before);
    assert_eq!(f.count("catalog_dirty"), 0);
    let id = f.submit("after");
    assert!(!id.is_empty());
    assert_eq!(f.count("catalog_items"), 1);
}
#[test]
fn file_ids_survive_search_database_rebuild_and_preserve_historical_records() {
    let mut f = Fixture::new();
    let bytes = b"A recorded file\n";
    let sha = store::hash(bytes);
    fs::write(f.app.bulk.join("receipt.txt"), bytes).unwrap();
    let original = json!({"original":{"label":"Search fixture"},"locations":[{"source_ref":"original/receipt.txt","target_ref":"receipt.txt","machine_id":"fixture","storage_id":"fixture-data","size":bytes.len(),"sha256":sha,"status":"available"}]});
    f.app
        .db
        .execute(
            "INSERT INTO history_records VALUES('catalog','dataset','preserved','r1','s1',?)",
            [original.to_string()],
        )
        .unwrap();
    for _ in 0..4 {
        f.app.index_once().unwrap();
    }
    let r = f
        .call("results", "search_items", json!({"text":"Search fixture"}))
        .unwrap();
    assert_eq!(r["items"].as_array().unwrap().len(), 1);
    let parent = r["items"][0]["item_id"].clone();
    let files = f
        .call("results", "item_files", json!({"item_id":parent}))
        .unwrap();
    let file = files["items"][0]["item_id"].clone();
    let read = f
        .call("results", "read_file", json!({"file_item_id":file}))
        .unwrap();
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(read["data_base64"].as_str().unwrap())
            .unwrap(),
        bytes
    );
    let content = f
        .call(
            "results",
            "search_items",
            json!({"text":"A recorded file","kinds":["file"]}),
        )
        .unwrap();
    assert_eq!(content["items"].as_array().unwrap().len(), 1);
    f.app.config.storage_id = "revoked-fixture-store".into();
    let revoked = f
        .call(
            "results",
            "search_items",
            json!({"text":"A recorded file","kinds":["file"]}),
        )
        .unwrap();
    assert!(revoked["items"].as_array().unwrap().is_empty());
    assert_eq!(revoked["next_cursor"], Value::Null);
    f.app.config.storage_id = "fixture-data".into();
    fs::write(f.app.bulk.join("relocated.txt"), bytes).unwrap();
    f.call("results","bind_historical_location",json!({"key":"file-id-relocation","identity":{"source":"catalog","kind":"dataset","id":"preserved","revision":"r1","snapshot":"s1","file":"original/receipt.txt"},"locator":{"machine_id":"fixture","storage_id":"fixture-data","ref":"relocated.txt"},"sha256":sha,"size":bytes.len()})).unwrap();
    fs::remove_file(f.app.bulk.join("receipt.txt")).unwrap();
    assert_eq!(
        f.call("results", "get_file", json!({"file_item_id":file}))
            .unwrap()["item_id"],
        file
    );
    let moved = f
        .call("results", "read_file", json!({"file_item_id":file}))
        .unwrap();
    assert_eq!(moved["data_base64"], read["data_base64"]);
    let index = f.app.local.join("search");
    fs::remove_dir_all(&index).unwrap();
    assert_eq!(
        f.call("system", "search_status", json!({})).unwrap()["status"],
        "index_unavailable"
    );
    f.app.index_once().unwrap();
    let rebuilt = f
        .call("results", "item_files", json!({"item_id":parent}))
        .unwrap();
    assert_eq!(rebuilt["items"][0]["item_id"], file);
    assert_eq!(f.count("history_records"), 1);
    assert_eq!(f.count("jobs"), 0);
    let body: String = f
        .app
        .db
        .query_row("SELECT body FROM history_records", [], |r| r.get(0))
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), original);
}
#[test]
fn search_cursor_pins_mvcc_versions_and_rejects_a_different_query_or_actor() {
    let mut f = Fixture::new();
    for n in 0..4 {
        f.submit(&format!("page-{n}"));
    }
    f.app.index_once().unwrap();
    let q = json!({"kinds":["job"],"view":"all","role":"all","limit":2});
    let first = f.call("results", "search_items", q.clone()).unwrap();
    let mut page = q.clone();
    page["cursor"] = first["next_cursor"].clone();
    assert!(page["cursor"].is_string());
    let id = first["items"][0]["native_ref"]["identity"]["id"]
        .as_str()
        .unwrap();
    f.call("jobs", "cancel", json!({"id":id})).unwrap();
    f.app.index_once().unwrap();
    let second = f.call("results", "search_items", page.clone()).unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        second["consistency"]["snapshot"],
        first["consistency"]["snapshot"]
    );
    page["text"] = json!("changed");
    assert_eq!(
        f.call("results", "search_items", page).unwrap_err().code,
        "cursor_expired"
    );
}

#[test]
fn exact_aliases_preserve_equal_definition_revisions_and_latest_backfill_order() {
    let mut f = Fixture::new();
    let m = json!({"label":"Same source"});
    f.app
        .db
        .execute(
            "INSERT INTO scenarios VALUES('exact','Same source',2,0,100,'owner')",
            [],
        )
        .unwrap();
    // Insert in reverse revision order: adoption must never move latest backwards.
    for revision in [2, 1] {
        f.app
            .db
            .execute(
                "INSERT INTO scenario_revisions VALUES('exact',?,?,'sha',?, ?,'fixture')",
                rusqlite::params![revision, m.to_string(), 100., "owner"],
            )
            .unwrap();
    }
    f.app.index_once().unwrap();
    let a = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"scenario","identity":{"id":"exact","revision":1}}}),
        )
        .unwrap();
    let b = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"scenario","identity":{"id":"exact","revision":2}}}),
        )
        .unwrap();
    assert_eq!(a["item_id"], b["item_id"]);
    assert_ne!(a["revision_id"], b["revision_id"]);
    let latest = f
        .call("results", "get_item", json!({"item_id":a["item_id"]}))
        .unwrap();
    assert_eq!(latest["native_ref"]["identity"]["revision"], 2);
}
#[test]
fn unicode_queries_and_cursor_access_revalidation_do_not_change_domain_events() {
    let mut f = Fixture::new();
    for n in 0..3 {
        f.call("results","register_scenario",json!({"key":format!("unicode-{n}"),"definition":{"label":format!("로봇 정렬 실험 {n} <img>")}})).unwrap();
    }
    f.app.index_once().unwrap();
    let before = f.count("events");
    let q = json!({"text":"정렬 실험","limit":1});
    let page = f.call("results", "search_items", q.clone()).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let mut cursor = q.clone();
    cursor["cursor"] = page["next_cursor"].clone();
    let other = auth::person_actor("different");
    assert_eq!(
        f.app
            .execute(&other, "results", "search_items", cursor.clone())
            .unwrap_err()
            .code,
        "cursor_expired"
    );
    f.app
        .db
        .execute("UPDATE catalog_meta SET access_epoch=access_epoch+1", [])
        .unwrap();
    assert_eq!(
        f.call("results", "search_items", cursor).unwrap_err().code,
        "cursor_expired"
    );
    assert!(f
        .call("results", "search_items", json!({"text":"\" OR *"}))
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(f.count("events"), before);
}
#[test]
fn inventory_ids_are_not_content_identity_and_tampering_is_rejected() {
    let mut f = Fixture::new();
    let id = f.submit("inventory");
    let item = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"job","identity":{"id":id}}}),
        )
        .unwrap();
    let item_id = items::parse(&item["item_id"]).unwrap();
    let revision = items::parse(&item["revision_id"]).unwrap();
    let entries=(0..2).map(|n|Ok(json!({"entry_key":format!("file-{n}"),"path":format!("file-{n}"),"size":1,"sha256":store::hash("x"),"copies":[]})));
    let p = inventory::prepare_entries(&f.app, &item_id, &revision, entries).unwrap();
    f.app.commit_inventory(p.clone()).unwrap();
    f.app.index_once().unwrap();
    let files = f
        .call("results", "item_files", json!({"item_id":item["item_id"]}))
        .unwrap();
    assert_eq!(files["items"].as_array().unwrap().len(), 2);
    assert_ne!(files["items"][0]["item_id"], files["items"][1]["item_id"]);
    let file = files["items"][0]["item_id"].clone();
    let chunk = p.descriptor["chunks"][0]["name"].as_str().unwrap();
    let path = inventory::root(&f.app)
        .join(item["revision_id"].as_str().unwrap())
        .join(chunk);
    fs::write(path, b"{}\n").unwrap();
    assert_eq!(
        f.call("results", "get_file", json!({"file_item_id":file}))
            .unwrap_err()
            .code,
        "inventory"
    );
    assert_eq!(f.count("jobs"), 1);
}
#[test]
fn inventory_projection_is_chunked_and_core_growth_does_not_follow_file_count() {
    let mut f = Fixture::new();
    let job = f.submit("many-files");
    let r = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"job","identity":{"id":job}}}),
        )
        .unwrap();
    let before = f.count("catalog_items");
    let p=inventory::prepare_entries(&f.app,&items::parse(&r["item_id"]).unwrap(),&items::parse(&r["revision_id"]).unwrap(),(0..100000).map(|n|Ok(json!({"entry_key":format!("record-{n:08}"),"path":format!("record-{n:08}.json"),"size":1,"sha256":store::hash("x"),"copies":[]})))).unwrap();
    assert!(p.descriptor["chunks"].as_array().unwrap().len() > 1);
    f.app.commit_inventory(p).unwrap();
    for _ in 0..40 {
        f.app.index_once().unwrap();
    }
    let page = f
        .call(
            "results",
            "item_files",
            json!({"item_id":r["item_id"],"limit":200}),
        )
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 200);
    assert!(page["next_cursor"].is_string());
    assert_eq!(before, f.count("catalog_items"));
    assert_eq!(f.count("catalog_manifests"), 1);
    assert_eq!(f.count("entity_locations"), 0);
}

#[test]
fn log_ranges_and_search_keep_raw_offsets_effect_identity_and_cursor_snapshot() {
    let mut f = Fixture::new();
    f.submit("log-ranges");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    fs::create_dir_all(log_index::path(&f.app.local, &work).parent().unwrap()).unwrap();
    let mut bytes = b"needle\n".repeat(1500000);
    bytes.extend([0xff, 0x00, 0xfe]);
    fs::write(log_index::path(&f.app.local, &work), &bytes).unwrap();
    fs::write(
        log_index::state_path(&f.app.local, &work),
        json!({"generation":"fixture-generation","closed":true,"gap":false}).to_string(),
    )
    .unwrap();
    for _ in 0..4 {
        f.app.index_once().unwrap();
    }
    let streams = f
        .call("jobs", "log_streams", json!({"id":work.job}))
        .unwrap();
    let id = streams["items"][0]["item_id"].clone();
    let read = f
        .call(
            "jobs",
            "read_log",
            json!({"stream_id":id,"offset":(bytes.len()-3).to_string(),"limit":3}),
        )
        .unwrap();
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(read["data_base64"].as_str().unwrap())
            .unwrap(),
        [0xff, 0, 0xfe]
    );
    assert!(read["eof"].as_bool().unwrap());
    assert_eq!(read["next_offset"], bytes.len().to_string());
    let page = f
        .call(
            "jobs",
            "search_logs",
            json!({"stream_id":id,"text":"needle","limit":1}),
        )
        .unwrap();
    assert!(page["next_cursor"].is_string());
    let next = f
        .call(
            "jobs",
            "search_logs",
            json!({"stream_id":id,"text":"needle","limit":1,"cursor":page["next_cursor"]}),
        )
        .unwrap();
    assert_ne!(page["items"][0]["offset"], next["items"][0]["offset"]);
    assert_eq!(
        page["consistency"]["snapshot"],
        next["consistency"]["snapshot"]
    );
    assert_eq!(
        f.call(
            "jobs",
            "read_log",
            json!({"stream_id":id,"source_generation":"00000000-0000-4000-8000-000000000001"})
        )
        .unwrap_err()
        .code,
        "source_generation_changed"
    );
    let bounded = f
        .call("jobs", "read_log", json!({"stream_id":id,"limit":262144}))
        .unwrap();
    assert!(bounded.to_string().len() < 1024 * 1024);
}

#[test]
fn exact_native_alias_rejects_changed_immutable_definition_without_reassigning_ids() {
    let mut f = Fixture::new();
    let created = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"immutable-alias","definition":{"label":"Original"}}),
        )
        .unwrap();
    let native = json!({"kind":"scenario","identity":{"id":created["id"],"revision":1}});
    let item = f
        .call("results", "resolve_item", json!({"native_ref":native}))
        .unwrap();
    let tx = f.app.transaction().unwrap();
    tx.execute(
        "UPDATE scenario_revisions SET definition=? WHERE scenario_id=? AND revision=1",
        rusqlite::params![
            json!({"label":"Changed outside the version contract"}).to_string(),
            created["id"].as_str()
        ],
    )
    .unwrap();
    assert_eq!(tx.commit().unwrap_err().code, "conflicting_identity");
    assert_eq!(
        item,
        f.call("results", "resolve_item", json!({"native_ref":native}))
            .unwrap()
    );
    let source = f
        .call(
            "results",
            "get_scenario",
            json!({"id":created["id"],"revision":1}),
        )
        .unwrap();
    assert_eq!(source["revision"]["definition"]["label"], "Original");
}
#[test]
fn archived_scenario_hides_from_default_item_search_and_exact_reference_survives() {
    let mut f = Fixture::new();
    let s = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"search-archive","definition":{"label":"Archived searchable context"}}),
        )
        .unwrap();
    f.app.index_once().unwrap();
    let item = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"scenario","identity":{"id":s["id"],"revision":1}}}),
        )
        .unwrap();
    f.call("results","archive_scenario",json!({"key":"archive-search","id":s["id"],"expected_revision":1,"reason":"Explicit archive"})).unwrap();
    f.app.index_once().unwrap();
    assert!(f
        .call(
            "results",
            "search_items",
            json!({"text":"Archived searchable context","kinds":["scenario"]})
        )
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        f.call(
            "results",
            "search_items",
            json!({"text":"Archived searchable context","kinds":["scenario"],"view":"all"})
        )
        .unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        f.call("results", "get_item", json!({"item_id":item["item_id"]}))
            .unwrap()["native_ref"]["identity"]["revision"],
        1
    );
}

#[test]
fn interrupted_inventory_rejects_changed_content_but_reuses_ids_after_copy_relocation() {
    let mut f = Fixture::new();
    let job = f.submit("resume-inventory");
    let resolved = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"job","identity":{"id":job}}}),
        )
        .unwrap();
    let item = items::parse(&resolved["item_id"]).unwrap();
    let revision = items::parse(&resolved["revision_id"]).unwrap();
    let entry = |n, copy: &str, sha: &str| json!({"entry_key":format!("source-{n:08}"),"path":format!("{n:08}/{}.rs","x".repeat(250)),"role":"source","size":1,"sha256":sha,"copies":[{"type":"storage","machine_id":"fixture","storage_id":"fixture-data","ref":copy}]});
    let sha = store::hash("x");
    let interrupted = (0..10000)
        .map(|n| Ok(entry(n, "original", &sha)))
        .chain(std::iter::once(Err(Error::new(
            "interrupted",
            "Fixture interruption",
            503,
        ))));
    assert_eq!(
        inventory::prepare_entries(&f.app, &item, &revision, interrupted)
            .unwrap_err()
            .code,
        "interrupted"
    );
    let folder = inventory::root(&f.app).join(resolved["revision_id"].as_str().unwrap());
    let intent: Value =
        serde_json::from_slice(&fs::read(folder.join("intent.json")).unwrap()).unwrap();
    let first =
        fs::read_to_string(folder.join(intent["chunks"][0]["name"].as_str().unwrap())).unwrap();
    let original: Value = serde_json::from_str(first.lines().next().unwrap()).unwrap();
    let changed = (0..10000).map(|n| Ok(entry(n, "moved", &store::hash("different"))));
    assert_eq!(
        inventory::prepare_entries(&f.app, &item, &revision, changed)
            .unwrap_err()
            .code,
        "inventory"
    );
    let p = inventory::prepare_entries(
        &f.app,
        &item,
        &revision,
        (0..10000).map(|n| Ok(entry(n, "moved", &sha))),
    )
    .unwrap();
    assert_eq!(p.descriptor["entries"], 10000);
    let first =
        fs::read_to_string(folder.join(p.descriptor["chunks"][0]["name"].as_str().unwrap()))
            .unwrap();
    let resumed: Value = serde_json::from_str(first.lines().next().unwrap()).unwrap();
    assert_eq!(resumed["item_id"], original["item_id"]);
    f.app.commit_inventory(p).unwrap();
    assert_eq!(f.count("catalog_items"), 1);
    assert_eq!(f.count("catalog_manifests"), 1);
}

#[test]
fn native_writer_requires_explicit_catalog_upgrade_and_never_recreates_lost_origin() {
    let mut f = Fixture::new();
    let job = f.submit("origin-preservation");
    f.app.db.execute("DELETE FROM catalog_meta", []).unwrap();
    let result = Application::open(
        &f.app.local,
        &f.app.bulk,
        &f.app.components,
        f.app.config.clone(),
    );
    assert_eq!(result.err().unwrap().code, "catalog_authority_missing");
    assert_eq!(f.count("catalog_meta"), 0);
    assert_eq!(f.call("jobs", "get", json!({"id":job})).unwrap()["id"], job);
    let legacy = f.root.join("schema-five");
    let app = Application::open_schema(
        &legacy,
        &f.app.bulk,
        &f.app.components,
        f.app.config.clone(),
        5,
    )
    .unwrap();
    drop(app);
    assert_eq!(
        Application::open(
            &legacy,
            &f.app.bulk,
            &f.app.components,
            f.app.config.clone()
        )
        .err()
        .unwrap()
        .code,
        "schema_migration_required"
    );
}

#[test]
fn historical_job_items_keep_declared_presentation_without_creating_executions() {
    let mut f = Fixture::new();
    for (id, scope, role) in [
        ("preserved-main", "research", "main"),
        ("preserved-install", "operations", "test"),
    ] {
        let body = json!({"record":{"state":"complete","created":123.,"spec":{"presentation":{"label":id,"scope":scope,"role":role}}}});
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES('queue','job_view',?,'r1','snapshot',?)",
                rusqlite::params![id, body.to_string()],
            )
            .unwrap();
    }
    for _ in 0..4 {
        f.app.index_once().unwrap();
    }
    let main = f
        .call("results", "search_items", json!({"kinds":["job"]}))
        .unwrap();
    assert_eq!(main["items"].as_array().unwrap().len(), 1);
    assert_eq!(main["items"][0]["label"], "preserved-main");
    assert_eq!(main["items"][0]["native_ref"]["kind"], "historical");
    assert_eq!(
        f.call(
            "results",
            "search_items",
            json!({"kinds":["job"],"view":"all","role":"all"})
        )
        .unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(f.count("jobs"), 0);
    assert_eq!(f.count("attempts"), 0);
}
#[test]
fn file_search_uses_parent_presentation_at_the_pinned_projection_version() {
    let mut f = Fixture::new();
    let job = f.submit("curated-file-scope");
    let r = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"job","identity":{"id":job}}}),
        )
        .unwrap();
    let p=inventory::prepare_entries(&f.app,&items::parse(&r["item_id"]).unwrap(),&items::parse(&r["revision_id"]).unwrap(),std::iter::once(Ok(json!({"entry_key":"receipt","path":"receipt.txt","role":"member","size":1,"sha256":store::hash("x"),"copies":[]})))).unwrap();
    f.app.commit_inventory(p).unwrap();
    f.app.index_once().unwrap();
    assert!(f
        .call("results", "search_items", json!({"kinds":["file"]}))
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .is_empty());
    f.call("jobs","curate",json!({"key":"show-file-scope","reason":"Fixture classification","dry_run":false,"changes":[{"kind":"job","id":job,"presentation":{"scope":"research","role":"main"}}]})).unwrap();
    f.app.index_once().unwrap();
    let found = f
        .call("results", "search_items", json!({"kinds":["file"]}))
        .unwrap();
    assert_eq!(found["items"].as_array().unwrap().len(), 1);
    assert_eq!(found["items"][0]["scope"], "research");
    assert_eq!(found["items"][0]["role"], "main");
    assert_eq!(f.count("jobs"), 1);
}

#[test]
fn disposable_index_configuration_cannot_overlap_preserved_authority_or_payloads() {
    let mut f = Fixture::new();
    for path in [
        f.app.local.clone(),
        f.app.local.join("catalog-manifests"),
        f.app.local.join("log-spool"),
        f.app.bulk.clone(),
        f.app.components.clone(),
    ] {
        f.app
            .config
            .extra
            .insert("catalog".into(), json!({"index_directory":path}));
        assert_eq!(
            crate::index_store::IndexWriter::open(&f.app)
                .err()
                .unwrap()
                .code,
            "catalog_layout"
        );
        assert!(f.app.local.join("ops.sqlite3").is_file());
    }
    let alias = f.root.join("linked-state");
    std::os::unix::fs::symlink(&f.app.local, &alias).unwrap();
    f.app.config.extra.insert(
        "catalog".into(),
        json!({"index_directory":alias.join("catalog-manifests")}),
    );
    assert_eq!(
        crate::index_store::IndexWriter::open(&f.app)
            .err()
            .unwrap()
            .code,
        "catalog_path"
    );
    f.app.config.extra.remove("catalog");
    assert!(crate::index_store::IndexWriter::open(&f.app).is_ok());
}

#[test]
fn log_projection_and_ranges_never_adopt_an_uncommitted_spool_tail() {
    let mut f = Fixture::new();
    f.submit("log-tail-commit");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    fs::create_dir_all(log_index::path(&f.app.local, &work).parent().unwrap()).unwrap();
    fs::write(
        log_index::path(&f.app.local, &work),
        b"committed\nuncommitted-secret\n",
    )
    .unwrap();
    fs::write(
        log_index::state_path(&f.app.local, &work),
        json!({"generation":"committed-source","offset":10,"closed":false,"gap":false}).to_string(),
    )
    .unwrap();
    f.app.index_once().unwrap();
    let streams = f
        .call("jobs", "log_streams", json!({"id":work.job}))
        .unwrap();
    let id = streams["items"][0]["item_id"].clone();
    let page = f.call("jobs", "read_log", json!({"stream_id":id})).unwrap();
    assert_eq!(page["text"], "committed\n");
    assert_eq!(page["total_bytes"], "10");
    let search = f
        .call(
            "jobs",
            "search_logs",
            json!({"stream_id":id,"text":"uncommitted-secret"}),
        )
        .unwrap();
    assert!(search["items"].as_array().unwrap().is_empty());
    fs::write(
        log_index::state_path(&f.app.local, &work),
        json!({"generation":"committed-source","offset":500,"closed":true}).to_string(),
    )
    .unwrap();
    assert_eq!(
        f.call("jobs", "read_log", json!({"stream_id":id}))
            .unwrap_err()
            .code,
        "retention_gap"
    );
}

#[test]
fn relation_pages_preserve_all_edges_without_allocating_child_items() {
    let mut f = Fixture::new();
    let root = f.submit("relations-bound");
    let item = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"job","identity":{"id":root}}}),
        )
        .unwrap();
    for n in 0..205 {
        f.app
            .db
            .execute(
                "INSERT INTO workflow_nodes VALUES(?,?,?,?)",
                rusqlite::params![
                    root,
                    format!("step-{n:03}"),
                    "preserved step",
                    format!("preserved-child-{n}")
                ],
            )
            .unwrap();
    }
    let before = f.count("catalog_items");
    let page = f
        .call(
            "results",
            "item_relations",
            json!({"item_id":item["item_id"]}),
        )
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 50);
    let mut edges = page["items"].as_array().unwrap().clone();
    let mut cursor = page["next_cursor"].clone();
    while cursor.is_string() {
        let page = f
            .call(
                "results",
                "item_relations",
                json!({"item_id":item["item_id"],"cursor":cursor}),
            )
            .unwrap();
        edges.extend(page["items"].as_array().unwrap().clone());
        cursor = page["next_cursor"].clone();
    }
    assert_eq!(edges.len(), 205);
    let unique: std::collections::BTreeSet<_> = edges
        .iter()
        .map(|v| v["native_ref"]["identity"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(unique.len(), 205);
    assert_eq!(f.count("catalog_items"), before);
    assert_eq!(f.count("jobs"), 1);
}

#[test]
fn reconciliation_import_replays_without_new_jobs_and_rejects_mutated_source_evidence() {
    let mut f = Fixture::new();
    fs::write(f.app.bulk.join("weights.json"), b"{\"weight\":1}").unwrap();
    let proposal = json!({"schema":"loom-reconcile/v1","operations":[{"operation":"import","schema":"loom-asset/v1","source":{"system":"preserved-fixture","id":"selected-model","revision":"r1"},"type":"model","name":"Selected model","properties":{"declared_steps":20000},"primary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"weights.json"}}]});
    let plan = f
        .call(
            "results",
            "reconcile_plan",
            json!({"key":"model-import","proposal":proposal}),
        )
        .unwrap();
    let args = json!({"plan_id":plan["plan_id"]});
    assert_eq!(plan["proposal"]["operations"][0]["schema"], "loom-asset/v2");
    assert_eq!(
        f.call(
            "results",
            "reconcile_plan",
            json!({"key":"model-import","proposal":proposal})
        )
        .unwrap()["plan_id"],
        plan["plan_id"]
    );
    assert_eq!(
        f.call("results", "reconcile_apply", args.clone())
            .unwrap_err()
            .code,
        "validation_required"
    );
    f.call("results", "reconcile_validate", args.clone())
        .unwrap();
    let applied = f.call("results", "reconcile_apply", args.clone()).unwrap();
    assert_eq!(applied["receipt"]["created_jobs"], 0);
    assert_eq!(f.count("jobs"), 0);
    assert_eq!(f.count("attempts"), 0);
    assert_eq!(f.count("asset_revisions"), 1);
    let listed = f.call("results", "list", json!({"view":"all"})).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0]["job"].is_null());
    let native = f
        .call("results", "get", json!({"id":listed[0]["id"],"revision":1}))
        .unwrap();
    assert_eq!(native["kind"], "model");
    assert_eq!(f.call("results", "reconcile_apply", args).unwrap(), applied);
    assert_eq!(f.count("asset_revisions"), 1);
    let mut conflicting = proposal.clone();
    conflicting["operations"][0]["name"] = json!("Different interpretation");
    assert_eq!(
        f.call(
            "results",
            "reconcile_plan",
            json!({"key":"other-key","proposal":conflicting})
        )
        .unwrap_err()
        .code,
        "source_conflict"
    );
}

#[test]
fn reconciliation_fences_source_changes_grants_and_batch_failure() {
    let mut f = Fixture::new();
    fs::write(f.app.bulk.join("original.txt"), b"original").unwrap();
    let op = json!({"operation":"import","schema":"loom-asset/v2","source":{"system":"fixture","id":"source-change","revision":"1"},"definition":{"kind":"artifact","label":"File","primary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"original.txt"}}});
    let p=f.call("results","reconcile_plan",json!({"key":"source-change","proposal":{"schema":"loom-reconcile/v1","operations":[op]}})).unwrap();
    let args = json!({"plan_id":p["plan_id"]});
    f.call("results", "reconcile_validate", args.clone())
        .unwrap();
    fs::write(f.app.bulk.join("original.txt"), b"mutated").unwrap();
    assert_eq!(
        f.call("results", "reconcile_apply", args.clone())
            .unwrap_err()
            .code,
        "artifact"
    );
    assert_eq!(f.count("entity_revisions"), 0);
    fs::write(f.app.bulk.join("original.txt"), b"original").unwrap();
    f.call("results", "reconcile_validate", args.clone())
        .unwrap();
    f.app.config.storage_id = "revoked-store".into();
    assert_eq!(
        f.call("results", "reconcile_apply", args).unwrap_err().code,
        "plan_stale"
    );
    assert_eq!(f.count("asset_imports"), 0);
}

#[test]
fn inventory_publication_and_exact_member_inputs_do_not_expand_the_core_registry() {
    let mut f = Fixture::new();
    let id = f.submit("inventory-publisher");
    let run = f.app.next_work(&Default::default()).unwrap().unwrap();
    fs::create_dir_all(&run.folder).unwrap();
    let mut chunk = vec![];
    for n in 0..300 {
        let name = format!("f{n:04}.txt");
        let data = format!("member {n}");
        fs::write(run.folder.join(&name), data.as_bytes()).unwrap();
        chunk.extend(
            serde_json::to_vec(
                &json!({"key":name,"path":name,"size":data.len(),"sha256":hash(data)}),
            )
            .unwrap(),
        );
        chunk.push(b'\n');
    }
    fs::write(run.folder.join("members.jsonl"), &chunk).unwrap();
    let declaration = json!({"schema":"loom-output-inventory/v1","entries":300,"chunks":[{"ref":"members.jsonl","entries":300,"sha256":hash(&chunk)}]});
    f.app
        .stage_returned(&run, 0, json!({"returned":true}))
        .unwrap();
    let publish = f.app.next_work(&Default::default()).unwrap().unwrap();
    let output = json!({"entities":[{"id":"bundle","kind":"artifact","path":"f0000.txt","inventory":declaration}],"metrics":[]});
    f.app.publish_work(&publish, output).unwrap();
    assert_eq!(f.count("entity_locations"), 1);
    let native = format!("{id}.bundle");
    assert_eq!(
        f.call("results", "get", json!({"id":native,"revision":1}))
            .unwrap_err()
            .code,
        "paging_required"
    );
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let item = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"entity","identity":{"id":native,"revision":1}}}),
        )
        .unwrap();
    let page = f
        .call(
            "results",
            "item_files",
            json!({"item_id":item["item_id"],"limit":200}),
        )
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 200);
    assert!(page["next_cursor"].is_string());
    assert!(f.count("catalog_items") < 10);
    fs::remove_dir_all(crate::index_store::directory(&f.app.local, &f.app.config)).unwrap();
    let input = f
        .app
        .artifact_input(&json!({"id":native,"revision":1}), Some("f0299.txt"))
        .unwrap();
    assert_eq!(input["sha256"], hash("member 299"));
    assert!(input["ref"].as_str().unwrap().ends_with("f0299.txt"));
}

#[test]
fn reconciliation_rejects_conflicting_batch_identities_and_declared_digest_mismatch() {
    let mut f = Fixture::new();
    fs::write(f.app.bulk.join("source.txt"), "original").unwrap();
    let op = json!({"operation":"import","schema":"loom-asset/v2","source":{"system":"fixture","id":"same","revision":"1"},"definition":{"kind":"artifact","label":"Source","primary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"source.txt","sha256":"wrong"}}});
    let mut other = op.clone();
    other["definition"]["label"] = json!("Different");
    assert_eq!(f.call("results","reconcile_plan",json!({"key":"conflicting","proposal":{"schema":"loom-reconcile/v1","operations":[op,other]}})).unwrap_err().code,"source_conflict");
    assert_eq!(f.count("reconcile_plans"), 0);
    let plan = f
        .call(
            "results",
            "reconcile_plan",
            json!({"key":"digest","proposal":{"schema":"loom-reconcile/v1","operations":[op]}}),
        )
        .unwrap();
    assert_eq!(
        f.call(
            "results",
            "reconcile_validate",
            json!({"plan_id":plan["plan_id"]})
        )
        .unwrap_err()
        .code,
        "content_changed"
    );
    assert_eq!(f.count("asset_revisions"), 0);
}

#[test]
fn dependency_items_survive_partial_observations_and_have_exact_runtime_edges() {
    let mut f = Fixture::new();
    let manifest = json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[{"name":"Python","version":"3.12"},{"name":"support-package","version":"1.0"}]});
    let observation = json!({"status":"ok","complete":true,"runtimes":[{"id":"runtime-one","revision":1,"sha256":store::hash(compat::seal(&manifest)),"manifest":manifest}]});
    f.app.observe_runtimes(observation.clone()).unwrap();
    f.app.index_once().unwrap();
    let all = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["dependency"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(all["items"].as_array().unwrap().len(), 2);
    let page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["dependency"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(page["entities"].as_array().unwrap().len(), 2);
    for entity in page["entities"].as_array().unwrap() {
        assert!(entity["item_id"].is_string());
        assert_eq!(
            entity["loom_dependency"]["runtimes"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let detail = f
            .call(
                "system",
                "dashboard_entity",
                json!({"item_id":entity["item_id"],"revision_id":entity["item_revision_id"]}),
            )
            .unwrap();
        assert_eq!(
            detail["entities"][0]["loom_dependency"]["versions"],
            entity["loom_dependency"]["versions"]
        );
    }
    let primary = f
        .call("results", "search_items", json!({"kinds":["dependency"]}))
        .unwrap();
    assert_eq!(primary["items"].as_array().unwrap().len(), 1);
    let runtime=f.call("results","resolve_item",json!({"native_ref":{"kind":"runtime","identity":{"id":"runtime-one","revision":1,"sha256":observation["runtimes"][0]["sha256"]}}})).unwrap();
    let edges = f
        .call(
            "results",
            "item_relations",
            json!({"item_id":runtime["item_id"],"include_support":true,"direction":"outgoing"}),
        )
        .unwrap();
    assert_eq!(edges["items"].as_array().unwrap().len(), 2);
    f.app
        .observe_runtimes(json!({"status":"unavailable","complete":false,"runtimes":[]}))
        .unwrap();
    f.app.index_once().unwrap();
    let after = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["dependency"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(after["items"].as_array().unwrap().len(), 2);
}

#[test]
fn ranked_history_and_relation_search_keep_exact_revision_and_bounded_waits() {
    let mut f = Fixture::new();
    let scenario = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"search-origin","definition":{"label":"Exact target"}}),
        )
        .unwrap();
    f.call("results","update_scenario",json!({"key":"search-update","id":scenario["id"],"expected_revision":1,"definition":{"label":"Exact target updated"},"reason":"revision fixture"})).unwrap();
    f.app.index_once().unwrap();
    let all = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["scenario"],"history":"all","view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(all["items"].as_array().unwrap().len(), 2);
    let latest = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["scenario"],"text":"Exact target updated","limit":1}),
        )
        .unwrap();
    assert_eq!(latest["items"][0]["label"], "Exact target updated");
    let wait = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["scenario"],"at_least":"9223372036854775807","timeout_ms":1}),
        )
        .unwrap();
    assert_eq!(wait["partial"], true);
    assert_eq!(wait["consistency"]["at_least_satisfied"], false);
}

#[test]
fn dashboard_pages_project_only_selected_kinds_and_use_server_cursors() {
    let mut f = Fixture::new();
    for n in 0..4 {
        f.call(
            "results",
            "register_scenario",
            json!({"key":format!("dashboard-{n}"),"definition":{"label":format!("Scenario {n}")}}),
        )
        .unwrap();
    }
    f.submit("not-a-scenario");
    f.app.index_once().unwrap();
    let first = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["scenario"],"limit":2}),
        )
        .unwrap();
    assert_eq!(first["entities"].as_array().unwrap().len(), 2);
    assert!(first["entities"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["kind"] == "scenario"));
    let second = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["scenario"],"limit":2,"cursor":first["next_cursor"]}),
        )
        .unwrap();
    assert_eq!(second["entities"].as_array().unwrap().len(), 2);
    assert_ne!(first["entities"][0]["id"], second["entities"][0]["id"]);
    assert!(second["next_cursor"].is_null());
}

#[test]
fn explicit_log_retention_rejects_live_sources_and_preserves_expired_identity() {
    let mut f = Fixture::new();
    f.submit("retention-fixture");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    let generation = uuid::Uuid::now_v7().to_string();
    fs::create_dir_all(log_index::path(&f.app.local, &work).parent().unwrap()).unwrap();
    fs::write(log_index::path(&f.app.local, &work), b"expire this secret").unwrap();
    fs::write(
        log_index::state_path(&f.app.local, &work),
        json!({"generation":generation,"offset":18,"closed":true,"gap":false}).to_string(),
    )
    .unwrap();
    f.app.index_once().unwrap();
    let stream = f
        .call("jobs", "log_streams", json!({"id":work.job}))
        .unwrap()["items"][0]["item_id"]
        .clone();
    let request = json!({"key":"expire-one","reason":"fixture retention","streams":[{"stream_id":stream,"source_generation":generation}]});
    assert_eq!(
        f.call("jobs", "prune_logs", request.clone())
            .unwrap_err()
            .code,
        "retention_busy"
    );
    f.app
        .stage_returned(&work, 1, json!({"returned":true,"exit_code":1}))
        .unwrap();
    let task = f.call("jobs", "prune_logs", request.clone()).unwrap();
    assert_eq!(
        f.call("jobs", "prune_logs", request).unwrap()["id"],
        task["id"]
    );
    assert_eq!(
        f.call("jobs", "read_log", json!({"stream_id":stream}))
            .unwrap_err()
            .code,
        "retention_gap"
    );
    assert_eq!(
        f.call(
            "jobs",
            "logs",
            json!({"id":work.job,"attempt":work.attempt,"offset":0,"limit":1024})
        )
        .unwrap_err()
        .code,
        "retention_gap"
    );
    assert_eq!(
        f.call("jobs", "log_streams", json!({"id":work.job}))
            .unwrap()["items"][0]["availability"],
        "expired"
    );
    f.app.index_once().unwrap();
    assert!(f
        .call(
            "jobs",
            "search_logs",
            json!({"id":work.job,"text":"secret"})
        )
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(f.count("jobs"), 1);
    assert_eq!(f.count("log_retention"), 1);
}

#[test]
fn shadow_search_switch_keeps_open_readers_and_expires_old_cursors() {
    let mut f = Fixture::new();
    for n in 0..3 {
        f.submit(&format!("shadow-{n}"));
    }
    f.app.index_once().unwrap();
    let args = json!({"kinds":["job"],"view":"all","role":"all","limit":1});
    let page = f.call("results", "search_items", args.clone()).unwrap();
    let reader = index_store::read(&f.app).unwrap();
    let generation: String = reader
        .query_row("SELECT generation FROM meta WHERE id=1", [], |r| r.get(0))
        .unwrap();
    let task = f
        .call(
            "system",
            "rebuild_search",
            json!({"key":"new-generation","max_batches":100}),
        )
        .unwrap();
    let id = task["id"].as_str().unwrap();
    let shadow = index_store::directory(&f.app.local, &f.app.config)
        .join("shadows")
        .join(id);
    index_store::secure_dir(&shadow).unwrap();
    let mut config = f.app.config.clone();
    config
        .extra
        .insert("catalog".into(), json!({"index_directory":shadow}));
    let source =
        Application::open_read_only(&f.app.local, &f.app.bulk, &f.app.components, config).unwrap();
    let mut writer = index_store::IndexWriter::open(&source).unwrap();
    writer.sync(&source, 1000).unwrap();
    assert_eq!(
        f.app
            .advance_maintenance(id, &json!({"complete":true}), None)
            .unwrap()["complete"],
        true
    );
    let old: String = reader
        .query_row("SELECT generation FROM meta WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(old, generation);
    let mut next = args.clone();
    next["cursor"] = page["next_cursor"].clone();
    assert_eq!(
        f.call("results", "search_items", next).unwrap_err().code,
        "cursor_expired"
    );
    assert_eq!(
        f.call("results", "search_items", args).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn historical_search_defaults_to_latest_source_but_exact_snapshot_detail_is_preserved() {
    let mut f = Fixture::new();
    for (snapshot, label) in [("s1", "Original snapshot"), ("s2", "Later snapshot")] {
        let body = json!({"original":{"id":"same-native-id","kind":"dataset","label":label,"properties":{},"components":[]}});
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES('catalog','dataset','same-native-id','r1',?,?)",
                rusqlite::params![snapshot, body.to_string()],
            )
            .unwrap();
    }
    for _ in 0..4 {
        f.app.index_once().unwrap();
    }
    let current = f
        .call("results", "search_items", json!({"kinds":["dataset"]}))
        .unwrap();
    assert_eq!(current["items"].as_array().unwrap().len(), 1);
    assert_eq!(current["items"][0]["label"], "Later snapshot");
    let all = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["dataset"],"history":"all"}),
        )
        .unwrap();
    assert_eq!(all["items"].as_array().unwrap().len(), 2);
    let old=f.call("results","resolve_item",json!({"native_ref":{"kind":"historical","identity":{"source":"catalog","kind":"dataset","id":"same-native-id","revision":"r1","snapshot":"s1"}}})).unwrap();
    let detail = f
        .call(
            "system",
            "dashboard_entity",
            json!({"item_id":old["item_id"],"revision_id":old["revision_id"]}),
        )
        .unwrap();
    assert_eq!(detail["entities"].as_array().unwrap().len(), 1);
    assert_eq!(detail["entities"][0]["label"], "Original snapshot");
    assert_eq!(detail["entities"][0]["loom_history"]["snapshot"], "s1");
    assert_eq!(f.count("history_records"), 2);
}

#[test]
fn compact_activity_rollup_preserves_duration_and_excludes_operational_jobs() {
    let mut f = Fixture::new();
    let id = f.submit("activity-main");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app
        .stage_returned(&work, 1, json!({"returned":true,"exit_code":1}))
        .unwrap();
    let clock = store::now();
    f.app.db.execute("UPDATE jobs SET spec=json_set(spec,'$.presentation.scope','research','$.presentation.role','main') WHERE id=?",[&id]).unwrap();
    f.app
        .db
        .execute(
            "UPDATE attempts SET started=?,ended=? WHERE job=?",
            rusqlite::params![clock - 20., clock - 10., id],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "DELETE FROM rust_attempt_starts WHERE attempt=?",
            [&work.attempt],
        )
        .unwrap();
    let research = f
        .call("system", "dashboard_activity", json!({"view":"research"}))
        .unwrap();
    assert_eq!(
        research["days"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["job_count"].as_i64().unwrap())
            .sum::<i64>(),
        1
    );
    let milliseconds: f64 = research["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["milliseconds"].as_f64().unwrap())
        .sum();
    assert!((milliseconds - 10000.).abs() < 0.1);
    assert!(f
        .call("system", "dashboard_activity", json!({"view":"operations"}))
        .unwrap()["days"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn exact_scenario_dashboard_and_profile_graph_never_substitute_current_definitions() {
    let mut f = Fixture::new();
    let s = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"exact-browser","definition":{"label":"Original task"}}),
        )
        .unwrap();
    let old = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"scenario","identity":{"id":s["id"],"revision":1}}}),
        )
        .unwrap();
    f.call("results","update_scenario",json!({"key":"exact-browser-update","id":s["id"],"expected_revision":1,"definition":{"label":"Current task"},"reason":"revision fixture"})).unwrap();
    let display = f
        .call(
            "system",
            "dashboard_entity",
            json!({"item_id":old["item_id"],"revision_id":old["revision_id"]}),
        )
        .unwrap();
    assert_eq!(display["entities"][0]["label"], "Original task");
    assert_eq!(display["entities"][0]["loom_scenario"]["revision"], 1);

    let profile = json!({"schema":"loom-execution/v2","revision":"mutable.fixture.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"executor":"container","runtime_id":"cpu","command":["/bin/true"]},"inputs":{},"outputs":null,"limits":{"timeout_seconds":30,"max_attempts":1},"usage":{"description":"Original accepted Profile"}});
    f.app
        .config
        .profiles
        .insert("mutable.fixture.r1".into(), profile.clone());
    f.app.cache_catalog_sources().unwrap();
    f.call("jobs","submit",json!({"key":"frozen-definition","kind":"mutable.fixture.r1","owner":"owner","notify":"owner","params":{}})).unwrap();
    let original=f.call("results","resolve_item",json!({"native_ref":{"kind":"profile","identity":{"kind":"mutable.fixture.r1","sha256":"a".repeat(64)}}})).unwrap();
    let mut current = profile;
    current["sha256"] = json!("b".repeat(64));
    current["usage"]["description"] = json!("Current replacement");
    f.app
        .config
        .profiles
        .insert("mutable.fixture.r1".into(), current);
    f.app.cache_catalog_sources().unwrap();
    let graph = f
        .call(
            "results",
            "item_relations",
            json!({"item_id":original["item_id"],"revision_id":original["revision_id"]}),
        )
        .unwrap();
    assert!(graph["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["relation"] == "used_by"));
    let display = f
        .call(
            "system",
            "dashboard_entity",
            json!({"item_id":original["item_id"],"revision_id":original["revision_id"]}),
        )
        .unwrap();
    assert_eq!(
        display["entities"][0]["loom_execution_profile"]["description"],
        "Original accepted Profile"
    );
}

#[test]
fn dependency_identity_does_not_multiply_when_only_runtime_inventory_revision_changes() {
    let mut f = Fixture::new();
    let manifest = json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[{"name":"Python","version":"3.12"}]});
    for revision in [1, 2] {
        let mut current = manifest.clone();
        current["inventory_revision"] = json!(revision);
        let digest = hash(crate::compat::seal(&current));
        f.app.observe_runtimes(json!({"status":"ok","complete":true,"runtimes":[{"id":"same-installation","revision":revision,"sha256":digest,"manifest":current}]})).unwrap();
        f.app.index_once().unwrap();
    }
    let page = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["dependency"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
}

#[test]
fn search_initialization_without_committed_header_is_explicitly_unavailable() {
    let mut f = Fixture::new();
    let id = f.submit("header-readiness");
    let directory = crate::index_store::directory(&f.app.local, &f.app.config);
    fs::create_dir_all(&directory).unwrap();
    let db = rusqlite::Connection::open(directory.join("catalog.sqlite3")).unwrap();
    assert_eq!(
        f.call("results", "search_items", json!({}))
            .unwrap_err()
            .code,
        "index_unavailable"
    );
    db.execute_batch(crate::index_store::SCHEMA).unwrap();
    assert_eq!(
        f.call("results", "search_items", json!({}))
            .unwrap_err()
            .code,
        "index_unavailable"
    );
    drop(db);
    f.app.index_once().unwrap();
    let page = f
        .call(
            "results",
            "search_items",
            json!({"text":id,"view":"all","role":"all"}),
        )
        .unwrap();
    assert!(!page["items"].as_array().unwrap().is_empty());
    assert_eq!(f.count("jobs"), 1);
}

#[test]
fn task_context_pages_use_exact_additive_links_and_preserve_accepted_jobs() {
    let mut f = Fixture::new();
    let a = f.submit("linked-job");
    let b = f.submit("unrelated-job");
    let before = f.app.job_detail(&a).unwrap()["spec"].clone();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"task-group","definition":{"label":"Common grasp task"}}),
        )
        .unwrap();
    let link=f.call("results","link_scenario",json!({"key":"task-link","scenario_ref":{"id":task["id"],"revision":1},"target":{"kind":"job","identity":{"id":a}},"relation":"produced_for","provenance":{"reason":"Exact accepted source"}})).unwrap();
    f.app.index_once().unwrap();
    let query = json!({"kinds":["job"],"view":"all","role":"all","scenario_ref":{"id":task["id"],"revision":1}});
    let page = f.call("system", "dashboard_page", query.clone()).unwrap();
    let entities = page["entities"].as_array().unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0]["id"], a);
    assert_eq!(entities[0]["task_refs"][0]["label"], "Common grasp task");
    assert!(page["research_tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == task["id"]));
    assert_eq!(f.app.job_detail(&a).unwrap()["spec"], before);
    f.call(
        "results",
        "supersede_scenario_link",
        json!({"key":"withdraw-task-link","id":link["id"],"reason":"Incorrect association"}),
    )
    .unwrap();
    let page = f.call("system", "dashboard_page", query).unwrap();
    assert!(page["entities"].as_array().unwrap().is_empty());
    let catalog = f.app.dashboard_catalog().unwrap();
    assert!(catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == b)
        .unwrap()["task_refs"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn task_filter_spans_definition_revisions_without_repinning_sources() {
    let mut f = Fixture::new();
    let s = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"all-revisions","definition":{"label":"Old task"}}),
        )
        .unwrap();
    let a=f.call("jobs","submit",json!({"key":"pinned-old","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"scenario_ref":{"id":s["id"],"revision":1}})).unwrap();
    f.call("results","update_scenario",json!({"key":"new-task-definition","id":s["id"],"expected_revision":1,"definition":{"label":"Current task"},"reason":"Clarify label"})).unwrap();
    let b=f.call("jobs","submit",json!({"key":"pinned-new","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"scenario_ref":{"id":s["id"],"revision":2}})).unwrap();
    f.app.index_once().unwrap();
    let all = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["job"],"view":"all","role":"all","task_id":s["id"]}),
        )
        .unwrap();
    assert_eq!(all["entities"].as_array().unwrap().len(), 2);
    let exact=f.call("system","dashboard_page",json!({"kinds":["job"],"view":"all","role":"all","scenario_ref":{"id":s["id"],"revision":1}})).unwrap();
    assert_eq!(exact["entities"][0]["id"], a["id"]);
    assert_eq!(exact["entities"].as_array().unwrap().len(), 1);
    assert_eq!(
        f.app.job_detail(b["id"].as_str().unwrap()).unwrap()["scenario_ref"]["revision"],
        2
    );
    assert!(f
        .call(
            "system",
            "dashboard_page",
            json!({"task_id":s["id"],"scenario_ref":{"id":s["id"],"revision":1}})
        )
        .is_err());
}

#[test]
fn historical_dashboard_role_matches_the_research_index_overlay() {
    let mut f = Fixture::new();
    let identity = json!({"source":"queue","kind":"job_view","id":"work:kept-research","revision":"r1","snapshot":"s1"});
    let original = json!({"id":"work:kept-research","kind":"job","label":"Original label","properties":{"record_role":"work"},"relations":[],"artifacts":[]});
    f.app.db.execute("INSERT INTO history_records(source,kind,id,revision,snapshot,body) VALUES('queue','job_view','work:kept-research','r1','s1',?)",[json!({"original":original}).to_string()]).unwrap();
    f.app.db.execute("INSERT INTO research_overlays(kind,id,presentation,actor,reason,updated) VALUES('job','work:kept-research',?,'owner','Preserved classification',0)",[json!({"scope":"research","role":"support","label":"Preserved research"}).to_string()]).unwrap();
    f.app.index_once().unwrap();
    let page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["job"],"view":"research","role":"all"}),
        )
        .unwrap();
    let e = page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["loom_history"] == identity)
        .unwrap();
    assert_eq!(e["presentation"]["role"], "support");
    assert_eq!(e["label"], "Preserved research");
    assert!(f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["job"],"view":"research","role":"main"})
        )
        .unwrap()["entities"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn dashboard_keeps_frozen_consolidation_and_does_not_hide_a_new_source_attempt() {
    let mut f = Fixture::new();
    let source = f.submit("source-execution");
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='complete',result='{\"artifact_ready\":true}' WHERE id=?",
            [&source],
        )
        .unwrap();
    let root=f.call("jobs","submit",json!({"key":"reuse-execution","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"consolidates":[source]})).unwrap()["id"].as_str().unwrap().to_string();
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='complete',result='{\"artifact_ready\":true}' WHERE id=?",
            [&root],
        )
        .unwrap();
    let catalog = f.app.dashboard_catalog().unwrap();
    let child = catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == source)
        .unwrap();
    assert_eq!(child["properties"]["consolidated_into"], root);
    assert_eq!(
        f.app.job_detail(&root).unwrap()["consolidation"]["sources"][0]["job"],
        source
    );
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='running',attempt='later-attempt' WHERE id=?",
            [&source],
        )
        .unwrap();
    assert!(f.app.consolidation_target(&source).unwrap().is_none());
}

#[test]
fn output_name_curation_preserves_sources_and_agrees_across_current_read_models() {
    let mut f = Fixture::new();
    let job = f.submit("label-source");
    f.app.db.execute("UPDATE jobs SET spec=json_set(spec,'$.presentation.label','Parent Job title') WHERE id=?",[&job]).unwrap();
    let id = format!("{job}.policy");
    let manifest=json!({"id":id,"revision":1,"job":job,"kind":"model","label":"Recorded model","components":[],"relations":[],"metadata":{"selected_step":2000}}).to_string();
    f.app
        .db
        .execute(
            "INSERT INTO entity_revisions VALUES(?,1,?,'model','owner','owner',?)",
            rusqlite::params![id, job, manifest],
        )
        .unwrap();
    let original=json!({"original":{"id":"source-dataset","kind":"dataset","label":"Recorded dataset","properties":{"episodes":20},"metrics":[],"relations":[],"artifacts":[]},"locations":[]}).to_string();
    f.app.db.execute("INSERT INTO history_records VALUES('catalog','dataset','source-dataset','r1','fixture',?)",[&original]).unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["model"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(page["entities"][0]["label"], "Recorded model");
    let refs = [
        json!({"kind":"entity","identity":{"id":id,"revision":1}}),
        json!({"kind":"historical","identity":{"source":"catalog","kind":"dataset","id":"source-dataset","revision":"r1","snapshot":"fixture"}}),
    ];
    let resolved: Vec<Value> = refs
        .iter()
        .map(|r| {
            f.call("results", "resolve_item", json!({"native_ref":r}))
                .unwrap()
        })
        .collect();
    for (iteration, model_label, dataset_label) in [
        (
            1,
            "Policy · RGB · Curated model",
            "Grasp · RGB-D · Curated dataset",
        ),
        (
            2,
            "Policy · RGB · Revised name",
            "Grasp · RGB-D · Revised name",
        ),
    ] {
        f.call("jobs","curate",json!({"key":format!("curated-label-{iteration}"),"reason":"User requested a display-name change","dry_run":false,"changes":[{"kind":"entity","id":id,"presentation":{"label":model_label}}]})).unwrap();
        f.app.db.execute("INSERT INTO research_overlays VALUES('dataset','source-dataset',?,'owner','Historical display curation',0) ON CONFLICT(kind,id) DO UPDATE SET presentation=excluded.presentation",[json!({"label":dataset_label}).to_string()]).unwrap();
        for _ in 0..8 {
            f.app.index_once().unwrap();
        }
        for (index, label) in [model_label, dataset_label].into_iter().enumerate() {
            let exact = &resolved[index];
            let current = f
                .call("results", "resolve_item", json!({"native_ref":refs[index]}))
                .unwrap();
            assert_eq!(&current, exact);
            let item = f
                .call(
                    "results",
                    "get_item",
                    json!({"item_id":exact["item_id"],"revision_id":exact["revision_id"]}),
                )
                .unwrap();
            assert_eq!(item["label"], label);
            let detail = f
                .call(
                    "system",
                    "dashboard_entity",
                    json!({"item_id":exact["item_id"],"revision_id":exact["revision_id"]}),
                )
                .unwrap();
            assert_eq!(detail["entities"][0]["label"], label);
            let page=f.call("system","dashboard_page",json!({"kinds":[if index==0{"model"}else{"dataset"}],"view":"all","role":"all"})).unwrap();
            assert_eq!(page["entities"][0]["label"], label);
            let found = f
                .call(
                    "results",
                    "search_items",
                    json!({"text":label,"view":"all","role":"all"}),
                )
                .unwrap();
            assert!(found["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["item_id"] == exact["item_id"] && e["label"] == label));
        }
        let native = f
            .call("results", "get", json!({"id":id,"revision":1}))
            .unwrap();
        assert_eq!(native["label"], model_label);
        assert_eq!(native["metadata"]["selected_step"], 2000);
        let list = f.call("results", "list", json!({"view":"all"})).unwrap();
        assert!(list
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == id && e["label"] == model_label));
        assert!(list
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["record_type"] == "historical" && e["label"] == dataset_label));
        assert_eq!(
            f.app
                .db
                .query_row(
                    "SELECT manifest FROM entity_revisions WHERE id=?",
                    [&id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            manifest
        );
        assert_eq!(
            f.app
                .db
                .query_row(
                    "SELECT body FROM history_records WHERE id='source-dataset'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            original
        );
        assert_eq!(f.count("entity_revisions"), 1);
        assert_eq!(f.count("history_records"), 1);
    }
}

#[test]
fn scientific_summary_recovers_verified_physics_and_observer_accuracy_without_used_placeholders() {
    let e = json!({"id":"fixture-observer","kind":"model","properties":{"model_role":"observer","inputs":[{"kind":"rgb","status":"present","shape":[2,48,64,3]},{"kind":"depth","status":"present","shape":[2,48,64]},{"kind":"instruction","status":"absent"}],"validation_mean_point_error_m":0.004}});
    let source =
        json!({"kind":"file","ref":"readiness.json","sha256":"a".repeat(64),"status":"verified"});
    let doc = json!({"train_frames":80,"validation_frames":20,"ensemble_validation":{"mean_point_error_m":0.004,"point_p95_m":0.009,"selection_data":"validation only","test_frames_read":0},"physics_hz":240,"native_pins":{"robot_gravity_disabled":false,"fixed_mass_properties":{"mass":0.125}}});
    let original = e.clone();
    let summary = crate::research_metadata::summarize(&e, &[(doc, source)]);
    let facts = summary["facts"].as_array().unwrap();
    let fact = |id: &str| facts.iter().find(|v| v["id"] == id).unwrap();
    assert_eq!(fact("validation_mean_error")["value"], 0.004);
    assert_eq!(fact("validation_mean_error")["unit"], "m");
    assert_eq!(fact("physics_hz")["value"], 240);
    assert_eq!(fact("robot_gravity_disabled")["value"], false);
    assert_eq!(fact("payload_mass")["value"], 0.125);
    assert_eq!(summary["inputs"][0]["shape"], json!([2, 48, 64, 3]));
    assert_eq!(summary["inputs"][2]["status"], "absent");
    assert_eq!(fact("test_frames_read")["value"], 0);
    assert_eq!(e, original);
    assert!(fact("physics_hz")["evidence"][0]["source"]["sha256"].is_string());
}

#[test]
fn scientific_summary_does_not_confuse_validation_checks_with_policy_success_or_hide_conflicts() {
    let e = json!({"id":"fixture-evaluation","kind":"experiment","properties":{}});
    let source = json!({"kind":"file","ref":"summary.json"});
    let summary = crate::research_metadata::summarize(
        &e,
        &[
            (
                json!({"counts":{"passed":4,"failed":1},"physics_hz":120}),
                source.clone(),
            ),
            (json!({"physics_hz":240}), source),
        ],
    );
    let facts = summary["facts"].as_array().unwrap();
    assert!(!facts
        .iter()
        .any(|v| ["valid", "planned", "success"].contains(&v["id"].as_str().unwrap_or(""))));
    assert_eq!(
        facts.iter().find(|v| v["id"] == "physics_hz").unwrap()["status"],
        "conflict"
    );
    assert!(!facts.iter().any(|v| v["id"] == "environment"));
}

#[test]
fn scientific_json_enrichment_checks_hashes_and_preserves_original_database_rows() {
    let f = Fixture::new();
    let data = f.root.join("data");
    let raw=br#"{"physics_hz":240,"sampling_clocks_hz":{"command":80,"RGB":20},"totals":{"adopted":4},"hdf5":{"rgb":"obs/rgb","action":"actions"}}"#;
    fs::write(data.join("scientific.json"), raw).unwrap();
    let manifest = json!({"id":"scientific","revision":1,"kind":"dataset","components":[{"ref":"scientific.json","size":raw.len(),"sha256":hash(raw)}],"relations":[]});
    f.app
        .db
        .execute(
            "INSERT INTO asset_revisions VALUES('scientific',1,NULL,'dataset','owner','owner',?)",
            [manifest.to_string()],
        )
        .unwrap();
    f.app.db.execute("INSERT INTO asset_locations VALUES('scientific',1,'scientific.json','fixture','fixture-data','scientific.json','available')",[]).unwrap();
    let selected =
        [json!({"native_ref":{"kind":"entity","identity":{"id":"scientific","revision":1}}})];
    let page = f.app.dashboard_catalog_selected(Some(&selected)).unwrap();
    let entity = page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "scientific")
        .unwrap();
    let summary = &entity["research_summary"];
    assert_eq!(summary["sources"][0]["status"], "verified");
    assert!(summary["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == "physics_hz" && v["value"] == 240));
    assert_eq!(summary["inputs"][0]["kind"], "rgb");
    let original: String = f
        .app
        .db
        .query_row(
            "SELECT manifest FROM asset_revisions WHERE id='scientific'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&original).unwrap(), manifest);
    // A selected page always revalidates bytes, independently of the whole-catalog cache.
    fs::write(data.join("scientific.json"), vec![b' '; raw.len()]).unwrap();
    let selected =
        [json!({"native_ref":{"kind":"entity","identity":{"id":"scientific","revision":1}}})];
    let page = f.app.dashboard_catalog_selected(Some(&selected)).unwrap();
    let entity = page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "scientific")
        .unwrap();
    assert_eq!(
        entity["research_summary"]["sources"][0]["status"],
        "unavailable"
    );
    assert!(!entity["research_summary"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == "physics_hz"));
}

#[test]
fn scientific_sibling_context_requires_exact_publication_product_hash_and_unique_kind() {
    let mut f = Fixture::new();
    let job = f.submit("scientific-context");
    let data = f.root.join("data");
    let weights = b"synthetic model";
    fs::write(data.join("weights.bin"), weights).unwrap();
    let report = json!({"model":{"train_snapshots":40,"validation_snapshots":10,"initialization":"scratch"},"validation":{"error_distributions":{"point_m":{"mean":0.003}}},"artifacts":{"weights.bin":{"sha256":hash(weights)}}});
    let raw = report.to_string();
    fs::write(data.join("report.json"), &raw).unwrap();
    let model = json!({"id":"scientific-model","revision":1,"kind":"model","attempt":"scientific-attempt","components":[{"ref":"weights.bin","sha256":hash(weights),"size":weights.len()}],"relations":[]});
    let evidence = json!({"id":"scientific-report","revision":1,"kind":"experiment","attempt":"scientific-attempt","components":[{"ref":"report.json","sha256":hash(raw.as_bytes()),"size":raw.len()}],"relations":[]});
    for (id, kind, m, component) in [
        ("scientific-model", "model", &model, "weights.bin"),
        ("scientific-report", "experiment", &evidence, "report.json"),
    ] {
        f.app
            .db
            .execute(
                "INSERT INTO asset_revisions VALUES(?,1,?,?, 'owner','owner',?)",
                rusqlite::params![id, job, kind, m.to_string()],
            )
            .unwrap();
        f.app
            .db
            .execute(
                "INSERT INTO asset_locations VALUES(?,1,?,'fixture','fixture-data',?,'available')",
                rusqlite::params![id, component, component],
            )
            .unwrap();
    }
    let selected =
        [json!({"native_ref":{"kind":"entity","identity":{"id":"scientific-model","revision":1}}})];
    let page = f.app.dashboard_catalog_selected(Some(&selected)).unwrap();
    let summary = &page["entities"][0]["research_summary"];
    assert!(summary["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == "validation_mean_error" && v["value"] == 0.003));
    assert_eq!(
        summary["sources"][0]["binding"]["kind"],
        "publication_artifact_sha256"
    );
    let other = json!({"id":"other-model","revision":1,"kind":"model","attempt":"scientific-attempt","components":[],"relations":[]});
    f.app
        .db
        .execute(
            "INSERT INTO asset_revisions VALUES('other-model',1,?,'model','owner','owner',?)",
            rusqlite::params![job, other.to_string()],
        )
        .unwrap();
    let page = f.app.dashboard_catalog_selected(Some(&selected)).unwrap();
    assert!(page["entities"][0]["research_summary"]["facts"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn scientific_summary_keeps_training_losses_snapshot_units_and_explicit_failure_context() {
    let report = json!({"id":"training-report", "kind":"experiment", "properties":{}, "control_policy":{"physics_hz":[120,240],"overwrite_hz":60}, "metadata":{"research":{"physics":{"control_hz":30}}}});
    let document = json!({"selection":{"selected_step":2400,"training_updates":4000,"validation_loss":0.0000003,"criterion":"lowest held-out loss"},"model":{"train_snapshots":700},"outcome":"policy_failure","terminal":{"reason":"missing_estimate"},"clock":{"clock_hz":{"physics":120,"observer":30}},"result":{"action_metrics":{"all_phases":{"rmse_radians":0.002}}}});
    let s = crate::research_metadata::summarize(
        &report,
        &[(document, json!({"kind":"file","ref":"report.json"}))],
    );
    let facts = s["facts"].as_array().unwrap();
    let fact = |id: &str| facts.iter().find(|f| f["id"] == id).unwrap();
    assert_eq!(fact("selected_step")["value"], 2400);
    assert_eq!(fact("selection_validation_loss")["value"], 0.0000003);
    assert_eq!(fact("train_snapshots")["value"], 700);
    assert!(!facts.iter().any(|f| f["id"] == "train_frames"));
    assert_eq!(fact("terminal_reason")["value"], "missing_estimate");
    assert_eq!(fact("control_hz")["value"], 30);
    assert_eq!(fact("physics_hz")["value"], json!([120, 240]));
    assert_eq!(fact("test_action_rmse")["unit"], "rad");
    assert!(!facts.iter().any(|f| f["id"] == "success"));
}

#[test]
fn scientific_condition_outcomes_keep_populations_separate_and_reject_check_counters() {
    let e = json!({"id":"comparison-fixture","kind":"experiment","metadata":{"research":{"evaluation":{"conditions":[{"condition":"RGB","valid":20,"planned":24,"strict_success":12,"functional_success":14},{"condition":"Depth","valid":10,"planned":10,"strict_success":8},{"condition":"Checks","valid":4,"passed":4,"failed":0},{"condition":"Impossible","valid":2,"strict_success":3}]}}}});
    let s = crate::research_metadata::summarize(&e, &[]);
    let f = s["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "condition_outcomes")
        .unwrap();
    assert_eq!(f["value"].as_array().unwrap().len(), 2);
    assert_eq!(f["value"][0]["strict_success"], 12);
    assert_eq!(f["value"][0]["valid"], 20);
    assert_eq!(f["value"][1]["valid"], 10);
    assert_eq!(
        f["evidence"][0]["pointer"],
        "/research/evaluation/conditions"
    );
    let source = json!({"kind":"file","ref":"comparison.json"});
    let first = json!({"conditions":[{"condition":"RGB","valid":20,"strict_success":12}]});
    let second = json!({"conditions":[{"condition":"RGB","valid":20,"strict_success":13}]});
    let conflict = crate::research_metadata::summarize(
        &json!({"id":"files-only","kind":"experiment"}),
        &[(first, source.clone()), (second, source)],
    );
    let c = conflict["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "condition_outcomes")
        .unwrap();
    assert_eq!(c["status"], "conflict");
    assert_eq!(c["alternatives"].as_array().unwrap().len(), 1);
    assert!(!s["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "success"));
}

#[test]
fn metadata_materialization_preserves_source_survives_restart_and_file_loss() {
    let mut f = Fixture::new();
    let data = f.root.join("data");
    let raw = br#"{"physics_hz":240,"sampling_clocks_hz":{"command":80},"hdf5":{"rgb":"obs/rgb"}}"#;
    fs::write(data.join("captured.json"), raw).unwrap();
    let manifest = json!({"id":"capture","revision":1,"kind":"dataset","components":[{"ref":"captured.json","size":raw.len(),"sha256":hash(raw)}],"relations":[]});
    f.app
        .db
        .execute(
            "INSERT INTO asset_revisions VALUES('capture',1,NULL,'dataset','owner','owner',?)",
            [manifest.to_string()],
        )
        .unwrap();
    f.app.db.execute("INSERT INTO asset_locations VALUES('capture',1,'captured.json','fixture','fixture-data','captured.json','available')",[]).unwrap();
    let target = json!({"kind":"entity","identity":{"id":"capture","revision":1}});
    let preview = f
        .call(
            "results",
            "refresh_metadata",
            json!({"target":target,"reason":"fixture"}),
        )
        .unwrap();
    assert_eq!(preview["dry_run"], true);
    assert_eq!(f.count("rust_annotations"), 0);
    let args = json!({"target":target,"reason":"fixture","dry_run":false,"key":"capture-fixture","expected_source_sha256":preview["source_sha256"]});
    let mut wrong = args.clone();
    wrong["expected_source_sha256"] = json!("f".repeat(64));
    assert_eq!(
        f.call("results", "refresh_metadata", wrong)
            .unwrap_err()
            .code,
        "source_changed"
    );
    assert_eq!(f.count("rust_annotations"), 0);
    let stored = f.call("results", "refresh_metadata", args.clone()).unwrap();
    assert_eq!(stored["stored"], true);
    assert_eq!(f.call("results", "refresh_metadata", args).unwrap(), stored);
    assert_eq!(
        f.app
            .db
            .query_row(
                "SELECT manifest FROM asset_revisions WHERE id='capture'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        manifest.to_string()
    );
    fs::remove_file(data.join("captured.json")).unwrap();
    let reopened = Application::open(
        &f.app.local,
        &f.app.bulk,
        &f.app.components,
        f.app.config.clone(),
    )
    .unwrap();
    let catalog = reopened.dashboard_catalog().unwrap();
    let summary = &catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "capture")
        .unwrap()["research_summary"];
    assert!(summary["materialized"]["captured_at"].is_number());
    assert!(summary["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == "physics_hz" && v["value"] == 240));
    let fresh = f
        .call(
            "results",
            "refresh_metadata",
            json!({"target":target,"reason":"new preview"}),
        )
        .unwrap();
    assert_eq!(fresh["facts"], 0);
    f.app.db.execute("UPDATE asset_revisions SET manifest=json_set(manifest,'$.metadata.changed',1) WHERE id='capture'",[]).unwrap();
    let selected = json!({"native_ref":target});
    let changed = f.app.dashboard_catalog_selected(Some(&[selected])).unwrap();
    assert!(changed["entities"][0]["research_summary"]["materialized"].is_null());
}

#[test]
fn metadata_materialization_is_exact_to_historical_snapshot_and_scope() {
    let mut f = Fixture::new();
    for (snapshot, hz) in [("first", 120), ("second", 240)] {
        let body = json!({"entity":{"id":"legacy","kind":"dataset","label":"Dataset","properties":{"physics_hz":hz},"relations":[],"artifacts":[]}});
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES('source','dataset','legacy','r1',?,?)",
                rusqlite::params![snapshot, body.to_string()],
            )
            .unwrap();
    }
    let target = json!({"kind":"historical","identity":{"source":"source","kind":"dataset","id":"legacy","revision":"r1","snapshot":"first"}});
    let preview = f
        .call(
            "results",
            "refresh_metadata",
            json!({"target":target,"reason":"fixture"}),
        )
        .unwrap();
    let args = json!({"target":target,"reason":"fixture","dry_run":false,"key":"historical-capture","expected_source_sha256":preview["source_sha256"]});
    let mut actor = f.actor.clone();
    actor.scopes = vec!["read".into()];
    assert_eq!(
        f.app
            .execute(&actor, "results", "refresh_metadata", args.clone())
            .unwrap_err()
            .status,
        403
    );
    f.call("results", "refresh_metadata", args).unwrap();
    assert_eq!(f.count("rust_annotations"), 1);
    assert_eq!(f.count("history_records"), 2);
    let mut entity = json!({"kind":"dataset","loom_history":{"source":"source","kind":"dataset","id":"legacy","revision":"r1","snapshot":"second"}});
    crate::research_metadata::attach_materialized(&f.app, &mut entity).unwrap();
    assert!(entity["research_summary"].is_null());
}
