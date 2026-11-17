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
    let job = f.submit("report-publication");
    let folder = f.app.bulk.join("report-publication");
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
