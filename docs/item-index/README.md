# Common Items, file inventory and log search — implementation plan

Status: historical design and migration record. The native implementation is present in this repository; release/deployment evidence is separate from this original plan. For current use read [the documentation index](../README.md), [implementation boundaries](../IMPLEMENTATION.md) and the generated API reference. Original proposed phases, baseline commit IDs and review counts below retain their historical meaning.

## Decisions

1. Give externally addressable research/software/execution records a common Item
   reference. Preserve existing native IDs, immutable revisions, manifests, scopes,
   Job request keys and current Runtimes navigation.
2. Use UUIDv7-generated 128-bit Item IDs, encoded as **decimal JSON strings**.
   Existing IDs remain native aliases; internal SQLite rowids never become public.
   Time ordering is approximate; no globally serialized number service is added.
3. Keep a compact authoritative registry/change journal in the core DB. Keep
   potentially millions of file entries in sealed, segmented inventory manifests
   and a **separate rebuildable search SQLite DB**, rather than one core row per file.
4. Distinguish logical file identity, immutable content evidence and physical
   copies. Equal bytes do not merge scientific records; moving a verified copy
   does not change a file's Item ID.
5. Treat logs as streams tied to Attempt, stage and effect generation; segments and
   lines are not top-level Items. Index text incrementally outside execution locks.
6. Use one authenticated query service for MCP, HTTP and Dashboard. Search queries never
   scan the filesystem, invoke Docker, install packages or infer scientific lineage.
   Registered byte reads may use the protected live collector capability.
7. Keep reconcile bounded and evidence-based. Derived repair is automatic;
   authority changes require a keyed, verified command. No autonomous host cleanup,
   inferred scenario assignment or general reconciler is introduced.

## Read in this order

| Document | Decision covered |
|---|---|
| [REVIEW](REVIEW.md) | Observed implementation boundaries, gaps and prerequisite corrections |
| [ARCHITECTURE](ARCHITECTURE.md) | Authority, DB split, Rust modules, crash protocol and backpressure |
| [IDENTITY](IDENTITY.md) | Item/revision/alias/file/content/location/stream identity |
| [FILES](FILES.md) | Stable inventories, path search, hashing, content extraction and scale |
| [LOGS](LOGS.md) | Live/closed streams, effect generations, rotation, retention and retrieval |
| [API](API.md) | Proposed actions, envelopes, cursors, authorization and client bounds |
| [RECONCILE](RECONCILE.md) | Evidence states and allowed automatic versus explicit repairs |
| [MIGRATION](MIGRATION.md) | Additive backfill, snapshots, restore, upgrade and rollback |
| [VERIFICATION](VERIFICATION.md) | Planned native/protocol/browser/fault/scale acceptance |
| [ROADMAP](ROADMAP.md) | Dependency order, concrete deliverables and stop conditions |
| [REQUIREMENTS.json](REQUIREMENTS.json) | Requirements-to-design/phase/case trace |

## All 19 existing feature blocks reviewed

Direct means a proposed contract or implementation changes. Associated means review
and regression only unless a concrete source finding requires a change.

| Existing feature | Impact | Detailed plan |
|---|---|---|
| identity | Direct: ID aliases and authorization at query boundaries | [identity](blocks/identity.md) |
| api | Direct: common Item/file/log discovery and bounded queries | [api](blocks/api.md) |
| queue | Associated: occupancy and scheduling must remain independent | [queue](blocks/queue.md) |
| execution | Direct: stream identity and nonblocking return/log capture | [execution](blocks/execution.md) |
| recovery | Direct: replay/fences and log generations | [recovery](blocks/recovery.md) |
| results | Direct: stable file inventories and content/copy evidence | [results](blocks/results.md) |
| extensions | Direct: Component/Tool/Profile/Runtime/dependency adapters | [extensions](blocks/extensions.md) |
| catalog | Direct: projections, query index and legacy/current resolution | [catalog](blocks/catalog.md) |
| observations | Associated: truthful index/collector observations | [observations](blocks/observations.md) |
| dashboard | Direct: search and related file/log navigation | [dashboard](blocks/dashboard.md) |
| onboarding | Direct: discoverable schema/guide/client co-change | [onboarding](blocks/onboarding.md) |
| network | Associated: existing ingress/Sharing permissions | [network](blocks/network.md) |
| portability | Direct: preserved identity and index rebuild across copies | [portability](blocks/portability.md) |
| installation | Direct: roots, quotas, schema and service lifecycle | [installation](blocks/installation.md) |
| testing | Direct: independent proof and feature ownership | [testing](blocks/testing.md) |
| ssh | Associated: personal SSH is not a search authority | [ssh](blocks/ssh.md) |
| diagnostics | Direct: strict separation from ordinary execution logs | [diagnostics](blocks/diagnostics.md) |
| scenarios | Direct: typed links and context-aware search | [scenarios](blocks/scenarios.md) |
| authoring | Direct: producer manifests and source/build inventory | [authoring](blocks/authoring.md) |

## Scope and completion

The first release includes structured/lexical retrieval, files, execution logs,
explicit verified bindings and deterministic rebuild. It does not index every
metric sample, tensor, log line, OS package or filesystem entry as a dashboard Item.
Credentials, private diagnostics and recipient-specific messages are not ordinary
search content. IDs do not change current sharing semantics or catalog curation.

Implementation completion requires the planned native and cross-boundary cases,
source-qualified feature audit, crash-safe rebuild and immutable image acceptance.
Operational rollout remains a separately requested phase. The target design does not itself establish implementation or
performance acceptance; the implementation ledger records actual evidence.
