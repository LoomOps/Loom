# Public baseline and compatibility boundary

The public baseline ships only the current platform. One-off research migration/repair tools, personal operator integrations, the token console, the Agent document application, unused visualization-host SDK and embedded demo screens/metrics, tailnet membership enrollment and the earlier producer launcher are not part of this distribution.

## Supported contracts

- Native Tailscale identity, machine Sharing, the current Gateway and integrated Dashboard administration.
- Six grouped MCP tools and documented HTTP routes. Flat MCP tool names are rejected. Repair uses `recovery`; scientific corrections use producer-supplied `results.annotate`.
- Registered components and execution profiles using the current execution schema. CPU diagnostics remain built in. A profile's optional `runtime_id` selects an explicitly configured host runtime; this is an active feature, not a research dependency.
- Loom-to-Loom export, verified transfer and fencing, rather than scripts tied to a particular former machine or research dataset.

## Preserved state

Existing identity mappings, owner IDs, immutable history, annotations, artifacts, component/profile revisions, executor tickets and user overrides remain installation data. Internal SQLite state values and stored IDs are not renamed for cosmetic reasons. The worker does not dispatch imported history. Historical loss-summary extension keys remain read-only renderers so preserved results are still understandable; they do not install or execute an old analysis pipeline.

Stored old membership requests remain archival rows, but no code invites tailnet members or approves client devices. Current request processing uses only machine Sharing tables. Unsupported authentication configuration must be corrected explicitly before an update; no provider is contacted implicitly.

Existing frozen Jobs using an unsupported producer schema remain readable. Finish or cancel pending work and register a current profile before upgrading. The updater checks for these pending Jobs before changing services. No historical recipe is silently converted into executable work.

## Regression coverage

The complete test runner validates all distributable Python files, generated UI consistency, feature ownership, documentation links, licenses, removed public routes and the absence of private operator references. Behavioral tests cover native/shared identity, cancellation/revocation, owner continuity, queued preparation, recovery budgets, result publication, preserved history and explicit update/rollback. Firefox checks the actual current workspace.

Private archives and review evidence are not release contents. Tests do not prove real external VPN clients, arbitrary third-party Job images or GPU experiments; those need installation-specific acceptance.

## Compatibility boundaries retained deliberately

The internal `OPS_BULK` binding and `/loom/bulk` alias still anchor primary output
paths on older installations. They are not an extra service or an additional
user-facing store. Replacing them without reconciling data/N mounts can redirect
existing Job outputs, so image updates preserve them. Stored `node_control` and
archive fields likewise retain their schema names. Historical rendering and
scoped orphan-policy cleanup remain supported data/maintenance features.

Local short-lived token issuance is removed. Previously issued credentials keep
their original expiry only; current resident tokens come from Dashboard Admin.
