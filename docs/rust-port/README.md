# Loom Rust redesign — complete development plan

Status: historical design and migration record. The native implementation is present in this repository; release/deployment evidence is separate from this original plan. For current use read [the documentation index](../README.md), [implementation boundaries](../IMPLEMENTATION.md) and the generated API reference. Original proposed phases, baseline commit IDs and review counts below retain their historical meaning.

## Reference inputs and audit boundary

- Frozen Loom reference: `07c90e4913c5335b79789fe8ae066d3cb7a5e7ab`, including the
  active-queue/history-filter correction. The prior Rust planning branch has been
  merged with this existing correction, without changing its product behavior.
- Frozen Loom Use reference: `11bc20b` (alpha.9; full identity in the ledger).
- Separate older working-copy candidates: 31 files on top of `208f261`; 10 are
  byte-identical to the reference and 21 differ. They are evidence for review,
  not an overlay to apply blindly. The user's original edits remain untouched.
- Independent inventory: 297 Loom files + 34 Loom Use files + 31 candidate versions
  = 362 file versions / 63,517 lines. Every file is hashed and structurally inspected;
  semantic conclusions are scoped to the documented boundaries, not a claim of
  line-by-line proof. This planning package is excluded from that frozen inventory.

Read the [independent investigation](REVIEW.md), [file ledger](FILE-REVIEW.json),
[mutation candidates](audit/MUTATIONS.json) and [contract inventory](audit/CONTRACTS.json)
separately from the implementation. Existing feature verification blocks
are a cross-check, not the method used to choose migration targets.

## Decisions replacing the old proposal

1. Design around typed aggregates, commands, receipts and query projections.
   Existing Python files are behavioral/compatibility references, not Rust module
   templates. Do not expose generic SQL/helper RPC or keep a Python state machine.
2. Rust owns new platform Job/queue/recovery/result/software/Scenario decisions and
   the performance-sensitive host executor. Keep web assets and established
   browser/identity-provider integrations. Existing administrator drivers may stay
   Python behind narrow Rust contracts; their privilege does not move into Ops.
3. New agent-authored Components, Tools, Job runners and platform built-ins default
   to Rust. Python is allowed for a concrete external SDK/library integration.
   Keep existing user revisions and Python scientific environments executable;
   language preference does not silently rewrite accepted Jobs or deny their grants.
4. Add **Scenario** as a versioned research description/context. It relates Jobs,
   datasets, models, experiments and software; it is not a scheduler, Profile,
   Runtime, a coordination Task or a new synthetic execution receipt.
5. Research/Operations, main/support/test and curated catalog visibility are query
   dimensions. Actual queue occupants remain visible, and history defaults to All
   with independent filters. Only explicit execution membership folds real nodes.
6. Correct inherited business behavior where documented. Python reference output is
   an oracle for preserved contracts, not for known bugs. Each deliberate change
   has a failing-before/expected-after acceptance case and a preservation decision.

## Design package

| Document | Decision it makes |
|---|---|
| [ARCHITECTURE](ARCHITECTURE.md) | Crates, process authorities, concurrency, ports and dependency choices |
| [DOMAIN](DOMAIN.md) | Job/execution/Step/Attempt/Scenario types and state/effect invariants |
| [AUTHORING](AUTHORING.md) | Rust SDK/templates, built-ins, hybrid integrations and Loom Use co-change |
| [API-CHANGES](API-CHANGES.md) | Shared action registry, additive APIs and compatibility |
| [MIGRATION](MIGRATION.md) | Additive schema, storage, publication, cumulative update and rollback |
| [COMPATIBILITY](COMPATIBILITY.md) | Exact old bytes/IDs/protocols and retained boundaries |
| [ROADMAP](ROADMAP.md) | Dependencies, deliverables, exit tests and implementation sequence |
| [REQUIREMENTS](REQUIREMENTS.json) | User requirements mapped to designs, phases and acceptance cases |
| [REVIEW](REVIEW.md) | Independent findings, uncertain evidence and correction backlog |

## Block designs

| Block | Primary responsibility |
|---|---|
| [identity](blocks/identity.md) | Principal/scope decisions; retained browser/provider adapters |
| [api](blocks/api.md) | HTTP/MCP application adapters and one action registry |
| [queue](blocks/queue.md) | Eligibility, order, holds, lanes and authoritative occupancy |
| [execution](blocks/execution.md) | Plans, Steps, Attempts, tickets, return and callbacks |
| [recovery](blocks/recovery.md) | Leases, shared retry budget and attempt/finalization recovery |
| [results](blocks/results.md) | Immutable publication, lineage and historical location bindings |
| [extensions](blocks/extensions.md) | Component/Tool/Profile/Runtime/dependency registries |
| [catalog](blocks/catalog.md) | Typed read projections and immutable historical views |
| [scenario](blocks/scenario.md) | Versioned research description and many-to-many links |
| [research-operations](blocks/research-operations.md) | Purpose, visibility, grouping and history filters |
| [authoring](blocks/authoring.md) | Rust-first packages, SDK, templates and agent guidance |
| [dashboard](blocks/dashboard.md) | Retained web plus Scenario and composition views |
| [observations](blocks/observations.md) | Bounded host/resource observation services |
| [diagnostics](blocks/diagnostics.md) | Private bounded diagnostic records |
| [onboarding](blocks/onboarding.md) | Live schemas/guides, client pins and authoring discovery |
| [network](blocks/network.md) | Retained provider/Sharing/Gateway boundaries |
| [ssh](blocks/ssh.md) | Retained protected personal-account helper |
| [portability](blocks/portability.md) | Identity-preserving snapshots and writer fencing |
| [installation](blocks/installation.md) | Matching binaries/images, roots, upgrades and rollback |
| [testing](blocks/testing.md) | Independent Rust, protocol, browser, package and performance evidence |

## Completion definition and non-goals

A Rust build alone is insufficient. Operational completion requires the frozen compatibility
matrix, corrected business cases, actual retained-client/browser acceptance, durable
fault tests, immutable package acceptance and a separately authorized cutover.
No research dataset re-generation, model re-training, host cleanup, external sharing
change or automatic general reconciler is part of this redesign. Bounded verified
historical bindings and explicit Scenario suggestions are included; autonomous
inference/repair remains future work. No performance gain for Isaac/ML compute is
promised merely because the control plane changes language.
