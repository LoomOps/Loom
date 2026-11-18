# Whole-tree build and dependency audit

Current implementation: [BUILD.md](../BUILD.md). This document retains the broader target design/backlog.

Status: proposed — findings inform the migration plan; no product refactoring is claimed.

## Method and coverage

The baseline is the tracked working tree, including pending functional edits.
Every file was read and hashed; Python sources were AST-parsed, Rust imports,
includes and Cargo path dependencies inspected, and feature/test sharing counted.
No production source module was imported to discover dependencies. In particular,
`deploy/build.py` was read as text because importing it performs work.

| Repository | Tracked/read files | Python files | Rust files | Static read/parse errors |
|---|---:|---:|---:|---:|
| Loom | 532 / 532 | 190 | 78 | 0 |
| Pale | 112 / 112 | 59 | 15 | 0 |
| Loom-Use | 40 / 40 | 21 | 0 | 0 |

Counts are inventory counts, not executed test counts. Generated assets,
contracts and documentation are included. Source correctness outside the build
and verification boundaries has not been exhaustively proven.

[audit-inventory.json](audit-inventory.json) covers every baseline path, its source
hash, current feature owner, proposed artifact family and review depth. `structural`
means static inspection; `build-path` means the surrounding orchestration/consumer
logic was also read. It is not a claim of line-by-line semantic verification of
all operational modules. Dynamic imports and host effects require the pilot.

## Findings

### F01 — Compile fingerprint crosses independent crates

[`core/build.rs`](../../rust/crates/reference/build.rs) recursively hashes `rust/` and
`loom_use/`, emits those paths as Cargo rebuild inputs, and writes `LOOM_RUST_SOURCE`
into core. This includes independently declared host/SDK/service source and guides.
Thus a source-provenance convenience creates reverse invalidation absent from the
Cargo graph. The six-crate workspace does not isolate these edits effectively.

Fix: separate source provenance from compiler semantic inputs; give guide
code-generation its declared inputs and write output only when content changes.
Directory membership must also be tracked so newly added inputs cannot disappear
from invalidation. Cargo's [build-script change detection](https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection)
is the implementation baseline, not an argument to ignore real dependencies.

### F02 — Release identity invalidates code identity

[`snapshot.py`](../../deploy/snapshot.py) writes tracked Ops/Dashboard build-info
files and instance build.env. [`release.py`](../../deploy/release.py) generates
build-info in its immutable source copy; it does not mutate the checkout in the
same way, but still injects global release/version/timestamp identity before
compilation. [`Cargo.toml`](../../Cargo.toml) applies the release version to all
six crates. Core and binary consumers use compile-time identity.

Fix: distinguish component code identity, compile/action identity and release
assembly identity. Until the updater/status contract is adapted, keep current
numbered release identity and optimize compilation layers only. Do not silently
reuse an old image while claiming it was built from a new source snapshot.

### F03 — Full invalidation in OCI builder stage

[`Dockerfile.rust`](../../ops/Dockerfile.rust) copies source/guides/build-info before
one workspace release build. It has no registry/target cache mounts. Layer hits
help unchanged inputs; a changed input loses the compile RUN layer and its
intermediate output unless an explicit cache is retained. Loom's base tags are
not digest-pinned here; Pale's Dockerfile does pin them.

Fix: declare dependency/toolchain/base digests, use correct per-platform caches,
and eventually assemble final images from the one accepted native artifact set.
BuildKit's [cache guidance](https://docs.docker.com/build/cache/optimize/) supports
cache mounts and external caches; it does not make host Docker state a test input
that can be ignored.

### F04 — Pure build and host effects share entry points

[`build.py`](../../deploy/build.py) escalates/captures/builds at module scope.
[`compose.sh`](../../deploy/compose.sh) discovers installation environment and may
create host directories even for a build invocation. Default paths refer to
installation state. `release.py` accepts an instance parameter although its image
build commands primarily consume the snapshot.

Fix: pure plan/generate/build functions and a guarded CLI; explicit output roots;
separate install/deploy commands. Parsing/help and import must have no host effect.
Do not wire these commands unmodified into a cacheable action.

### F05 — Review graph saturates and misses some inputs

[`features.py`](../../deploy/features.py) is an ownership/review tool. Its
transitive samples return 18 blocks / 88 test files for workspace CSS, queue UI,
core query, host wire, SDK and the core build script. `loom_use/jobs.md` returns
no owner/tests through that path. Markdown does have separate documentation
classification, so this is a cross-tool mapping gap, not an unowned guide product.

Fix: add an actual artifact/test graph. Keep feature consumers as review hints;
explicit target inputs include the guide catalog and every referenced guide.
Unknown changes are explained and conservatively selected, never treated as no-op.

### F06 — Current audit cannot consume mixed incremental evidence

[`tests/suite.py`](../../tests/suite.py) records hashes of all catalog inputs.
[`features.audit`](../../deploy/features.py) checks a whole-suite successful flag,
each feature's expected cases and feature-owned source hashes across reports.
It has no per-target dependency closure, execution environment identity or
validated cache-reference record.

Fix: target receipts and current-input verification, with executed/reused/unknown
states. Keep the existing complete audit available during transition. Global
source hashing is not itself the dominant time cost and is not a replacement
for actual dependency declaration.

### F07 — Test execution and test compilation are coupled too broadly

[`tests/run.py`](../../tests/run.py) runs Rust/native, Ops, Dashboard, packaging and
optionally Firefox in sequence. [`tests/rust/run.py`](../../tests/rust/run.py)
compiles the workspace, lists all binaries, launches each case separately, builds
all binaries and runs every protocol fixture. The process-per-case isolation is
intentional; retain isolation when using another scheduler.

The previous measured suite totals were Rust60.0s, native59.2s, Ops62.1s,
Dashboard22.8s, packaging8.6s, Firefox153.5s. These are one observed warm-host run,
not universal or cold-build estimates. The 100k inventory correctness case alone
was about28s. No new performance run was performed for this planning task.

### F08 — Feature boundaries do not equal compilation/test targets

[`application.rs`](../../tests/rust/application.rs) is 4,653 lines and shared by
15 feature blocks; [`test_service.py`](../../tests/rust/test_service.py) is shared
by9. Core has40 source modules and30 `impl Application` modules. These share
crate-private transactional helpers and compile together.

Fix: split feature test modules/expensive scenarios and identify stable library
boundaries. Moving files within one crate alone does not create a compile cache
boundary. Do not expose raw operational SQL or change transactions only to make
integration-test targets easier.

### F09 — Host consumes transactional core for small shared primitives

[`host/executor.rs`](../../rust/crates/host/src/executor.rs),
[`host/runtime.rs`](../../rust/crates/host/src/runtime.rs) and
[`host/wire.rs`](../../rust/crates/host/src/wire.rs) import core errors, hashing,
rows/clock, storage helpers, compatibility validation and Runtime selection.
Ops also composes both host and core. This is a real extraction boundary, not a
reason to split every core module into a service.

Fix: extract a small typed protocol/security/filesystem boundary and pure Runtime
manifest validation; preserve exact wire/hash/permission behavior. Keep host
Docker access and core database mutations separate.

### F10 — Guides, schema descriptions and runtime handlers share core

[`onboarding.rs`](../../rust/crates/core/src/onboarding.rs) embeds guides,
client lock, action descriptions and Job schema. [`contracts.rs`](../../rust/crates/core/src/contracts.rs)
references the lineage transformation enum in an implementation module.
[`documentation.py`](../../deploy/documentation.py) loads the compiled native
reference and also reads the HTTP dispatcher source for convenience routes.

Fix: one contracts authority plus a separately built reference/guide asset.
Move shared enum definitions toward that authority; handlers import them.
A new guide/reference crate that core still depends on would preserve much of the
unwanted invalidation. Inject the reference/provider at service composition.

### F11 — Working-directory, environment and external inputs are implicit

42 Python test files use dynamic import or sys.path mechanisms. Native/browser
fixtures address `target/debug/loom-ops` directly; image tests inspect Docker;
client fixtures acquire a pinned external package; browser tests need Firefox,
geckodriver, fonts and system libraries. These are not all hermetic/cacheable today.

Fix: explicit artifact/fixture arguments and runfile-aware resources; pinned
fetch/toolchain targets; local uncached targets for daemon/live boundaries.
Do not assume that an AST import list covers subprocess or dynamically loaded data.

### F12 — Release transfer/preservation is another cost center

[`install-kit.py`](../../deploy/install-kit.py) exports complete image archives;
[`source_install.py`](../../deploy/source_install.py) has a useful completed-build
resume path but still copies full source/platform trees. The updater preserves
DBs/settings and validates real host parity. Compilation/result caching cannot
replace these checks.

Fix: content-qualified artifact transport and affected-component update planning
only after release identity and compatibility are explicit. Never cache drain,
health, storage identity, pending-effect or rollback-preservation observations.

### F13 — Pale already has selective reuse worth retaining

[`scripts/build_core.py`](https://github.com/LQD-labs/Pale-dispatcher/blob/main/scripts/build_core.py)
checks the existing native engine against source and binary receipts. Its core
fingerprint is limited to Rust/schema/Cargo/build inputs. CI already caches Rust
and pip. Package fingerprint is intentionally broader than engine fingerprint.

Its bundle builder writes native receipts under source and tar metadata is not
fully normalized. Docker rebuilding still lacks persistent target cache mounts.
Refactor packaging into explicit-output actions; do not rebuild the engine for a
Markdown adapter edit or remove existing source matching. Preserve wheel/bundle
behavior without borrowing a development binary.

### F14 — Loom-Use needs packaging/test targets, not Rust migration

The client is21 Python files and has a deterministic sorted ZIP builder with
normalized timestamps/modes. Its independent source tree/lock belongs in the
server-client compatibility target. Integration CI currently builds a selected
server ref (default main); cache keys must resolve that ref to its exact commit.

Keep its small implementation and Python tests. Feed the produced archive and
its SHA into the native MCP fixture; do not duplicate server contract prose or
rewrite the client in Rust just to share a build engine.

## Preservation and non-findings

- Cargo and Docker caches do exist; there is no evidence that every compiler
  invocation starts from zero.
- Pale's engine source/binary receipt and Loom-Use's deterministic ZIP are useful
  existing building blocks, not legacy code to discard.
- Shared SQLite transactions, independent host authority and a pinned empty-data
  Dashboard template are intentional boundaries. Preserve them.
- Compatibility-test deletion requires a demonstrated replacement or obsolete
  contract. Python language usage or a large test count is insufficient evidence.
- No source-runtime DB, credential, scientific artifact or host installation was
  accessed to produce this build audit.
