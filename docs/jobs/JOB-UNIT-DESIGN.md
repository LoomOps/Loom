# Job, Unit, and catalog execution design

Status: historical design and rollout record. See [Job and Unit boundaries](BOUNDARIES.md) and the [generated API reference](../reference/README.md) for current behavior.

## Baseline and audit findings

The audit used v0.2.0-alpha.56 as its baseline. It reviewed execution, identity, resource, publication and visibility assumptions at the entry points below. The examples use synthetic IDs and inputs.

| Observed coupling | Source paths | Required treatment |
|---|---|---|
| A physical execution is stored in jobs. The same public Job vocabulary also covers legacy workflow roots, members and a one-execution reservation. | [Persistence schema](../../rust/contracts/legacy-schema.sql), [queue and admission](../../rust/crates/core/src/jobs.rs), [plans](../../rust/crates/core/src/plans.rs) | Give research outcomes and physical Units distinct, typed identities. Preserve exact old IDs and original membership. Avoid an independent second state machine for the same physical work. |
| Stage, Attempt, effect and resource tickets all point to physical Job identity. Steps are producer-reported progress. | [engine](../../rust/crates/core/src/execution.rs), [progress](../../rust/crates/core/src/progress.rs), [SDK](../../rust/crates/sdk/src/lib.rs), [worker](../../rust/crates/ops/src/worker.rs) | Reuse the fenced physical lifecycle for a Unit. Project old Steps as observed Units with no independent execution claims. |
| Readiness and dispatch use binary CPU/GPU lane occupancy. Host Executor checks every live GPU ticket. | [readiness](../../rust/crates/core/src/jobs.rs), [effect occupancy](../../rust/crates/core/src/execution.rs), [host executor](../../rust/crates/host/src/executor.rs), [Profile validation](../../rust/crates/core/src/software.rs) | Implement bounded CPU concurrency and device-level shared/exclusive admission in both Ops and Executor. A UI split alone cannot make B and C prepare concurrently. |
| A publication resolves to a physical Job and Attempt. Downstream inputs use exact producer outputs. A related Job can also be added for presentation. | [results](../../rust/crates/core/src/results.rs), [outcomes](../../rust/crates/core/src/plans.rs), [composition](../../rust/crates/core/src/composition.rs), [exact reads](../../rust/crates/core/src/query.rs) | Record logical Job, physical Unit and Attempt. Introduce accepted Job-output alias resolution. Presentation grouping carries no execution or lineage authority. |
| History, queue and Activity use several aliases for workflows, parents and native rows. Output filters infer purpose from scope, role, kind and metadata. | [catalog projection](../../rust/crates/core/src/catalog.rs), [activity](../../rust/crates/core/src/activity.rs), [index](../../rust/crates/core/src/index_store.rs), [Dashboard queue](../../dashboard/workspace/queue.js), [output lists](../../dashboard/workspace/outputs.js) | Return authoritative queue/history projections and explicit publication class. Keep execution, visibility, lineage and administrative scope separate. |
| Pale binds work to exact source Job/Attempt IDs and wakes coordinators through event receipts. Loom-use routes versioned native guides. | Pale rust/src/{loom,loom_types,tasks,outbox}.rs and src/pale_dispatcher/adapters/loom_use.{py,md}; [native guide bundle](../../rust/crates/reference/src/lib.rs), [guide index](../../loom_use/entry.md) | Version the identity bindings and event payloads. Keep legacy clients readable, original coordinator ownership and one reviewed user-facing completion. |

## Domain, execution boundaries and history

| Object | Definition and behavior |
|---|---|
| **Job** | One independently accepted research outcome, ordinarily one Regular Dataset, Policy or Experiment. Owns the accepted outcome, required deliverable and validation contract, planned Units and links to upstream results. The Job is visible in Job Queue while open and once in terminal history. |
| **Unit** | Concrete work with a purpose. A scheduled Unit has prerequisites, a frozen Profile, resource claims and execution receipts. It can be standalone or owned by one Job. Standalone work includes Observer setup, Pilot, Smoke, Runtime checks and maintenance. Its own row can appear in both the Unit Queue and terminal history. |
| **Stage** | Engine lifecycle for a scheduled Unit, including prepare, run and finalize. A Unit can itself serve as preparation from the parent Job's perspective. Preserve Stage-level fences and return receipts. |
| **Attempt** | A try of a scheduled Unit. Same-outcome recovery may create another Attempt under the same Unit and Job. Publication-only recovery reuses the original Attempt and its immutable evidence. |
| **Observed Unit** | Bounded progress reported by a producer during an execution, such as an episode batch. Project old Steps as observed Units. These rows share the executing Unit's Attempt and GPU lease. The runtime exposes active progress without fabricating another physical execution. |
| **Task** | Research grouping in Loom or coordination in Pale. A user request can span several research Jobs and independent Units. Its identity provides context for the work. |

A completed SDG Dataset includes its membership, split and distribution evidence. A completed Policy includes its checkpoint selection and training reports. Physical batches, intermediate checkpoints and reports can coexist under the same outcome. Use separate Jobs when outcomes have independent acceptance and can be consumed separately. A matched checkpoint sweep can produce one Experiment with per-checkpoint outcomes. A small final evaluation is a Job, while a preflight for a planned run is a Unit. Classification follows purpose and acceptance, regardless of sample count.

Resolve execution inputs from the latest user request, relevant prior decisions, exact accepted Job and catalog revisions, and verified current Profile values. The agent plans batching, ordering and resource strategy. Ask a focused question if an unresolved choice changes the cohort, evaluation protocol, environment, budget, or meaning of the outcome. Defer the affected submission while continuing independent reads and planning.

## Domain storage, writes and scheduling

- Keep the existing physical Job row as the backing execution record during cutover. Add a research Job table and a Unit membership/identity table with a unique link to that backing row. Use the existing physical transition, Attempt, effect and outbox rows as the sole authority for Unit status. Add one transaction for Job admission, its initial Unit graph and idempotent receipt. Standalone Unit admission uses the same execution machinery without Job membership.
- Freeze current Profile, Runtime/stage pins, parameters, resource requirements, store grants and promised output aliases at acceptance. Admit bounded Unit DAGs. Validate unique names, ownership, cycles, producer promises, and typed references. A completion dependency represents a technical gate; an output dependency resolves to a verified artifact or exact accepted Job-output alias. An unresolved or failed required producer keeps its dependents blocked. Preserve old producer lookups by Job, entity and member.
- Schedule Units independently. Revision of a pending plan uses its expected revision and preserves admitted execution membership and accepted inputs. The existing Runtime reservation path converts into a single pending research outcome with a later frozen Unit binding. Add new execution within the accepted outcome. A scientifically changed request has its own identity and an explicit continuation link.
- Derive Job status from the snapshot of required Units and publication. A research Unit running gives Running. Active preparation alone gives Preparing; output validation and publication give Finalizing. A held or blocked Job displays the actual Unit gate. Exhausted required failures stop completion. A terminal success and publication of the required deliverable complete the Job. Scientific acceptance and the count of valid evaluation cases remain independently queryable.
- Use actual start and return receipts for elapsed time. Keep wall span and active time separate. Union overlapping intervals for a Job. Resource occupancy counts live physical tickets once. Never promote a producer progress label into an execution or resource fact.

### Resource and ordering contract

Allocate concrete device identities on the chosen host. New Unit plans distinguish CPU-only, shared GPU and exclusive GPU requests. Legacy GPU lane requests retain current exclusive behavior during compatibility mode.

| Rule | Implementation |
|---|---|
| CPU concurrency | Replace binary CPU readiness with bounded worker and host admission budgets. Include pending, accepted and unknown effects. Budget prepare, run, finalization and hooks without starving reconciliation. CPU preparation can proceed during GPU work. |
| Shared GPU | Permit co-residency with shared Units within configured slot and reserved memory budgets. Bind actual device IDs and validated resource claims in the receipt. Budgets control admission; hard partitioning uses a supported hardware/runtime capability. |
| Exclusive GPU | Check that every selected device has no live or unresolved competing claim. Acquire a multi-GPU set atomically, retain the lease through confirmed return and release all devices together. Continuous simulator initialization and rollout can share one Unit and lease. |
| Exclusive ordering | Accept claims in stable order per overlapping device set. A dependency-blocked exclusive head preserves its turn against later exclusive claims. Once the head becomes runnable, stop new shared claims on its devices and drain existing shared work. Disjoint devices and CPU keep progressing. Explicit hold suspends its admission. Cancel and verified return clear reservations. |
| Deadlock prevention | Order admissions of same-device GPU prerequisites ahead of their consumers using the accepted DAG. Validate plan edits against dependency edges and reserved device order before inserting them. Multi-device admission never holds a partial device set. |
| Fencing and restart | The host Executor owns the cross-instance lease ledger and resolves live device claims. Ops carries pending grants and reconciles by ticket, Unit, Attempt and epoch. Unknown return continues to fence the device. Host-wide guarantees require enforceable host policy and verified unmanaged occupancy. An older Executor keeps legacy behavior until negotiation succeeds. |

Acceptance must show A's GPU Unit running while B and C prepare on CPU, then shared and exclusive contention under the same physical resource accounting. Use representative training, throughput benchmarking and functional Observer validation. Measured peak VRAM is evidence for resource planning, with an explicit margin and configuration limits.

## Regular and Reference publication

| Producer or source | Publication contract |
|---|---|
| Job member Unit emitting the accepted research deliverable | Regular after validation of alias, kind, template, reports, provenance and scientific outcome commitments. Record logical Job ID, actual Unit ID and Attempt ID. |
| Job member Unit producing reusable intermediate evidence | Reference when catalog browsing is useful. Ordinary diagnostic files and logs can stay on the Unit. |
| Standalone Unit | May publish Reference Dataset, Policy or Experiment for Pilot or diagnostic evidence. Tool, Component and Runtime registration remains in the existing software registry. |
| External import | Preserve origin, exact version, source checksums and import receipt. A verified canonical external Dataset can be admitted as Regular. Supporting or unresolved imports are Reference. Keep imported assets independent of fabricated execution records. |
| Historical migration or promotion | Determine class per exact revision using accepted outcomes, producer evidence, intent and validation. Preview/apply an additive overlay with reason and source digest. Keep unconfirmed records discoverable in Reference pending review. A transition to Regular validates its provenance and applicable contract. |

An accepted Job-output alias resolves to an immutable Item revision with exact source Unit. A downstream Unit may bind a verified published alias when its dependency contract permits it. Publication failure preserves the exact Attempt evidence and never presents an unvalidated partial output as a Regular deliverable. Evaluation reporting retains planned, executed, invalid and valid counts. Zero valid cases leave success or a representative checkpoint undefined under the applicable report contract.

Add class to the authoritative projection and search index before pagination. Include the class selection in cursor and cache identity. Keep lifecycle, owner, imported origin, related Task and class orthogonal. Update revisions atomically so a changing index cannot temporarily erase visible results. Preserve scientific bodies, producer pins, exact IDs, files, lineage and splits.

## API, SDK and agent consumers

Names and payload details will be finalized in native schemas when implemented.

| Interface | Design |
|---|---|
| system.status / describe / guide | Announce the Job/Unit model, publication classes and resource capabilities. An old server continues using the guides it advertises. Scope-filtered action discovery remains the source for authorization and payloads. |
| Job and Unit operations | Keep jobs for research outcome reads and plan admission. Add typed Unit submission and control, proposed as a units action group. Preserve legacy input and read behavior through a capability-aware adapter. Hold/cancel applied to a Job returns the exact affected Unit IDs; queue ordering targets independently schedulable Units. Idempotency records the original request and stable accepted IDs. |
| Queues, detail and history | Return one revision-consistent Dashboard snapshot with Job and Unit rows and separate paged terminal history. Expose exact Unit prerequisites, held/blocked reasons, requested mode and observed claim. Provide current and historical views without inferring membership from labels. |
| Attempts, recovery, logs and events | Key physical effects, event correlation and actual resource return to Unit and Attempt. Preserve exact access to existing Job and Attempt identities. Version events and make delivery and replay idempotent. |
| SDK and authoring | Add explicit unit_id and optional logical job_id in a negotiated run envelope. Preserve legacy job as the physical identity for old producers. Version execution plan, progress and publication manifest handling. Keep old Profile, Runtime and request snapshots readable. |
| Loom-use | Route users to the short native definitions and matching request example. Inspect context, pins and Profile requirements before writing. Pair server guides with actual capabilities, and retain legacy routing for older installs. Update release bundle pin, docs and scoped MCP acceptance together. |
| Pale | Observe typed source/Job/Unit/Attempt references. Keep original task continuations and event receipts, coordinating multiple execution IDs under one user-request outcome. Member Unit completion remains evidence for that outcome. Verify that retries, cross-source observation, references and child completion preserve one reviewed Master completion. |

### Draft contract fields

| Record or request | Required shape for the detailed schema |
|---|---|
| Job admission | Stable request key, owner, recipient, intent, optional Task revision, required output alias/kind, accepted evidence requirements, Unit plan. Each required alias names one authorized publishing Unit and its declared output entity. |
| Unit admission | Stable unit name within a Job or an independent request key and owner, purpose, Profile and validated parameters, prerequisite references, output declarations, and resource request. A resource claim identifies the mode and effective device IDs after admission. Profile grants bound the requested resources. |
| Operation target | Typed target with kind and exact ID. Use kind Job for the research plan and kind Unit for a physical execution. Use a separate Attempt ID for retry receipts, artifacts, logs and publication repair. |
| Dependency | Exact verified catalog revision, accepted Job output alias, or a Unit completion requirement. Preserve the originating edge type in the receipt and use it to explain waiting or failure. |
| Dashboard snapshot | One source revision for Job and Unit rows. Each Unit exposes its owning Job when present, active progress label, observed phase and resource state. Terminal rows identify their kind and exact detail target. Bounded queries and signed cursors retain their existing security properties. |
| Publication | Exact producer Unit/Attempt, optional parent Job, authorized deliverable alias, class, exact catalog revision and origin. Return eligibility errors before committing a visible revision; release the artifact dependency only on authoritative publication. |

Add version-aware tests for the MCP tool list and Dashboard relay. Existing client tests assume six tool groups; negotiated Unit support adds a new group, while a legacy target retains the original list. Audit generated documentation and the independent Loom-use packaging rules for the new route.

## Dashboard and prior UI audit

**Job Queue** shows research Jobs with status, linked active Units and waiting causes. **Unit Queue** shows actionable standalone and member Units, prerequisites, resource mode and real execution status. Show the active observed Unit label, such as the third named operation out of five, in the executing Unit row. It uses that same attempt and resource claim. **Terminal history** has one row per Job or standalone Unit, labeled Job or Unit. Job detail contains its members and Attempts. System Unit visibility uses existing authorization boundaries, with access to System activity for authorized operators. Job/Unit history and Activity are independent of scientific output filtering.

**Outputs** replaces the purpose toggle with two checkboxes, Regular and Reference. First entry selects Regular alone. Both selected shows both classes; neither selected gives an empty selection state. Apply the same behavior across Dataset, Policy and Experiment lists and paged results. Retain the independent global catalog selector for other surfaces; it cannot silently override these classes. Preserve filters and selected records when navigating across panes. Comparison can explicitly read Reference revisions and show their designation.

| Earlier UI request | Audit in this checkout |
|---|---|
| All as initial global view and complete first-load history without changing a filter | Present in [workspace selection](../../dashboard/workspace/research.js), [catalog paging](../../dashboard/workspace/catalog-pages.js) and first-load browser tests. The new Regular default belongs on Outputs. |
| Remove the new Hold button | Present in [queue controls](../../dashboard/workspace/queue.js). Release of existing holds and scoped reordering remain. |
| Space Imported and BC-readiness badges, match the Dataset detail badge font | Present in [Dataset rows](../../dashboard/workspace/dataset-browser.js) and [Dashboard CSS](../../dashboard/workspace/operations-desk.css). |
| Group Overview facts into a consistent card and separate the Episode control label from its select | Present in [report workspace](../../dashboard/workspace/reporting.js) and [report styles](../../dashboard/workspace/reporting.css). |
| Hide model hashes in the Base model value and align Dependency category counts | Present in [metadata rendering](../../dashboard/workspace/research-metadata.js) and [Dashboard CSS](../../dashboard/workspace/operations-desk.css). Exact digest remains provenance. |
| Clarify the More button | Dataset pagination already uses an explicit label inside its list. This review updates the remaining Policy and Experiment list labels. Cursor behavior is preserved. |
| Show loading while Runtime pages are empty | Present in [catalog paging](../../dashboard/workspace/catalog-pages.js), with distinct Loading and Indexing labels. Projection latency is a separate performance issue. |
| Keep Comparison reachable and preserve scrolling after refresh | Comparison remains top-level. Native-browser cases exercise direct candidate paging and preserve scroll on background refresh. Carry those behaviors into the new views. |

## Migration and rollout

1. Inventory current Job/Attempt histories, explicit workflow memberships, active effects and external tickets, outboxes, producer aliases and frozen pins. Generate a dry-run receipt with counts, proposed exact mappings and unknowns per installation. Explicit execution membership and verified source identity take precedence over presentation-only parent links.
2. Backfill stable typed aliases for historical single executions and retained roots. Preserve source IDs, original states and timestamps. Imported history remains read-only. Completed legacy roots appear once with directly readable original members. Old consolidated runs retain real source Attempts. Incomplete plans keep a legacy compatibility path unless a reviewed conversion preserves their exact accepted commitments.
3. Keep catalog class backfill separate from execution conversion. Batch reviewed revisions, stage overlay receipts, verify unchanged item IDs/revisions, scientific evidence, files, grants, user/token rows and Runtime pins. Leave unresolved historical classifications as Reference while retaining exact lookup.
4. Drain dispatch, verify external GPU/CPU returns and durable publication state, then checkpoint DB/config and each independent host baseline. Negotiate Ops/Executor and client capabilities. Extend schema/minimum writer fencing and startup reconciliation. Carry preserved accounts, storage identity and installed software pins through release.
5. Rehearse on isolated snapshots with prior single Jobs, multi-node roots, invalid Pilot labels, external imports and Reference candidates. Verify restart, request replay, outbox reindex, Job/Unit readback and Dashboard/SDK paths. Rollback fences protect writes accepted after upgrade; post-admission rollback needs a data-preserving conversion or forward repair.

## Documentation plan and reference coverage

Put normative definitions in [Concepts](../CONCEPTS.md), [Jobs](../../loom_use/jobs.md), [Workflows](../../loom_use/workflows.md) and [Results](../../loom_use/results.md). Update [Job boundaries](BOUNDARIES.md), quickstart, recovery, reporting, runtime, authoring, SDK and exact API examples once the new actions exist. Package native guides and route installed Loom-use and Pale to their matching version. Keep implementation failures and historical incidents in engineering evidence. Ship compact examples of the correct workflow, with synthetic IDs and controlled inputs derived from demonstrated cases.

| Worked request | Concrete behavior to show and exercise |
|---|---|
| Reconstruct 140 demonstrations, train X-VLA and evaluate | Three Job admissions, one independent research outcome each. Show multiple generation Units, validated Dataset alias, consumed checkpoint and Experiment readback. |
| Prepare an Observer, then reuse it | Standalone Unit posts optional Reference evidence and records validated software. Later Job pins the exact Tool/Component revision. Show one history row per top-level Job or standalone Unit. |
| Continue a BC-first benchmark through 20,000 updates | Reuse exact split, input and protocol. Train/evaluate by method and distribution. Read per-checkpoint evidence and a unique representative when supported; a tie or incomplete cohort remains visible. Comparing completed results uses existing records. |
| Keep preparing during a GPU run | Show simultaneous CPU Units, shared GPU admission and an exclusive claim with drain, order and return receipts. Check both queue views. |
| Repeat an accepted recipe or resolve ambiguous inputs | Inspect prior exact revisions and the live Profile. Reuse established defaults and confirm remaining changes. Ask one question when alternatives change the science and submit after resolution. |
| Resume interrupted generation or publish a returned result | Show checkpoint-compatible resumption and preserved partial outputs. Distinguish a returned successful Attempt needing publication from a failed evaluator with zero valid cases and a recoverable Unit. |

The original example inventory informed the focused request walkthroughs. Built-in best-success-rate requires matched valid development/validation cases and a unique winner for a representative checkpoint.

## Feature coverage and implementation order

The following map covers all current blocks in the [feature catalog](../features/README.md) along with retained compatibility and client paths.

| Feature or owner | Main touch points |
|---|---|
| identity | core/auth.rs, system_ownership.rs and operation scopes. Typed identities with unchanged access and current credential grants. |
| api and onboarding | rust/crates/contracts, core/http.rs, native reference, loom_use, client distribution and release checks. Versioned contracts and selective guides. |
| queue and execution | core/jobs.rs, plans.rs, query.rs and execution.rs; domain states, ops/worker.rs, host/executor.rs and wire.rs. Typed DAG, resource lanes, persistent claims, state projections. |
| recovery | Same-Unit retry, result-only recovery, hook ownership, delivery receipts, cancellation and effect fences. |
| results | results.rs, reporting.rs, metadata_templates.rs and lineage. Validate class/deliverable and exact provenance before publishing or admitting consumers. |
| extensions and authoring | software.rs, runtime.rs, composition.rs, SDK and runtime manifests. Keep Profile pins, resource plans and actual usage distinct; preserve old producers. |
| catalog | catalog.rs, items.rs, index_store.rs, reconcile.rs, history and exact class overlay. Class-aware projection, graph, import and pagination. |
| lifecycle and scenarios | registry_lifecycle.rs and scenario.rs. Reverse dependency preview sees Units; stable exact Task links and archived revisions. |
| dashboard | Python relay and workspace renderer, Activity and Runtime consumers, Comparison/search, old saved tabs, generated HTML. Queue and catalog changes with independent UI protections. |
| observations and diagnostics | Host device observation, exact grants, reported memory, logs and incidents by Unit/Attempt. Preserve redaction. |
| portability and installation | migrate crate, deployment update/migrations/snapshot, image/build ownership and installer packaging. Capabilities and writer fencing, preserve export/restore. |
| network and SSH | Review action relay allowlist, scopes and supported reads. Existing Tailscale, sharing and SSH transport continue serving exact authorized data. |
| testing | Native action/worker tests, Firefox, packaging and isolated-host acceptance. Update feature ownership and declared build inputs. |
| retained compatibility paths | ops/, executor/, older clients and packaging fixtures. Maintain readback and legacy producer protocol without creating another scheduler. |

1. **Domain and contracts.** Implement typed IDs, plan and dependency semantics, compatibility adapters, append-only migration and stable accepted receipts. Verify legacy readback and ambiguous input paths.
2. **Scheduling and recovery.** Introduce the Unit adapter over the physical engine, bounded CPU dispatch, scoped shared/exclusive host claims and restart reconciliation. Verify pending effects, hard return, dependencies, two-instance fencing, multi-device atomic admission, cancellation and lease expiry.
3. **Publication and catalog.** Implement aliases, producer evidence, Regular/Reference, class migration and index queries. Verify standalone Regular rejection, member publication, import, matched reporting, idempotency, class-aware pages and recovery.
4. **Dashboard.** Add both queues, typed history, Unit and Job details, compatible Runtime usage, catalog filters and Activity deduplication. Verify native DTOs and Firefox in both themes and narrow/split panes alongside the existing UI regressions.
5. **Consumers and documentation.** Verify old and current Loom-use and Pale against native transports, guide refresh, exact IDs and deliberate failure scenarios. Regenerate current references after the implemented schemas stabilize. Check one reviewed Master outcome despite multi-Unit completion.
6. **Migration and release acceptance.** Rehearse facts and counts against isolated representative snapshots, including restart and stale-writer rejection. Run native Rust/HTTP/MCP, Dashboard, packaging/update and client suites plus scoped real CPU and GPU trials. Record verified host behavior independently from mock evidence. Use the [verification blocks](../features/VERIFICATION.md) and feature impact checker before rollout.


## Remaining release acceptance

The current implementation orders overlapping GPU requests within one Ops queue. The shared Executor fences live grants across its managed instances. A shared queue for pending exclusive requests across independent Ops instances remains a separate design concern. Hardware verification and per-installation historical classification must use their own evidence and rollout scope.
