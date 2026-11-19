use super::*;
use crate::reporting;

fn builtin(f: &mut Fixture, id: &str) -> Value {
    f.call(
        "results",
        "get_report_contract",
        json!({"id":id,"revision":1}),
    )
    .unwrap()
}
fn registered(f: &mut Fixture, id: &str, definition: Value) -> Value {
    f.call(
        "results",
        "register_report_contract",
        json!({"id":id,"definition":definition,"key":id,"reason":"Fixture analysis coverage"}),
    )
    .unwrap()
}
fn data() -> Value {
    json!({"inputs":[],"members":[{"id":"e1","split":"train","source":"demo0"}],"count":1,"split_counts":{"train":1},"invalid_values":0})
}
fn fixture_report(
    f: &mut Fixture,
    contract: &Value,
    payloads: &[(&str, Value)],
) -> (String, std::path::PathBuf, Value) {
    fixture_report_for(f, "report-publication", contract, payloads)
}
fn fixture_report_for(
    f: &mut Fixture,
    key: &str,
    contract: &Value,
    payloads: &[(&str, Value)],
) -> (String, std::path::PathBuf, Value) {
    let job = f.submit(key);
    let folder = f.app.bulk.join(key);
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("data.json"), "{}").unwrap();
    let kind = contract["definition"]["output_kind"].as_str().unwrap();
    let profile = json!({"revision":"fixture.r1","sha256":"a".repeat(64),"reporting_policy":reporting::POLICY,"reporting":{"conditioning":"language"},"reporting_pins":{kind:contract},"outputs":{"data":kind}});
    let mut spec = jobs::load(&f.app.db, &job).unwrap().spec;
    spec["profile"] = profile.clone();
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='processing',attempt='report-attempt',spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), job],
        )
        .unwrap();
    let mut reports = vec![];
    let mut components = vec![];
    for (id, data) in payloads {
        let name = format!("{id}.json");
        let report = json!({"schema":"loom-report/v1","requirement_id":id,"contract_ref":reporting::reference(contract),"producer":{"job":job,"attempt":"report-attempt","profile_sha256":profile["sha256"]},"data":data});
        let bytes = serde_json::to_vec(&report).unwrap();
        fs::write(folder.join(&name), &bytes).unwrap();
        components.push(json!(name));
        reports.push(json!({"requirement_id":id,"component":name,"sha256":hash(bytes)}));
    }
    let manifest = json!({"entities":[{"id":"data","kind":kind,"path":"data.json","label":"Recorded scientific output","components":components,"relations":[],"reporting":{"contract_ref":reporting::reference(contract),"reports":reports}}],"metrics":[]});
    (job, folder, manifest)
}
#[test]
fn reporting_registry_replays_pins_revisions_and_shares_common_item_identity() {
    let mut f = Fixture::new();
    let base = builtin(&mut f, "loom.report.dataset");
    let args = json!({"id":"fixture.dataset","definition":base["definition"],"reason":"Reusable coverage","key":"registry"});
    let first = f
        .call("results", "register_report_contract", args.clone())
        .unwrap();
    assert_eq!(
        first,
        f.call("results", "register_report_contract", args.clone())
            .unwrap()
    );
    let mut revised = args.clone();
    revised["key"] = json!("r2");
    revised["expected_revision"] = json!(1);
    revised["definition"]["label"] = json!("Revised display");
    let second = f
        .call("results", "register_report_contract", revised)
        .unwrap();
    assert_eq!(second["revision"], 2);
    assert_eq!(
        f.call(
            "results",
            "get_report_contract",
            json!({"id":"fixture.dataset","revision":1})
        )
        .unwrap(),
        first
    );
    let mut wrong = reporting::reference(&first);
    wrong["sha256"] = json!("f".repeat(64));
    assert!(reporting::resolve(&f.app.db, "report_contract", &wrong).is_err());
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..6 {
        f.app.index_once().unwrap();
    }
    let mut ids = vec![];
    for revision in [1, 2] {
        ids.push(f.call("results","resolve_item",json!({"native_ref":{"kind":"report_contract","identity":{"id":"fixture.dataset","revision":revision}}})).unwrap());
    }
    assert_eq!(ids[0]["item_id"], ids[1]["item_id"]);
    assert_ne!(ids[0]["revision_id"], ids[1]["revision_id"]);
    let list = f
        .call(
            "results",
            "report_contracts",
            json!({"kind":"dataset","limit":1}),
        )
        .unwrap();
    assert!(!list["items"][0].get("definition").is_some());
    assert!(list["next_offset"].is_number());
}
#[test]
fn reporting_exclusions_are_source_bound_prospective_and_cannot_be_laundered() {
    let mut f = Fixture::new();
    let base = builtin(&mut f, "loom.report.policy_import");
    let job = f.submit("evidence");
    let id = format!("{job}.policy");
    let body = json!({"id":id,"kind":"model","metadata":{"language_free":true}}).to_string();
    f.app
        .db
        .execute(
            "INSERT INTO entity_revisions VALUES(?,1,?,'model','owner','owner',?)",
            rusqlite::params![id, job, body],
        )
        .unwrap();
    let evidence = f.call("results", "get_report_evidence", json!({"target":{"kind":"entity","identity":{"id":id,"revision":1}},"pointer":"/metadata/language_free"})).unwrap();
    assert_eq!(evidence["sha256"], hash(&body));
    assert_eq!(evidence["value"], true);
    assert!(f.call("results", "get_report_evidence", json!({"target":{"kind":"entity","identity":{"id":id}},"pointer":"/metadata/language_free"})).is_err());
    let definition = json!({"label":"Language-free imported Policy","output_kind":"model","activity":"policy_import","base_ref":reporting::reference(&base),"requirements":[],"adaptations":[{"requirement_id":"instructions","disposition":"excluded","reason":"Source Policy has no language input","analysis_impact":"No instruction-following claim","evidence":[evidence],"when":{"pointer":"/profile/reporting/conditioning","equals":"none"}}]});
    let custom = registered(&mut f, "fixture.language_free", definition.clone());
    assert_eq!(
        custom["definition"]["excluded_requirements"][0]["role"],
        "instructions"
    );
    assert_eq!(
        custom["definition"]["effective_requirements"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut p = json!({"outputs":{"policy":"model"},"reporting":{"contracts":{"model":reporting::reference(&custom)},"capabilities":["policy_inputs"],"conditioning":"none"}});
    reporting::bind_profile(&f.app.db, &mut p).unwrap();
    let mut spec = json!({"profile":p,"params":{}});
    assert!(reporting::validate_plan(&f.app.db, &spec).is_ok());
    spec["profile"]["reporting"]["conditioning"] = json!("language");
    assert!(reporting::validate_plan(&f.app.db, &spec).is_err());
    let mut invalid = definition.clone();
    invalid["adaptations"][0]["evidence"] = json!([]);
    assert!(f
        .call(
            "results",
            "validate_report_contract",
            json!({"definition":invalid})
        )
        .is_err());
    let derived = registered(
        &mut f,
        "fixture.derived",
        json!({"label":"Derived display","output_kind":"model","activity":"policy_import","base_ref":reporting::reference(&custom),"requirements":[],"adaptations":[]}),
    );
    spec["profile"]["reporting_pins"]["model"] = derived;
    f.app
        .db
        .execute("UPDATE entity_revisions SET manifest='{}' WHERE id=?", [id])
        .unwrap();
    spec["profile"]["reporting"]["conditioning"] = json!("none");
    assert_eq!(
        reporting::validate_plan(&f.app.db, &spec).unwrap_err().code,
        "report_evidence_changed"
    );
}
#[test]
fn reporting_contracts_reject_unknown_roles_schema_fetch_and_empty_root_coverage() {
    let mut f = Fixture::new();
    let mut d = builtin(&mut f, "loom.report.dataset")["definition"].clone();
    d["requirements"][0]["schema"] =
        json!({"type":"object","$ref":"https://example.invalid/schema"});
    assert!(f
        .call(
            "results",
            "validate_report_contract",
            json!({"definition":d})
        )
        .is_err());
    let d = json!({"label":"Empty","activity":"evaluation","output_kind":"experiment","requirements":[]});
    assert!(f
        .call(
            "results",
            "validate_report_contract",
            json!({"definition":d})
        )
        .is_err());
    let mut d = builtin(&mut f, "loom.report.evaluation")["definition"].clone();
    d["activity"] = json!("comparison");
    assert!(f
        .call(
            "results",
            "validate_report_contract",
            json!({"definition":d})
        )
        .is_err());
}
#[test]
fn report_publication_is_atomic_hash_bound_and_readable_with_bounded_pages() {
    let mut f = Fixture::new();
    let contract = builtin(&mut f, "loom.report.dataset");
    let (job, folder, manifest) = fixture_report(&mut f, &contract, &[("membership", data())]);
    let mut bad = manifest.clone();
    bad["entities"][0]["reporting"]["reports"] = json!([]);
    assert!(f
        .app
        .publish_result(&job, "report-attempt", bad, &folder)
        .is_err());
    assert_eq!(f.count("entity_revisions"), 0);
    let mut bad = manifest.clone();
    bad["entities"][0]["reporting"]["reports"][0]["sha256"] = json!("f".repeat(64));
    assert!(f
        .app
        .publish_result(&job, "report-attempt", bad, &folder)
        .is_err());
    assert_eq!(f.count("entity_revisions"), 0);
    f.app
        .publish_result(&job, "report-attempt", manifest, &folder)
        .unwrap();
    // Stale or altered registered copies must not mask another verified copy.
    fs::write(f.app.bulk.join("000bad-report.json"), "{}").unwrap();
    for stale in ["000missing-report.json", "000bad-report.json"] {
        f.app.db.execute("INSERT INTO entity_locations VALUES(?,1,'membership.json','fixture','fixture-data',?,'available')", rusqlite::params![format!("{job}.data"),stale]).unwrap();
    }
    let args = json!({"id":format!("{job}.data"),"revision":1,"requirement_id":"membership","pointer":"/members","limit":1});
    let report = f.call("results", "read_report", args.clone()).unwrap();
    assert_eq!(report["value"][0]["id"], "e1");
    assert_eq!(report["total"], 1);
    assert_eq!(report["verification"], "sha256_verified");
    let entity = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.data"),"revision":1}),
        )
        .unwrap();
    assert_eq!(entity["reporting"]["state"], "validated");
    fs::write(folder.join("membership.json"), "{}").unwrap();
    assert_eq!(
        f.call("results", "read_report", args).unwrap_err().code,
        "report_changed"
    );
}

#[test]
fn distribution_matrix_requires_consistent_cells() {
    let mut fixture = Fixture::new();
    let base = builtin(&mut fixture, "loom.report.dataset");
    let contract = registered(
        &mut fixture,
        "fixture.coverage",
        json!({
            "label":"Crossed coverage","output_kind":"dataset","activity":"dataset_import",
            "base_ref":reporting::reference(&base),"adaptations":[],
            "requirements":[{"id":"coverage","role":"distribution","label":"Coverage","definitions":[],
                "schema":{"type":"object","required":["cells"],"properties":{"cells":{"type":"array"}}},
                "fields":[{"id":"matrix","label":"Entry by posture","pointer":"/cells","format":"coverage_matrix"}]}]
        }),
    );
    let (job, folder, manifest) = fixture_report(
        &mut fixture,
        &contract,
        &[
            ("membership", data()),
            (
                "coverage",
                json!({"cells":[{"row":"entry1","column":"posture0","assigned":2,"admitted":3}]}),
            ),
        ],
    );
    assert!(fixture
        .app
        .publish_result(&job, "report-attempt", manifest, &folder)
        .is_err());
    assert_eq!(fixture.count("entity_revisions"), 0);
}

#[test]
fn report_expectations_bind_accepted_targets_and_inventory_keeps_reports_searchable() {
    let mut f = Fixture::new();
    let contract = builtin(&mut f, "loom.report.dataset");
    let (job, folder, mut manifest) = fixture_report(&mut f, &contract, &[("membership", data())]);
    let mut spec = jobs::load(&f.app.db, &job).unwrap().spec;
    spec["params"]["target_count"] = json!(2);
    spec["profile"]["reporting"]["expectations"] = json!({"dataset":{"membership":[{"pointer":"/count","context_pointer":"/params/target_count"}]}});
    reporting::validate_plan(&f.app.db, &spec).unwrap();
    f.app
        .db
        .execute(
            "UPDATE jobs SET spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), job],
        )
        .unwrap();
    assert!(f
        .app
        .publish_result(&job, "report-attempt", manifest.clone(), &folder)
        .unwrap_err()
        .message
        .contains("pre-execution plan"));
    let report = folder.join("membership.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    value["data"]["members"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"e2","split":"train","source":"demo0"}));
    value["data"]["count"] = json!(2);
    value["data"]["split_counts"]["train"] = json!(2);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(report, &bytes).unwrap();
    manifest["entities"][0]["reporting"]["reports"][0]["sha256"] = json!(hash(bytes));
    let chunk = format!(
        "{}\n",
        json!({"key":"data.json","path":"data.json","role":"primary","size":2,"sha256":hash("{}")})
    );
    fs::write(folder.join("inventory.jsonl"), &chunk).unwrap();
    manifest["entities"][0]["inventory"] = json!({"schema":"loom-output-inventory/v1","entries":1,"chunks":[{"ref":"inventory.jsonl","entries":1,"sha256":hash(chunk)}]});
    f.app
        .publish_result(&job, "report-attempt", manifest, &folder)
        .unwrap();
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..6 {
        f.app.index_once().unwrap();
    }
    let item=f.call("results","resolve_item",json!({"native_ref":{"kind":"entity","identity":{"id":format!("{job}.data"),"revision":1}}})).unwrap();
    let files = f
        .call(
            "results",
            "item_files",
            json!({"item_id":item["item_id"],"revision_id":item["revision_id"],"limit":10}),
        )
        .unwrap();
    assert!(files["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "membership.json"));
    assert!(files["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "data.json"));
}
#[test]
fn report_publication_rejects_other_attempt_and_wrong_dataset_population_without_rerun() {
    let mut f = Fixture::new();
    let contract = builtin(&mut f, "loom.report.dataset");
    let (job, folder, mut manifest) = fixture_report(&mut f, &contract, &[("membership", data())]);
    let path = folder.join("membership.json");
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for scope in ["attempt", "population", "duplicate"] {
        let mut bad = original.clone();
        match scope {
            "attempt" => bad["producer"]["attempt"] = json!("another"),
            "population" => bad["data"]["count"] = json!(2),
            _ => {
                bad["data"]["members"] =
                    json!([bad["data"]["members"][0], bad["data"]["members"][0]])
            }
        };
        let bytes = serde_json::to_vec(&bad).unwrap();
        fs::write(&path, &bytes).unwrap();
        manifest["entities"][0]["reporting"]["reports"][0]["sha256"] = json!(hash(bytes));
        assert!(f
            .app
            .publish_result(&job, "report-attempt", manifest.clone(), &folder)
            .is_err());
        assert_eq!(f.count("entity_revisions"), 0);
        assert_eq!(f.count("attempts"), 0);
        assert_eq!(
            jobs::load(&f.app.db, &job).unwrap().aggregate.state,
            loom_domain::JobState::Processing
        );
    }
}
#[test]
fn sdg_reports_reconcile_planned_conditions_readbacks_and_final_dataset_membership() {
    let mut f = Fixture::new();
    let c = builtin(&mut f, "loom.report.sdg");
    let pin = c["definition"]["requirements"][1]["definitions"][0].clone();
    let plan = json!({"factor_refs":[pin],"frame":"demo XY","sampling":"uniform","bounds":{"xy_mm":[-10,10]},"fixed":{"yaw_deg":0},"conditions":[{"id":"c1","split":"train","values":{"xy_mm":[1,2]}}]});
    let readback = json!({"factor_refs":[pin],"frame":"demo XY","observations":[{"id":"c1","member_id":"wrong-member","attempt":"report-attempt","state":"accepted","requested":{"xy_mm":[1,2]},"observed":{"xy_mm":[1,2]},"phase":"postwarm"}]});
    let (job, folder, mut m) = fixture_report(
        &mut f,
        &c,
        &[
            ("membership", data()),
            ("conditions", plan),
            ("readback", readback),
        ],
    );
    assert!(f
        .app
        .publish_result(&job, "report-attempt", m.clone(), &folder)
        .is_err());
    assert_eq!(f.count("entity_revisions"), 0);
    let path = folder.join("readback.json");
    let mut r: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    r["data"]["observations"][0]["member_id"] = json!("e1");
    let bytes = serde_json::to_vec(&r).unwrap();
    fs::write(path, &bytes).unwrap();
    m["entities"][0]["reporting"]["reports"][2]["sha256"] = json!(hash(bytes));
    f.app
        .publish_result(&job, "report-attempt", m, &folder)
        .unwrap();
}
#[test]
fn new_scientific_admission_requires_reporting_but_frozen_legacy_execution_is_readable() {
    let mut f = Fixture::new();
    let mut p = json!({"outputs":{"data":"dataset"}});
    assert!(reporting::bind_profile(&f.app.db, &mut p).is_err());
    let c = builtin(&mut f, "loom.report.dataset");
    p["reporting"] = json!({"contracts":{"dataset":reporting::reference(&c)},"capabilities":["dataset_statistics"]});
    reporting::bind_profile(&f.app.db, &mut p).unwrap();
    assert!(reporting::validate_new_admission(&json!({"profile":p})).is_ok());
    assert_eq!(
        reporting::validate_new_admission(&json!({"profile":{"outputs":{"data":"dataset"}}}))
            .unwrap_err()
            .code,
        "report_profile_legacy"
    );
    assert!(reporting::validate_plan(
        &f.app.db,
        &json!({"profile":{"outputs":{"data":"dataset"}}})
    )
    .is_ok());
    let mut p = json!({"outputs":{"receipt":"artifact"}});
    reporting::bind_profile(&f.app.db, &mut p).unwrap();
    assert!(reporting::validate_declaration(&p, &json!({"id":"hidden","kind":"dataset"})).is_err());
}

#[test]
fn new_reference_units_require_a_task_and_artifact_only_smoke_can_omit_it() {
    let mut f = Fixture::new();
    let contract = builtin(&mut f, "loom.report.dataset");
    let mut profile = json!({
        "schema":"loom-execution/v2","revision":"reference.r1","sha256":"a".repeat(64),
        "params_schema":{"type":"object"},"inputs":{},"outputs":{"data":"dataset"},
        "runtime":{"executor":"container","lane":"cpu","command":["/bin/true"]},
        "limits":{"max_attempts":1,"timeout_seconds":30},
        "reporting":{"contracts":{"dataset":reporting::reference(&contract)},"capabilities":["dataset_statistics"]}
    });
    reporting::bind_profile(&f.app.db, &mut profile).unwrap();
    f.app.config.profiles.insert("reference.r1".into(), profile);
    let mut unit = json!({"key":"reference","kind":"reference.r1","owner":"owner","notify":"owner","params":{}});
    assert_eq!(
        f.call("units", "submit", unit.clone()).unwrap_err().code,
        "task_required"
    );
    assert_eq!(f.count("jobs"), 0);
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"reference-task","definition":{"label":"Grasp"}}),
        )
        .unwrap();
    unit["scenario_ref"] = json!({"id":task["id"],"revision":1});
    let accepted = f.call("units", "submit", unit.clone()).unwrap();
    assert_eq!(f.call("units", "submit", unit.clone()).unwrap(), accepted);
    let spec = jobs::load(&f.app.db, accepted["id"].as_str().unwrap())
        .unwrap()
        .spec;
    assert_eq!(spec["scenario_ref"]["id"], task["id"]);
    let smoke = f.call("units","submit",json!({"key":"smoke","kind":"cpu.smoke","owner":"owner","notify":"owner","params":{"seconds":0,"fail":false}})).unwrap();
    assert!(jobs::load(&f.app.db, smoke["id"].as_str().unwrap())
        .unwrap()
        .spec["scenario_ref"]
        .is_null());
}

#[test]
fn new_catalog_admission_requires_exact_dataset_and_policy_under_the_task() {
    let mut f = Fixture::new();
    let job = f.submit("typed-input-producer");
    for (id, kind, task) in [
        ("d1", "dataset", "square"),
        ("d2", "dataset", "other-task"),
        ("policy", "model", "square"),
    ] {
        let manifest = json!({"id":id,"kind":kind,"scenario_ref":{"id":task,"revision":1}});
        f.app
            .db
            .execute(
                "INSERT INTO entity_revisions VALUES(?,1,?,?,'owner','owner',?)",
                rusqlite::params![id, job, kind, manifest.to_string()],
            )
            .unwrap();
    }
    let mut spec = json!({
        "scenario_ref":{"id":"square","revision":2},
        "profile":{"outputs":{"result":"experiment"},"reporting":{"input_kinds":{"dataset":"dataset"}}},
        "params":{"dataset":{"artifact":{"id":"d1","revision":1}}}
    });
    let check = |spec: &Value| reporting::validate_catalog_admission(&f.app.db, spec);
    assert_eq!(check(&spec).unwrap_err().code, "catalog_input_required");
    spec["profile"]["reporting"]["input_kinds"]["policy"] = json!("model");
    spec["params"]["policy"] = json!({"artifact":{"id":"policy","revision":1}});
    check(&spec).unwrap();
    spec["params"]["dataset"]["artifact"]["id"] = json!("d2");
    assert_eq!(check(&spec).unwrap_err().code, "catalog_input_task");

    // Revision 1 receipts omit registry_ref. Use the published entity's Task
    // context even if the producing execution predates Task pins.
    let published_id = format!("{job}.data");
    f.app
        .db
        .execute(
            "INSERT INTO entity_revisions VALUES(?,1,?,'dataset','owner','owner',?)",
            rusqlite::params![
                published_id,
                job,
                json!({"scenario_ref":{"id":"square","revision":1}}).to_string()
            ],
        )
        .unwrap();
    f.app
        .db
        .execute(
            "UPDATE jobs SET result=? WHERE id=?",
            rusqlite::params![
                json!({"entities":[{"id":"data","kind":"dataset"}]}).to_string(),
                job
            ],
        )
        .unwrap();
    spec["params"]["dataset"] = json!({"job":job,"entity":"data"});
    check(&spec).unwrap();
}

#[test]
fn new_dynamic_outputs_cannot_bypass_reporting_and_old_accepted_outputs_are_preserved() {
    let mut f = Fixture::new();
    let job = f.submit("dynamic-output");
    let mut spec = jobs::load(&f.app.db, &job).unwrap().spec;
    assert_eq!(spec["_reporting_required"], true);
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='processing',attempt='legacy-attempt' WHERE id=?",
            [&job],
        )
        .unwrap();
    let folder = f.app.bulk.join("legacy-output");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("data.json"), "{}").unwrap();
    let m = json!({"entities":[{"id":"data","kind":"dataset","path":"data.json","relations":[]}],"metrics":[]});
    assert_eq!(
        f.app
            .publish_result(&job, "legacy-attempt", m.clone(), &folder)
            .unwrap_err()
            .code,
        "report_profile_legacy"
    );
    assert_eq!(f.count("entity_revisions"), 0);
    // A source accepted before this policy has no marker and is not retrofitted.
    spec.as_object_mut().unwrap().remove("_reporting_required");
    f.app
        .db
        .execute(
            "UPDATE jobs SET spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), job],
        )
        .unwrap();
    f.app
        .publish_result(&job, "legacy-attempt", m, &folder)
        .unwrap();
}

#[test]
fn policy_reports_preserve_mechanical_dof_action_channels_and_squared_error_units() {
    let mut f = Fixture::new();
    let base = builtin(&mut f, "loom.report.policy_import");
    let training = builtin(&mut f, "loom.report.policy");
    let metric = training["definition"]["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role"] == "action_metrics")
        .unwrap()
        .clone();
    let c = registered(
        &mut f,
        "fixture.policy_errors",
        json!({"label":"Policy with channel errors","activity":"policy_import","output_kind":"model","base_ref":reporting::reference(&base),"requirements":[metric],"adaptations":[]}),
    );
    let channels = json!([{"index":0,"name":"left_joint1","kind":"joint_position","unit":"rad","joint":"left_joint1"}]);
    let instructions = json!({"mode":"fixed","texts":[{"id":"task","text":"Grasp the target object."}],"assignments":[{"selector":"all","instruction_id":"task"}],"coverage":{"total":10,"assigned":10},"tokenizer":"pinned processor"});
    let inputs = json!({"inputs":[],"checkpoint":{"step":20,"sha256":hash("{}")},"normalization":"train split only","image_processing":"RGB crop","state_dimension":1,"action_dimension":1,"chunk_size":1,"robot_dof":null,"controlled_joint_dof":1,"action_channels":channels,"action_representation":"joint_position"});
    let mse = training["definition"]["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["role"] == "action_metrics")
        .unwrap()["definitions"][1]
        .clone();
    let errors = json!({"inputs":[],"channels":channels,"representation":"joint_position","normalization":{"mode":"physical","definition":"inverse train normalization","fitted_split":"train"},"measurements":[{"definition_ref":mse,"value":0.04,"population":10,"split":"test","phase":"all","channel_index":0,"error":"mse","unit":"rad"}]});
    let (job, folder, mut manifest) = fixture_report(
        &mut f,
        &c,
        &[
            ("instructions", instructions),
            ("inputs", inputs),
            ("action_metrics", errors),
        ],
    );
    assert!(f
        .app
        .publish_result(&job, "report-attempt", manifest.clone(), &folder)
        .is_err());
    let path = folder.join("action_metrics.json");
    let mut report: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    report["data"]["measurements"][0]["unit"] = json!("rad²");
    let bytes = serde_json::to_vec(&report).unwrap();
    fs::write(path, &bytes).unwrap();
    manifest["entities"][0]["reporting"]["reports"][2]["sha256"] = json!(hash(bytes));
    f.app
        .publish_result(&job, "report-attempt", manifest, &folder)
        .unwrap();
    let e = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.data"),"revision":1}),
        )
        .unwrap();
    assert!(e["reporting"]["reports"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r["fields"].as_array().unwrap())
        .any(|f| f["id"] == "robot_dof" && f["state"] == "missing"));
}

#[test]
fn evaluation_reports_recompute_predicate_and_do_not_accept_a_training_or_comparison_record() {
    let mut f = Fixture::new();
    let base = builtin(&mut f, "loom.report.evaluation");
    let metric = f
        .call(
            "results",
            "get_report_definition",
            json!({"id":"loom.metric.success","revision":1}),
        )
        .unwrap();
    let protocol=f.call("results","register_report_definition",json!({"id":"fixture.protocol","key":"protocol","reason":"Test geometric task","definition":{"kind":"protocol","label":"Alignment success","mode":"closed_loop","environment":"sim","controller":"hybrid","scope":{"evaluator":{"kind":"producer"},"phases":[{"id":"align","controller":"hybrid"}],"gt_access":{"policy":false,"planner":true,"scoring":true},"policy_active_during_hold":false,"physics_hz":60,"control_hz":30,"goal_frame":"world XY"},"primary_metric_ref":reporting::reference(&metric),"predicate":{"all":[{"pointer":"/measurements/distance","op":"lte","value":5,"unit":"mm"}]}}})).unwrap();
    let mut definition = base["definition"].clone();
    definition["requirements"][0]["definitions"]
        .as_array_mut()
        .unwrap()
        .push(reporting::reference(&protocol));
    let c = registered(&mut f, "fixture.evaluation", definition);
    let data = json!({"inputs":[],"protocol_ref":reporting::reference(&protocol),"counts":{"planned":1,"executed":1,"unexecuted":0,"valid":1,"successes":1,"infra":0,"invalid":0},"cases":[{"id":"case1","seed":1,"valid":true,"outcome":"success","failure_stage":null,"factors":{},"measurements":{"distance":{"value":6,"unit":"mm"}}}]});
    let (job, folder, mut m) = fixture_report(&mut f, &c, &[("evaluation", data)]);
    assert!(f
        .app
        .publish_result(&job, "report-attempt", m.clone(), &folder)
        .is_err());
    let path = folder.join("evaluation.json");
    let mut report: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    report["data"]["cases"][0]["measurements"]["distance"]["value"] = json!(4);
    let bytes = serde_json::to_vec(&report).unwrap();
    fs::write(path, &bytes).unwrap();
    m["entities"][0]["reporting"]["reports"][0]["sha256"] = json!(hash(bytes));
    f.app
        .publish_result(&job, "report-attempt", m, &folder)
        .unwrap();
    let e = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.data"),"revision":1}),
        )
        .unwrap();
    assert_eq!(e["reporting"]["primary_outcome"]["numerator"], 1);
}

#[test]
fn checkpoint_evaluation_keeps_paired_cases_and_a_separate_representative_score() {
    for mode in ["declared", "mismatched", "legacy"] {
        let mismatch = mode == "mismatched";
        let legacy = mode == "legacy";
        let mut f = Fixture::new();
        let base = builtin(&mut f, "loom.report.checkpoint_evaluation");
        let plain = builtin(&mut f, "loom.report.evaluation");
        assert!(
            plain["definition"]["requirements"][0]["schema"]["properties"]["checkpoint_selection"]
                .is_null()
        );
        let metric = f
            .call(
                "results",
                "get_report_definition",
                json!({"id":"loom.metric.success","revision":1}),
            )
            .unwrap();
        let protocol=f.call("results","register_report_definition",json!({"id":"fixture.checkpoint_protocol","key":"protocol","reason":"Test paired resets","definition":{"kind":"protocol","label":"Goal reached","mode":"closed_loop","environment":"sim","controller":"policy","scope":{"evaluator":{"kind":"producer"},"phases":[{"id":"reach","controller":"policy"}],"gt_access":{"policy":false,"planner":false,"scoring":true},"policy_active_during_hold":null,"physics_hz":60,"control_hz":30,"goal_frame":"world XY"},"primary_metric_ref":reporting::reference(&metric),"predicate":{"all":[{"pointer":"/measurements/distance","op":"lte","value":5,"unit":"mm"}]}}})).unwrap();
        let mut definition = if legacy { &plain } else { &base }["definition"].clone();
        definition["requirements"][0]["definitions"]
            .as_array_mut()
            .unwrap()
            .push(reporting::reference(&protocol));
        let contract = registered(&mut f, "fixture.checkpoints", definition);
        let cases: Vec<_> = [(10, "a", 9), (20, "b", 3)].iter().map(|(step, pin, measured)| json!({
            "id":format!("step-{step}"),"seed":1,"valid":true,"outcome":if *measured<=5{"success"}else{"failure"},"failure_stage":null,
            "factors":{"update":step,"checkpoint_sha256":pin.repeat(64),"reset_state_sha256":if mismatch&&*step==20{"d"}else{"c"}.repeat(64)},
            "measurements":{"distance":{"value":measured,"unit":"mm"}}
        })).collect();
        let mut data = json!({"inputs":[],"protocol_ref":reporting::reference(&protocol),"counts":{"planned":2,"executed":2,"unexecuted":0,"valid":2,"successes":1,"infra":0,"invalid":0},"cases":cases});
        if !legacy {
            data["checkpoint_selection"] =
                json!({"split":"development","rule":"best_success_rate","selected_step":null});
        }
        let (job, folder, mut manifest) =
            fixture_report(&mut f, &contract, &[("evaluation", data)]);
        if legacy {
            f.app.db.execute("UPDATE jobs SET spec=json_set(spec,'$.params.updates',json('[10,20]')) WHERE id=?",[&job]).unwrap();
            manifest["entities"][0]["metadata"] =
                json!({"research":{"evaluation":{"split":"development matched resets"}}});
        }
        let result = f
            .app
            .publish_result(&job, "report-attempt", manifest, &folder);
        if mismatch {
            assert_eq!(result.unwrap_err().code, "report_contract");
            continue;
        }
        result.unwrap();
        let id = format!("{job}.data");
        let mut e = f
            .call("results", "get", json!({"id":id,"revision":1}))
            .unwrap();
        if legacy {
            assert_eq!(e["reporting"]["primary_outcome"]["denominator"], 2);
            let catalog = f
                .app
                .dashboard_catalog_selected(Some(&[
                    json!({"native_ref":{"kind":"entity","identity":{"id":id,"revision":1}}}),
                ]))
                .unwrap();
            e = catalog["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap()
                .clone();
        }
        assert!(e["reporting"]["primary_outcome"].is_null());
        assert_eq!(e["reporting"]["evaluation_summary"]["counts"]["valid"], 2);
        let group = &e["reporting"]["checkpoint_sweep"];
        assert_eq!(
            group["selected_step"],
            if legacy { Value::Null } else { json!(20) }
        );
        assert_eq!(group["best_steps"], json!([20]));
        assert_eq!(group["checkpoints"][1]["successes"], 1);
        assert_eq!(group["checkpoints"][1]["valid"], 1);
    }
}

#[test]
fn checkpoint_supplement_preserves_exact_sources_and_recomputes_the_series() {
    let mut f = Fixture::new();
    let template = builtin(&mut f, "loom.report.checkpoint_evaluation");
    let metric = f
        .call(
            "results",
            "get_report_definition",
            json!({"id":"loom.metric.success","revision":1}),
        )
        .unwrap();
    let protocol=f.call("results","register_report_definition",json!({"id":"fixture.supplement_protocol","key":"supplement-protocol","reason":"Test matched evaluation",
        "definition":{"kind":"protocol","label":"Goal reached","mode":"closed_loop","environment":"sim","controller":"policy",
        "scope":{"evaluator":{"kind":"producer"},"phases":[{"id":"reach","controller":"policy"}],"gt_access":{"policy":false,"planner":false,"scoring":true},"policy_active_during_hold":null,"physics_hz":60,"control_hz":30,"goal_frame":"world XY"},
        "primary_metric_ref":reporting::reference(&metric),"predicate":{"all":[{"pointer":"/measurements/distance","op":"lte","value":5,"unit":"mm"}]}}})).unwrap();
    let mut definition = template["definition"].clone();
    definition["requirements"][0]["definitions"]
        .as_array_mut()
        .unwrap()
        .push(reporting::reference(&protocol));
    let contract = registered(&mut f, "fixture.supplement", definition);
    let upstream = f.submit("evaluation-inputs");
    for (id, kind) in [
        ("cohort", "dataset"),
        ("policy-a", "model"),
        ("policy-b", "model"),
    ] {
        let body = json!({"id":id,"revision":1,"kind":kind,"components":[]});
        f.app
            .db
            .execute(
                "INSERT INTO entity_revisions VALUES(?,1,?,?,'owner','owner',?)",
                rusqlite::params![id, upstream, kind, body.to_string()],
            )
            .unwrap();
    }
    let run = |f: &mut Fixture,
               key: &str,
               step: u64,
               sha: &str,
               reset: &str,
               policy: &str,
               success: bool,
               select: bool| {
        let params = json!({"dataset":{"artifact":{"id":"cohort","revision":1}},"policy":{"artifact":{"id":policy,"revision":1}}});
        let inputs = crate::results::input_relations(&f.app.db, &params).unwrap();
        let row = json!({"id":format!("step-{step}"),"seed":1,"valid":true,"outcome":if success {"success"}else{"failure"},"failure_stage":null,
            "factors":{"update":step,"checkpoint_sha256":sha.repeat(64),"reset_state_sha256":reset.repeat(64)},
            "measurements":{"distance":{"value":if success {3}else{9},"unit":"mm"}}});
        let mut data = json!({"inputs":inputs,"protocol_ref":reporting::reference(&protocol),"cases":[row],
            "counts":{"planned":1,"executed":1,"unexecuted":0,"valid":1,"successes":u8::from(success),"infra":0,"invalid":0}});
        if select {
            data["checkpoint_selection"] =
                json!({"split":"development","rule":"best_success_rate","selected_step":null});
        }
        let batch = fixture_report_for(f, key, &contract, &[("evaluation", data)]);
        f.app
            .db
            .execute(
                "UPDATE jobs SET spec=json_set(spec,'$.params',json(?)) WHERE id=?",
                rusqlite::params![params.to_string(), batch.0],
            )
            .unwrap();
        batch
    };
    let (first, folder, manifest) = run(
        &mut f,
        "original-evaluation",
        10,
        "a",
        "c",
        "policy-a",
        false,
        true,
    );
    f.app
        .publish_result(&first, "report-attempt", manifest, &folder)
        .unwrap();
    let id = format!("{first}.data");
    f.app.db.execute("INSERT INTO reviewed_outputs VALUES('entity','experiment',?,'1','','regular','Reviewed publication')",[&id]).unwrap();
    let original = f
        .call("results", "get", json!({"id":id,"revision":1}))
        .unwrap();
    assert_eq!(original["catalog_class"], "regular");
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"supplement-task","definition":{"label":"Checkpoint evaluation"}}),
        )
        .unwrap();
    let admission=f.call("jobs","submit_research",json!({"key":"accept-supplement","owner":"owner","notify":"owner","scenario_ref":{"id":task["id"],"revision":1},
        "outcome":{"goal":"Evaluate more checkpoints","deliverables":[{"alias":"evaluation","kind":"experiment","from":{"unit":"evaluate","entity":"data"},"supplements":{"id":id,"revision":1}}]},
        "plan":{"sealed":false,"units":[{"id":"evaluate","label":"Evaluate"}]}})).unwrap();
    let root = admission["id"].as_str().unwrap();
    let (unit, folder, manifest) = run(
        &mut f,
        "supplement-evaluation",
        20,
        "b",
        "c",
        "policy-a",
        true,
        true,
    );
    f.app
        .db
        .execute(
            "INSERT INTO execution_units VALUES(?,?,'evaluate','Evaluate','run')",
            rusqlite::params![unit, root],
        )
        .unwrap();
    let outcome = f
        .app
        .publish_result(&unit, "report-attempt", manifest, &folder)
        .unwrap();
    let exact = json!({"id":id,"revision":2});
    assert_eq!(outcome["entities"][0]["registry_ref"], exact);
    let latest = f.call("results", "get", exact.clone()).unwrap();
    assert_eq!(latest["reporting"]["checkpoint_sweep"]["selected_step"], 20);
    assert_eq!(
        latest["reporting"]["evaluation_summary"]["counts"]["valid"],
        2
    );
    assert_eq!(latest["producer"]["job_id"], root);
    assert_eq!(
        f.call("results", "get", json!({"id":id,"revision":1}))
            .unwrap(),
        original
    );
    let read=f.call("results","read_report",json!({"id":id,"revision":2,"requirement_id":"evaluation","pointer":"/cases","offset":1,"limit":1})).unwrap();
    assert_eq!(read["verification"], "source_sha256_verified");
    assert_eq!(read["total"], 2);
    assert_eq!(read["value"][0]["factors"]["update"], 20);
    assert_eq!(
        crate::results::input_relations(
            &f.app.db,
            &json!({"evaluation":{"job":unit,"entity":"data"}})
        )
        .unwrap()[0]["id"],
        id
    );
    assert!(crate::experiment_supplements::target(
        &f.app.db,
        &json!({"id":id,"revision":1}),
        "owner"
    )
    .is_err());
    let (wrong, wrong_folder, wrong_manifest) = run(
        &mut f,
        "mismatched-reset",
        30,
        "d",
        "e",
        "policy-a",
        true,
        true,
    );
    let next=f.call("jobs","submit_research",json!({"key":"accept-invalid-batch","owner":"owner","notify":"owner","scenario_ref":{"id":task["id"],"revision":1},
        "outcome":{"goal":"Evaluate another checkpoint","deliverables":[{"alias":"evaluation","kind":"experiment","from":{"unit":"evaluate","entity":"data"},"supplements":exact}]},
        "plan":{"sealed":false,"units":[{"id":"evaluate","label":"Evaluate"}]}})).unwrap();
    f.app
        .db
        .execute(
            "INSERT INTO execution_units VALUES(?,?,'evaluate','Evaluate','run')",
            rusqlite::params![wrong, next["id"].as_str()],
        )
        .unwrap();
    let rejected = f
        .app
        .publish_result(&wrong, "report-attempt", wrong_manifest, &wrong_folder)
        .unwrap_err();
    assert_eq!(rejected.status, 409);
    assert_eq!(f.call("results", "get", exact.clone()).unwrap(), latest);
    let conflicting = run(
        &mut f,
        "other-policy-evaluation",
        20,
        "d",
        "c",
        "policy-b",
        true,
        true,
    );
    let other = format!("{}.data", conflicting.0);
    f.app
        .publish_result(
            &conflicting.0,
            "report-attempt",
            conflicting.2,
            &conflicting.1,
        )
        .unwrap();
    f.app.db.execute("INSERT INTO reviewed_outputs VALUES('entity','experiment',?,'1','','regular','Reviewed publication')",[&other]).unwrap();
    let args = json!({"key":"matched-comparison","label":"Checkpoint study","experiments":[exact,{"id":other,"revision":1}]});
    let group = f
        .call("results", "group_evaluations", args.clone())
        .unwrap();
    assert_eq!(group, f.call("results", "group_evaluations", args).unwrap());
    assert_eq!(group["policies"].as_array().unwrap().len(), 2);
    assert_eq!(
        f.call("results", "get", json!({"id":id,"revision":2}))
            .unwrap()["evaluation_group"],
        group
    );
    for _ in 0..8 {
        f.app.index_once().unwrap();
    }
    let page=f.call("system","dashboard_page",json!({"kinds":["experiment"],"catalog_classes":["regular"],"view":"all","role":"all","records_only":true})).unwrap();
    let listed: Vec<_> = page["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["id"] == id)
        .collect();
    assert_eq!(listed.len(), 1, "{page}");
    assert_eq!(listed[0]["revision"], 2);
}

#[test]
fn unit_catalog_records_keep_executing_producer_and_class() {
    for member in [false, true] {
        let mut f = Fixture::new();
        let contract = builtin(&mut f, "loom.report.dataset");
        let (unit, folder, manifest) = fixture_report(&mut f, &contract, &[("membership", data())]);
        let job = member.then_some("research-fixture");
        if let Some(job) = job {
            let request = json!({"outcome":{"deliverables":[{"alias":"dataset","kind":"dataset","from":{"node":"produce","entity":"data"}}]}});
            f.app.db.execute("INSERT INTO job_workflows(id,submitter,key,request,owner,notify,created,state) VALUES(?,'owner','k',?,'owner','owner',0,'queued')",
                             rusqlite::params![job,request.to_string()]).unwrap();
            f.app
                .db
                .execute("INSERT INTO research_jobs VALUES(?)", [job])
                .unwrap();
        }
        f.app
            .db
            .execute(
                "INSERT INTO execution_units VALUES(?,?,'produce','Produce','run')",
                rusqlite::params![unit, job],
            )
            .unwrap();
        f.app
            .publish_result(&unit, "report-attempt", manifest, &folder)
            .unwrap();
        let published = f
            .call(
                "results",
                "get",
                json!({"id":format!("{unit}.data"),"revision":1}),
            )
            .unwrap();
        assert_eq!(
            published["catalog_class"],
            if member { "regular" } else { "reference" }
        );
        assert_eq!(published["producer"]["unit_id"], unit);
        assert_eq!(published["producer"]["job_id"], json!(job));
        assert_eq!(published["producer"]["attempt_id"], "report-attempt");
    }
}

#[test]
fn direct_scientific_output_keeps_the_accepted_job_identity_without_a_group() {
    let mut f = Fixture::new();
    let contract = builtin(&mut f, "loom.report.dataset");
    let template = f
        .call(
            "results",
            "get_metadata_template",
            json!({"id":"loom.dataset","revision":1}),
        )
        .unwrap();
    let (job, folder, manifest) = fixture_report(&mut f, &contract, &[("membership", data())]);
    let mut spec = jobs::load(&f.app.db, &job).unwrap().spec;
    spec["outcome"] = json!({"goal":"Import dataset","deliverables":[{"alias":"cohort","entity":"data","kind":"dataset"}],
        "science":{"deliverables":{"cohort":{"activity":"dataset_import","template_ref":crate::metadata_templates::reference(&template),
        "contract_ref":reporting::reference(&contract)}}}});
    spec["science_digest"] = f
        .call(
            "jobs",
            "preview_outcome",
            json!({"outcome":spec["outcome"]}),
        )
        .unwrap()["science_digest"]
        .clone();
    f.app
        .db
        .execute(
            "UPDATE jobs SET spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), job],
        )
        .unwrap();
    f.app
        .publish_result(&job, "report-attempt", manifest, &folder)
        .unwrap();
    let published = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.data"),"revision":1}),
        )
        .unwrap();
    assert_eq!(published["science"]["outcome"], job);
    assert_eq!(published["science"]["digest"], spec["science_digest"]);
    assert!(published["science"]["deliverables"]["cohort"].is_object());
    assert_eq!(f.count("job_workflows"), 0);
}

#[test]
fn outcome_preview_and_publication_bind_hypothesis_and_verified_setup_sessions() {
    let mut f = Fixture::new();
    let contract = builtin(&mut f, "loom.report.dataset");
    let template = f
        .call(
            "results",
            "get_metadata_template",
            json!({"id":"loom.dataset","revision":1}),
        )
        .unwrap();
    let pin = json!({"id":"setup","revision":1,"sha256":"a".repeat(64)});
    let setup = json!({"id":"setup","kind":"artifact","components":[{"ref":"setup.json","sha256":pin["sha256"]}],
        "metadata":{"experimental_setup":{"domain":"simulation"}}});
    f.app
        .db
        .execute(
            "INSERT INTO asset_revisions VALUES('setup',1,NULL,'artifact','owner','owner',?)",
            [setup.to_string()],
        )
        .unwrap();
    let mut request = json!({"key":"science-preview","owner":"owner","notify":"owner","outcome":{"goal":"Collect a demonstrated cohort",
        "deliverables":[{"alias":"dataset","kind":"dataset","from":{"node":"record","entity":"data"}}],
        "science":{"deliverables":{"dataset":{"activity":"dataset_import","template_ref":crate::metadata_templates::reference(&template),
            "contract_ref":reporting::reference(&contract),"sessions":[{"id":"sim1","domain":"simulation","configuration":pin}]}}}},
        "plan":{"sealed":false,"units":[{"id":"record","label":"Record"}]}});
    let preview = f
        .call(
            "jobs",
            "preview_outcome",
            json!({"outcome":request["outcome"]}),
        )
        .unwrap();
    assert_eq!(preview["mode"], "exploratory");
    assert_eq!(
        preview["coverage"]["dataset"]["requirements"][0]["id"],
        "membership"
    );
    assert_eq!(f.count("job_workflows"), 0);
    let mut wrong = request["outcome"].clone();
    wrong["science"]["deliverables"]["dataset"]["sessions"][0]["domain"] = json!("real");
    assert!(f
        .call("jobs", "preview_outcome", json!({"outcome":wrong}))
        .is_err());
    request["outcome"]["science"]["hypothesis"] = json!({"statement":"Cohort reaches the target","analysis":"Compare recorded cases","population":"Accepted members","metric":"coverage","aliases":["dataset"]});
    let preview = f
        .call(
            "jobs",
            "preview_outcome",
            json!({"outcome":request["outcome"]}),
        )
        .unwrap();
    assert_eq!(
        f.call("jobs", "create", request.clone()).unwrap_err().code,
        "task_required"
    );
    let task = f
        .call(
            "results",
            "register_scenario",
            json!({"key":"science-task","definition":{"label":"Collect demonstrations"}}),
        )
        .unwrap();
    request["scenario_ref"] = json!({"id":task["id"],"revision":1});
    let opened = f.call("jobs", "create", request.clone()).unwrap();
    assert_eq!(opened["science_digest"], preview["science_digest"]);
    request["outcome"]["science"]["hypothesis"]["metric"] = json!("changed");
    assert_eq!(
        f.call("jobs", "create", request).unwrap_err().code,
        "idempotency_conflict"
    );

    // Reuse the existing report-publication fixture to simulate the accepted
    // producer after its execution has returned.
    let (job, folder, mut manifest) = fixture_report(&mut f, &contract, &[("membership", data())]);
    f.app
        .db
        .execute(
            "INSERT INTO workflow_nodes VALUES(?,'record','Record',?)",
            rusqlite::params![opened["id"].as_str(), job],
        )
        .unwrap();
    assert!(f
        .app
        .publish_result(&job, "report-attempt", manifest.clone(), &folder)
        .is_err());
    let mut evidence = json!({"schema":"loom-science-evidence/v1","job":job,"attempt":"wrong",
        "alias":"dataset","science_digest":opened["science_digest"],
        "analysis":{"metric":"coverage","summary":"Target was not reached"},
        "sessions":[{"id":"sim1","domain":"simulation","configuration":pin,"members":["e1"],"readback":{"physics_step_hz":120}}]});
    assert_eq!(f.count("entity_revisions"), 0);
    manifest["entities"][0]["components"]
        .as_array_mut()
        .unwrap()
        .push(json!("science.json"));
    for valid in [false, true] {
        if valid {
            evidence["attempt"] = json!("report-attempt");
        }
        let bytes = serde_json::to_vec(&evidence).unwrap();
        fs::write(folder.join("science.json"), &bytes).unwrap();
        manifest["entities"][0]["metadata"]["science_evidence"]["dataset"] =
            json!({"component":"science.json","sha256":hash(bytes)});
        let publication = f
            .app
            .publish_result(&job, "report-attempt", manifest.clone(), &folder);
        if valid {
            publication.unwrap();
        } else {
            assert_eq!(publication.unwrap_err().code, "science");
        }
    }
    let result = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.data"),"revision":1}),
        )
        .unwrap();
    let note = json!({"id":opened["id"],"annotation_id":"later-analysis","reason":"Interpret the first cohort",
        "evidence":{"output":{"id":format!("{job}.data"),"revision":1},"members":["e1"]}});
    let first = f.call("results", "annotate", note.clone()).unwrap();
    assert_eq!(f.call("results", "annotate", note).unwrap(), first);
    let accepted = f.call("jobs", "get", json!({"id":opened["id"]})).unwrap();
    assert_eq!(accepted["annotations"][0], first);
    assert_eq!(accepted["spec"]["science_digest"], opened["science_digest"]);
    assert_eq!(result["science"]["digest"], opened["science_digest"]);
    assert_eq!(
        result["science"]["deliverables"]["dataset"]["sessions"][0]["readback"]["physics_step_hz"],
        120
    );
}
