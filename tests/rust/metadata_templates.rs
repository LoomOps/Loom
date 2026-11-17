use super::*;
fn dataset(f: &mut Fixture) -> Value {
    f.call(
        "results",
        "get_metadata_template",
        json!({"id":"loom.dataset","revision":1}),
    )
    .unwrap()
}
fn research() -> Value {
    json!({"robot":{"status":"recorded","model":"Franka Panda"},"collection":{"status":"recorded","count":0,"unit":"episodes","split":{"train":0,"validation":"unrecorded","test":"not_applicable"}}})
}
fn custom(base: &Value) -> Value {
    json!({"label":"Contact demonstrations","kind":"dataset","base_ref":crate::metadata_templates::reference(base),"schema":{"type":"object","properties":{"extensions":{"type":"object","properties":{"friction":{"type":"number","minimum":0}},"required":["friction"],"additionalProperties":false}},"required":["extensions"]},"fields":[{"id":"friction","label":"Friction","pointer":"/extensions/friction","section":"extensions","format":"number"}]})
}

#[test]
fn metadata_templates_are_immutable_shared_revisions_with_common_item_identity() {
    let mut f = Fixture::new();
    let base = dataset(&mut f);
    let first_args = json!({"key":"format-r1","id":"contact.dataset","reason":"Measured contact parameters","definition":custom(&base)});
    let first = f
        .call("results", "register_metadata_template", first_args.clone())
        .unwrap();
    assert_eq!(
        first,
        f.call("results", "register_metadata_template", first_args)
            .unwrap()
    );
    let mut next = custom(&base);
    next["label"] = json!("Contact dataset revised");
    let args = json!({"key":"format-r2","id":"contact.dataset","expected_revision":1,"reason":"Presentation revision","definition":next});
    let second = f
        .call("results", "register_metadata_template", args.clone())
        .unwrap();
    assert_eq!(second["revision"], 2);
    let mut stale = args;
    stale["key"] = json!("stale-write");
    assert!(f
        .call("results", "register_metadata_template", stale)
        .is_err());
    assert_eq!(
        f.call(
            "results",
            "get_metadata_template",
            json!({"id":"contact.dataset","revision":1})
        )
        .unwrap(),
        first
    );
    f.app.cache_catalog_sources().unwrap();
    for _ in 0..6 {
        f.app.index_once().unwrap();
    }
    let a=f.call("results","resolve_item",json!({"native_ref":{"kind":"metadata_template","identity":{"id":"contact.dataset","revision":1}}})).unwrap();
    let b=f.call("results","resolve_item",json!({"native_ref":{"kind":"metadata_template","identity":{"id":"contact.dataset","revision":2}}})).unwrap();
    assert_eq!(a["item_id"], b["item_id"]);
    assert_ne!(a["revision_id"], b["revision_id"]);
}

#[test]
fn metadata_template_types_reject_raw_episode_arrays_and_preserve_unknown_and_zero() {
    let mut f = Fixture::new();
    let t = dataset(&mut f);
    let pin = crate::metadata_templates::reference(&t);
    let args = json!({"kind":"dataset","template_ref":pin,"research":research()});
    assert!(
        f.call("results", "validate_metadata", args.clone())
            .unwrap()["valid"]
            == true
    );
    for status in ["unrecorded", "not_applicable", "multiple"] {
        let mut v = args.clone();
        v["research"]["robot"] = json!({"status":status});
        if status == "multiple" {
            v["research"]["robot"]["models"] = json!(["Robot A", "Robot B"]);
        }
        let result = f.call("results", "validate_metadata", v).unwrap();
        assert!(result["summary"]["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["id"] == "robot" && f["status"] == status));
    }
    for bad in [json!([{"episode":1}]), json!({"episode":1}), json!(-1)] {
        let mut v = args.clone();
        v["research"]["collection"]["count"] = bad;
        assert!(f.call("results", "validate_metadata", v).is_err());
    }
    let mut v = args.clone();
    v["research"]["robot"] = json!({"status":"unrecorded","model":"Invented"});
    assert!(f.call("results", "validate_metadata", v).is_err());
    let mut v = args.clone();
    v["kind"] = json!("model");
    assert!(f.call("results", "validate_metadata", v).is_err());
    let mut v = args.clone();
    v["research"]["quality"] = json!({"bc_ready":false,"positive_coverage":0.25});
    v["research"]["inputs"] = json!([{"kind":"depth_validity","status":"present"}]);
    let result = f.call("results", "validate_metadata", v).unwrap();
    assert!(result["summary"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "bc_ready" && f["value"] == false));
    let mut v = args.clone();
    v["research"]["model"] = json!({"role":"observer"});
    assert!(f.call("results", "validate_metadata", v).is_err());
    let mut v = args;
    v["research"]["extensions"] = json!({"unknown":1});
    assert!(f.call("results", "validate_metadata", v).is_err());
    let mut r = research();
    r["collection"]["status"] = json!("unrecorded");
    assert!(f
        .call(
            "results",
            "validate_metadata",
            json!({"kind":"dataset","template_ref":pin,"research":r})
        )
        .is_err());
    let mut r = research();
    r["organization"] = json!({"path":[{"id":"aligned","label":"Aligned demonstrations"}]});
    assert!(f
        .call(
            "results",
            "validate_metadata",
            json!({"kind":"dataset","template_ref":pin,"research":r})
        )
        .is_ok());
}

#[test]
fn metadata_template_extensions_cannot_weaken_base_or_fetch_external_schemas() {
    let mut f = Fixture::new();
    let base = dataset(&mut f);
    let mut d = custom(&base);
    d["schema"] = json!({"$ref":"https://example.invalid/schema.json"});
    assert!(f
        .call(
            "results",
            "register_metadata_template",
            json!({"id":"bad","key":"bad","reason":"fixture","definition":d})
        )
        .is_err());
    let template = f
        .call(
            "results",
            "register_metadata_template",
            json!({"id":"contact","key":"contact","reason":"fixture","definition":custom(&base)}),
        )
        .unwrap();
    let mut r = research();
    r["extensions"] = json!({"friction":0.0});
    assert!(f.call("results","validate_metadata",json!({"kind":"dataset","template_ref":crate::metadata_templates::reference(&template),"research":r})).is_ok());
    r["robot"] = json!({"status":"recorded"});
    assert!(f.call("results","validate_metadata",json!({"kind":"dataset","template_ref":crate::metadata_templates::reference(&template),"research":r})).is_err());
}

#[test]
fn metadata_template_local_definitions_keep_their_resource_scope_when_extended() {
    let mut f = Fixture::new();
    let base = dataset(&mut f);
    let mut definition = custom(&base);
    definition["schema"] = json!({"$defs":{"coefficient":{"type":"number","minimum":0}},"type":"object","properties":{"extensions":{"type":"object","properties":{"friction":{"$ref":"#/$defs/coefficient"}}}},"required":["extensions"]});
    let t=f.call("results","register_metadata_template",json!({"id":"local.refs","key":"local-refs","reason":"Scoped schema definitions","definition":definition})).unwrap();
    let mut r = research();
    r["extensions"] = json!({"friction":0.2});
    assert!(f.call("results","validate_metadata",json!({"kind":"dataset","template_ref":crate::metadata_templates::reference(&t),"research":r})).is_ok());
}

#[test]
fn metadata_profile_pins_validate_new_outputs_without_rewriting_legacy_profiles() {
    let mut f = Fixture::new();
    let mut profile = json!({});
    crate::metadata_templates::bind_profile(&f.app.db, &mut profile).unwrap();
    let pin = crate::metadata_templates::reference(&profile["metadata_template_pins"]["dataset"]);
    let mut entity = json!({"id":"data","kind":"dataset","label":"Grasp demonstrations","metadata":{"template_ref":pin,"research":research()}});
    assert!(
        crate::metadata_templates::validate_output(&profile, &entity)
            .unwrap()
            .is_some()
    );
    entity["metadata"]["template_ref"]["sha256"] = json!("b".repeat(64));
    assert!(crate::metadata_templates::validate_output(&profile, &entity).is_err());
    assert!(
        crate::metadata_templates::validate_output(&json!({}), &entity)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        crate::metadata_templates::robot_prefix(&research(), "Grasp demonstrations"),
        "Franka Panda · Grasp demonstrations"
    );
    assert_eq!(
        crate::metadata_templates::robot_prefix(&research(), "Franka Panda · Grasp demonstrations"),
        "Franka Panda · Grasp demonstrations"
    );
    let _ = dataset(&mut f);
}

#[test]
fn metadata_adoption_is_source_bound_replayable_and_preserves_original_execution() {
    let mut f = Fixture::new();
    let t = dataset(&mut f);
    let job = f.submit("metadata-source");
    let id = format!("{job}.data");
    let manifest=json!({"id":id,"revision":1,"kind":"dataset","label":"Recorded name","job":job,"metadata":{},"components":[],"relations":[]}).to_string();
    f.app
        .db
        .execute(
            "INSERT INTO entity_revisions VALUES(?,1,?,'dataset','owner','owner',?)",
            rusqlite::params![id, job, manifest],
        )
        .unwrap();
    let jobs = f.count("jobs");
    let attempts = f.count("attempts");
    let target = json!({"kind":"entity","identity":{"id":id,"revision":1}});
    let args = json!({"target":target,"template_ref":crate::metadata_templates::reference(&t),"research":research(),"reason":"Operator-declared exact source normalization"});
    let mut missing_reference = args.clone();
    missing_reference["format_reference"] =
        json!({"kind":"entity","identity":{"id":"missing","revision":1}});
    assert!(f
        .call("results", "adopt_metadata", missing_reference)
        .is_err());
    let preview = f.call("results", "adopt_metadata", args.clone()).unwrap();
    assert_eq!(f.count("rust_annotations"), 0);
    let mut apply = args;
    apply["key"] = json!("metadata-apply");
    apply["dry_run"] = json!(false);
    apply["expected_digest"] = preview["expected_digest"].clone();
    let stored = f.call("results", "adopt_metadata", apply.clone()).unwrap();
    assert_eq!(
        stored,
        f.call("results", "adopt_metadata", apply.clone()).unwrap()
    );
    apply["key"] = json!("stale-preview");
    assert!(f.call("results", "adopt_metadata", apply).is_err());
    let current = f
        .call("results", "get", json!({"id":id,"revision":1}))
        .unwrap();
    assert_eq!(current["label"], "Franka Panda · Recorded name");
    assert_eq!(current["robot"]["model"], "Franka Panda");
    assert_eq!(
        current["metadata_provenance"]["declaration_level"],
        "operator_asserted"
    );
    assert_eq!(current["metadata_provenance"]["target"], target);
    f.app
        .db
        .execute(
            "INSERT INTO research_overlays VALUES('entity',?,?,'owner','Display curation',0)",
            rusqlite::params![id, json!({"label":"Curated display name"}).to_string()],
        )
        .unwrap();
    crate::items::adopt(
        &f.app.db,
        "entity_revisions",
        &json!({"id":id,"revision":1}),
    )
    .unwrap();
    for _ in 0..6 {
        f.app.index_once().unwrap();
    }
    let item = f
        .call("results", "resolve_item", json!({"native_ref":target}))
        .unwrap();
    let item = f
        .call("results", "get_item", json!({"item_id":item["item_id"]}))
        .unwrap();
    assert_eq!(item["label"], "Franka Panda · Curated display name");
    let original: String = f
        .app
        .db
        .query_row(
            "SELECT manifest FROM entity_revisions WHERE id=?",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(original, manifest);
    assert_eq!(f.count("jobs"), jobs);
    assert_eq!(f.count("attempts"), attempts);
    f.app.db.execute("UPDATE entity_revisions SET manifest=json_set(manifest,'$.label','Changed original') WHERE id=?",[&id]).unwrap();
    assert!(
        crate::metadata_templates::annotation(&f.app.db, &contracts::decode(target).unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn legacy_metadata_never_treats_episode_records_as_counts() {
    let summary = crate::research_metadata::summarize(
        &json!({"kind":"dataset","properties":{"episodes":[{"id":1}],"frames":{"a":10},"physics_hz":[120,240]}}),
        &[],
    );
    assert!(!summary["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| ["episodes", "frames"].contains(&f["id"].as_str().unwrap_or(""))));
    assert!(summary["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "physics_hz"));
}

#[test]
fn legacy_capture_count_alternatives_do_not_return_raw_episode_records() {
    let mut f = Fixture::new();
    let job = f.submit("captured-source");
    let id = format!("{job}.data");
    let manifest = json!({"id":id,"revision":1,"kind":"dataset","job":job}).to_string();
    f.app
        .db
        .execute(
            "INSERT INTO entity_revisions VALUES(?,1,?,'dataset','owner','owner',?)",
            rusqlite::params![id, job, manifest],
        )
        .unwrap();
    let target: contracts::Target =
        contracts::decode(json!({"kind":"entity","identity":{"id":id,"revision":1}})).unwrap();
    let key = crate::research_metadata::snapshot_key(&target).unwrap();
    let capture=json!({"schema":"loom-materialized-research/v1","extractor_version":2,"target":target,"source_sha256":crate::store::hash(&manifest),"summary":{"facts":[{"id":"count","label":"Count","section":"collection","status":"recorded","value":9,"alternatives":[{"value":[{"episode":1,"frames":[1,2,3]}]}]}],"inputs":[],"sources":[]}}).to_string();
    f.app
        .db
        .execute(
            "INSERT INTO rust_annotations VALUES('research_metadata',?,?,0)",
            rusqlite::params![key, capture],
        )
        .unwrap();
    let mut record = json!({"kind":"dataset","loom_registry":{"id":id,"revision":1},"research_summary":{"facts":[],"inputs":[],"sources":[]}});
    crate::research_metadata::attach_materialized(&f.app, &mut record).unwrap();
    assert_eq!(record["research_summary"]["facts"][0]["value"], 9);
    assert!(record["research_summary"]["facts"][0]
        .get("alternatives")
        .is_none());
    let original: String = f
        .app
        .db
        .query_row(
            "SELECT body FROM rust_annotations WHERE kind='research_metadata' AND id=?",
            [key],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(original, capture);
}

#[test]
fn template_publication_freezes_robot_name_and_rejects_bad_metadata_before_indexing() {
    let mut f = Fixture::new();
    let job = f.submit("template-output");
    let mut spec = jobs::load(&f.app.db, &job).unwrap().spec;
    let mut profile =
        json!({"revision":"fixture.r1","sha256":"a".repeat(64),"outputs":{"data":"dataset"}});
    crate::metadata_templates::bind_profile(&f.app.db, &mut profile).unwrap();
    spec["profile"] = profile;
    // This fixture exercises a previously accepted metadata-only contract.
    spec.as_object_mut().unwrap().remove("_reporting_required");
    f.app
        .db
        .execute(
            "UPDATE jobs SET state='processing',attempt='template-attempt',spec=? WHERE id=?",
            rusqlite::params![spec.to_string(), job],
        )
        .unwrap();
    let folder = f.app.bulk.join("template-output");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("data.json"), b"{}").unwrap();
    let manifest = json!({"entities":[{"id":"data","kind":"dataset","label":"Grasp demonstrations","path":"data.json","provenance":{},"relations":[],"metadata":{"research":research()}}],"metrics":[]});
    let mut bad = manifest.clone();
    bad["entities"][0]["metadata"]["research"]["collection"]["count"] = json!([{"episode":1}]);
    assert!(f
        .app
        .publish_result(&job, "template-attempt", bad, &folder)
        .is_err());
    assert_eq!(f.count("entity_revisions"), 0);
    f.app
        .publish_result(&job, "template-attempt", manifest, &folder)
        .unwrap();
    let result = f
        .call(
            "results",
            "get",
            json!({"id":format!("{job}.data"),"revision":1}),
        )
        .unwrap();
    assert_eq!(result["label"], "Franka Panda · Grasp demonstrations");
    assert_eq!(result["metadata_status"]["state"], "validated");
    assert!(result["research_summary"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "robot"
            && f["evidence"][0]["pointer"] == "/metadata/research/robot/model"));
    assert_eq!(result["metadata_template"]["id"], "loom.dataset");
}
