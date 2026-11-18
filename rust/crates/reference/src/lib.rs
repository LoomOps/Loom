//! Immutable reference payload linked at the executable boundary.
include!(concat!(env!("OUT_DIR"), "/guides.rs"));
pub use loom_primitives::reference::MCP_INSTRUCTIONS;
pub const PACKAGED: &[(&str, &str)] = &[
    ("mcp-instructions.txt", MCP_INSTRUCTIONS),
    ("LICENSE", include_str!("../../../../loom_use/LICENSE")),
    (
        "THIRD_PARTY_NOTICES.md",
        include_str!("../../../../loom_use/THIRD_PARTY_NOTICES.md"),
    ),
    ("README.md", include_str!("../../../../loom_use/README.md")),
    (
        "client-release.json",
        include_str!("../../../../loom_use/client-release.json"),
    ),
    (
        "skills/loom-use/SKILL.md",
        include_str!("../../../../loom_use/skills/loom-use/SKILL.md"),
    ),
];
pub const CATALOG: &str = include_str!("../../../../loom_use/catalog.json");
pub const ACTION_DOCS: &str = include_str!("../../../contracts/action-docs.json");
pub const JOB_CONTRACT: &str = include_str!("../../../contracts/job-contract.json");

pub fn assets() -> &'static loom_primitives::reference::ReferenceAssets {
    static ASSETS: loom_primitives::reference::ReferenceAssets =
        loom_primitives::reference::ReferenceAssets {
            documents: DOCUMENTS,
            packaged: PACKAGED,
            catalog: CATALOG,
            action_docs: ACTION_DOCS,
            job_contract: JOB_CONTRACT,
            templates: &[
                ("hybrid.rs", include_str!("../../../templates/hybrid.rs")),
                ("runner.rs", include_str!("../../../templates/runner.rs")),
                ("worker.py", include_str!("../../../templates/worker.py")),
                (
                    "sdk-Cargo.toml",
                    include_str!("../../../templates/sdk-Cargo.toml"),
                ),
                ("sdk.rs", include_str!("../../sdk/src/lib.rs")),
            ],
            client_release: include_str!("../../../../loom_use/client-release.json"),
            release_version: env!("LOOM_RELEASE_VERSION"),
            build_id: env!("LOOM_BUILD_ID"),
            git_commit: env!("LOOM_GIT_COMMIT"),
            source_sha256: env!("LOOM_RUST_SOURCE"),
        };
    &ASSETS
}
