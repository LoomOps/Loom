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
