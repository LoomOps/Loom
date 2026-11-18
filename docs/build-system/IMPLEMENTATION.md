# Implementation stages and affected files

Current implementation: [BUILD.md](../BUILD.md). This document retains the broader target design/backlog.

Status: proposed — this is an implementation backlog, not a record of completed refactoring.

## P0 — Establish the comparison before changing architecture

Inputs: current working source snapshot, toolchain/platform and existing suite
receipts. Preserve pending functional changes as a separate baseline; do not mix
new Job behavior into a build-system migration commit.

Produce cold/warm/no-op timing and rebuild-reason records for:
unchanged tree, CSS-only, guide-only, core query, host wire, SDK, dependency lock,
test-only and release-envelope-only changes. Use disposable worktrees/output roots.
Record compile, generation, tests, image assembly, transfer, preservation and CI
queue time separately. Do not run operational deployment to benchmark a compiler.

- Read Cargo JSON/timing/fingerprint diagnostics and BuildKit progress.
- Count actual compiler/link/test executions and output digests, not just elapsed
  time; record CPU/memory, cache size and clean-build baseline.
- No-op generation must compare bytes and leave outputs unchanged.
- Exclude real credentials, DBs and research assets from all benchmark fixtures.

Exit: observed action sets and distributions for repeated runs. Existing ~366s
suite time is historical context; it is not the baseline for every future host.

## P1 — Make current entry points safe and declared-input friendly

| Files | Change | Consumers to verify |
|---|---|---|
| `deploy/build.py`, `snapshot.py`, `release.py` | Guarded main; pure plan/capture/build APIs; explicit source/output inputs; no import-time escalation | CLI/help, release tests, immutable numbering |
| `deploy/compose.sh` and build wrappers | Separate image compilation from installation Compose environment selection | Dev-build/install wrappers, no default host-state writes |
| `dashboard/workspace/build.py`, `lazy_logs.py`, `dashboard/dev.py` | Exhaustive input/file-set declaration, explicit output, atomic write-if-changed | Generated workspace hash, live dev refresh, browser fixtures |
| `deploy/rust_build.py`, `native_artifacts.py` | Explicit selected targets/profile/toolchain and artifact manifest | SDK kit, image extraction, matching native artifacts |
| `deploy/install-kit.py`, `source_install.py` | Package prebuilt accepted outputs; retain completed-build resume | Offline/local kit equivalence and source matching |

Keep operational paths out of pure generator/build interfaces. A compatibility
CLI may still select an installation explicitly for a later operation. Do not
remove existing commands before wrappers/tests have migrated.

Add persistent BuildKit caches where appropriate, partitioned by toolchain,
platform/features/profile and with safe concurrent sharing. Preserve dependency
and final artifact verification. Pin builder/runtime image digests through a
reviewed update mechanism; do not make "use whatever is latest" the cache input.

Exit: current Cargo/BuildKit path benefits before any Bazel commitment; old/new
entry points produce the same logical artifacts and no build mutates host state.

## P2 — Remove the high-fanout dependencies

### P2a. Compile identity versus source/release identity

- Narrow `rust/crates/core/build.rs` to real generated inputs and their file-set
  membership. Do not hide guide/schema changes that still affect core code.
- Move the comprehensive provenance digest to release/component manifest creation.
- Make `onboarding.rs`, `store.rs` status and service CLI identity consume a typed
  composition-level identity provider rather than universal core compile env.
- Keep the installed build/status and numbered release contract until its explicit
  versioned replacement is ready. Core must not import a changing metadata crate.
- Review `Cargo.toml`, SDK source kit/templates and every `CARGO_PKG_VERSION` use
  before separating crate versions from the release ordinal.

### P2b. Contracts and reference assets

- Extract pure action schemas/DTOs/shared enums from `contracts.rs` and implementation
  enums such as lineage transforms into one authority.
- Build guide/reference assets separately from `loom_use/catalog.json`, referenced
  documents, client release lock and action docs. Maintain scoped serving,
  deterministic bundle manifests and offline delivery.
- Replace source-regex route documentation extraction with a shared deterministic
  route description/export when feasible; until then declare the dispatcher source
  as an explicit generator input.
- Adapt the Ops composition layer and reference-generation command. Runtime
  handlers keep using the same action registry; no parallel copied schema table.

### P2c. Host protocol and secure helpers

- Inventory `host/executor.rs`, `runtime.rs`, `runtime_files.rs`, `wire.rs`, `logs.rs`
  and callers in Ops/migrate before moving error/result, hash/canonical, bounded
  file/atomic I/O and pure Runtime manifest selection.
- Extract only the shared protocol/secure primitive layer. Host ticket/runtime DBs
  remain host-owned; Job/outbox/recovery transactions remain core-owned.
- Validate canonical bytes, UID/socket authentication, path traversal/symlink
  rejection, exact Runtime hashes and restart/unknown Docker effects through the
  same fault scenarios after extraction.
- Avoid replacing known error contracts or creating a generic public raw-SQL helper
  API for tests. Shared internal DB conveniences may remain duplicated only if
  they are genuinely unrelated implementation details; never copy policy logic.

Do not split all40 core modules in this phase. Keep the transactional aggregate;
start with the edges that compile unrelated material. Catalog/read-model extraction
is a later measured optimization if core remains the dominant incremental cost.

Exit: guide edits leave transactional core/host compilation untouched where
semantically independent; SDK changes leave unrelated core/host targets untouched;
all preserved behavior/failure boundaries still pass.

## P3 — Representative Bazel pilot, with an explicit adoption decision

Add pinned Bazel/module/toolchain declarations in an isolated development branch.
Use Cargo.toml/Cargo.lock-derived third-party targets through rules_rust and pinned
Python tools through rules_python. Include SQLite's C build, ring, proc macros,
actual Rust1.100 compatibility and the intended Debian runtime ABI.

The pilot must include all of:

1. The real empty-data Dashboard generator and its source/hash check.
2. Native core/domain/host/Ops dependencies and one real HTTP/MCP fixture.
3. A browser fixture consuming the newly generated HTML and the chosen native
   artifact, using explicit executable/resource inputs.
4. One image assembly/CPU acceptance path for the resulting binary, explicitly
   local and uncached if it uses the host Docker daemon.

Pure domain-only hello-world success is insufficient. Wrapping the old whole
workspace build in a genrule is also insufficient.

Adopt only if warm no-op/CSS/guide/native edits show correct action sets, cache
hits survive validated worktree/platform moves, clean builds pass, no implicit
host reads remain in cacheable targets, and maintenance is smaller than writing
a private scheduler/result cache. Record overhead as well as saved executions.

If the candidate fails, keep the P1/P2 improvements and scoped current runners;
record the blocker, do not partially replace release authority. The choice is
based on the representative pilot, not the tool's feature list alone.

## P4 — Verification graph and CI

- Replace suite-level all-or-nothing execution with declared test targets and
  exact artifact/runfile inputs. Class/file targets are the first Python/browser
  isolation unit; retained fixtures share state and cannot be arbitrarily cached
  per test method.
- Split `tests/rust/application.rs` by responsibility and relocate the100k scale
  correctness scenario to a separately scheduled target without shrinking it.
  Moving ordinary tests to modules improves ownership but does not alone split
  their compiled binary. Use stable APIs/protocol fixtures for independent targets.
- Classify retained Python tests as active integration, differential compatibility
  or obsolete-with-proven-replacement. Remove only the last category, with a mapping
  to the replacement behavior. Avoid new tests that assert build-file strings.
- Identify cases by repository, target and fully qualified test name. Replace the
  current global Rust function-name/regex association without losing existing
  case coverage; version the report reader during transition.
- Add graph-derived target references to the feature audit. Preserve one source
  owner and the separate review relation; do not duplicate the action input list
  in a second custom execution manifest.
- Keep current runners as compatibility entry points delegating to selected
  targets. Reject unmapped code/test/resource changes with a concrete explanation.
- Use engine target/action queries to explain invalidation; do not maintain a
  second reverse-dependency algorithm for the same execution graph.

Proposed developer experience after implementation:

```text
bazel build //dashboard:workspace
bazel test //dashboard:workspace_checks
bazel test //tests:developer             # hermetic targets; cached passes explicit
bazel test //tests:release              # complete required candidate scope
```

Names are proposed, not currently supported. The developer set excludes separately
scheduled scale/local-Docker/live checks; the release set includes the required
scale and actual final-artifact checks. Non-hermetic local acceptance is never
made current by a reused result. Test selection while editing is not a complete
release pass.

CI adds the same pinned graph/toolchains and read-only/authorized cache access.
Preserve logs/JUnit and per-target executed/reused states. No duplicate full run
for an unchanged main/tag candidate is required merely because it has two Git
references; publication still verifies the exact tested commit/artifacts.

Exit: no-op CI reuses valid targets, changed core artifacts invalidate their real
consumers, and complete audit cannot mistake omitted/skipped targets for passes.

## P5 — Component-aware release and update delivery

Only after P2 identity and P3/P4 artifact authority:

1. Assemble images from one accepted native/web artifact set; remove the second
   compilation currently embedded in image assembly only when equivalent-platform
   inputs and exact image/artifact matching are proven.
2. Separate release source inventory, runtime payload, developer/test payload and
   offline installer payload. Do not distribute tests to production just because
   the old snapshot includes them; preserve source/provenance/legal requirements.
3. Let a numbered release reference unchanged component digests explicitly.
   Update status/footer, guides, native manifest verification and kit/resume paths
   together. Compatibility with existing releases/updaters must be tested.
4. Transfer only missing content-qualified components/layers; do not load/export
   already-present complete images unnecessarily.
5. Add a distinct affected-component update plan. UI-only restart is allowed only
   when native/host/shared auth/network/storage/migrations are unchanged. Otherwise
   use the existing drained full update/preservation path.

Do not optimize a full DB backup away by assuming the installation is idle. Treat
operational preservation and rollback as their own contract. No deployment is a
Bazel cacheable target or a consequence of building `//...`.

## P6 — Pale and Loom-Use integration

Pale:
- Retain engine-source and binary SHA receipt validation in `build_core.py`.
- Keep Rust/schema inputs separate from Python adapters/instructions/package ID.
- Generate wheels/bundles into declared outputs; preserve normalized platform/ABI
  and engine handshake checks. Normalize archive metadata where reproducibility is
  required; avoid writing generated binaries/receipts into the working source.
- Reuse current Rust/pip CI caches; compare new graph against this baseline rather
  than claiming it has no incrementality.
- No service/role/Task/credential changes follow from build-system migration.

Loom-Use:
- Keep deterministic ZIP packaging and Python implementation.
- Add explicit module/test/package targets only when the shared approach is proven.
- Compatibility CI consumes exact candidate client/server artifacts; resolve any
  input branch name to its commit. No implicit use of a live user's connection.
- Do not add a Rust toolchain dependency to basic client installation or tests.

## Change organization and rollback

Use separate reviewable changes: pure entry points; build identity; reference
assets; host protocol; pilot rules; test targets/evidence; release assembly; updater;
Pale/client adoption. Each one lists changed behavior, preserved public contracts,
consumer tests and a measurable exit condition. Keep old build entry points until
the new path has passed clean and incremental cases. Do not keep two independent
release builders as permanent authorities.

A failed pilot removes its rules/tool configuration while retaining validated
P1/P2 improvements. A release/update failure follows its existing protected
recovery records; never restore an old operational DB to undo a build-tool change.

## Effort and next decision

P1 narrow invalidation and pure entry points are the smallest first deliverable.
Identity/provider extraction and host protocol work are medium scope. Bazel plus
ABI, fixtures, release/CI migration is a multi-change engineering project, not a
one-file configuration addition. No reliable calendar or speedup estimate is
claimed before P0 and the native pilot. Do not block useful P1 improvements on a
whole application rewrite.
