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

pub const MCP_INSTRUCTIONS: &str = "Confirm the target with system.status. Use system.guide to load the relevant guide and system.describe for exact action inputs. Use exact reads for progress and comparisons. For a new run, use jobs and job-cases to decide Job and Unit boundaries. Resolve required inputs from the request, exact prior records and the current Profile. Ask about unresolved required choices before submitting. Use catalog-cases when choosing how to register or publish a result. Retain accepted IDs and replay an uncertain submission with the original key and arguments.";
