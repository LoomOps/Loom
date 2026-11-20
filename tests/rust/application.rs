//! Real application/SQLite/filesystem tests. No installed instance or user data.
use super::*;
use crate::store::hash;
use serde_json::{json, Value};
use std::fs;

#[test]
fn queue_forecast_uses_fresh_progress_for_the_current_running_unit() {
    let mut f = Fixture::new();
    f.call(
        "units",
        "submit",
        json!({"key":"forecast-unit","kind":"cpu.smoke",
        "owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}}),
    )
    .unwrap();
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    let unit = |app: &Application| app.queue().unwrap()["unit_entries"][0].clone();
    assert_eq!(unit(&f.app)["state"], "running");
    assert!(unit(&f.app).get("forecast").is_none());
    fs::create_dir_all(&work.folder).unwrap();
    let path = work.folder.join("work-progress.json");
    let now = crate::store::now();
    let mut report = json!({"schema":"loom-work-progress/v1","job":work.job,
        "attempt":work.attempt,"stage":"run","phase":"training","unit":"updates","total":1000,
        "sample":{"completed":100,"recorded":now-30.0},"previous":null});
    fs::write(&path, report.to_string()).unwrap();
    assert_eq!(unit(&f.app)["forecast"]["status"], "measuring");
    report["previous"] = json!({"completed":50,"recorded":now-90.0});
    fs::write(&path, report.to_string()).unwrap();
    let forecast = unit(&f.app)["forecast"].clone();
    assert_eq!(forecast["phase"], "training");
    assert_eq!(forecast["status"], "estimated");
    assert!((forecast["finish_at"].as_f64().unwrap() - (now + 1050.0)).abs() < 0.001);
    let dashboard = f.call("system", "dashboard_queue", json!({})).unwrap();
    assert_eq!(
        dashboard["queue"]["unit_entries"][0]["forecast"]["finish_at"],
        forecast["finish_at"]
    );
    report["attempt"] = json!("old-attempt");
    fs::write(&path, report.to_string()).unwrap();
    assert!(unit(&f.app).get("forecast").is_none());
    report["attempt"] = json!(work.attempt);
    report["sample"]["recorded"] = json!(now - 901.0);
    fs::write(&path, report.to_string()).unwrap();
    assert!(unit(&f.app).get("forecast").is_none());
    report["sample"]["recorded"] = json!(now - 30.0);
    report["previous"]["completed"] = report["sample"]["completed"].clone();
    fs::write(path, report.to_string()).unwrap();
    assert!(unit(&f.app).get("forecast").is_none());
}

#[test]
fn accepted_taskless_research_can_only_replay_its_original_request() {
    let mut f = Fixture::new();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"old-job-task","definition":{"label":"Grasp"}}),
        )
        .unwrap();
    let mut args = json!({"key":"accepted-before-task","owner":"owner","notify":"owner","scenario_ref":{"id":task["id"],"revision":1},
        "outcome":{"goal":"Dataset","deliverables":[{"alias":"dataset","kind":"dataset","from":{"unit":"generate","entity":"dataset"}}]},
        "plan":{"sealed":false,"units":[{"id":"generate","label":"Generate"}]}});
    let original = f.call("jobs", "submit_research", args.clone()).unwrap();
    let job = original["id"].as_str().unwrap();
    f.app
        .db
        .execute(
            "UPDATE job_workflows SET request=json_remove(request,'$.scenario_ref') WHERE id=?",
            [job],
        )
        .unwrap();
    f.app.db.execute("UPDATE rust_plan_revisions SET request=json_remove(request,'$.scenario_ref') WHERE root=?", [job]).unwrap();
    args.as_object_mut().unwrap().remove("scenario_ref");
    assert_eq!(
        f.call("jobs", "submit_research", args.clone()).unwrap(),
        original
    );
    args["outcome"]["goal"] = json!("Different dataset");
    assert_eq!(
        f.call("jobs", "submit_research", args.clone())
            .unwrap_err()
            .code,
        "idempotency_conflict"
    );
    args["key"] = json!("new-without-task");
    assert_eq!(
        f.call("jobs", "submit_research", args).unwrap_err().code,
        "task_required"
    );
}

#[test]
fn exhausted_research_unit_keeps_one_failed_job_and_releases_waiting_units() {
    let mut f = Fixture::new();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"exhausted-task","definition":{"label":"Dataset preparation"}}),
        )
        .unwrap();
    let run = json!({"kind":"cpu.smoke","params":{"seconds":0,"fail":true}});
    let job = f.call("jobs", "submit_research", json!({"key":"typed-exhausted","owner":"owner","notify":"owner","scenario_ref":{"id":task["id"],"revision":1},
        "outcome":{"goal":"Dataset preparation","deliverables":[{"alias":"dataset","kind":"dataset","from":{"unit":"generate","entity":"dataset"}}]},
        "plan":{"sealed":false,"units":[{"id":"check","label":"Check","execution":run},
            {"id":"confirm","label":"Confirm","depends_on":["check"],"execution":run},
            {"id":"generate","label":"Generate","depends_on":["confirm"]}]}})).unwrap();
    let unit = job["units"]["check"].as_str().unwrap();
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(work.job, unit);
    f.app
        .db
        .execute(
            "INSERT INTO recovery(job,state,rounds,updated) VALUES(?,'executing',3,0)",
            [unit],
        )
        .unwrap();
    f.app
        .stage_returned(&work, 1, json!({"returned":true}))
        .unwrap();
    let failed = f.call("jobs", "get", json!({"id":job["id"]})).unwrap();
    assert_eq!(failed["state"], "failed");
    assert_eq!(
        f.call("units", "get", json!({"id":job["units"]["confirm"]}))
            .unwrap()["state"],
        "cancelled"
    );
    assert_eq!(f.app.queue().unwrap()["job_entries"], json!([]));
    assert_eq!(f.count("events"), 1);
}

#[test]
fn research_recovery_names_the_failed_unit_and_rejects_the_parent() {
    let mut f = Fixture::new();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"recovery-task","definition":{"label":"Dataset preparation"}}),
        )
        .unwrap();
    let job = f.call("jobs", "submit_research", json!({"key":"unit-repair","owner":"owner","notify":"owner",
        "scenario_ref":{"id":task["id"],"revision":1},
        "outcome":{"goal":"Dataset","deliverables":[{"alias":"dataset","kind":"dataset","from":{"unit":"generate","entity":"dataset"}}]},
        "plan":{"sealed":false,"units":[{"id":"check","label":"Check","execution":{"kind":"cpu.smoke","params":{"seconds":0,"fail":true}}},
                                        {"id":"generate","label":"Generate","depends_on":["check"]}]}})).unwrap();
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    let unit = &job["units"]["check"];
    assert_eq!(work.job, unit.as_str().unwrap());
    f.app
        .stage_returned(&work, 1, json!({"returned":true}))
        .unwrap();
    let root = f.call("jobs", "get", json!({"id":job["id"]})).unwrap();
    assert_eq!(root["units"][0]["id"], *unit);
    assert_eq!(root["units"][0]["recovery_state"], "waiting");
    assert_eq!(root["units"][0]["recovery_reason"], "exit_nonzero");
    assert_eq!(
        f.app.queue().unwrap()["job_entries"][0]["reason"],
        "unit_failed"
    );
    let error = f
        .call("recovery", "claim", json!({"id":job["id"]}))
        .unwrap_err();
    assert_eq!(
        (error.code.as_str(), error.status),
        ("recovery_unit_required", 409)
    );
    f.call("recovery", "claim", json!({"id":unit})).unwrap();
    let root = f.call("jobs", "get", json!({"id":job["id"]})).unwrap();
    assert_eq!(root["units"][0]["recovery_state"], "claimed");
}

#[test]
fn first_typed_submission_fences_old_writer_and_preserves_exact_reads() {
    let mut f = Fixture::new();
    let legacy = f.submit("legacy-before-units");
    let accepted = f.call("units", "submit", json!({"key":"first-typed","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}})).unwrap();
    let unit = accepted["id"].as_str().unwrap();
    let (local, bulk, components, config) =
        (&f.app.local, &f.app.bulk, &f.app.components, &f.app.config);
    let older_writer = Application::open_schema(local, bulk, components, config.clone(), 7);
    assert_eq!(older_writer.err().unwrap().code, "schema");
    let writer = Application::open(local, bulk, components, config.clone()).unwrap();
    let reader = Application::open_read_only(local, bulk, components, config.clone()).unwrap();
    for app in [&writer, &reader] {
        assert_eq!(app.inspect_job(unit).unwrap()["entry_type"], "unit");
        assert_eq!(app.inspect_job(&legacy).unwrap()["id"], legacy);
        assert_eq!(
            app.db
                .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }
}

#[test]
fn local_source_registration_rejects_special_and_oversized_files_before_publication() {
    use std::os::unix::ffi::OsStrExt;
    let mut f = Fixture::new();
    let source = f.app.components.join("owner/local/src");
    fs::create_dir_all(&source).unwrap();
    let file = source.join("worker.rs");
    let name = std::ffi::CString::new(file.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let args = json!({"id":"local","manifest":{},"files":{}});
    assert_eq!(
        f.call("components", "register", args.clone())
            .unwrap_err()
            .code,
        "artifact"
    );
    fs::remove_file(&file).unwrap();
    fs::File::create(&file)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        f.call("components", "register", args.clone())
            .unwrap_err()
            .code,
        "package"
    );
    assert_eq!(f.count("components"), 0);
    assert!(!source.parent().unwrap().join("revisions").exists());
    fs::write(file, "fn main() {}\n").unwrap();
    assert_eq!(
        f.call("components", "register", args).unwrap()["revision"],
        1
    );
}

#[test]
fn recovery_recomputes_inferred_dependencies_and_preserves_graph_and_store_identity() {
    let mut f = Fixture::new();
    let old = f.submit("old-input");
    let explicit = f.submit("explicit-input");
    let new = f.submit("new-input");
    // Dependency readiness is the fixture boundary; no source bytes are read.
    for id in [&old, &explicit, &new] {
        f.app
            .db
            .execute("UPDATE jobs SET state='complete' WHERE id=?", [id])
            .unwrap();
    }
    let profile = json!({"schema":"loom-execution/v2","revision":"recover.input.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"executor":"container","runtime_id":"cpu","lane":"cpu","command":["/bin/true"]},"inputs":{"data":"."},"outputs":null,"limits":{"max_attempts":1,"timeout_seconds":30}});
    f.app
        .config
        .profiles
        .insert("recover.input.r1".into(), profile);
    let target=f.call("jobs","submit",json!({"key":"repair-input","kind":"recover.input.r1","owner":"owner","notify":"owner","depends_on":[explicit],"params":{"data":{"job":old,"entity":"data"}}})).unwrap()["id"].as_str().unwrap().to_string();
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(work.job, target);
    f.app
        .stage_returned(&work, 1, json!({"returned":true}))
        .unwrap();
    let downstream=f.call("jobs","submit",json!({"key":"downstream","kind":"cpu.smoke","owner":"owner","notify":"owner","depends_on":[target],"params":{"seconds":0,"fail":false}})).unwrap()["id"].clone();
    let workflow=f.legacy_workflow(json!({"key":"downstream-graph","label":"Dependent graph","owner":"owner","notify":"owner","nodes":[{"id":"one","label":"Dependent","spec":{"kind":"cpu.smoke","depends_on":[target],"params":{"seconds":0,"fail":false}}}],"outputs":{}})).unwrap()["id"].clone();
    let lease = f.call("recovery", "claim", json!({"id":target})).unwrap()["lease"].clone();
    for dependency in [json!(target), downstream, workflow] {
        let error=f.call("recovery","resume",json!({"id":target,"lease":lease,"patch":{"params":{"data":{"job":dependency,"entity":"data"}}}})).unwrap_err();
        assert_eq!(error.code, "dependency_cycle");
        assert_eq!(
            jobs::load(&f.app.db, &target).unwrap().aggregate.state,
            loom_domain::JobState::Failed
        );
    }
    let request =
        json!({"id":target,"lease":lease,"patch":{"params":{"data":{"job":new,"entity":"data"}}}});
    f.app.config.storage_id = "replacement".into();
    fs::write(
        f.app.bulk.join(".loom-storage.json"),
        r#"{"storage_id":"replacement"}"#,
    )
    .unwrap();
    assert_eq!(
        f.call("recovery", "resume", request.clone())
            .unwrap_err()
            .code,
        "storage"
    );
    f.app.config.storage_id = "fixture-data".into();
    fs::write(
        f.app.bulk.join(".loom-storage.json"),
        r#"{"storage_id":"fixture-data"}"#,
    )
    .unwrap();
    f.call("recovery", "resume", request).unwrap();
    let repaired = jobs::load(&f.app.db, &target).unwrap();
    assert_eq!(repaired.spec["_requested_dependencies"], json!([explicit]));
    assert_eq!(repaired.spec["depends_on"], json!([explicit, new]));
    assert_eq!(repaired.spec["_output_storage_id"], "fixture-data");
    assert_eq!(f.count("attempts"), 1);
}

#[test]
fn invalid_upstream_members_cannot_silently_select_the_whole_artifact() {
    let mut f = Fixture::new();
    let upstream = f.submit("member-source");
    let mut work = f.app.next_work(&Default::default()).unwrap().unwrap();
    let profile = json!({"schema":"loom-execution/v2","revision":"member.consumer.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"executor":"container","runtime_id":"cpu","lane":"cpu","command":["/bin/true"]},"inputs":{"data":"."},"outputs":null,"limits":{"max_attempts":1,"timeout_seconds":30}});
    f.app
        .config
        .profiles
        .insert("member.consumer.r1".into(), profile.clone());
    for member in [json!(false), json!(7), json!(null), json!([]), json!({})] {
        let input = json!({"job":upstream,"entity":"observation","member":member});
        let error=f.call("jobs","submit",json!({"key":"invalid-member","kind":"member.consumer.r1","owner":"owner","notify":"owner","params":{"data":input}})).unwrap_err();
        assert_eq!(error.code, "input");
        assert_eq!(f.count("jobs"), 1);
        // Old accepted/corrupt snapshots must fail at execution too, rather than
        // treating a non-string member as an absent selection.
        work.spec["profile"] = profile.clone();
        work.spec["params"] = json!({"data":input});
        assert_eq!(
            f.app.request_for(&work).unwrap_err().message,
            "Input member must be a string"
        );
    }
    let accepted=f.call("jobs","submit",json!({"key":"valid-member","kind":"member.consumer.r1","owner":"owner","notify":"owner","params":{"data":{"job":upstream,"entity":"observation","member":"selected.json"}}})).unwrap();
    assert!(accepted["id"].is_string());
}

#[test]
fn dashboard_and_tailscale_resolve_the_same_casefolded_user_binding() {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    let mut f = Fixture::new();
    let key = f.root.join("relay-key");
    let secret = b"fixture-only-relay-key-00000000000000";
    fs::write(&key, secret).unwrap();
    f.app.config.extra.insert("tailscale_auth".into(), json!({"enabled":true,"trusted_proxies":["127.0.0.1"],"users":{"Straße@EXAMPLE.test":"owner"}}));
    f.app.config.extra.insert(
        "dashboard_auth".into(),
        json!({"enabled":true,"issuer":"fixture","relay_key_file":key,"link_tailscale_email":true}),
    );
    let login = "strasse@example.test";
    let claims = json!({"aud":"loom-dashboard-relay","exp":store::now()+30.,"identity":{"issuer":"fixture","subject":"selected-user","login":login}});
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).unwrap();
    mac.update(payload.as_bytes());
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    for headers in [
        std::collections::BTreeMap::from([("tailscale-user-login".into(), vec![login.into()])]),
        std::collections::BTreeMap::from([(
            "x-loom-dashboard-identity".into(),
            vec![format!("{payload}.{signature}")],
        )]),
    ] {
        let actor =
            auth::authenticate(&f.app.db, &f.app.config, &headers, "127.0.0.1", false).unwrap();
        assert_eq!(actor.id, "owner");
        assert_eq!(actor.scopes, f.actor.scopes);
    }
}

#[test]
fn maintenance_keys_cannot_replay_another_action_with_identical_arguments() {
    let mut f = Fixture::new();
    let args = json!({"key":"one-operation"});
    let first = f.call("system", "prune_search", args.clone()).unwrap();
    assert_eq!(
        first,
        f.call("system", "prune_search", args.clone()).unwrap()
    );
    assert_eq!(
        f.call("system", "rebuild_search", args).unwrap_err().code,
        "idempotency"
    );
    assert_eq!(f.count("catalog_tasks"), 1);
}

#[test]
fn component_and_tool_build_files_retain_binary_size_and_hash() {
    let mut f = Fixture::new();
    let sha = store::hash(b"built bytes");
    for (table, kind) in [("components", "component"), ("tools", "tool")] {
        let manifest = json!({"label":"Built producer","build_receipt":{"binary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"producer","size":11,"sha256":sha}}});
        let sql = if table == "tools" {
            "INSERT INTO tools VALUES('owner','built',1,?,'source')"
        } else {
            "INSERT INTO components(owner,id,revision,manifest,profiles) VALUES('owner','built',1,?,'{}')"
        };
        f.app.db.execute(sql, [manifest.to_string()]).unwrap();
        f.app.index_once().unwrap();
        let parent=f.call("results","resolve_item",json!({"native_ref":{"kind":kind,"identity":{"owner":"owner","id":"built","revision":1}}})).unwrap();
        f.app.index_once().unwrap();
        let files = f
            .call(
                "results",
                "item_files",
                json!({"item_id":parent["item_id"]}),
            )
            .unwrap();
        let binary = &files["items"][0];
        assert_eq!(binary["size"], 11);
        assert_eq!(binary["sha256"], sha);
        let file = f
            .call(
                "results",
                "get_file",
                json!({"file_item_id":binary["item_id"]}),
            )
            .unwrap();
        assert_eq!(file["size"], 11);
        assert_eq!(file["sha256"], sha);
    }
}

#[test]
fn file_and_identity_probes_reject_unconnected_fifos_and_close_descriptors_on_exec() {
    use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
    use std::{sync::mpsc, thread, time::Duration};
    let f = Fixture::new();
    let fifo = f.root.join("not-a-file");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    for probe in ["hash", "inventory", "identity"] {
        let path = fifo.clone();
        let mut config = f.app.config.clone();
        config.extra.insert(
            "dashboard_auth".into(),
            json!({"enabled":true,"relay_key_file":path}),
        );
        let (send, recv) = mpsc::channel();
        let reader = thread::spawn(move || {
            let rejected = match probe {
                "inventory" => crate::file_set::bounded(&path, 100).is_err(),
                "identity" => {
                    let db = rusqlite::Connection::open_in_memory().unwrap();
                    let headers = std::collections::BTreeMap::from([(
                        "x-loom-dashboard-identity".into(),
                        vec!["claims.signature".into()],
                    )]);
                    auth::authenticate(&db, &config, &headers, "127.0.0.1", false)
                        .unwrap_err()
                        .status
                        == 503
                }
                _ => crate::storage::file_hash(&path).is_err(),
            };
            send.send(rejected).unwrap();
        });
        let result = recv.recv_timeout(Duration::from_secs(1));
        if result.is_err() {
            // Release an old blocking implementation before failing the assertion.
            drop(
                fs::OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&fifo),
            );
        }
        reader.join().unwrap();
        assert!(
            result.unwrap(),
            "Special-file probe must not wait for a writer"
        );
    }
    fs::write(f.root.join("regular"), b"bytes").unwrap();
    let file = crate::storage::open_registered(&f.root, "regular").unwrap();
    assert_ne!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
}

#[test]
fn imported_dataset_mirrors_preserve_content_hashes_and_publication_state() {
    let f = Fixture::new();
    let mut keys = vec![];
    for (source, count, state, sha) in [
        ("published", 4, "final", "a"),
        ("catalog", 4, "final", "a"),
        ("different-count", 5, "final", "a"),
        ("different-bytes", 4, "final", "b"),
        ("different-state", 4, "legacy_import", "a"),
    ] {
        let mut original = json!({"id":"same","kind":"dataset","properties":{"episodes":count},"artifacts":[{"path":"manifest.json"}]});
        let mut body = json!({"original":original,"locations":[{"source_ref":"manifest.json","sha256":sha.repeat(64),"size":20}]});
        if source == "published" {
            body["publication_state"] = json!(state);
        } else {
            original["artifacts"][0]["id"] = json!("artifact-0123456789abcdef0123");
            original["publication"] = json!({"state":state,"delivery_status":"indexed","publication_id":"import-id","entity_revision":2,"contract_revision":"fixture"});
            body["original"] = original;
        }
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES(?,'dataset','same','r1','fixture',?)",
                rusqlite::params![source, serde_json::to_string_pretty(&body).unwrap()],
            )
            .unwrap();
        let mut e = json!({"kind":"dataset","loom_history":{"source":source,"kind":"dataset","id":"same","revision":"r1","snapshot":"fixture"}});
        crate::dataset_lineage::attach(&f.app, &mut e).unwrap();
        keys.push(e["dataset_mirror_key"].clone());
    }
    assert_eq!(keys[0], keys[1]);
    for key in &keys[2..] {
        assert_ne!(*key, keys[0]);
    }
}

#[test]
fn historical_dataset_mirrors_require_identical_original_bodies_not_matching_names() {
    let f = Fixture::new();
    let mut keys = vec![];
    for (source, count) in [("published", 4), ("catalog", 4), ("different", 5)] {
        let body = json!({"entity":{"id":"legacy","kind":"dataset","label":"Same label","properties":{"count":count}}});
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES(?,'dataset','legacy','r1','fixture',?)",
                rusqlite::params![source, body.to_string()],
            )
            .unwrap();
        let mut e = json!({"kind":"dataset","loom_history":{"source":source,"kind":"dataset","id":"legacy","revision":"r1","snapshot":"fixture"}});
        crate::dataset_lineage::attach(&f.app, &mut e).unwrap();
        keys.push(e["dataset_mirror_key"].clone());
    }
    assert_eq!(keys[0], keys[1]);
    assert_ne!(keys[0], keys[2]);
}

#[test]
fn historical_policy_mirrors_use_exact_publication_and_preserve_selected_steps() {
    let f = Fixture::new();
    let mut selectors = Vec::new();
    for (source, revision, snapshot, digest) in [
        ("catalog", "842", "migration", "a"),
        ("publication", "2", "migration", "a"),
        ("publication", "3", "migration", "a"),
        ("publication", "2", "different-file", "b"),
    ] {
        let mut body = json!({
            "original":{"id":"legacy-policy","kind":"model","label":"Recorded policy","properties":{"selected_step":5500},
                "artifacts":[{"path":"model.json"},{"path":"resume-latest.pt"}]},
            "locations":[{"source_ref":"model.json","size":20,"sha256":digest.repeat(64)},{"source_ref":"resume-latest.pt","status":"missing"}]
        });
        if source == "catalog" {
            body["original"]["publication"] = json!({"state":"final","entity_revision":2});
            body["original"]["artifacts"][0]["id"] = json!("artifact-0123456789abcdef0123");
            body["original"]["artifacts"][1]["id"] = json!("artifact-1123456789abcdef0123");
        } else {
            body["publication_state"] = json!("final");
        }
        let item_id = format!("{source}-{revision}-{snapshot}");
        f.app
            .db
            .execute(
                "INSERT INTO history_records VALUES(?,'model','legacy-policy',?,?,?)",
                rusqlite::params![source, revision, snapshot, body.to_string()],
            )
            .unwrap();
        selectors.push(json!({"item_id":item_id,"kind":"model","native_ref":{"kind":"historical","identity":{
            "source":source,"kind":"model","id":"legacy-policy","revision":revision,"snapshot":snapshot
        }}}));
    }
    let page = f.app.dashboard_catalog_selected(Some(&selectors)).unwrap();
    let entities = page["entities"].as_array().unwrap();
    let records: Vec<_> = selectors
        .iter()
        .map(|item| {
            let id = format!("history:{}", item["item_id"].as_str().unwrap());
            entities.iter().find(|entity| entity["id"] == id).unwrap()
        })
        .collect();
    assert_eq!(
        records[0]["catalog_mirror_key"],
        records[1]["catalog_mirror_key"]
    );
    assert_ne!(
        records[0]["catalog_mirror_key"],
        records[2]["catalog_mirror_key"]
    );
    assert_ne!(
        records[0]["catalog_mirror_key"],
        records[3]["catalog_mirror_key"]
    );
    assert!(records[0]["research_summary"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|fact| fact["id"] == "selected_step" && fact["value"] == 5500));
}

#[test]
fn retired_policy_revision_stays_discoverable_when_another_job_revises_it() {
    let mut f = Fixture::new();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"policy-task","definition":{"label":"Train a policy"}}),
        )
        .unwrap();
    let task_ref = json!({"id":task["id"],"revision":1});
    let revision_one = json!({"id":"policy","revision":1,"kind":"model","catalog_class":"regular",
        "scenario_ref":task_ref,"metadata":{"selected_step":10000},"components":[],"relations":[]});
    f.app
        .db
        .execute(
            "INSERT INTO asset_revisions VALUES('policy',1,NULL,'model','owner','owner',?)",
            [revision_one.to_string()],
        )
        .unwrap();
    let base = json!({"id":"policy","revision":1});
    let retire = json!({"key":"discard-r1","reason":"Superseded training","state":"discarded",
        "targets":[{"kind":"entity","identity":base}]});
    let researcher = crate::auth::person_actor("researcher");
    let mut reader = researcher.clone();
    reader.scopes = vec!["read".into()];
    assert_eq!(
        f.app
            .execute(&reader, "results", "set_output_disposition", retire.clone())
            .unwrap_err()
            .status,
        403
    );
    let receipt = f
        .app
        .execute(
            &researcher,
            "results",
            "set_output_disposition",
            retire.clone(),
        )
        .unwrap();
    assert_eq!(receipt["changed"][0]["disposition"]["actor"], "researcher");
    assert_eq!(
        receipt,
        f.app
            .execute(
                &researcher,
                "results",
                "set_output_disposition",
                retire.clone()
            )
            .unwrap()
    );
    let mut restore = retire.clone();
    restore["key"] = json!("restore-r1");
    restore["state"] = json!("current");
    assert_eq!(
        f.app
            .execute(
                &reader,
                "results",
                "set_output_disposition",
                restore.clone()
            )
            .unwrap_err()
            .status,
        403
    );
    f.app
        .execute(&researcher, "results", "set_output_disposition", restore)
        .unwrap();
    assert_eq!(
        f.call("results", "get", base.clone()).unwrap()["disposition"]["state"],
        "current"
    );
    let mut delete = retire.clone();
    delete["state"] = json!("deleted");
    assert_eq!(
        f.app
            .execute(&researcher, "results", "set_output_disposition", delete)
            .unwrap_err()
            .status,
        400
    );
    let mut retire_again = retire;
    retire_again["key"] = json!("discard-r1-again");
    f.app
        .execute(
            &researcher,
            "results",
            "set_output_disposition",
            retire_again,
        )
        .unwrap();

    let mut request = json!({"key":"revise-policy","owner":"owner","notify":"owner","scenario_ref":task_ref,
        "outcome":{"goal":"Train the next Policy revision","deliverables":[
            {"alias":"policy","kind":"model","from":{"unit":"train","entity":"weights"},"revises":base}]},
        "plan":{"sealed":false,"units":[{"id":"train","label":"Training"}]}});
    let accepted = f.call("jobs", "submit_research", request.clone()).unwrap();
    let producer = f.submit("policy-training-attempt");
    let mut spec = jobs::load(&f.app.db, &producer).unwrap().spec;
    spec.as_object_mut().unwrap().remove("_reporting_required");
    spec["scenario_ref"] = task_ref;
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='processing',attempt='fixture-attempt',spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), producer],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "INSERT INTO execution_units VALUES(?,?,'train','Training','run')",
            rusqlite::params![producer, accepted["id"].as_str()],
        )
        .unwrap();
    let folder = f.app.bulk.join("retraining");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("weights.json"), b"{}").unwrap();
    let manifest = json!({"entities":[{"id":"weights","kind":"model","path":"weights.json",
        "provenance":{},"relations":[],"metadata":{"selected_step":20000}}],"metrics":[]});
    let output = f
        .app
        .publish_result(&producer, "fixture-attempt", manifest, &folder)
        .unwrap();
    assert_eq!(
        output["entities"][0]["registry_ref"],
        json!({"id":"policy","revision":2})
    );
    assert_eq!(
        f.call("results", "get", base).unwrap()["disposition"]["state"],
        "discarded"
    );
    assert!(f
        .call("results", "get", json!({"id":"policy","revision":2}))
        .unwrap()["disposition"]
        .is_null());
    assert_eq!(
        crate::results::input_relations(
            &f.app.db,
            &json!({"policy":{"job":producer,"entity":"weights"}})
        )
        .unwrap()[0]["revision"],
        2
    );
    request["key"] = json!("stale-base");
    assert_eq!(
        f.call("jobs", "submit_research", request).unwrap_err().code,
        "policy_revision"
    );

    for _ in 0..12 {
        f.app.index_once().unwrap();
    }
    let current = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["model"],"view":"all","role":"all","output_state":"current","records_only":true}),
        )
        .unwrap();
    assert_eq!(current["entities"].as_array().unwrap().len(), 1);
    assert_eq!(current["entities"][0]["loom_registry"]["revision"], 2);
    assert_eq!(current["task_index"], json!([task["id"]]));
    let retired = f.call("system","dashboard_page",json!({"kinds":["model"],"view":"all","role":"all","history":"all","output_state":"discarded","records_only":true})).unwrap();
    assert_eq!(retired["entities"].as_array().unwrap().len(), 1);
    assert_eq!(retired["entities"][0]["loom_registry"]["revision"], 1);
    assert_eq!(retired["entities"][0]["disposition"]["state"], "discarded");
    assert_eq!(retired["task_index"], json!([task["id"]]));
    assert!(retired["entities"][0]["id"]
        .as_str()
        .unwrap()
        .starts_with("revision:"));
}

#[test]
fn dataset_organization_is_source_bound_keyed_and_distinct_from_lineage() {
    let mut f = Fixture::new();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"family","definition":{"label":"Fixture family"}}),
        )
        .unwrap();
    let task_ref = json!({"id":task["id"],"revision":1});
    for id in ["one", "two"] {
        let manifest =
            json!({"id":id,"revision":1,"kind":"dataset","components":[],"relations":[]});
        f.app
            .db
            .execute(
                "INSERT INTO asset_revisions VALUES(?,1,NULL,'dataset','owner','owner',?)",
                rusqlite::params![id, manifest.to_string()],
            )
            .unwrap();
    }
    let target = json!({"kind":"entity","identity":{"id":"one","revision":1}});
    let mut args = json!({"target":target,"task_ref":task_ref,"path":[{"id":"cohort","label":"Cohort"},{"id":"part","label":"Part"}],"reason":"Verified collection context"});
    assert_eq!(
        f.call("results", "organize_dataset", args.clone())
            .unwrap_err()
            .code,
        "organization_task"
    );
    for id in ["one", "two"] {
        f.call("results","link_scenario",json!({"key":format!("link-{id}"),"scenario_ref":task_ref,"target":{"kind":"entity","identity":{"id":id,"revision":1}},"relation":"produced_for","provenance":{"reason":"Fixture"}})).unwrap();
    }
    let preview = f.call("results", "organize_dataset", args.clone()).unwrap();
    assert_eq!(f.count("rust_annotations"), 0);
    args["dry_run"] = json!(false);
    args["key"] = json!("organize");
    args["expected_digest"] = preview["digest"].clone();
    let applied = f.call("results", "organize_dataset", args.clone()).unwrap();
    assert_eq!(applied["stored"], true);
    assert_eq!(
        f.call("results", "organize_dataset", args.clone()).unwrap(),
        applied
    );
    let result = f
        .call("results", "get", json!({"id":"one","revision":1}))
        .unwrap();
    assert_eq!(
        result["dataset_organization"]["path"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(result["dataset_lineage"]["parents"]
        .as_array()
        .unwrap()
        .is_empty());
    let mut other = args.clone();
    other["dry_run"] = json!(true);
    other["target"]["identity"]["id"] = json!("two");
    other["path"][0]["label"] = json!("Wrong");
    assert_eq!(
        f.call("results", "organize_dataset", other)
            .unwrap_err()
            .code,
        "organization_label"
    );
    let mut move_args = args.clone();
    move_args["path"] = json!([]);
    move_args["dry_run"] = json!(true);
    let moved_preview = f
        .call("results", "organize_dataset", move_args.clone())
        .unwrap();
    move_args["expected_digest"] = moved_preview["digest"].clone();
    move_args["key"] = json!("move");
    move_args["dry_run"] = json!(false);
    f.call("results", "organize_dataset", move_args).unwrap();
    args["key"] = json!("stale-preview");
    assert_eq!(
        f.call("results", "organize_dataset", args)
            .unwrap_err()
            .code,
        "source_changed"
    );
    assert_eq!(f.count("rust_annotations"), 2);
    f.app.db.execute("UPDATE asset_revisions SET manifest=json_set(manifest,'$.label','changed') WHERE id='one'",[]).unwrap();
    let changed = f
        .call("results", "get", json!({"id":"one","revision":1}))
        .unwrap();
    assert!(changed.get("dataset_organization").is_none());
}

#[test]
fn historical_dataset_organization_preserves_original_and_requires_active_task_link() {
    let mut f = Fixture::new();
    let body=json!({"original":{"id":"old-data","kind":"dataset","label":"Original label","properties":{},"relations":[],"artifacts":[]}}).to_string();
    f.app
        .db
        .execute(
            "INSERT INTO history_records VALUES('catalog','dataset','old-data','r1','fixture',?)",
            [&body],
        )
        .unwrap();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"legacy-family","definition":{"label":"Family"}}),
        )
        .unwrap();
    let target = json!({"kind":"historical","identity":{"source":"catalog","kind":"dataset","id":"old-data","revision":"r1","snapshot":"fixture"}});
    let pin = json!({"id":task["id"],"revision":1});
    f.call("results","link_scenario",json!({"key":"legacy-link","scenario_ref":pin,"target":target,"relation":"produced_for","provenance":{"reason":"Fixture"}})).unwrap();
    let mut args = json!({"target":target,"task_ref":pin,"path":[{"id":"cohort","label":"Cohort"}],"reason":"Explicit historical grouping"});
    let preview = f.call("results", "organize_dataset", args.clone()).unwrap();
    args["expected_digest"] = preview["digest"].clone();
    args["key"] = json!("legacy-group");
    args["dry_run"] = json!(false);
    f.call("results", "organize_dataset", args).unwrap();
    let catalog = f.app.dashboard_catalog().unwrap();
    let e = catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["loom_history"]["id"] == "old-data")
        .unwrap();
    assert_eq!(e["dataset_organization"]["path"][0]["id"], "cohort");
    let stored: String = f
        .app
        .db
        .query_row(
            "SELECT body FROM history_records WHERE id='old-data'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, body);
    f.app.db.execute("DELETE FROM scenario_links", []).unwrap();
    let catalog = f.app.dashboard_catalog().unwrap();
    let e = catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["loom_history"]["id"] == "old-data")
        .unwrap();
    assert!(e.get("dataset_organization").is_none());
}

#[test]
fn dataset_organization_declarations_reject_ambiguous_or_unbounded_paths() {
    use crate::dataset_organization::validate_declaration as validate;
    assert!(validate(&json!({"path":[],"task_ref":{"id":"task","revision":1}})).is_ok());
    for value in [
        json!({"path":[{"id":"space id","label":"Invalid"}]}),
        json!({"path":[{"id":"ok","label":"  "}]}),
        json!({"path":[],"task_ref":{"id":"task"}}),
        json!({"path":[],"extra":true}),
        json!({"path":vec![json!({"id":"group","label":"Group"});13]}),
    ] {
        assert!(validate(&value).is_err());
    }
}

#[test]
fn native_dataset_publication_resolves_only_declared_data_parent_inputs() {
    let mut f = Fixture::new();
    let parent =
        json!({"id":"source-dataset","revision":1,"kind":"dataset","components":[],"relations":[]});
    f.app.db.execute("INSERT INTO asset_revisions VALUES('source-dataset',1,NULL,'dataset','owner','owner',?)",[parent.to_string()]).unwrap();
    let job = f.submit("derived-publication");
    let mut spec = jobs::load(&f.app.db, &job).unwrap().spec;
    // The lineage boundary also applies to executions accepted before reporting.
    spec.as_object_mut().unwrap().remove("_reporting_required");
    spec["params"] = json!({"data":{"artifact":{"id":"source-dataset","revision":1}}});
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"native-family","definition":{"label":"Native family"}}),
        )
        .unwrap();
    spec["scenario_ref"] = json!({"id":task["id"],"revision":1,"sha256":task["sha256"]});
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='processing',attempt='fixture-attempt',spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), job],
        )
        .unwrap();
    let folder = f.root.join("data").join("derived");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("dataset.json"), b"{}").unwrap();
    let manifest = json!({"entities":[{"id":"selected","kind":"dataset","path":"dataset.json","provenance":{},"relations":[],"metadata":{"research":{"organization":{"path":[{"id":"cohort","label":"Cohort"}],"task_ref":{"id":task["id"],"revision":1}}}},"lineage":[{"input":"data","transformation":{"kind":"condition_selection","details":"Selected two conditions","member_count":2}}]}],"metrics":[]});
    let mut missing = manifest.clone();
    missing["entities"][0]["lineage"][0]["input"] = json!("not-an-input");
    assert_eq!(
        f.app
            .publish_result(&job, "fixture-attempt", missing, &folder)
            .unwrap_err()
            .code,
        "lineage_input"
    );
    assert_eq!(f.count("entity_revisions"), 0);
    let mut wrong_task = manifest.clone();
    wrong_task["entities"][0]["metadata"]["research"]["organization"]["task_ref"] =
        json!({"id":"other-task","revision":1});
    assert_eq!(
        f.app
            .publish_result(&job, "fixture-attempt", wrong_task, &folder)
            .unwrap_err()
            .code,
        "organization_task"
    );
    f.app
        .publish_result(&job, "fixture-attempt", manifest, &folder)
        .unwrap();
    let result = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.selected"),"revision":1}),
        )
        .unwrap();
    let links = result["dataset_lineage"]["parents"].as_array().unwrap();
    assert_eq!(result["dataset_organization"]["task_ref"]["id"], task["id"]);
    assert_eq!(result["dataset_organization"]["path"][0]["id"], "cohort");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["parent"]["identity"]["id"], "source-dataset");
    assert_eq!(links[0]["transformation"]["kind"], "condition_selection");
    assert_eq!(
        result["relations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "consumes")
            .count(),
        1
    );
}

#[test]
fn dataset_lineage_requires_exact_sources_evidence_and_acyclic_relations() {
    let mut f = Fixture::new();
    let mut manifests = vec![];
    for id in ["parent", "child", "other"] {
        let manifest = json!({"id":id,"revision":1,"kind":"dataset","label":id,"components":[{"ref":"manifest.json","sha256":hash(id),"size":id.len()}],"relations":[]});
        f.app
            .db
            .execute(
                "INSERT INTO asset_revisions VALUES(?,1,NULL,'dataset','owner','owner',?)",
                rusqlite::params![id, manifest.to_string()],
            )
            .unwrap();
        manifests.push(manifest);
    }
    let parent = json!({"kind":"entity","identity":{"id":"parent","revision":1}});
    let child = json!({"kind":"entity","identity":{"id":"child","revision":1}});
    let mut args = json!({"target":child,"parent":parent,"transformation":{"kind":"subset","details":"Two selected members","member_count":2},"evidence":[{"target":parent,"component":"manifest.json","sha256":hash("parent")}],"reason":"Bounded fixture"});
    let preview = f.call("results", "link_dataset", args.clone()).unwrap();
    assert_eq!(preview["stored"], false);
    assert_eq!(f.count("rust_annotations"), 0);
    let mut alias = args.clone();
    alias["parent"] = alias["target"].clone();
    alias["parent"]["identity"]["ignored"] = json!(true);
    assert_eq!(
        f.call("results", "link_dataset", alias).unwrap_err().code,
        "lineage_target"
    );
    args["dry_run"] = json!(false);
    args["key"] = json!("link");
    args["expected_digest"] = json!("f".repeat(64));
    assert_eq!(
        f.call("results", "link_dataset", args.clone())
            .unwrap_err()
            .code,
        "source_changed"
    );
    args["expected_digest"] = preview["digest"].clone();
    let applied = f.call("results", "link_dataset", args.clone()).unwrap();
    assert_eq!(applied["stored"], true);
    assert_eq!(
        f.call("results", "link_dataset", args.clone()).unwrap(),
        applied
    );
    assert_eq!(f.count("rust_annotations"), 1);
    let mut reverse = args.clone();
    reverse["target"] = parent.clone();
    reverse["parent"] = child.clone();
    reverse["dry_run"] = json!(true);
    reverse["key"] = json!("cycle");
    assert_eq!(
        f.call("results", "link_dataset", reverse).unwrap_err().code,
        "lineage_cycle"
    );
    let mut bad = args.clone();
    bad["evidence"][0]["sha256"] = json!("a".repeat(64));
    assert_eq!(
        f.call("results", "link_dataset", bad).unwrap_err().code,
        "lineage_evidence"
    );
    let mut missing = args;
    missing["parent"]["identity"]["revision"] = json!(99);
    assert!(f.call("results", "link_dataset", missing).is_err());
    let result = f
        .call("results", "get", json!({"id":"child","revision":1}))
        .unwrap();
    assert_eq!(result["dataset_lineage"]["parents"][0]["parent"], parent);
    for manifest in manifests {
        let text: String = f
            .app
            .db
            .query_row(
                "SELECT manifest FROM asset_revisions WHERE id=?",
                [manifest["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), manifest);
    }
}

#[test]
fn observer_annotation_source_cannot_imply_stored_language_or_depth() {
    let entity = json!({"id":"paired","kind":"dataset","dataset_overview":{"count":4,"count_unit":"episodes","inputs":[{"kind":"vision","label":"Stored","status":"present","value":"RGB-D","source_ref":"manifest.json#dynamic_instruction_source"}]}});
    let doc = json!({"BC_ready":false,"positive_coverage":{"train":0,"validation":0},"dynamic_instruction_source":"observer-current-and-past","frames":40});
    let summary = crate::research_metadata::summarize(
        &entity,
        &[(doc, json!({"kind":"file","ref":"manifest.json"}))],
    );
    let facts = summary["facts"].as_array().unwrap();
    assert!(facts
        .iter()
        .any(|f| f["id"] == "bc_ready" && f["value"] == false));
    assert!(facts
        .iter()
        .any(|f| f["id"] == "positive_coverage" && f["value"]["train"] == 0));
    assert!(!facts.iter().any(|f| f["id"] == "instruction_source"));
    assert!(summary["inputs"].as_array().unwrap().is_empty());
}

#[test]
fn obsolete_captured_observer_aliases_cannot_return_when_source_file_is_missing() {
    let f = Fixture::new();
    let manifest = json!({"id":"old","revision":1,"kind":"dataset","components":[],"relations":[]});
    f.app
        .db
        .execute(
            "INSERT INTO asset_revisions VALUES('old',1,NULL,'dataset','owner','owner',?)",
            [manifest.to_string()],
        )
        .unwrap();
    let target: contracts::Target =
        serde_json::from_value(json!({"kind":"entity","identity":{"id":"old","revision":1}}))
            .unwrap();
    let snapshot = json!({"extractor_version":1,"source_sha256":hash(manifest.to_string()),"summary":{"facts":[{"id":"instruction_source","evidence":[{"pointer":"/dynamic_instruction_source"}],"value":"observer"},{"id":"physics_hz","value":120}],"inputs":[{"kind":"vision","source_ref":"manifest.json#dynamic_instruction_source"}],"sources":[]}});
    f.app
        .db
        .execute(
            "INSERT INTO rust_annotations VALUES('research_metadata',?,?,0)",
            rusqlite::params![
                crate::research_metadata::snapshot_key(&target).unwrap(),
                snapshot.to_string()
            ],
        )
        .unwrap();
    let mut e = json!({"kind":"dataset","loom_registry":{"id":"old","revision":1}});
    crate::research_metadata::attach_materialized(&f.app, &mut e).unwrap();
    assert_eq!(e["research_summary"]["facts"].as_array().unwrap().len(), 1);
    assert_eq!(e["research_summary"]["facts"][0]["id"], "physics_hz");
    assert!(e["research_summary"]["inputs"]
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn twelve_existing_jobs_remain_individually_pageable_and_new_graphs_are_refused() {
    let mut f = Fixture::new();
    let units:Vec<_>=(0..12).map(|i|json!({"id":format!("j{i}"),"label":format!("Run {i}"),
        "spec":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false},"execution_mode":if i<2 {"full"}else{"pilot"}}})).collect();
    let raw = json!({"key":"prior-series","label":"Research series","owner":"owner","notify":"owner","nodes":units,"outputs":{}});
    assert_eq!(
        f.call("jobs", "submit_workflow", raw.clone())
            .unwrap_err()
            .code,
        "separate_jobs"
    );
    let old = f.legacy_workflow(raw.clone()).unwrap();
    assert_eq!(f.call("jobs", "submit_workflow", raw).unwrap(), old);
    let mut ids = std::collections::BTreeSet::new();
    for offset in [0, 5, 10] {
        let page = f
            .call("jobs", "list", json!({"limit":5,"offset":offset}))
            .unwrap();
        for job in page.as_array().unwrap() {
            assert!(ids.insert(job["id"].as_str().unwrap().to_owned()));
        }
    }
    assert_eq!(ids.len(), 12);
    assert!(!ids.contains(old["id"].as_str().unwrap()));
    let pilots = f
        .call("jobs", "list", json!({"execution_mode":"pilot"}))
        .unwrap();
    assert_eq!(pilots.as_array().unwrap().len(), 10);
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let mut cursor = Value::Null;
    let mut page_ids = std::collections::BTreeSet::new();
    loop {
        let mut args =
            json!({"kinds":["job"],"role":"all","view":"all","limit":5,"records_only":true});
        if !cursor.is_null() {
            args["cursor"] = cursor;
        }
        let page = f.call("system", "dashboard_page", args).unwrap();
        for job in page["entities"].as_array().unwrap() {
            assert!(page_ids.insert(job["id"].as_str().unwrap().to_owned()));
        }
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(page_ids, ids);
    let occupancy = f.app.dashboard_occupancy(&f.actor).unwrap();
    assert_eq!(occupancy["entities"].as_array().unwrap().len(), 12);
    assert!(occupancy["queue"]["logical_entries"]
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn direct_pilot_and_output_belong_to_the_execution_and_reservation_binds_just_one_job() {
    let mut f = Fixture::new();
    let mut raw = json!({"key":"preflight","owner":"owner","notify":"owner","kind":"cpu.smoke","params":{"seconds":0,"fail":false},
        "execution_mode":"pilot","outcome":{"goal":"Validate publication","deliverables":[{"alias":"receipt","kind":"artifact","entity":"observation"}]}});
    assert_eq!(
        f.call("jobs", "submit", raw.clone()).unwrap_err().code,
        "pilot_scope"
    );
    raw["pilot"] = json!({"preflight_for":"full collection","reduced_scope":"one episode"});
    let mut invalid = raw.clone();
    invalid["outcome"]["deliverables"][0]["entity"] = json!("absent");
    assert_eq!(
        f.call("jobs", "submit", invalid).unwrap_err().code,
        "deliverable"
    );
    let job = f.call("jobs", "submit", raw.clone()).unwrap();
    assert_eq!(job, f.call("jobs", "submit", raw.clone()).unwrap());
    let detail = f.call("jobs", "get", job.clone()).unwrap();
    assert_eq!(detail["spec"]["outcome"], raw["outcome"]);
    assert_eq!(f.count("job_workflows"), 0);
    raw["pilot"]["reduced_scope"] = json!("two episodes");
    assert_eq!(
        f.call("jobs", "submit", raw).unwrap_err().code,
        "idempotency_conflict"
    );
    let open = json!({"key":"reserve","owner":"owner","notify":"owner","outcome":{"goal":"Check the pending Runtime",
        "deliverables":[{"alias":"receipt","kind":"artifact","entity":"observation"}]},"plan":{"sealed":false,"run":{"label":"Check Runtime"}}});
    let mut multiple = open.clone();
    multiple["plan"] =
        json!({"sealed":false,"units":[{"id":"a","label":"A"},{"id":"b","label":"B"}]});
    multiple["outcome"] = json!({"goal":"Multiple runs"});
    assert_eq!(
        f.call("jobs", "create", multiple).unwrap_err().code,
        "separate_jobs"
    );
    let reserved = f.call("jobs", "create", open.clone()).unwrap();
    assert_eq!(reserved, f.call("jobs", "create", open).unwrap());
    assert!(reserved["execution_job"].is_null());
    let bind = json!({"id":reserved["id"],"key":"bind","expected_revision":1,"reason":"Ready","plan":
        {"sealed":true,"run":{"label":"Check Runtime","execution":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false}}}}});
    let receipt = f.call("jobs", "revise_plan", bind.clone()).unwrap();
    assert!(receipt["execution_job"].is_string());
    assert_eq!(receipt, f.call("jobs", "revise_plan", bind).unwrap());
    assert_eq!(f.count("jobs"), 2);
}
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
    // Reconstruct pre-upgrade fixtures for compatibility reads, hold, recovery and callbacks.
    fn legacy_workflow(&mut self, raw: Value) -> Result<Value> {
        let request: contracts::Workflow = serde_json::from_value(raw.clone())?;
        let root = uuid::Uuid::new_v4().simple().to_string();
        let config = self.app.config.clone();
        let bulk = self.app.bulk.clone();
        let tx = self.app.transaction()?;
        tx.execute("INSERT INTO job_workflows(id,submitter,key,request,owner,notify,created,state) VALUES(?,?,?,?,?,?,?,'queued')",
            rusqlite::params![root,self.actor.id,request.key,crate::compat::dump(&raw),request.owner,request.notify,store::now()])?;
        let mut jobs = serde_json::Map::new();
        for unit in &request.nodes {
            let job = crate::jobs::admit_node(
                &tx,
                (&config, &bulk),
                &self.actor,
                (&root, None),
                &request,
                unit,
                &jobs,
            )?;
            jobs.insert(unit.id.clone(), json!(job));
        }
        tx.commit()?;
        Ok(json!({"id":root,"nodes":jobs}))
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
    f.app
        .db
        .execute("UPDATE node_control SET mode='draining' WHERE id=1", [])
        .unwrap();
    assert_eq!(f.submit("one"), id);
    assert_eq!(f.call("jobs","submit",json!({"key":"new-drained","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}})).unwrap_err().code, "admission_fenced");
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
    assert_eq!(first, f.call("recovery", "resume", resume.clone()).unwrap());
    f.app
        .db
        .execute("UPDATE node_control SET mode='draining' WHERE id=1", [])
        .unwrap();
    let revisions = f.count("job_revisions");
    assert_eq!(first, f.call("recovery", "resume", resume.clone()).unwrap());
    assert_eq!(f.count("job_revisions"), revisions);
    let mut changed = resume;
    changed["reason"] = json!("A different request");
    assert_eq!(
        f.call("recovery", "resume", changed).unwrap_err().code,
        "idempotency_conflict"
    );
    assert_eq!(
        f.call("recovery", "claim", json!({"id":id}))
            .unwrap_err()
            .code,
        "admission_fenced"
    );
    f.app
        .db
        .execute("UPDATE node_control SET mode='active' WHERE id=1", [])
        .unwrap();
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
fn preparation_family_preserves_workflow_states_pins_and_execution_membership() {
    let mut f = Fixture::new();
    let spec = json!({"kind":"cpu.smoke","params":{"seconds":0,"fail":false}});
    let main=f.legacy_workflow(json!({"key":"preparation-root","label":"Requested preparation","owner":"owner","notify":"owner","nodes":[{"id":"pilot","label":"Pilot","spec":spec}],"outputs":{},"presentation":{"scope":"operations","role":"test"}})).unwrap()["id"].clone();
    let repair=f.legacy_workflow(json!({"key":"preparation-repair","label":"Render repair","owner":"owner","notify":"owner","nodes":[{"id":"render","label":"Render","spec":spec}],"outputs":{}})).unwrap()["id"].clone();
    let original = f.call("jobs", "get", json!({"id":repair})).unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let native = json!({"kind":"job","identity":{"id":main}});
    let main_item = f
        .call("results", "resolve_item", json!({"native_ref":native}))
        .unwrap();
    // Reproduce the old routing bug: treating a workflow as an absent physical Job.
    crate::items::adopt(&f.app.db, "jobs", &json!({"id":main})).unwrap();
    assert!(f
        .call("results", "resolve_item", json!({"native_ref":native}))
        .is_err());
    let node = original["executions"][0]["job"].clone();
    let membership = f.call("jobs", "get", json!({"id":node})).unwrap();
    assert_eq!(
        membership["execution_membership"],
        json!({"root":repair,"node":"render"})
    );
    let independent = f.submit("independent-followup");
    f.call("jobs","curate",json!({"key":"independent-parent","reason":"Same outcome, independent execution","dry_run":false,"changes":[{"kind":"job","id":independent,"presentation":{"parent_job":main,"role":"support"}}]})).unwrap();
    let standalone = f.call("jobs", "get", json!({"id":independent})).unwrap();
    assert!(standalone.get("execution_membership").unwrap().is_null());
    assert_eq!(standalone["presentation"]["parent_job"], main);
    let changes = json!([{"kind":"job","id":main,"presentation":{"scope":"operations","role":"main"}},{"kind":"job","id":repair,"presentation":{"scope":"operations","role":"support","parent_job":main}}]);
    let args = json!({"key":"link-preparation","reason":"Same requested pipeline","dry_run":false,"changes":changes});
    let receipt = f.call("jobs", "curate", args.clone()).unwrap();
    assert_eq!(receipt, f.call("jobs", "curate", args).unwrap());
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    assert_eq!(
        main_item,
        f.call("results", "resolve_item", json!({"native_ref":native}))
            .unwrap()
    );
    let page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["job"],"view":"all","role":"all","limit":100}),
        )
        .unwrap();
    assert!(!page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == main));
    assert!(!page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == repair));
    let detail = f.call("jobs", "get", json!({"id":repair})).unwrap();
    assert_eq!(detail["state"], original["state"]);
    assert_eq!(detail["spec"], original["spec"]);
    assert_eq!(detail["executions"], original["executions"]);
    assert_eq!(detail["presentation"]["parent_job"], main);
    assert_eq!(detail["family"]["root"], main);
    assert_eq!(detail["family"]["total"], 5);
    assert!(detail["family"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == node && m["parent"] == repair && m["relation"] == "execution_node"));
    // A partial edit keeps the prior parent, and cycles use the merged graph.
    f.call("jobs","curate",json!({"key":"label-preparation","reason":"Display only","dry_run":false,"changes":[{"kind":"job","id":repair,"presentation":{"label":"Corrected render"}}]})).unwrap();
    assert_eq!(
        f.call("jobs", "get", json!({"id":node})).unwrap()["family"]["root"],
        main
    );
    assert!(f.call("jobs","curate",json!({"key":"cycle-after-label","reason":"Invalid","dry_run":false,"changes":[{"kind":"job","id":main,"presentation":{"parent_job":repair}}]})).is_err());
    assert!(f.call("jobs","curate",json!({"key":"reparent-execution","reason":"Invalid","dry_run":false,"changes":[{"kind":"job","id":node,"presentation":{"parent_job":main}}]})).is_err());
    f.call("jobs","curate",json!({"key":"clear-preparation","reason":"Restore separate display","dry_run":false,"changes":[{"kind":"job","id":repair,"presentation":{"parent_job":null}}]})).unwrap();
    assert_eq!(
        f.call("jobs", "get", json!({"id":node})).unwrap()["family"]["root"],
        repair
    );
    assert_eq!(f.count("jobs"), 3);
    assert_eq!(f.count("attempts"), 0);
}

#[test]
fn runtime_usage_uses_frozen_stage_receipts_even_for_failed_operational_jobs() {
    let mut f = Fixture::new();
    let main = f.submit("runtime-main");
    let child = f.submit("runtime-prerequisite");
    let manifest1 = json!({"runtime":{"image":"image-one"},"dependencies":[]});
    let manifest2 = json!({"runtime":{"image":"image-two"},"dependencies":[]});
    let sha1 = hash(compat::seal(&manifest1));
    let sha2 = hash(compat::seal(&manifest2));
    let pin = json!({"id":"sim","revision":1,"sha256":sha1});
    let mut spec: Value = serde_json::from_str(
        &f.app
            .db
            .query_row("SELECT spec FROM jobs WHERE id=?", [&child], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
    )
    .unwrap();
    spec["profile"] = json!({"sha256":"old-profile","prepare":{"runtime_pin":pin},"runtime":{"runtime_pin":{"id":"sim","revision":2,"sha256":sha2}}});
    f.app
        .db
        .execute(
            "UPDATE jobs SET spec=?,state='failed' WHERE id=?",
            rusqlite::params![spec.to_string(), child],
        )
        .unwrap();
    f.call("jobs","curate",json!({"key":"runtime-history","reason":"User preparation","dry_run":false,"changes":[{"kind":"job","id":child,"presentation":{"parent_job":main}}]})).unwrap();
    let observation = json!({"runtimes":[{"id":"sim","revision":1,"sha256":sha1,"manifest":manifest1},{"id":"sim","revision":2,"sha256":sha2,"manifest":manifest2}]});
    f.app.observe_runtimes(observation).unwrap();
    let catalog = f.app.runtime_inventory().unwrap();
    assert_eq!(
        catalog["runtimes"][0]["job_usage"][0]["profile_sha256"],
        "old-profile"
    );
    assert_eq!(catalog["runtimes"][0]["job_usage"][0]["group_root"], main);
    assert_eq!(catalog["runtimes"][0]["job_usage"][0]["state"], "failed");
    assert_eq!(catalog["runtimes"][0]["executed_jobs"], json!([]));
    // A prepare-only execution must not claim the different run Runtime executed.
    f.app.db.execute("INSERT INTO stage_runs(job,attempt,stage,state,started) VALUES(?,'attempt-proof','prepare','failed',1)",[&child]).unwrap();
    let catalog = f.call("components", "runtimes", json!({})).unwrap();
    assert_eq!(catalog["runtimes"][0]["executed_jobs"], json!([child]));
    assert_eq!(catalog["runtimes"][0]["started_job_count"], 1);
    assert_eq!(
        catalog["runtimes"][0]["job_usage"][0]["execution_owner"],
        child
    );
    assert_eq!(catalog["runtimes"][1]["executed_jobs"], json!([]));
    assert_eq!(
        catalog["runtimes"][0]["job_usage"][0]["execution_receipts"][0]["attempt"],
        "attempt-proof"
    );
    let detail = f.call("jobs", "get", json!({"id":child})).unwrap();
    assert_eq!(detail["family"]["root"], main);
    assert_eq!(detail["spec"]["profile"], spec["profile"]);
    // After recovery changes prepare to revision two, the original start
    // still belongs exclusively to revision one.
    f.app
        .db
        .execute(
            "UPDATE job_revisions SET spec=?,created=0 WHERE job=? AND revision=0",
            rusqlite::params![spec.to_string(), child],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "INSERT INTO attempts VALUES('attempt-proof',?,'failed',1,2)",
            [&child],
        )
        .unwrap();
    spec["profile"]["prepare"] = spec["profile"]["runtime"].clone();
    spec["profile"]["sha256"] = json!("replacement-profile");
    f.app
        .db
        .execute(
            "UPDATE jobs SET spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), child],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "INSERT INTO job_revisions VALUES(?,1,?,'recovery',3)",
            rusqlite::params![child, spec.to_string()],
        )
        .unwrap();
    let catalog = f.app.runtime_inventory().unwrap();
    assert_eq!(catalog["runtimes"][1]["executed_jobs"], json!([]));
    assert_eq!(
        catalog["runtimes"][0]["job_usage"][0]["execution_receipts"][0]["profile_sha256"],
        "old-profile"
    );
}
#[test]
fn group_summary_tracks_later_nested_execution_without_rewriting_the_completed_root() {
    let mut f = Fixture::new();
    let main = f.submit("group-original");
    let workflow = f.legacy_workflow(json!({"key":"group-followup","label":"Later repair","owner":"owner","notify":"owner","nodes":[{"id":"one","label":"Later execution","spec":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false}}}],"outputs":{}})).unwrap()["id"].as_str().unwrap().to_string();
    let child: String = f
        .app
        .db
        .query_row(
            "SELECT job FROM workflow_nodes WHERE root=?",
            [&workflow],
            |r| r.get(0),
        )
        .unwrap();
    f.call("jobs","curate",json!({"key":"group-link","reason":"Same outcome","dry_run":false,"changes":[{"kind":"job","id":workflow,"presentation":{"parent_job":main}}]})).unwrap();
    let clock = store::now() - 1000.;
    for (id, start, end) in [
        (&main, clock - 86400., clock - 86390.),
        (&child, clock, clock + 20.),
    ] {
        f.app
            .db
            .execute("UPDATE jobs SET state='complete' WHERE id=?", [id])
            .unwrap();
        f.app
            .db
            .execute(
                "INSERT INTO stage_runs VALUES(?,'receipt','run',?,?,'returned')",
                rusqlite::params![id, start, end],
            )
            .unwrap();
    }
    let before = f.app.job_detail(&main).unwrap();
    let group = f.call("jobs", "get", json!({"id":main})).unwrap()["family"]["summary"].clone();
    assert_eq!(group["execution_count"], 2); // The workflow wrapper adds no Job/time.
    assert_eq!(group["member_count"], 3);
    assert_eq!(group["latest_job"]["id"], child);
    assert_eq!(group["first_started"], clock - 86400.);
    assert_eq!(group["last_ended"], clock + 20.);
    assert_eq!(group["states"]["complete"], 2);
    f.app.index_once().unwrap();
    let page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["job"],"view":"all","role":"all","limit":50}),
        )
        .unwrap();
    let row = page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == main)
        .unwrap();
    assert_eq!(row["group_summary"], group);
    assert_eq!(row["state"], "complete");
    f.app
        .db
        .execute("UPDATE jobs SET state='running' WHERE id=?", [&child])
        .unwrap();
    f.app
        .db
        .execute(
            "UPDATE stage_runs SET ended=NULL,state='running' WHERE job=?",
            [&child],
        )
        .unwrap();
    let running = f.app.job_group_summary(&main).unwrap();
    assert_eq!(running["active_state"], "running");
    assert!(running["last_ended"].is_null());
    let after = f.app.job_detail(&main).unwrap();
    for field in [
        "state",
        "spec",
        "created",
        "attempts",
        "stages",
        "execution_intervals",
    ] {
        assert_eq!(
            after[field], before[field],
            "Original root field changed: {field}"
        );
    }
    // Unknown exits remain visible rather than silently losing their start.
    f.app
        .db
        .execute("UPDATE jobs SET state='failed' WHERE id=?", [&child])
        .unwrap();
    let failed = f.app.job_group_summary(&main).unwrap();
    assert_eq!(failed["states"]["failed"], 1);
    assert_eq!(failed["unknown_time"], true);
    assert_eq!(f.count("attempts"), 0);
}

#[test]
fn activity_counts_standalone_jobs_across_kst_midnight_despite_presentation_links() {
    let mut f = Fixture::new();
    let main = f.submit("top-level");
    let child = f.submit("child");
    let grandchild = f.submit("grandchild");
    f.call("jobs","curate",json!({"key":"nested-activity","reason":"Same outcome","dry_run":false,"changes":[{"kind":"job","id":child,"presentation":{"parent_job":main}},{"kind":"job","id":grandchild,"presentation":{"parent_job":child}}]})).unwrap();
    let midnight = (((store::now() + 32400.) / 86400.).floor() - 1.) * 86400. - 32400.;
    for (id, attempt, start, end) in [
        (&main, "a", midnight - 20., midnight - 5.),
        (&child, "b", midnight - 10., midnight + 10.),
        (&grandchild, "c", midnight + 5., midnight + 20.),
        (&grandchild, "d", midnight + 30., midnight + 40.),
    ] {
        f.app
            .db
            .execute(
                "INSERT INTO stage_runs VALUES(?,?,'run',?,?,'returned')",
                rusqlite::params![id, attempt, start, end],
            )
            .unwrap();
    }
    let history = f
        .call(
            "jobs",
            "preview_history",
            json!({"ids":[main,child,grandchild]}),
        )
        .unwrap();
    assert_eq!(history["changes"], json!([]));
    assert_eq!(history["records"][1]["authority"], "independent");
    assert_eq!(history["records"][1]["presentation_parent"], main);
    assert_eq!(history["records"][1]["review_required"], true);
    let result = f.app.dashboard_activity("all").unwrap();
    let days = result["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert!(days.iter().all(|day| day["job_count"] == 2));
    assert_eq!(days[0]["milliseconds"], 25000.);
    assert_eq!(days[1]["milliseconds"], 35000.);
}

#[test]
fn activity_counts_scheduled_jobs_and_unions_overlapping_intervals() {
    let mut f = Fixture::new();
    let plan=f.legacy_workflow(json!({"key":"activity-preparation-main","label":"Preparation",
        "owner":"owner","notify":"owner",
        "nodes":[{"id":"prepare","label":"Prepare","spec":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false}}},
                 {"id":"repair","label":"Repair","spec":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false}}}],
        "outputs":{},"presentation":{"scope":"operations","role":"support"}})).unwrap();
    let prepare = plan["nodes"]["prepare"].as_str().unwrap();
    let child = plan["nodes"]["repair"].as_str().unwrap();
    let day = ((store::now() + 32400.) / 86400.).floor() - 1.;
    let start = day * 86400. - 32400. + 43200.;
    for (id, a, b) in [
        (prepare, start, start + 20.),
        (child, start + 10., start + 40.),
    ] {
        f.app
            .db
            .execute(
                "INSERT INTO stage_runs VALUES(?,'proof','run',?,?,'returned')",
                rusqlite::params![id, a, b],
            )
            .unwrap();
    }
    f.app
        .db
        .execute("UPDATE jobs SET state='failed' WHERE id=?", [&child])
        .unwrap();
    let all = f.app.dashboard_activity("all").unwrap();
    assert_eq!(all["days"].as_array().unwrap().len(), 1);
    assert_eq!(all["days"][0]["job_count"], 2);
    for id in [prepare, child] {
        assert!(all["days"][0]["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|j| j["id"] == id));
    }
    assert_eq!(all["days"][0]["milliseconds"], 50000.);
    assert_eq!(
        f.call("jobs", "get", json!({"id":child})).unwrap()["state"],
        "failed"
    );
    assert_eq!(
        f.app.dashboard_activity("research").unwrap()["days"],
        json!([])
    );
}
#[test]
fn activity_unions_reviewed_units_with_their_job_and_counts_standalone_work_once() {
    let mut f = Fixture::new();
    let job = f.submit("reviewed-research");
    let unit = f.submit("reviewed-preparation");
    let standalone = f.submit("reviewed-standalone");
    let imported = "preserved-preparation";
    let start = ((store::now() + 32400.) / 86400.).floor() * 86400. - 32400. - 43200.;
    for (id, parent) in [
        (&*unit, Some(&*job)),
        (imported, Some(&*job)),
        (&*standalone, None),
    ] {
        f.app
            .db
            .execute(
                "INSERT INTO reviewed_work VALUES(?,'unit',?,?,'Verified historical relationship')",
                rusqlite::params![id, parent, parent.is_none()],
            )
            .unwrap();
    }
    for (id, offset, end) in [(&job, 10., 30.), (&unit, 0., 20.), (&standalone, 0., 5.)] {
        f.app
            .db
            .execute(
                "INSERT INTO stage_runs VALUES(?,'proof','run',?,?,'returned')",
                rusqlite::params![id, start + offset, start + end],
            )
            .unwrap();
    }
    let preserved = json!({"entity":{"execution_intervals":[{"start":start+25.,"end":start+40.}]}});
    f.app
        .db
        .execute(
            "INSERT INTO history_records VALUES('queue','job_view',?,'1','snapshot',?)",
            rusqlite::params![imported, preserved.to_string()],
        )
        .unwrap();
    let activity = f.app.dashboard_activity("all").unwrap();
    let day = &activity["days"][0];
    assert_eq!(day["job_count"], 2);
    assert_eq!(day["milliseconds"], 45000.);
    let ids: std::collections::BTreeSet<_> = day["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        [job.as_str(), standalone.as_str()].into_iter().collect()
    );
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
    let w=f.legacy_workflow(json!({"key":"hold-plan","label":"Outcome","owner":"owner","notify":"owner","nodes":[{"id":"a","label":"First","spec":spec},{"id":"b","label":"Second","depends_on":["a"],"spec":spec}],"outputs":{"out":{"node":"b","entity":"observation"}}})).unwrap();
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
    let first = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(first.job, w["nodes"]["a"]);
    f.call("jobs", "hold", json!({"id":w["id"]})).unwrap();
    fs::create_dir_all(&first.folder).unwrap();
    fs::write(first.folder.join("manifest.txt"), b"verified batch").unwrap();
    f.app
        .stage_returned(&first, 0, json!({"returned":true}))
        .unwrap();
    let publication = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert!(publication.publication);
    f.app.publish_work(&publication, json!({"entities":[{"id":"observation","kind":"artifact","path":"manifest.txt"}],"metrics":[]})).unwrap();
    assert!(f.app.next_work(&Default::default()).unwrap().is_none());
    f.call("jobs", "resume", json!({"id":w["id"]})).unwrap();
    let second = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(second.job, w["nodes"]["b"]);
    let completed = f.call("jobs", "get", json!({"id":first.job})).unwrap();
    assert_eq!(completed["state"], "complete");
    assert_eq!(completed["attempts"].as_array().unwrap().len(), 1);
}
#[test]
fn workflow_held_during_cpu_preparation_waits_before_gpu_run() {
    let mut f = Fixture::new();
    let profile = json!({"schema":"loom-execution/v2","revision":"staged.r1","sha256":"b".repeat(64),
        "params_schema":{"type":"object"},"prepare":{"runtime_id":"prepare","lane":"cpu","depends_on":[],"inputs":[],"command":["/bin/true"]},
        "runtime":{"executor":"container","runtime_id":"train","lane":"gpu","command":["/bin/true"]},
        "inputs":{},"outputs":null,"limits":{"max_attempts":1,"timeout_seconds":30}});
    f.app.config.profiles.insert("staged.r1".into(), profile);
    let root = f.legacy_workflow(json!({"key":"stage-hold","label":"Generation","owner":"owner","notify":"owner",
        "nodes":[{"id":"batch","label":"Batch","spec":{"kind":"staged.r1","params":{}}}],"outputs":{}})).unwrap();
    let prepare = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(prepare.stage, "prepare");
    let queue = f.call("jobs", "hold", json!({"id":root["id"]})).unwrap();
    let member = queue["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == prepare.job)
        .unwrap();
    assert_eq!(
        (&member["stage"], &member["lane"]),
        (&json!("prepare"), &json!("cpu"))
    );
    assert_eq!(member["can_resume"], true);
    assert_eq!(member["can_reorder"], false);
    f.app
        .stage_returned(&prepare, 0, json!({"returned":true}))
        .unwrap();
    let mut reopened = Application::open(
        &f.app.local,
        &f.app.bulk,
        &f.app.components,
        f.app.config.clone(),
    )
    .unwrap();
    assert!(reopened.next_work(&Default::default()).unwrap().is_none());
    reopened
        .execute(&f.actor, "jobs", "resume", json!({"id":root["id"]}))
        .unwrap();
    let run = reopened.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(run.stage, "run");
    assert_eq!(run.attempt, prepare.attempt);

    let downstream = reopened
        .execute(
            &f.actor,
            "jobs",
            "submit",
            json!({
                "key":"overlapped", "kind":"staged.r1", "owner":"owner", "notify":"owner",
                "params":{}, "depends_on":[run.job]
            }),
        )
        .unwrap();
    let in_flight = [run.effect.clone()].into_iter().collect();
    let next_prepare = reopened.next_work(&in_flight).unwrap().unwrap();
    assert_eq!(next_prepare.job, downstream["id"]);
    assert_eq!(next_prepare.stage, "prepare");
    reopened
        .stage_returned(&next_prepare, 0, json!({"returned": true}))
        .unwrap();
    assert!(reopened.next_work(&in_flight).unwrap().is_none());
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
    let root=f.legacy_workflow(json!({"key":"failed-plan","label":"One outcome","owner":"owner","notify":"owner","nodes":[{"id":"a","label":"A","spec":spec},{"id":"b","label":"B","spec":spec}],"outputs":{"out":{"node":"b","entity":"observation"}}})).unwrap();
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
    let root=f.legacy_workflow(json!({"key":"move-plan","label":"One outcome","owner":"owner","notify":"owner","nodes":[{"id":"a","label":"A","spec":spec},{"id":"b","label":"B","depends_on":["a"],"spec":spec}],"outputs":{"out":{"node":"b","entity":"observation"}}})).unwrap();
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
    assert_eq!(queue["logical_entries"][0]["reason"], "ready");
    f.call("jobs", "hold", json!({"id":root["id"]})).unwrap();
    assert_eq!(
        f.app.queue().unwrap()["logical_entries"][0]["reason"],
        "held"
    );
    f.call("jobs", "resume", json!({"id":root["id"]})).unwrap();
    let work = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(
        f.app.queue().unwrap()["logical_entries"][0]["reason"],
        "executing"
    );
    f.app
        .stage_returned(&work, 0, json!({"returned":true}))
        .unwrap();
    assert_eq!(
        f.app.queue().unwrap()["logical_entries"][0]["reason"],
        "finalizing"
    );
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
    let note = json!({"id":id,"annotation_id":"drained-note","reason":"Late note","evidence":{}});
    f.app
        .db
        .execute("UPDATE node_control SET mode='draining' WHERE id=1", [])
        .unwrap();
    assert_eq!(
        f.call("results", "annotate", note).unwrap_err().code,
        "admission_fenced"
    );
    assert_eq!(f.count("job_annotations"), 1);
    f.app
        .db
        .execute("UPDATE node_control SET mode='active' WHERE id=1", [])
        .unwrap();
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
    let diagnostics = f.app.local.join("diagnostics");
    fs::create_dir(&diagnostics).unwrap();
    let pipe = diagnostics.join("rust-incidents.jsonl");
    let name = std::ffi::CString::new(pipe.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(f
        .app
        .incident("jobs.submit", &Error::input("private"))
        .is_none());
    fs::remove_file(pipe).unwrap();
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
    // Both copies were verified. A later change to the first copy must not
    // conceal the still-readable exact copy from content search.
    for _ in 0..4 {
        f.app.index_once().unwrap();
    }
    fs::write(f.app.bulk.join("receipt.txt"), b"changed first copy").unwrap();
    for _ in 0..4 {
        f.app.index_once().unwrap();
    }
    assert_eq!(
        f.call("results", "read_file", json!({"file_item_id":file}))
            .unwrap()["data_base64"],
        read["data_base64"]
    );
    let readable = f
        .call(
            "results",
            "search_items",
            json!({"text":"A recorded file","kinds":["file"]}),
        )
        .unwrap();
    assert_eq!(
        readable["items"].as_array().unwrap().len(),
        1,
        "Content search must try another exact verified copy"
    );
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
    let secondary = f.root.join("secondary");
    let dependencies = f.root.join("dependencies");
    fs::create_dir_all(&secondary).unwrap();
    fs::create_dir_all(&dependencies).unwrap();
    f.app.config.data_stores.insert(
        "data/1".into(),
        json!({"path":secondary,"storage_id":"secondary-data","mode":"rw"}),
    );
    f.app
        .config
        .extra
        .insert("dependency_directory".into(), json!(dependencies));
    for key in ["index_directory", "manifest_directory"] {
        for path in [&secondary, &dependencies, &f.app.bulk, &f.app.components] {
            f.app
                .config
                .extra
                .insert("catalog".into(), json!({key:path}));
            assert_eq!(
                crate::index_store::IndexWriter::open(&f.app)
                    .err()
                    .expect("Derived state must not own research or dependency roots")
                    .code,
                "catalog_layout"
            );
        }
    }

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
fn standalone_profile_detail_retains_exact_identity_and_task_context() {
    let mut f = Fixture::new();
    let profile = json!({"revision":"owner.runner.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"command":["/bin/true"],"lane":"cpu"},"inputs":{},"outputs":null,"limits":{"timeout_seconds":30,"max_attempts":1}});
    let tx = f.app.transaction().unwrap();
    tx.execute(
        "INSERT INTO execution_profiles VALUES('owner','runner',1,?)",
        [profile.to_string()],
    )
    .unwrap();
    tx.commit().unwrap();
    let reference =
        json!({"kind":"profile","identity":{"owner":"owner","id":"runner","revision":1}});
    let item = f
        .call("results", "resolve_item", json!({"native_ref":reference}))
        .unwrap();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"profile-task","definition":{"label":"Reusable profile context"}}),
        )
        .unwrap();
    f.call("results","link_scenario",json!({"key":"profile-context","scenario_ref":{"id":task["id"],"revision":1},"target":reference,"relation":"implements","provenance":{"reason":"Exact standalone profile"}})).unwrap();
    let display = f
        .call(
            "system",
            "dashboard_entity",
            json!({"item_id":item["item_id"],"revision_id":item["revision_id"]}),
        )
        .unwrap();
    let entity = display["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["loom_execution_profile"]["kind"] == "owner.runner.r1")
        .expect("Exact standalone Profile must render");
    assert_eq!(
        entity["loom_execution_profile"]["sha256"],
        profile["sha256"]
    );
    assert_eq!(entity["task_refs"][0]["id"], task["id"]);
}

#[test]
fn relation_pages_preserve_all_edges_without_allocating_child_items() {
    let mut f = Fixture::new();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"relations-task","definition":{"label":"Relation context"}}),
        )
        .unwrap();
    let root=f.call("jobs","submit",json!({"key":"relations-bound","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"scenario_ref":{"id":task["id"],"revision":1}})).unwrap()["id"].as_str().unwrap().to_string();
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
    let boundary = f
        .call(
            "results",
            "item_relations",
            json!({"item_id":item["item_id"],"limit":1}),
        )
        .unwrap();
    assert_eq!(boundary["items"][0]["relation"], "scenario");
    assert!(boundary["next_cursor"].is_string());
    let incoming = f
        .call(
            "results",
            "item_relations",
            json!({"item_id":item["item_id"],"limit":1,"cursor":boundary["next_cursor"]}),
        )
        .unwrap();
    assert_eq!(incoming["items"][0]["relation"], "step");
    assert_eq!(
        incoming["items"][0]["execution_membership"],
        json!({"root": root, "node": "step-000"})
    );
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
    assert_eq!(edges.len(), 206);
    let unique: std::collections::BTreeSet<_> = edges
        .iter()
        .map(|v| v["native_ref"]["identity"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(unique.len(), 206);
    assert_eq!(f.count("catalog_items"), before);
    assert_eq!(f.count("jobs"), 1);
}

#[test]
fn reconciliation_import_replays_without_new_jobs_and_rejects_mutated_source_evidence() {
    let mut f = Fixture::new();
    fs::write(f.app.bulk.join("weights.json"), b"{\"weight\":1}").unwrap();
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"import-model-task","definition":{"label":"Selected model evaluation"}}),
        )
        .unwrap();
    let source = json!({"schema":"loom-reconcile/v1","operations":[{"operation":"import","schema":"loom-asset/v2",
        "source":{"system":"preserved-fixture","id":"training-data","revision":"r1"},"definition":{"kind":"dataset","label":"Training data",
        "scenario_ref":{"id":task["id"],"revision":1},"metadata":{"split_clips":[{"id":"train","count":700},{"id":"validation","count":150},{"id":"test","count":150}]},
        "primary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"weights.json"}}}]});
    let source = f
        .call(
            "results",
            "reconcile_plan",
            json!({"key":"source-data","proposal":source}),
        )
        .unwrap();
    let source_args = json!({"plan_id":source["plan_id"]});
    f.call("results", "reconcile_validate", source_args.clone())
        .unwrap();
    let source = f.call("results", "reconcile_apply", source_args).unwrap();
    let dataset = &source["receipt"]["operations"][0]["id"];
    let proposal = json!({"schema":"loom-reconcile/v1","operations":[{"operation":"import","schema":"loom-asset/v1","source":{"system":"preserved-fixture","id":"selected-model","revision":"r1"},"type":"model","scenario_ref":{"id":task["id"],"revision":1},"name":"Selected model","properties":{"declared_steps":20000},"relations":[{"kind":"trained_on","id":dataset,"revision":1}],"primary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"weights.json"}}]});
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
    assert_eq!(f.count("asset_revisions"), 2);
    let listed = f.call("results", "list", json!({"view":"all"})).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2);
    let id = &applied["receipt"]["operations"][0]["id"];
    let native = f
        .call("results", "get", json!({"id":id,"revision":1}))
        .unwrap();
    assert_eq!(native["kind"], "model");
    assert!(native["job"].is_null());
    let catalog = f.app.dashboard_catalog().unwrap();
    let policy = catalog["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["loom_registry"]["id"] == *id)
        .unwrap();
    let input = &policy["catalog_inputs"]["datasets"][0];
    assert_eq!(input["ref"]["identity"], json!({"id":dataset,"revision":1}));
    assert_eq!(
        input["split"],
        json!({"train":700,"validation":150,"test":150})
    );
    f.call(
        "jobs",
        "curate",
        json!({"key":"split-source-label","reason":"Match the dataset variant","dry_run":false,
        "changes":[{"kind":"entity","id":dataset,"presentation":{"label":"D1"}}]}),
    )
    .unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let pin = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":input["ref"]}),
        )
        .unwrap();
    let policy_page = f
        .call(
            "system",
            "dashboard_page",
            json!({"kinds":["model"],"view":"all","role":"all","records_only":true}),
        )
        .unwrap();
    let curated_input = &policy_page["entities"][0]["catalog_inputs"]["datasets"][0];
    assert_eq!(curated_input["label"], "D1");
    assert_eq!(curated_input["item_id"], pin["item_id"]);
    assert_eq!(curated_input["revision_id"], pin["revision_id"]);
    assert_eq!(curated_input["split"], input["split"]);
    assert_eq!(f.call("results", "reconcile_apply", args).unwrap(), applied);
    assert_eq!(f.count("asset_revisions"), 2);
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
    // Additive relocation remains usable for every supported input identity.
    let old_ref = f
        .app
        .db
        .query_row(
            "SELECT ref FROM entity_locations WHERE id=?",
            [&native],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    let new_ref = "relocated-primary.txt";
    fs::rename(f.app.bulk.join(&old_ref), f.app.bulk.join(new_ref)).unwrap();
    f.app.db.execute("INSERT INTO entity_locations(id,revision,component,machine_id,storage_id,ref,status) VALUES(?,1,'f0000.txt','fixture','fixture-data',?,'available')", rusqlite::params![native,new_ref]).unwrap();
    let profile = json!({"schema":"loom-execution/v2","revision":"copy.consumer.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"executor":"container","runtime_id":"cpu","lane":"cpu","command":["/bin/true"]},"inputs":{"data":"."},"outputs":null,"limits":{"max_attempts":1,"timeout_seconds":30}});
    f.app
        .config
        .profiles
        .insert("copy.consumer.r1".into(), profile);
    for (n, input) in [
        json!({"job":id,"entity":"bundle"}),
        json!({"job":id,"entity":"bundle","member":"f0000.txt"}),
        json!({"artifact":{"id":native,"revision":1}}),
    ]
    .into_iter()
    .enumerate()
    {
        let target=f.call("jobs","submit",json!({"key":format!("copy-{n}"),"kind":"copy.consumer.r1","owner":"owner","notify":"owner","params":{"data":input}})).unwrap()["id"].as_str().unwrap().to_string();
        let work = f.app.next_work(&Default::default()).unwrap().unwrap();
        assert_eq!(work.job, target);
        let request = f.app.request_for(&work).unwrap();
        assert!(request["verified_inputs"]["data"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["ref"] == new_ref));
        f.app
            .stage_returned(&work, 1, json!({"returned":true}))
            .unwrap();
    }
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
fn dashboard_task_index_covers_matching_outputs_beyond_first_page() {
    let mut f = Fixture::new();
    let mut tasks = vec![];
    for label in ["Current A", "Current B", "Discarded", "Empty"] {
        let task = f
            .call(
                "results",
                "register_scenario",
                json!({"key":label,"definition":{"label":label}}),
            )
            .unwrap();
        if label != "Empty" {
            let mut manifest = json!({"id":label,"kind":"dataset","label":label,"catalog_class":"regular",
                "scenario_ref":{"id":task["id"],"revision":1},"components":[],"relations":[]});
            if label == "Current B" {
                manifest.as_object_mut().unwrap().remove("scenario_ref");
            }
            f.app
                .db
                .execute(
                    "INSERT INTO asset_revisions VALUES(?,1,NULL,'dataset','owner','owner',?)",
                    rusqlite::params![label, manifest.to_string()],
                )
                .unwrap();
            if label == "Current B" {
                f.call("results","link_scenario",json!({"key":"link-b","scenario_ref":{"id":task["id"],"revision":1},
                    "target":{"kind":"entity","identity":{"id":label,"revision":1}},"relation":"produced_for",
                    "provenance":{"reason":"Exact historical link fixture"}})).unwrap();
            }
        }
        tasks.push(task["id"].clone());
    }
    f.call(
        "results",
        "set_output_disposition",
        json!({"key":"discard-dataset","reason":"Index fixture","state":"discarded",
        "targets":[{"kind":"entity","identity":{"id":"Discarded","revision":1}}]}),
    )
    .unwrap();
    for _ in 0..12 {
        f.app.index_once().unwrap();
    }
    let query = json!({"kinds":["dataset"],"view":"all","role":"all","output_state":"current","catalog_classes":["regular"],"records_only":true,"limit":1});
    let page = f.call("system", "dashboard_page", query.clone()).unwrap();
    assert_eq!(page["entities"].as_array().unwrap().len(), 1);
    let indexed = page["task_index"].as_array().unwrap();
    assert_eq!(indexed.len(), 2);
    assert!(tasks[..2].iter().all(|id| indexed.contains(id)));
    let mut next = query.clone();
    next["cursor"] = page["next_cursor"].clone();
    let next = f.call("system", "dashboard_page", next).unwrap();
    assert_eq!(next["entities"].as_array().unwrap().len(), 1);
    assert!(next["task_index"].is_null());
    let mut task_query = query.clone();
    task_query["task_id"] = tasks[1].clone();
    let linked = f.call("system", "dashboard_page", task_query).unwrap();
    assert_eq!(linked["entities"][0]["label"], "Current B");
    let mut discarded = query;
    discarded["output_state"] = json!("discarded");
    discarded["history"] = json!("all");
    let page = f.call("system", "dashboard_page", discarded).unwrap();
    assert_eq!(page["task_index"], json!([tasks[2]]));
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
    // Old attempts without stage receipts retain their recorded fallback.
    f.app
        .db
        .execute("DELETE FROM stage_runs WHERE job=?", [&id])
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
    f.app
        .db
        .execute(
            "INSERT INTO stage_runs VALUES(?,?, 'on_error',?,?, 'returned')",
            rusqlite::params![id, work.attempt, clock - 9., clock - 8.],
        )
        .unwrap();
    let detail = f.app.job_detail(&id).unwrap();
    assert_eq!(
        detail["execution_intervals"][0]["start"],
        json!(clock - 20.)
    );
    assert_eq!(detail["execution_intervals"][0]["end"], json!(clock - 10.));
    // Native prepare/run receipts exclude the wait between stages, even though
    // that gap lies inside the same Attempt wall time.
    for (stage, start, end) in [
        ("prepare", clock - 20., clock - 18.),
        ("run", clock - 12., clock - 10.),
    ] {
        f.app
            .db
            .execute(
                "INSERT INTO stage_runs VALUES(?,?,?,?,?,'returned')",
                rusqlite::params![id, work.attempt, stage, start, end],
            )
            .unwrap();
    }
    let native = f.app.dashboard_activity("research").unwrap();
    let active: f64 = native["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["milliseconds"].as_f64().unwrap())
        .sum();
    assert!((active - 4000.).abs() < 0.1);
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
    let job = f.submit("target-shape");
    let bad=f.call("results","link_scenario",json!({"key":"invalid-identity-shape","scenario_ref":{"id":s["id"],"revision":1},"target":{"kind":"job","identity":{"id":job,"unexpected_revision":999}},"relation":"used_by","provenance":{"reason":"Invalid extra selector must not become an invisible accepted link"}})).unwrap_err();
    assert_eq!(bad.status, 400);
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
    f.app
        .db
        .execute(
            "INSERT INTO attempts VALUES('preserved-attempt',?,'complete',110,200)",
            [&source],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "UPDATE jobs SET attempt='preserved-attempt' WHERE id=?",
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
    f.app
        .db
        .execute(
            "INSERT INTO attempts VALUES('continuation-attempt',?,'complete',300,340)",
            [&root],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "UPDATE jobs SET attempt='continuation-attempt' WHERE id=?",
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
    f.app
        .db
        .execute(
            "INSERT INTO attempts VALUES('later-attempt',?,'running',500,NULL)",
            [&source],
        )
        .unwrap();
    let detail = f.app.job_detail(&root).unwrap();
    assert_eq!(
        detail["source_executions"][0]["attempt"],
        "preserved-attempt"
    );
    assert_eq!(
        detail["source_executions"][0]["attempts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(detail["work_timeline"]["started"], 110.);
    assert_eq!(detail["work_timeline"]["ended"], 340.);
    assert_eq!(
        detail["internal_steps"]["steps"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        detail["work_timeline"]["executions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        detail["internal_steps"]["steps"][0]["source_execution"]["started"],
        110.
    );
    let listing = f.call("jobs", "list", json!({"view":"all"})).unwrap();
    assert!(
        listing
            .as_array()
            .unwrap()
            .iter()
            .any(|j| j["id"] == source),
        "A later source Attempt remains an independent current execution"
    );
    f.app
        .db
        .execute("UPDATE jobs SET state='complete' WHERE id=?", [&source])
        .unwrap();
    f.app
        .db
        .execute(
            "UPDATE attempts SET ended=600,state='complete' WHERE id='later-attempt'",
            [],
        )
        .unwrap();
    let next=f.call("jobs","submit",json!({"key":"reuse-new-source-attempt","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"consolidates":[source]})).unwrap();
    assert_ne!(next["id"], root);
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
    let document =
        json!({"input_features":{"camera/rgb~main":{"type":"VISUAL","shape":[3,64,64]}}});
    let shaped = crate::research_metadata::summarize(
        &json!({"kind":"model"}),
        &[(document.clone(), json!({"kind":"file"}))],
    );
    let pointer = shaped["inputs"][0]["evidence"]["pointer"].as_str().unwrap();
    assert_eq!(
        document.pointer(pointer).unwrap()["shape"],
        json!([3, 64, 64])
    );
    let sparse = crate::research_metadata::summarize(
        &json!({"kind":"model","inputs":[{"kind":"state"}]}),
        &[(
            json!({"input_features":{"camera":{"type":"VISUAL"}}}),
            json!({"kind":"file"}),
        )],
    );
    assert_eq!(sparse["inputs"][0]["status"], "unrecorded");
    assert!(sparse["inputs"][1].get("shape").is_none());
    crate::contracts::validate(
        &serde_json::from_str(include_str!(
            "../../rust/contracts/research-summary.schema.json"
        ))
        .unwrap(),
        &sparse,
    )
    .unwrap();
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

#[test]
fn content_search_can_continue_past_a_full_batch_of_unreadable_matches() {
    let mut f = Fixture::new();
    let bytes = b"needle uniquely recorded content";
    let locations: Vec<Value>=(0..270).map(|n|{
        let path=format!("entry-{n:04}.txt");
        fs::write(f.app.bulk.join(&path),bytes).unwrap();
        json!({"source_ref":path,"target_ref":path,"machine_id":"fixture","storage_id":"fixture-data","size":bytes.len(),"sha256":store::hash(bytes),"status":"available"})
    }).collect();
    let original = json!({"original":{"label":"Paged content fixture"},"locations":locations});
    f.app
        .db
        .execute(
            "INSERT INTO history_records VALUES('catalog','dataset','paged','r1','s1',?)",
            [original.to_string()],
        )
        .unwrap();
    for _ in 0..40 {
        f.app.index_once().unwrap();
    }
    let index = crate::index_store::IndexWriter::open(&f.app).unwrap();
    let mut q = index
        .db
        .prepare(
            "SELECT body FROM docs WHERE kind='file' AND valid_to IS NULL ORDER BY id LIMIT 256",
        )
        .unwrap();
    let paths = q
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(paths.len(), 256);
    for body in paths {
        let entry: Value = serde_json::from_str(&body).unwrap();
        fs::remove_file(f.app.bulk.join(entry["path"].as_str().unwrap())).unwrap();
    }
    drop(q);
    drop(index);
    let args = json!({"text":"needle uniquely recorded content","kinds":["file"],"limit":50});
    let first = f.call("results", "search_items", args.clone()).unwrap();
    assert!(first["items"].as_array().unwrap().is_empty());
    assert!(
        first["next_cursor"].is_string(),
        "A bounded batch must not hide later readable matches"
    );
    let mut next = args;
    next["cursor"] = first["next_cursor"].clone();
    let second = f.call("results", "search_items", next).unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 14);
    assert!(second["next_cursor"].is_null());
}

#[test]
fn nested_consolidation_has_one_logical_activity_and_profile_use() {
    let mut f = Fixture::new();
    let clock = store::now();
    let mut ids: Vec<String> = vec![];
    for n in 0..3 {
        let sources = ids.last().map(|id| vec![id.clone()]).unwrap_or_default();
        let mut request = json!({"key":format!("nested-{n}"),"kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false},"presentation":{"scope":"research","role":"main"}});
        if !sources.is_empty() {
            request["consolidates"] = json!(sources);
        }
        let id = f.call("jobs", "submit", request).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let attempt = format!("nested-attempt-{n}");
        f.app
            .db
            .execute(
                "INSERT INTO attempts VALUES(?,?,'complete',?,?)",
                rusqlite::params![
                    attempt,
                    id,
                    clock - 60. + n as f64 * 10.,
                    clock - 55. + n as f64 * 10.
                ],
            )
            .unwrap();
        f.app.db.execute("UPDATE jobs SET attempt=?,state='complete',result='{\"artifact_ready\":true}' WHERE id=?",rusqlite::params![attempt,id]).unwrap();
        ids.push(id);
    }
    assert_eq!(
        f.app.consolidation_target(&ids[0]).unwrap(),
        Some(ids[2].clone())
    );
    let detail = f.app.job_detail(&ids[2]).unwrap();
    assert_eq!(
        detail["work_timeline"]["executions"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        detail["internal_steps"]["steps"].as_array().unwrap().len(),
        3
    );
    let activity = f.app.dashboard_activity("research").unwrap();
    assert_eq!(
        activity["days"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["job_count"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    assert!(
        (activity["days"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["milliseconds"].as_f64().unwrap())
            .sum::<f64>()
            - 15000.)
            .abs()
            < 1.
    );
    let profiles = f.call("components", "profiles", json!({})).unwrap();
    let profile = profiles
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "cpu.smoke")
        .unwrap();
    assert_eq!(profile["usage_stats"]["logical_jobs"], 1);
    assert_eq!(profile["usage_stats"]["recorded_executions"], 3);
}

#[path = "metadata_templates.rs"]
mod metadata_template_tests;
#[path = "reporting.rs"]
mod reporting_tests;

#[path = "system_ownership.rs"]
mod system_ownership_tests;

#[path = "registry_lifecycle.rs"]
mod registry_lifecycle_tests;

#[test]
fn repeated_runtime_revisions_keep_dependency_pins_without_republishing() {
    let mut f = Fixture::new();
    let mut runtimes = Vec::new();
    for (revision, version) in [(1, "1.0"), (2, "2.0")] {
        let manifest = json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[{"name":"package","version":version}]});
        runtimes.push(json!({"id":"sim","revision":revision,"sha256":hash(crate::compat::seal(&manifest)),"manifest":manifest}));
    }
    let mut head = None;
    for _ in 0..3 {
        runtimes.reverse();
        f.app
            .observe_runtimes(json!({"status":"ok","complete":true,"runtimes":runtimes}))
            .unwrap();
        f.app.index_once().unwrap();
        let current = f.app.catalog_head().unwrap();
        assert_eq!(*head.get_or_insert(current), current);
        let mut revisions = Vec::new();
        for version in ["1.0", "2.0"] {
            let resolved=f.call("results","resolve_item",json!({"native_ref":{"kind":"dependency","identity":{"name":"package","version":version,"scope":"unrecorded","origin":{"type":"runtime","id":"sim"}}}})).unwrap();
            revisions.push((resolved["item_id"].clone(), resolved["revision_id"].clone()));
        }
        assert_eq!(revisions[0].0, revisions[1].0);
        assert_ne!(revisions[0].1, revisions[1].1);
        let page = f
            .call(
                "results",
                "search_items",
                json!({"kinds":["dependency"],"view":"all","role":"all"}),
            )
            .unwrap();
        assert_eq!(page["items"][0]["version"], "2.0");
    }
}

#[test]
fn image_os_dependencies_do_not_oscillate_between_distinct_runtime_images() {
    let mut f = Fixture::new();
    let mut runtimes = vec![];
    for (id, image, version) in [("ubuntu", "a", "1.3-ubuntu"), ("debian", "b", "1.3-debian")] {
        let manifest = json!({"runtime":{"image":format!("sha256:{}",image.repeat(64))},"dependencies":[{"name":"zlib","version":version,"scope":"image-os","origin":{"type":"image"}}]});
        runtimes.push(json!({"id":id,"revision":1,"sha256":hash(crate::compat::seal(&manifest)),"manifest":manifest}));
    }
    f.app
        .observe_runtimes(json!({"status":"ok","complete":true,"runtimes":runtimes}))
        .unwrap();
    f.app.index_once().unwrap();
    let head = f.app.catalog_head().unwrap();
    for _ in 0..3 {
        runtimes.reverse();
        f.app
            .observe_runtimes(json!({"status":"ok","complete":true,"runtimes":runtimes}))
            .unwrap();
        f.app.index_once().unwrap();
        assert_eq!(f.app.catalog_head().unwrap(), head);
    }
    let page = f
        .call(
            "results",
            "search_items",
            json!({"kinds":["dependency"],"view":"all","role":"all"}),
        )
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert!(page["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["native_ref"]["identity"]["origin"]["image"].is_string()));
}

#[test]
fn runtime_revision_links_resolve_old_exact_aliases_without_hash_fallback() {
    let mut f = Fixture::new();
    let first = json!({"label":"Original Runtime","runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[]});
    let later = json!({"label":"Later Runtime","runtime":{"image":format!("sha256:{}","b".repeat(64))},"dependencies":[]});
    let sha1 = hash(compat::seal(&first));
    let sha2 = hash(compat::seal(&later));
    f.app.observe_runtimes(json!({"runtimes":[{"id":"sim","revision":1,"sha256":sha1,"manifest":first},{"id":"sim","revision":2,"sha256":sha2,"manifest":later}]})).unwrap();
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let exact=f.call("results","resolve_item",json!({"native_ref":{"kind":"runtime","identity":{"id":"sim","revision":1,"sha256":sha1}}})).unwrap();
    let link = json!({"native_ref":{"kind":"runtime","identity":{"id":"sim","revision":1}}});
    let resolved = f.call("results", "resolve_item", link.clone()).unwrap();
    assert_eq!(resolved["item_id"], exact["item_id"]);
    assert_eq!(resolved["revision_id"], exact["revision_id"]);
    let detail = f
        .call(
            "system",
            "dashboard_entity",
            json!({"item_id":resolved["item_id"],"revision_id":resolved["revision_id"]}),
        )
        .unwrap();
    assert_eq!(detail["entities"][0]["loom_runtime"]["revision"], 1);
    assert_eq!(detail["entities"][0]["label"], "Original Runtime");
    assert!(f.call("results","resolve_item",json!({"native_ref":{"kind":"runtime","identity":{"id":"sim","revision":1,"sha256":sha2}}})).is_err());
    assert!(f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"runtime","identity":{"id":"sim","revision":99}}})
        )
        .is_err());
    let second = f
        .call(
            "results",
            "resolve_item",
            json!({"native_ref":{"kind":"runtime","identity":{"id":"sim","revision":2}}}),
        )
        .unwrap();
    f.app
        .db
        .execute(
            "INSERT INTO catalog_aliases VALUES('runtime',?,?,?)",
            rusqlite::params![
                json!({"id":"sim","revision":1,"sha256":sha2}).to_string(),
                crate::items::parse(&second["item_id"]).unwrap(),
                crate::items::parse(&second["revision_id"]).unwrap()
            ],
        )
        .unwrap();
    assert_eq!(
        f.call("results", "resolve_item", link).unwrap_err().code,
        "ambiguous_reference"
    );
    assert_eq!(f.call("results","resolve_item",json!({"native_ref":{"kind":"runtime","identity":{"id":"sim","revision":1,"sha256":sha1}}})).unwrap()["revision_id"],exact["revision_id"]);
}

#[test]
fn executable_outcome_without_deliverables_is_a_main_job() {
    let mut f = Fixture::new();
    let open = json!({"owner":"owner","notify":"owner","key":"inspect-sensor","outcome":{"goal":"Verify sensor readiness"},"plan":{"sealed":false,"units":[]}});
    let created = f.call("jobs", "create", open.clone()).unwrap();
    let id = &created["id"];
    assert_eq!(f.call("jobs", "create", open).unwrap(), created);
    let mut seal = json!({"id":id,"key":"seal","expected_revision":1,"reason":"Run the readiness check","plan":{"sealed":true,"units":[]}});
    assert_eq!(
        f.call("jobs", "revise_plan", seal.clone())
            .unwrap_err()
            .code,
        "plan_pending"
    );
    seal["plan"]["units"] = json!([{"id":"check","label":"Check","execution":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false}}}]);
    let accepted = f.call("jobs", "revise_plan", seal).unwrap();
    let node = accepted["nodes"]["check"].as_str().unwrap();
    f.app
        .db
        .execute("UPDATE jobs SET state='complete' WHERE id=?", [node])
        .unwrap();
    crate::plans::finish(&f.app.db, id.as_str().unwrap()).unwrap();
    assert_eq!(
        f.call("jobs", "get", json!({"id":id})).unwrap()["state"],
        "complete"
    );
}

#[test]
fn open_outcome_binds_a_granted_runtime_after_restart_and_publishes_once() {
    let mut f = Fixture::new();
    let target = json!({"id":"sim","revision":6});
    let slot = json!({"id":"evaluate","label":"Evaluate","requires":[{"stage":"runtime","runtime":target}]});
    let create = json!({"owner":"owner","notify":"owner","key":"evaluate-once","outcome":{"goal":"Publish the evaluation receipt","deliverables":[{"alias":"report","kind":"artifact","from":{"node":"evaluate","entity":"report"}}]},"plan":{"sealed":false,"units":[slot]}});
    let first = f.call("jobs", "create", create.clone()).unwrap();
    assert_eq!(f.count("jobs"), 0);
    let root = &first["id"];
    assert_eq!(
        f.app.queue().unwrap()["logical_entries"][0]["reason"],
        "awaiting_runtime_binding"
    );
    assert!(f.app.next_work(&Default::default()).unwrap().is_none());
    f.app = Application::open(
        &f.app.local,
        &f.app.bulk,
        &f.app.components,
        f.app.config.clone(),
    )
    .unwrap();
    let downstream=f.call("jobs","submit",json!({"key":"later","owner":"owner","notify":"owner","kind":"cpu.smoke","params":{"seconds":0,"fail":false},"depends_on":[root]})).unwrap();
    assert!(f.app.next_work(&Default::default()).unwrap().is_none());
    f.call("jobs", "hold", json!({"id":root})).unwrap();
    let mut revise = json!({"id":root,"key":"bind","expected_revision":1,"reason":"Verified installation","plan":{"sealed":true,"units":[slot]}});
    revise["plan"]["units"][0]["execution"] = json!({"kind":"eval.r1","params":{}});
    assert_eq!(
        f.call("jobs", "revise_plan", revise.clone())
            .unwrap_err()
            .code,
        "runtime_pending"
    );
    let manifest = json!({"runtime":{"image":format!("sha256:{}","f".repeat(64))},"validation":{"status":"passed"}});
    let sha = hash(compat::seal(&manifest));
    f.app
        .observe_runtimes(
            json!({"runtimes":[{"id":"sim","revision":6,"sha256":sha,"manifest":manifest}]}),
        )
        .unwrap();
    let profile = json!({"schema":"loom-execution/v2","revision":"eval.r1","sha256":"a".repeat(64),"params_schema":{"type":"object"},"runtime":{"executor":"container","lane":"cpu","command":["/bin/true"],"runtime_pin":{"id":"sim","revision":6,"sha256":"wrong"}},"inputs":{},"outputs":{"report":"artifact"},"limits":{"max_attempts":1,"timeout_seconds":30}});
    f.app
        .config
        .profiles
        .insert("eval.r1".into(), profile.clone());
    assert_eq!(
        f.call("jobs", "revise_plan", revise.clone())
            .unwrap_err()
            .code,
        "runtime_pin"
    );
    assert_eq!(f.count("workflow_nodes"), 0);
    let mut profile = profile;
    profile["runtime"]["runtime_pin"]["sha256"] = json!(sha);
    f.app.config.profiles.insert("eval.r1".into(), profile);
    let bound = f.call("jobs", "revise_plan", revise.clone()).unwrap();
    assert_eq!(&bound["id"], root);
    assert_eq!(
        f.call("jobs", "revise_plan", revise.clone()).unwrap(),
        bound
    );
    assert_eq!(f.call("jobs", "create", create).unwrap(), first);
    let mut stale = revise.clone();
    stale["key"] = json!("stale");
    assert_eq!(
        f.call("jobs", "revise_plan", stale).unwrap_err().code,
        "plan_revision"
    );
    assert!(f.app.next_work(&Default::default()).unwrap().is_none());
    f.call("jobs", "resume", json!({"id":root})).unwrap();
    let run = f.app.next_work(&Default::default()).unwrap().unwrap();
    assert_eq!(bound["nodes"]["evaluate"], run.job);
    fs::create_dir_all(&run.folder).unwrap();
    fs::write(run.folder.join("report.txt"), b"measured").unwrap();
    f.app
        .stage_returned(&run, 0, json!({"returned":true}))
        .unwrap();
    let publish = f.app.next_work(&Default::default()).unwrap().unwrap();
    f.app.publish_work(&publish,json!({"entities":[{"id":"report","kind":"artifact","path":"report.txt","provenance":{"profile":"eval.r1","producer_sha256":"a".repeat(64)},"relations":[]}],"metrics":[]})).unwrap();
    let detail = f.call("jobs", "get", json!({"id":root})).unwrap();
    assert_eq!(detail["state"], "complete");
    assert_eq!(
        detail["result"]["outputs"]["report"]["sha256"],
        hash(b"measured")
    );
    assert_eq!(
        f.app
            .db
            .query_row(
                "SELECT count(*) FROM events WHERE job=? AND code='complete'",
                [root.as_str().unwrap()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        f.app.next_work(&Default::default()).unwrap().unwrap().job,
        downstream["id"]
    );
}

#[test]
fn receipt_only_plans_require_fresh_host_evidence_and_no_synthetic_attempt() {
    let mut f = Fixture::new();
    let create = json!({"key":"install","owner":"owner","notify":"owner","outcome":{"goal":"Verify installation","deliverables":[{"kind":"runtime_receipt","alias":"installed","runtime":{"id":"compiler","revision":1}}]},"plan":{"sealed":false,"units":[]}});
    let root = f.call("jobs", "create", create.clone()).unwrap()["id"].clone();
    let seal = json!({"id":root,"key":"seal","expected_revision":1,"reason":"Verify receipt","plan":{"sealed":true,"units":[]}});
    assert_eq!(
        f.app
            .execute(
                &auth::person_actor("other"),
                "jobs",
                "revise_plan",
                seal.clone()
            )
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(
        f.call("jobs", "revise_plan", seal.clone())
            .unwrap_err()
            .code,
        "runtime_pending"
    );
    for status in ["failed", "passed"] {
        let m = json!({"validation":{"status":status}});
        f.app.observe_runtimes(json!({"runtimes":[{"id":"compiler","revision":1,"sha256":hash(compat::seal(&m)),"manifest":m}]})).unwrap();
        if status == "failed" {
            assert_eq!(
                f.call("jobs", "revise_plan", seal.clone())
                    .unwrap_err()
                    .code,
                "runtime_pending"
            );
        }
    }
    f.app
        .db
        .execute(
            "UPDATE rust_runtime_observation SET observed=observed-60",
            [],
        )
        .unwrap();
    assert_eq!(
        f.call("jobs", "revise_plan", seal.clone())
            .unwrap_err()
            .code,
        "runtime_pending"
    );
    f.app
        .db
        .execute(
            "UPDATE rust_runtime_observation SET observed=?",
            [store::now()],
        )
        .unwrap();
    let receipt = f.call("jobs", "revise_plan", seal.clone()).unwrap();
    assert_eq!(
        receipt["receipts"]["installed"]["validation"]["status"],
        "passed"
    );
    assert_eq!(
        f.call("jobs", "get", json!({"id":root})).unwrap()["state"],
        "complete"
    );
    assert_eq!(f.call("jobs", "revise_plan", seal).unwrap(), receipt);
    let detail = f.call("jobs", "get", json!({"id":root})).unwrap();
    assert_eq!(detail["result"]["artifact_ready"], false);
    assert_eq!(detail["result"]["receipts"], receipt["receipts"]);
    assert_eq!(f.count("attempts"), 0);
    assert_eq!(f.count("events"), 1);
    let mut duplicate = create.clone();
    duplicate["outcome"]["goal"] = json!("Other");
    assert_eq!(
        f.call("jobs", "create", duplicate).unwrap_err().code,
        "idempotency_conflict"
    );
    let mut cancelled = create;
    cancelled["key"] = json!("cancel-install");
    let id = f.call("jobs", "create", cancelled).unwrap()["id"].clone();
    assert_eq!(
        f.call("jobs", "cancel", json!({"id":id})).unwrap()["state"],
        "cancelled"
    );
    assert_eq!(f.app.queue().unwrap()["logical_entries"], json!([]));
}

#[test]
fn sealed_outcome_rejects_an_added_unit_before_execution() {
    let mut fixture = Fixture::new();
    let unit = json!({"id":"first","label":"First","execution":{"kind":"cpu.smoke",
        "params":{"seconds":0,"fail":false}}});
    let request = json!({"key":"sealed-before-run","owner":"owner","notify":"owner",
        "outcome":{"goal":"One readiness check"},"plan":{"sealed":true,"units":[unit]}});
    let receipt = fixture.call("jobs", "create", request.clone()).unwrap();
    let extra = json!({"id":"extra","label":"Extra","execution":{"kind":"cpu.smoke",
        "params":{"seconds":0,"fail":false}}});
    let extension = json!({"key":"extend","id":receipt["id"],"expected_revision":1,
        "reason":"Add another check","plan":{"sealed":true,"units":[unit,extra]}});
    assert_eq!(
        fixture
            .call("jobs", "revise_plan", extension)
            .unwrap_err()
            .code,
        "plan"
    );
    assert_eq!(fixture.count("jobs"), 1);
    assert_eq!(fixture.call("jobs", "create", request).unwrap(), receipt);
}

#[test]
fn open_plan_preserves_done_nodes_and_rejects_cycles_and_broken_publication() {
    let mut f = Fixture::new();
    let a = json!({"id":"a","label":"Collect","execution":{"kind":"cpu.smoke","params":{"seconds":0,"fail":false}}});
    let create = json!({"owner":"owner","notify":"owner","key":"open-run","outcome":{"goal":"Publish evidence","deliverables":[{"alias":"out","kind":"artifact","from":{"node":"a","entity":"observation"}}]},"plan":{"sealed":false,"units":[a]}});
    let root = f.call("jobs", "create", create.clone()).unwrap()["id"].clone();
    let run = f.app.next_work(&Default::default()).unwrap().unwrap();
    fs::create_dir_all(&run.folder).unwrap();
    fs::write(run.folder.join("out"), b"ok").unwrap();
    f.app
        .stage_returned(&run, 0, json!({"returned":true}))
        .unwrap();
    let publish = f.app.next_work(&Default::default()).unwrap().unwrap();
    let good =
        json!({"entities":[{"id":"observation","kind":"artifact","path":"out"}],"metrics":[]});
    let mut bad = good.clone();
    bad["entities"][0]["id"] = json!("other");
    assert_eq!(
        f.app
            .publish_result(&run.job, &run.attempt, bad, &run.folder)
            .unwrap_err()
            .code,
        "deliverable"
    );
    f.app.publish_work(&publish, good).unwrap();
    assert_eq!(f.count("events"), 0);
    assert_eq!(
        f.call("jobs", "get", json!({"id":root})).unwrap()["state"],
        "blocked"
    );
    let seal = json!({"id":root,"key":"seal","expected_revision":1,"reason":"Ready","plan":{"sealed":true,"units":[a]}});
    let mut changed = seal.clone();
    changed["plan"]["units"][0]["execution"]["params"]["seconds"] = json!(2);
    assert_eq!(
        f.call("jobs", "revise_plan", changed).unwrap_err().code,
        "plan"
    );
    let mut cycle = seal.clone();
    cycle["plan"]["units"].as_array_mut().unwrap().push(json!({"id":"b","label":"Circular","execution":{"kind":"cpu.smoke","depends_on":[root],"params":{"seconds":0,"fail":false}}}));
    assert!(f.call("jobs", "revise_plan", cycle).is_err());
    assert_eq!(f.count("jobs"), 1);
    f.call("jobs", "revise_plan", seal).unwrap();
    assert_eq!(
        f.call("jobs", "get", json!({"id":root})).unwrap()["state"],
        "complete"
    );
    assert_eq!(f.count("attempts"), 1);
    assert_eq!(f.count("events"), 1);
}
