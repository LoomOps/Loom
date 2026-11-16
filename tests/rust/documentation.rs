use super::*;

#[test]
fn guidance_identity_tracks_every_distributed_instruction_surface() {
    let baseline = guide_revision(DOCUMENTS, PACKAGED, CATALOG, ACTION_DOCS, JOB_CONTRACT);
    assert_eq!(baseline, identity()["guide_revision"]);
    for index in 0..DOCUMENTS.len() {
        let mut docs = DOCUMENTS.to_vec();
        let mut text = docs[index].2.to_string();
        text.push_str("\nA changed example.\n");
        docs[index].2 = &text;
        assert_ne!(
            baseline,
            guide_revision(&docs, PACKAGED, CATALOG, ACTION_DOCS, JOB_CONTRACT)
        );
        docs[index] = (
            DOCUMENTS[index].0,
            "Changed retrieval title",
            DOCUMENTS[index].2,
        );
        assert_ne!(
            baseline,
            guide_revision(&docs, PACKAGED, CATALOG, ACTION_DOCS, JOB_CONTRACT)
        );
    }
    for index in 0..PACKAGED.len() {
        let mut files = PACKAGED.to_vec();
        let changed = format!("{}\nChanged packaged instruction", files[index].1);
        files[index].1 = &changed;
        assert_ne!(
            baseline,
            guide_revision(DOCUMENTS, &files, CATALOG, ACTION_DOCS, JOB_CONTRACT)
        );
    }
    for (catalog, actions, job) in [
        (
            format!("{CATALOG} "),
            ACTION_DOCS.into(),
            JOB_CONTRACT.into(),
        ),
        (
            CATALOG.into(),
            format!("{ACTION_DOCS} "),
            JOB_CONTRACT.into(),
        ),
        (
            CATALOG.into(),
            ACTION_DOCS.into(),
            format!("{JOB_CONTRACT} "),
        ),
    ] {
        assert_ne!(
            baseline,
            guide_revision(DOCUMENTS, PACKAGED, &catalog, &actions, &job)
        );
    }
}

#[test]
fn native_reference_covers_every_action_with_valid_examples_and_ownership() {
    let actor = crate::auth::person_actor("documentation-fixture");
    let docs: Value = serde_json::from_str(ACTION_DOCS).unwrap();
    assert_eq!(docs.as_object().unwrap().len(), contracts::actions().len());
    for action in contracts::actions() {
        let doc = describe(&actor, &action.group, &action.action).unwrap();
        contracts::validate(&action.schema, &doc["example"])
            .unwrap_or_else(|error| panic!("{}.{}: {error:?}", action.group, action.action));
        assert!(doc["implementation"]["entrypoint"]
            .as_str()
            .unwrap()
            .starts_with("rust/"));
        assert!(doc["description"].as_str().unwrap().len() > 20);
    }
}

#[test]
fn reader_reference_never_advertises_mutating_actions() {
    let mut reader = crate::auth::person_actor("reader");
    reader.scopes = vec!["read".into()];
    let reference = reference(&reader).unwrap();
    for group in reference["groups"].as_array().unwrap() {
        for action in group["actions"].as_array().unwrap() {
            assert_eq!(action["scope"], "read");
        }
    }
    assert!(describe(&reader, "jobs", "submit").is_err());
    assert!(describe(&reader, "results", "reconcile_apply").is_err());
}

#[test]
fn guide_catalog_routes_exactly_to_compiled_content_and_revision_cache() {
    let index = read(&json!({})).unwrap();
    assert_eq!(
        index["documents"].as_array().unwrap().len(),
        DOCUMENTS.len()
    );
    for doc in index["documents"].as_array().unwrap() {
        assert!(doc["summary"].is_string());
        assert!(doc["type"].is_string());
        let text = read(&json!({"id":doc["id"]})).unwrap();
        assert_eq!(text["title"], doc["title"]);
        assert!(!text["content"].as_str().unwrap().is_empty());
    }
    assert_eq!(
        read(&json!({"known_revision":index["manifest"]["revision"]})).unwrap()["not_modified"],
        true
    );
    assert!(read(&json!({"known_revision":"old-guidance"})).unwrap()["documents"].is_array());
    assert!(read(&json!({"id":"../INSTALL.md"})).is_err());
}

#[test]
fn contract_identity_tracks_action_and_job_contract_changes_separately_from_guides() {
    let actions = json!(contracts::actions());
    let baseline = contract_revision(&actions, JOB_CONTRACT);
    assert_eq!(baseline, identity()["contract_revision"]);
    let mut job: Value = serde_json::from_str(JOB_CONTRACT).unwrap();
    job["schema"] = json!("changed-job-contract");
    assert_ne!(baseline, contract_revision(&actions, &job.to_string()));
    let mut changed = actions.clone();
    changed[0]["schema"]["properties"]["new_argument"] = json!({"type":"string"});
    assert_ne!(baseline, contract_revision(&changed, JOB_CONTRACT));
    let _changed_guides = guide_revision(
        DOCUMENTS,
        PACKAGED,
        CATALOG,
        "changed action prose",
        JOB_CONTRACT,
    );
    assert_eq!(baseline, contract_revision(&actions, JOB_CONTRACT));
}
