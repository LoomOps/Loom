//! Read-only build/reference inputs supplied by the executable composition root.
#[derive(Debug)]
pub struct ReferenceAssets {
    pub documents: &'static [(&'static str, &'static str, &'static str)],
    pub packaged: &'static [(&'static str, &'static str)],
    pub catalog: &'static str,
    pub action_docs: &'static str,
    pub job_contract: &'static str,
    pub templates: &'static [(&'static str, &'static str)],
    pub client_release: &'static str,
    pub release_version: &'static str,
    pub build_id: &'static str,
    pub git_commit: &'static str,
    pub source_sha256: &'static str,
}

pub const MCP_INSTRUCTIONS: &str = "Call system.status to confirm the target and system.guide for task routes. Use reads for status and comparisons of existing results. Derive necessary Jobs and catalog outputs from the requested outcome. Before Job submission load jobs and job-cases. Resolve required inputs from the request, exact prior records and live Profile. Ask about unresolved required choices before submitting. Before catalog registration or publication load catalog-cases. Load the exact system.describe action schema. Keep accepted Job IDs and pins.";
