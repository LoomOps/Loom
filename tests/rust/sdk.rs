use super::*;
#[test]
fn producer_reports_bind_frozen_coverage_context_exact_inputs_and_bytes() {
    let root = std::env::temp_dir().join(format!("loom-sdk-report-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let pin = json!({"id":"fixture","revision":1,"sha256":"b".repeat(64),"definition":{"requirements":[{"id":"membership"}]}});
    let request:Request=serde_json::from_value(json!({"job":"job1","attempt":"attempt1","output_dir":root,"profile":{"sha256":"a".repeat(64),"reporting_pins":{"output:data":pin}},"report_inputs":[{"id":"exact","revision":1}]})).unwrap();
    let mut p = Producer::new(request).unwrap();
    p.write_artifact("data", "data.json", b"{}").unwrap();
    p.report("data", "membership", json!({"inputs":[],"count":0}))
        .unwrap();
    let binding = &p.entities[0]["reporting"]["reports"][0];
    let raw = fs::read(root.join(binding["component"].as_str().unwrap())).unwrap();
    let report: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(report["producer"]["job"], "job1");
    assert_eq!(report["data"]["inputs"][0]["id"], "exact");
    assert_eq!(binding["sha256"], format!("{:x}", Sha256::digest(raw)));
    assert!(p.report("data", "membership", json!({})).is_err());
    assert!(p.report("data", "excluded", json!({})).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn template_metadata_keeps_pending_file_identity_and_unknown_states() {
    let root = std::env::temp_dir().join(format!("loom-sdk-metadata-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let request: Request = serde_json::from_value(
        json!({"output_dir":root,"profile":{"revision":"fixture.r1","sha256":"a".repeat(64)}}),
    )
    .unwrap();
    let mut p = Producer::new(request).unwrap();
    p.write_artifact("data", "data.json", b"{}").unwrap();
    let before = p.entities[0].clone();
    p.set_entity_metadata("data",json!({"id":"loom.dataset","revision":1,"sha256":"b".repeat(64)}),json!({"robot":{"status":"unrecorded"},"collection":{"status":"recorded","count":0,"unit":"episodes"}})).unwrap();
    for k in ["id", "kind", "path", "provenance"] {
        assert_eq!(p.entities[0][k], before[k]);
    }
    assert_eq!(
        p.entities[0]["metadata"]["research"]["collection"]["count"],
        0
    );
    assert_eq!(fs::read(root.join("data.json")).unwrap(), b"{}");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn dataset_derivation_pins_the_named_input_without_changing_artifact_bytes() {
    let root = std::env::temp_dir().join(format!("loom-sdk-lineage-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("data.json"), b"{}").unwrap();
    let request: Request = serde_json::from_value(
        json!({"output_dir":root,"profile":{"revision":"fixture.r1","sha256":"a".repeat(64)}}),
    )
    .unwrap();
    let mut producer = Producer::new(request).unwrap();
    producer
        .register_entity(
            json!({"id":"data","kind":"dataset","path":"data.json","provenance":{},"relations":[]}),
        )
        .unwrap();
    producer
        .derive_dataset(
            "data",
            "parent_data",
            json!({"kind":"subset","details":"Selected recorded members"}),
        )
        .unwrap();
    assert_eq!(producer.entities[0]["lineage"][0]["input"], "parent_data");
    assert!(producer
        .derive_dataset("data", "parent_data", json!({}))
        .is_err());
    assert_eq!(fs::read(root.join("data.json")).unwrap(), b"{}");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn source_digest_is_stable() {
    assert_eq!(
        Producer::source_hash(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
#[test]
fn output_escape_is_rejected() {
    let root = std::env::temp_dir();
    assert!(inside(&root, "../outside").is_err());
    assert!(inside(&root, "/outside").is_err());
}

#[test]
fn streamed_producer_inventory_hashes_real_bytes_and_requires_sorted_primary() {
    let root = std::env::temp_dir().join(format!("loom-sdk-inventory-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.txt"), "first").unwrap();
    fs::write(root.join("b.txt"), "second").unwrap();
    let request: Request = serde_json::from_value(
        json!({"output_dir":root,"profile":{"revision":"fixture.r1","sha256":"a".repeat(64)}}),
    )
    .unwrap();
    let mut producer = Producer::new(request).unwrap();
    assert!(producer
        .register_inventory(
            "bad",
            "artifact",
            "a.txt",
            ["b.txt", "a.txt"].into_iter().map(|s| Ok(s.into())),
            json!({})
        )
        .is_err());
    producer
        .register_inventory(
            "valid",
            "artifact",
            "a.txt",
            ["a.txt", "b.txt"].into_iter().map(|s| Ok(s.into())),
            json!({"scope":"fixture"}),
        )
        .unwrap();
    let declaration = &producer.entities[0]["inventory"];
    assert_eq!(declaration["entries"], 2);
    let bytes = fs::read(root.join(declaration["chunks"][0]["ref"].as_str().unwrap())).unwrap();
    assert_eq!(
        declaration["chunks"][0]["sha256"],
        Producer::source_hash(&bytes)
    );
    let entry: Value =
        serde_json::from_slice(bytes.split(|b| *b == b'\n').next().unwrap()).unwrap();
    assert_eq!(entry["sha256"], Producer::source_hash(b"first"));
    assert_eq!(entry["role"], "primary");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pending_output_label_preserves_identity_and_bytes_and_rejects_invalid_names() {
    let root = std::env::temp_dir().join(format!("loom-sdk-label-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let request: Request = serde_json::from_value(
        json!({"output_dir":root,"profile":{"revision":"fixture.r1","sha256":"a".repeat(64)}}),
    )
    .unwrap();
    let mut producer = Producer::new(request).unwrap();
    producer
        .write_artifact("checkpoint", "weights.bin", b"unchanged")
        .unwrap();
    let before = producer.entities[0].clone();
    producer
        .set_entity_label("checkpoint", " X-VLA · RGB-D · Grasp policy · step 20000 ")
        .unwrap();
    assert_eq!(
        producer.entities[0]["label"],
        "X-VLA · RGB-D · Grasp policy · step 20000"
    );
    for key in ["id", "kind", "path", "provenance"] {
        assert_eq!(producer.entities[0][key], before[key]);
    }
    assert_eq!(fs::read(root.join("weights.bin")).unwrap(), b"unchanged");
    for label in ["", " \n ", "multiline\nlabel"] {
        assert!(producer.set_entity_label("checkpoint", label).is_err());
    }
    assert!(producer
        .set_entity_label("checkpoint", &"x".repeat(201))
        .is_err());
    assert!(producer.set_entity_label("unknown", "Valid label").is_err());
    fs::remove_dir_all(root).unwrap();
}
