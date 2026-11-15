use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../..");
    let mut paths = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    fn collect(p: &std::path::Path, out: &mut Vec<PathBuf>) {
        for item in fs::read_dir(p).unwrap() {
            let p = item.unwrap().path();
            if p.is_dir() {
                collect(&p, out)
            } else if matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("rs" | "json" | "sql" | "toml" | "md")
            ) {
                out.push(p)
            }
        }
    }
    collect(&root.join("rust"), &mut paths);
    collect(&root.join("loom_use"), &mut paths);
    paths.sort();
    let build_path = root.join("ops/build-info.json");
    println!("cargo:rerun-if-changed={}", build_path.display());
    let info = fs::read(&build_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .unwrap_or_default();
    println!(
        "cargo:rustc-env=LOOM_BUILD_ID={}",
        info["build_id"].as_str().unwrap_or("rust.0.2.0-dev.1")
    );
    println!(
        "cargo:rustc-env=LOOM_GIT_COMMIT={}",
        info["git_commit"].as_str().unwrap_or("unrecorded")
    );
    let mut h = Sha256::new();
    for p in paths {
        println!("cargo:rerun-if-changed={}", p.display());
        h.update(p.strip_prefix(&root).unwrap().to_string_lossy().as_bytes());
        h.update([0]);
        h.update(fs::read(p).unwrap());
    }
    println!("cargo:rustc-env=LOOM_RUST_SOURCE={:x}", h.finalize());
}
