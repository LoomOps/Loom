use super::*;
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
