# Independent whole-tree investigation and migration target audit

This investigation is separate from the design and from the feature verification
blocks. It records existing evidence, candidate problems and target decisions;
it is not a report that the proposed Rust services or corrections have passed.

The frozen findings below are historical. Current implementation decisions and
independent correction cycles are recorded in [IMPLEMENTATION](IMPLEMENTATION.md);
[IMPLEMENTATION-REVIEW.json](IMPLEMENTATION-REVIEW.json) inventories the current
source separately, without rewriting this baseline.

## Inputs and method

The frozen references are recorded by full commit in [FILE-REVIEW](FILE-REVIEW.json).
Every one of 362 file versions is read for bytes/hash/size/line count and classified
for redesign, retention, harness adaptation or instruction co-change. All 203
Python file versions parse as AST; all eight JSON versions parse. AST review records
imports, top-level declarations, literal SQL mutation candidates and external-effect
call sites. This covers tests, scripts, configuration, licenses/assets and all
instructions, not only catalog-owned code. Dynamic SQL/effects still need focused
semantic inspection; candidate scanning is not a complete program proof.

Focused semantic reads inspect submit/request replay, plan/node membership,
queue/dependency eligibility, cancel/return/finalizer/recovery races, immutable
publication/current binding, Runtime/source pin ownership, web projections and
producer/client/installation instructions. Existing feature specs/tests are then
used as a separate cross-check of invariants and missing boundaries.

The separate older working-copy versions are never treated as a patched latest
tree. Their original files/staging are left intact. Ten match the reference bytes;
21 differ and require semantic comparison, not automatic adoption. This package
contains only source hashes/analysis; no host records, tokens or research evidence.

## Findings and action ledger

| ID | Source observation | Consequence | Proposed action | Design | Phase |
|---|---|---|---|---|---|
| R01 | ops/ops.py contains submit, HTTP/MCP, CPU producer, worker, finalization, hooks and schema ownership. | A file-to-file Rust translation preserves cross-domain coupling. | Typed domain/app/store/query and narrow effect adapters; no SQL/helper RPC. | [ARCHITECTURE.md](ARCHITECTURE.md) | D1/D4/D6 |
| R02 | Legacy codecs use different JSON whitespace, escaping, float and source-byte rules. | Default serde encoding can invalidate accepted keys, Profile pins and tickets. | Capture exact per-namespace fixtures and preserve raw sealed/history bytes. | [COMPATIBILITY.md](COMPATIBILITY.md) | D1 |
| R03 | No versioned Scenario aggregate/API exists; presentation.experiment/request_ref are annotations. | Names and loose labels cannot establish a task/protocol identity. | Add immutable Scenario revisions and qualified additive links; keep unassigned items. | [blocks/scenario.md](blocks/scenario.md) | D1/D5/D6 |
| R04 | research.view filters curated catalog visibility while queue projection needs actual occupants; 07c90e4 corrects the consumer path. | Reusing catalog filtering for queue hides resource-consuming operations/support work. | Separate queue occupancy, history selectors and curated research projections in Rust. | [blocks/research-operations.md](blocks/research-operations.md) | D4/D5/D6 |
| R05 | presentation.parent_job, workflow_nodes and consolidation sources express different relationships. | Treating any parent annotation as membership hides independent Jobs or duplicates summary counts. | Only durable execution membership folds nodes; preserve source drill-down and exact source timing. | [DOMAIN.md](DOMAIN.md) | D4/D5 |
| R06 | Jobs/attempts/stage_runs/internal progress/publication/recovery have distinct facts. | One status string or row count cannot explain active recovery or true Steps vs Attempts. | Typed independent facts and projections; no fabricated integration Step or prior execution timing. | [blocks/execution.md](blocks/execution.md) | D1/D4/D6 |
| R07 | Current and older candidate lifecycle regressions cover late Attempt returns and cancellation/finalization races. | An old owner or stale artifact_ready may overwrite/admit work incorrectly if redesigned casually. | Expected Job/Attempt/plan/epoch fences; cancel wins publication; verify downstream availability states. | [blocks/queue.md](blocks/queue.md) | D4 |
| R08 | recovery.py supports execution and result-only resume; submit replay compares original requested/resolved identity. | Re-admitting/retraining a finalizer retry or comparing a patched spec breaks durable intent. | Separate recovery commands, archived previous results and original-key replay. | [blocks/recovery.md](blocks/recovery.md) | D4 |
| R09 | Executor launch/cancel uses durable tickets, Docker labels, fingerprints and return observations. | Rust async cancellation does not prove a container/process has returned. | Durable external intents, matching identity reconciliation and actual return evidence. | [blocks/execution.md](blocks/execution.md) | D3/D4 |
| R10 | registry indexing, callbacks and immutable source publication cross SQLite and filesystem boundaries. | A DB rollback alone cannot undo a published file or prevent source overwrite. | Staging/hash/intent/publish/fsync/receipt protocol and immutable conflict checks. | [MIGRATION.md](MIGRATION.md) | D4/D5 |
| R11 | Historical raw records can be unavailable while a verified current preserved copy exists; native reconnect is separate. | Overwriting old availability or promoting records implicitly destroys evidence/count meaning. | Add qualified historical bindings; explicit promotion with origin lineage only. | [blocks/results.md](blocks/results.md) | D5 |
| R12 | Deployment/install/runtime tools read configured roots, markers, grants and effective mounts independently. | Health 200 cannot detect API/worker/Explorer/storage parity mismatch or old missing plans. | Cross-consumer effective-root/identity/access doctor and fenced correction procedure. | [blocks/installation.md](blocks/installation.md) | D7 |
| R13 | The current Rust proposal left producer language outside scope; server and Loom Use guides do not require Rust-first authoring. | A Rust scheduler alone still leaves agents generating Python orchestration and per-task copies. | Rust SDK/templates/build receipts, reuse-first Skill guidance and justified external Python workers. | [AUTHORING.md](AUTHORING.md) | D2/D6 |
| R14 | cpu.smoke is implemented inside ops.py; installer smoke emits a Python Component source string. | Platform built-ins remain Python even after control-plane replacement if omitted. | Release-built Rust smoke packages through the normal producer/Profile/Job path; legacy API adapters. | [blocks/authoring.md](blocks/authoring.md) | D2/D7 |
| R15 | Component/Tool registration accepts UTF-8 file content; Rust binaries/build scripts require a different artifact boundary. | Binary-as-text or API-side compilation expands messages, trust and privileges. | Verified binary locators plus source/build/target receipts; isolated authorized build Runtime. | [AUTHORING.md](AUTHORING.md) | D1/D2/D5 |
| R16 | runtime_store owns a separate metadata DB, grants and validation fences; Runtime tools are administrator operations. | Two metadata writers or a language-driven reinstall can corrupt accepted environments. | One host registrar; staged transfer and intent/receipt protocol; preserve exact existing revisions. | [blocks/extensions.md](blocks/extensions.md) | D3/D5/D7 |
| R17 | Dependencies is a representative view of Runtime manifests; installed inventory is not runtime import telemetry. | A large inventory or a selected primary package do not prove actual Job usage. | Representative summary plus exact inventory, provenance and truthful use evidence; no implicit environment adoption. | [blocks/extensions.md](blocks/extensions.md) | D5 |
| R18 | Profiles combine lifecycle stages, Tools and Runtime pins; namespace, origin and display attribution differ. | Relabeling research code Built-in or counting retries/nodes as reuse misrepresents provenance. | Explicit origin and stage composition; count distinct logical Jobs and preserve pins. | [blocks/extensions.md](blocks/extensions.md) | D5/D6 |
| R19 | The retained web owns substantial SSO/CSRF/iframe, generated HTML and layout behavior. | Rewriting web with core Rust introduces unrelated interface/auth regressions. | Retain web and provider libraries; update requested Scenario/composition/query consumers only. | [blocks/dashboard.md](blocks/dashboard.md) | D5/D6 |
| R20 | PLATFORM/static API overviews and some feature references trail the executable registry. | Stale lists cannot define Rust APIs or guide generation. | One action registry; regenerate references/examples and test advertised capabilities. | [API-CHANGES.md](API-CHANGES.md) | D1/D6 |
| R21 | Many existing tests import/patch Python internals; feature blocks enumerate only registered boundaries. | Old counts or passing mocks do not prove the new Rust implementation or cover unlisted files. | Independent file/effect audit, Rust semantic/fault tests and real black-box clients/browser harnesses. | [blocks/testing.md](blocks/testing.md) | D0–D7 |
| R22 | Snapshot/HOST_FILES/Compose/Dockerfiles enumerate executable/helper source paths. | Adding Cargo files does not make installed services execute matching Rust binaries. | Source-qualified target artifacts, SDK/built-ins and minimum helper/schema protocols in every distribution path. | [blocks/installation.md](blocks/installation.md) | D7 |
| R23 | Managed-resource atomic writes preserve private ownership and reconcile upstream approval state. | An unreadable root-owned journal can masquerade as upstream_unavailable after a careless rewrite. | Preserve intended UID/GID/mode and specific private diagnostics; retain provider intent/receipt behavior. | [blocks/network.md](blocks/network.md) | D6/D7 |
| R24 | An older dirty checkout contains 31 candidate file versions: 10 equal and 21 different from the frozen reference. | Blindly merging older corrections may regress newer Runtime/workflow/recovery behavior. | Keep original checkout untouched; record candidate hashes and review semantic deltas against 07c90e4. | [FILE-REVIEW.json](FILE-REVIEW.json) | D0/D1 |
| R25 | General reconciliation was previously only an idea; current requests require bounded explicit associations. | Autonomous broad repair would exceed scope and invent scientific context. | Include verified historical bindings and confirmed Scenario links; defer a general reconciler. | [blocks/scenario.md](blocks/scenario.md) | D5 |
| R26 | Feature audits are development evidence, and external coordination is independent of execution. | Turning rewrite checks or agent ACKs into runtime gates changes the research workflow. | Keep verification outside Job admission and completion; preserve existing operational scopes. | [ROADMAP.md](ROADMAP.md) | D1–D8 |

## Migration classification (file versions)

- `retain-review-packaging`: 17.
- `review-rewrite-contract-and-instructions`: 75.
- `retain-web-and-external-auth-boundary`: 54.
- `retain-operator-driver-adapt-rust-packaging`: 45.
- `redesign-rust`: 58.
- `retain-boundary-harness-adapt-internals`: 79.
- `retain-independent-client-and-tests`: 22.
- `revise-rust-first-authoring-guidance`: 12.

These totals include candidate versions, so they are not counts of distinct
production modules. Detailed per-file target crates, declarations, imports and
boundary sites are in the ledger; mutation candidates and contract captures are
separate files in `audit/`. A path-level target is an ownership decision, not a
promise to reproduce that Python file as a Rust file.

## Feature-block cross-check and new coverage

All 17 existing features are represented by individual rewritten block designs.
Scenario, research/operations presentation and Rust authoring add three designs
because the existing catalog does not express them as complete owned features.
The implementation must add actual Rust/new-feature ownership to the catalog when
files exist, and adapt tests/consumers rather than marking a hypothetical source
file as implemented. No feature entry or production API is changed by this plan.

No whole-repository concern is dismissed because current verification blocks pass.
In particular, public binary/package provenance, actual old schema/codec acceptance,
producer language, source/current-location distinction, client Skill rollout and
real occupancy before UI filtering require their own evidence.

## Audit limits and checkpoints

- Inventory/AST/schema/link coverage is structurally verified. Boundary reasoning
  is reviewed, but no universal line-level correctness or formal safety proof is claimed.
- No new Rust Loom crate or benchmark exists yet. Performance budgets/library
  versions/libc baseline are fixed by the D1 spike using representative workloads.
- This task reads local approved sources, not operational hosts or all users' chat
  histories. It does not classify actual private research records into Scenarios.
- GPU/Sim correctness, provider approval, full native-client behavior, actual reboot
  and power loss remain distinct acceptance surfaces, not implied by Python tests.
- Decisions: retain web/provider integration and justified scientific Python;
  redesign Rust authority and new producer defaults. SDK/ABI targets and exact
  additive API/DDL definitions must be frozen before D2–D6 implementation.
