use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=LOOM_DECLARED_BUILD_INPUTS");
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../..");
    // One catalog selects the compile-time guides and their retrieval metadata.
    let catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("loom_use/catalog.json")).expect("guide catalog"),
    )
    .expect("valid guide catalog");
    let mut documents = String::from("pub const DOCUMENTS: &[(&str, &str, &str)] = &[\n");
    let mut names = std::collections::BTreeSet::new();
    for document in catalog["documents"].as_array().expect("guide documents") {
        let id = document["id"].as_str().expect("guide id");
        assert!(
            !id.is_empty()
                && id.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
                && names.insert(id),
            "unique safe guide id"
        );
        let source = document["source"].as_str().expect("guide source");
        assert!(
            std::path::Path::new(source)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
            "relative guide source"
        );
        let path = root
            .join(source)
            .canonicalize()
            .expect("guide source exists");
        assert!(
            std::env::var_os("LOOM_DECLARED_BUILD_INPUTS").is_some()
                || path.starts_with(root.canonicalize().unwrap()),
            "guide inside repository"
        );
        documents.push_str(&format!(
            "({:?}, {:?}, {:?}),\n",
            id,
            document["title"].as_str().expect("guide title"),
            fs::read_to_string(path).expect("guide text"),
        ));
    }
    documents.push_str("];\n");
    let generated = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("guides.rs");
    if fs::read(&generated).ok().as_deref() != Some(documents.as_bytes()) {
        fs::write(generated, documents).expect("compile guide catalog");
    }
    let mut paths = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("rust/release.json"),
    ];
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
    for directory in [
        "domain",
        "primitives",
        "contracts",
        "core",
        "host",
        "ops",
        "reference",
    ] {
        let directory = root.join("rust/crates").join(directory);
        println!("cargo:rerun-if-changed={}", directory.display());
        collect(&directory, &mut paths);
    }
    collect(&root.join("rust/contracts"), &mut paths);
    println!("cargo:rerun-if-changed={}", root.join("loom_use").display());
    collect(&root.join("loom_use"), &mut paths);
    paths.sort();
    for directory in ["rust/templates", "rust/crates/sdk/src"] {
        let path = root.join(directory);
        println!("cargo:rerun-if-changed={}", path.display());
        collect(&path, &mut paths);
    }
    println!("cargo:rerun-if-env-changed=LOOM_BUILD_INFO");
    let info = if let Some(selected) = std::env::var_os("LOOM_BUILD_INFO") {
        let path = root.join(selected);
        println!("cargo:rerun-if-changed={}", path.display());
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(path).expect("selected build identity exists"),
        )
        .expect("selected build identity JSON")
    } else {
        serde_json::Value::Null
    };
    let release: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("rust/release.json")).expect("release descriptor"),
    )
    .expect("release JSON");
    let version = if info["channel"] == "alpha" {
        info["build_id"].as_str()
    } else {
        release["version"].as_str()
    }
    .expect("release version");
    println!("cargo:rustc-env=LOOM_RELEASE_VERSION={version}");
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
