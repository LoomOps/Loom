//! Hash verification runs outside the application writer lane.
use super::*;
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
    verify_inputs(&mut request).unwrap();
    assert_eq!(request["verified_inputs"]["data"]["path"], json!(valid));
    assert_eq!(request["verified_inputs"]["data"]["storage"], "data/1");
    assert!(request["verified_inputs"]["data"]
        .get("candidates")
        .is_none());
    std::fs::remove_file(&valid).unwrap();
    // Once selected, a frozen request cannot fall back to another copy.
    assert_eq!(verify_inputs(&mut request).unwrap_err().code, "input");
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
