use super::*;
fn admin(f: &mut Fixture) {
    let digest = store::hash("fixture administrator");
    f.actor.token_sha256 = Some(digest.clone());
    f.app.config.actors[0].token_sha256 = Some(digest);
}
#[test]
fn system_jobs_stay_in_mcp_and_execution_but_not_dashboard_paging_or_queue() {
    let mut f = Fixture::new();
    let human = f.submit("user-work");
    let system = f.submit("platform-smoke");
    let original = jobs::load(&f.app.db, &system).unwrap();
    assert_eq!(
        f.call(
            "jobs",
            "assign_owner",
            json!({"id":system,"owner":"system"})
        )
        .unwrap_err()
        .status,
        403
    );
    admin(&mut f);
    f.call(
        "jobs",
        "assign_owner",
        json!({"id":system,"owner":"system"}),
    )
    .unwrap();
    let detail = f.call("jobs", "get", json!({"id":system})).unwrap();
    assert_eq!(detail["owner"], "system");
    assert_eq!(detail["recorded_owner"], "owner");
    assert_eq!(jobs::load(&f.app.db, &system).unwrap().spec, original.spec);
    assert_eq!(
        f.call("jobs", "queue", json!({})).unwrap()["entries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        f.app.dashboard_queue().unwrap()["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
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
    let visible = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["job"],"view":"all","role":"all","include_system":false,"limit":1}),
        )
        .unwrap();
    assert_eq!(visible["items"][0]["native_ref"]["identity"]["id"], human);
    assert!(visible["next_cursor"].is_null());
    let page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["job"],"view":"all","role":"all","limit":1,"include_system":true}),
        )
        .unwrap();
    assert_eq!(page["entities"].as_array().unwrap().len(), 1);
    assert_eq!(page["entities"][0]["id"], human);
    assert!(!f.app.dashboard_catalog().unwrap()["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == system));
    let ordinary = auth::person_actor("owner");
    assert_eq!(
        f.app
            .execute(
                &ordinary,
                "jobs",
                "assign_owner",
                json!({"id":system,"owner":"owner"})
            )
            .unwrap_err()
            .status,
        403
    );
    f.call("jobs", "assign_owner", json!({"id":system,"owner":"owner"}))
        .unwrap();
    assert!(!f.app.system_job(&system).unwrap());
}
#[test]
fn system_software_inherits_profiles_without_hiding_user_runtime_development() {
    let mut f = Fixture::new();
    let fixture_manifest =
        json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[]});
    f.app.observe_runtimes(json!({"status":"ok","runtimes":[{"id":"platform-check","revision":1,"sha256":store::hash(compat::seal(&fixture_manifest)),"manifest":fixture_manifest}]})).unwrap();
    let profile = json!({"schema":"loom-execution/v2","inputs":{},"limits":{"timeout_seconds":30,"max_attempts":1},"params_schema":{"type":"object","additionalProperties":false},"outputs":{"proof":"artifact"},"runtime":{"executor":"container","runtime_ref":{"id":"platform-check","revision":1},"command":["/bin/true"],"lane":"cpu"},"usage":{"group":"platform-check","execution_modes":["smoke"]}});
    let mut registered = vec![];
    for id in ["platform-fixture", "isaac-runtime-smoke"] {
        registered.push(f.call("components","register",json!({"id":id,"manifest":{"implementation":{"language":"rust"},"profiles":{"check":profile}},"files":{"src/main.rs":"fn main(){}"}})).unwrap());
    }
    admin(&mut f);
    f.call(
        "components",
        "assign_owner",
        json!({"kind":"component","namespace":"owner","id":"platform-fixture","owner":"system"}),
    )
    .unwrap();
    let profiles = f.call("components", "profiles", json!({})).unwrap();
    assert_eq!(
        profiles
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "cpu.smoke")
            .unwrap()["owner_kind"],
        "system"
    );
    assert_eq!(
        profiles
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["origin"]["id"] == "platform-fixture")
            .unwrap()["owner_kind"],
        "system"
    );
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let dashboard = f.app.dashboard_catalog().unwrap();
    assert!(!dashboard["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["loom_execution_profile"]["origin"]["id"] == "platform-fixture"));
    assert!(dashboard["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["loom_execution_profile"]["origin"]["id"] == "isaac-runtime-smoke"));
    let manifest =
        json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[]});
    f.app.observe_runtimes(json!({"runtimes":[{"id":"platform-check","revision":1,"sha256":store::hash(compat::seal(&manifest)),"manifest":manifest}]})).unwrap();
    f.call(
        "components",
        "assign_owner",
        json!({"kind":"runtime","namespace":"host","id":"platform-check","owner":"system"}),
    )
    .unwrap();
    assert_eq!(
        f.call("components", "runtimes", json!({})).unwrap()["runtimes"][0]["owner"],
        "system"
    );
    assert!(!f.app.dashboard_catalog().unwrap()["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "runtime"));
    assert!(f
        .call(
            "components",
            "get",
            json!({"owner":"owner","id":"platform-fixture"})
        )
        .is_ok());
    // A user choosing an internal test Profile still owns their explicit Job.
    let job = f.submit("explicit-user-check");
    assert!(!f.app.system_job(&job).unwrap());
}
#[test]
fn system_ownership_removes_activity_without_rewriting_execution_and_rejects_stale_visibility_cursor(
) {
    let mut f = Fixture::new();
    let id = f.submit("activity");
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app
        .stage_returned(&work, 1, json!({"returned":true,"exit_code":1}))
        .unwrap();
    let clock = store::now();
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
    f.app
        .db
        .execute("DELETE FROM stage_runs WHERE job=?", [&id])
        .unwrap();
    let other = f.submit("other");
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let page = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["job"],"view":"all","role":"all","include_system":false,"limit":1}),
        )
        .unwrap();
    assert!(page["next_cursor"].is_string());
    let before = f.app.dashboard_activity("all").unwrap();
    assert!(before["days"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["job_count"].as_u64().unwrap() > 0));
    admin(&mut f);
    f.call("jobs", "assign_owner", json!({"id":id,"owner":"system"}))
        .unwrap();
    assert!(!f.app.dashboard_activity("all").unwrap()["days"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["job_count"].as_u64().unwrap() > 0));
    assert_eq!(f.call("results","search_items",json!({"kinds":["job"],"view":"all","role":"all","include_system":false,"limit":1,"cursor":page["next_cursor"]})).unwrap_err().code,"cursor_expired");
    assert!(!f.app.system_job(&other).unwrap());
}
