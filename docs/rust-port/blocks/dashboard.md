# Dashboard and retained web integration — detailed design

Status: proposed Rust redesign; no implemented/deployed claim. Phases: D5, D6.

## Responsibility and model

**Authority:** Existing HTML/JS/CSS/SSO relay; consumes Rust query DTOs and application routes.

**Typed model:** QueueRow, HistoryQuery, ScenarioCard/Graph, ProfileCombination, SoftwareOrigin and AvailabilityView DTOs.

**Operations:** Existing authorized UI actions plus Scenario register/link and source drill-down; all go through shared public application operations.

**Persistence:** Browser filter/navigation state only. No browser-owned Job state or locally inferred Scenario membership.

## Reference source and consumers

- [dashboard/.dockerignore](../../../dashboard/.dockerignore)
- [dashboard/Dockerfile](../../../dashboard/Dockerfile)
- [dashboard/branding/brand.css](../../../dashboard/branding/brand.css)
- [dashboard/branding/brand.js](../../../dashboard/branding/brand.js)
- [dashboard/branding/logo.svg](../../../dashboard/branding/logo.svg)
- [dashboard/branding/scene.js](../../../dashboard/branding/scene.js)
- [dashboard/dev.py](../../../dashboard/dev.py)
- [dashboard/server.py](../../../dashboard/server.py)
- [dashboard/theme/style.css](../../../dashboard/theme/style.css)
- [dashboard/theme/theme.js](../../../dashboard/theme/theme.js)
- [dashboard/workspace/access.html](../../../dashboard/workspace/access.html)
- [dashboard/workspace/activity.js](../../../dashboard/workspace/activity.js)
- [dashboard/workspace/build.py](../../../dashboard/workspace/build.py)
- [dashboard/workspace/details.js](../../../dashboard/workspace/details.js)
- [dashboard/workspace/explorer.js](../../../dashboard/workspace/explorer.js)
- [dashboard/workspace/extensions.js](../../../dashboard/workspace/extensions.js)
- [dashboard/workspace/frame.css](../../../dashboard/workspace/frame.css)
- [dashboard/workspace/frame.js](../../../dashboard/workspace/frame.js)
- [dashboard/workspace/icons.js](../../../dashboard/workspace/icons.js)
- [dashboard/workspace/lazy_logs.py](../../../dashboard/workspace/lazy_logs.py)
- [dashboard/workspace/mcp.js](../../../dashboard/workspace/mcp.js)
- [dashboard/workspace/media.js](../../../dashboard/workspace/media.js)
- [dashboard/workspace/operations-desk.css](../../../dashboard/workspace/operations-desk.css)
- [dashboard/workspace/parent.css](../../../dashboard/workspace/parent.css)
- [dashboard/workspace/parent.js](../../../dashboard/workspace/parent.js)
- [dashboard/workspace/projection.js](../../../dashboard/workspace/projection.js)
- [dashboard/workspace/queue.js](../../../dashboard/workspace/queue.js)
- [dashboard/workspace/surfaces.js](../../../dashboard/workspace/surfaces.js)
- [dashboard/workspace/template.html](../../../dashboard/workspace/template.html)
- [dashboard/workspace/text-preview.js](../../../dashboard/workspace/text-preview.js)
- [dashboard/workspace/job-activity.js](../../../dashboard/workspace/job-activity.js)
- [dashboard/workspace/research.js](../../../dashboard/workspace/research.js)
- [dashboard/workspace/dependencies.js](../../../dashboard/workspace/dependencies.js)

Existing feature dependencies: identity, api, catalog, observations, onboarding, diagnostics.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Keep the current build/empty-data template/iframe/relay contract. Generated HTML remains empty of host catalogs and private migration evidence.

2. Current queue uses all actual occupants; history defaults All with independent scope/status/category/mode filters and explicit consolidated-source drill-down.

3. Add a concise Scenario entry/list/detail view: task description, exact protocol revision, linked Jobs/datasets/models/experiments and unassigned/reusable items. Avoid explanatory filler.

4. Keep Runtimes grouping and separate Profiles/Components/Tools/Runtimes/Dependencies views; display stage composition, origin, tags, primary dependencies and evidence-backed use counts.

5. Show actual execution, publication and recovery facts without duplicate running labels; Steps and Attempts have distinct counts and source timing labels.

6. Add no prose explaining implementation languages to ordinary product flows. Implementation/build details belong in package detail when useful for compatibility decisions.

## Transaction and effect boundary

UI sends user commands and renders response revisions; it never mutates DBs, merges Jobs locally or assumes a stale action succeeded after timeout. Preserve existing CSRF and trusted-relay checks.

## Retained code and compatibility

All existing web assets and browser integration. Only requested query consumers and Scenario/composition UI are redesigned; no frontend framework rewrite.

Update generated HTML and pinned template hash only for actual structural changes. Verify exact built HTML/image hashes and Firefox consumers against Rust service fixtures.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| UI-01 | Light/dark at wide and narrow width | No fact/label fragmentation, overflowing composition or inaccessible controls. |
| VIS-01 | Operations Job occupies queue under Research navigation | Occupancy still visible; history filters remain independent. |
| UI-02 | One recovered running Job | One coherent status, prior failure retained in history. |
| SCN-03 | Unassigned asset and multi-Scenario link | Both discoverable with correct revision/source detail. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/dashboard/test_proxy.py](../../../tests/dashboard/test_proxy.py)
- [tests/dashboard/browser/test_workspace.py](../../../tests/dashboard/browser/test_workspace.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
