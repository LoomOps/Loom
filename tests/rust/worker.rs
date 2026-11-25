//! Hash verification runs outside the application writer lane.
use super::*;

#[test]
fn quiet_log_pages_preserve_the_pointer_and_commit_close_or_crash_repair() {
    use crate::log_collector::commit_page;
    use std::os::unix::fs::MetadataExt;
    let root = std::env::temp_dir().join(format!("loom-log-page-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("stream.log");
    let live = json!({"generation":"one","offset":3,"closed":false,"gap":false});
    assert_eq!(
        commit_page(&path, &Value::Null, 0, b"log", &live).unwrap(),
        0
    );
    let pointer = path.with_extension("json");
    let original = std::fs::File::open(&pointer).unwrap();
    assert_eq!(commit_page(&path, &live, 3, b"", &live).unwrap(), 3);
    assert_eq!(
        std::fs::metadata(&pointer).unwrap().ino(),
        original.metadata().unwrap().ino()
    );

    std::fs::write(&path, b"log-tail").unwrap();
    assert_eq!(commit_page(&path, &live, 3, b"", &live).unwrap(), 8);
    assert_eq!(std::fs::read(&path).unwrap(), b"log");
    let mut sealed = live.clone();
    sealed["closed"] = json!(true);
    assert_eq!(commit_page(&path, &live, 3, b"", &sealed).unwrap(), 3);
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&pointer).unwrap()).unwrap(),
        sealed
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn registered_input_copies_skip_missing_and_corrupt_bytes_without_rebinding_identity() {
    let root = std::env::temp_dir().join(format!("loom-copy-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let missing = root.join("missing");
    let corrupt = root.join("corrupt");
    let valid = root.join("valid");
    std::fs::write(&corrupt, b"wrong").unwrap();
    std::fs::write(&valid, b"right").unwrap();
    let (_, sha) = storage::file_hash(&valid).unwrap();
    let candidates:Vec<Value>=[&missing,&corrupt,&valid].into_iter().map(|path|json!({"path":path,"storage":"data/1","ref":path.file_name().unwrap().to_str().unwrap(),"sha256":sha})).collect();
    let mut request = json!({"verified_inputs":{"data":{"candidates":candidates}}});
    let proofs = InputProofs::default();
    verify_inputs(&mut request, &proofs).unwrap();
    assert_eq!(request["verified_inputs"]["data"]["path"], json!(valid));
    assert_eq!(request["verified_inputs"]["data"]["storage"], "data/1");
    assert!(request["verified_inputs"]["data"]
        .get("candidates")
        .is_none());
    let repeated = verify_inputs(&mut request, &proofs).unwrap();
    assert_eq!(repeated.hashed_bytes, 0);
    assert_eq!(repeated.reused_files, 1);
    std::fs::write(&valid, b"wrong").unwrap();
    assert_eq!(
        verify_inputs(&mut request, &proofs).unwrap_err().code,
        "input"
    );
    std::fs::remove_file(&valid).unwrap();
    // Once selected, a frozen request cannot fall back to another copy.
    assert_eq!(
        verify_inputs(&mut request, &proofs).unwrap_err().code,
        "input"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn producer_manifest_rejects_special_and_oversized_files_without_waiting_for_a_writer() {
    use std::os::unix::ffi::OsStrExt;
    let root = std::env::temp_dir().join(format!("loom-manifest-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("result.json");
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(read_manifest(&path).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::File::create(&path)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    assert!(read_manifest(&path).is_err());
    std::fs::write(&path, b"{\"entities\":[]}").unwrap();
    assert_eq!(read_manifest(&path).unwrap(), json!({"entities":[]}));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_receipt_is_bound_to_attempt_and_verified_bytes() {
    let root = std::env::temp_dir().join(format!("loom-pause-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("checkpoint");
    std::fs::write(&file, b"saved").unwrap();
    let receipt = json!({"schema":"loom-checkpoint-pause/v1","attempt":"attempt-one",
        "file":"checkpoint","sha256":hash(b"saved"),"progress":20});
    std::fs::write(
        root.join("pause-checkpoint.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    let proofs = InputProofs::default();
    assert_eq!(
        pause_checkpoint(&root, "attempt-one", &proofs).unwrap()["progress"],
        20
    );
    assert!(pause_checkpoint(&root, "attempt-other", &proofs).is_err());
    std::fs::write(&file, b"wrong").unwrap();
    assert!(pause_checkpoint(&root, "attempt-one", &proofs).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
