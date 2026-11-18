# Incremental build and verification plan

Status: proposed — original roadmap. Implemented commands and remaining release boundaries are tracked in [BUILD.md](../BUILD.md).

## Decision

Adopt a real target/action graph rather than adding more conditional shell commands.
Use **Bazel as the preferred integration candidate**, subject to the representative
pilot below. Retain Cargo manifests/lockfiles for Rust dependency authority and IDE
work, and retain BuildKit for OCI assembly. Do not build a private replacement for
Bazel's scheduler, content store, sandbox and test-result cache.

This is a staged decision, not a claim that Bazel is already compatible with the
current workspace. Remove the confirmed invalidation/side-effect problems first;
then evaluate native Rust, generated Dashboard assets and actual API tests together.
If that pilot fails the acceptance conditions, keep Cargo + BuildKit and scoped
checks while correcting the blocker. Do not migrate all repositories speculatively.

## What is already incremental

Cargo already retains compilation artifacts. Docker already has layer caching.
What is missing is a coherent graph connecting generated files, native binaries,
test inputs/results, image assembly and distribution. The current full-suite
runner is not incremental. These are distinct problems and need distinct evidence.

| Layer | Current state | Proposed boundary |
|---|---|---|
| Rust compilation | Cargo cache; overly broad core build-script inputs | Actual crate/contract inputs; isolated release identity |
| Generated assets/reference | Python generators, embedded guides and checked-in HTML | Pure declared-input targets, explicit output directories |
| Verification | Full sequential suites; coarse feature ownership | Declared test targets, affected selection and valid result reuse |
| Images/packages | Broad source snapshots and release identity before code compilation | Validated component artifacts followed by immutable release assembly |
| Deployment | Whole-stack preservation/restart workflow | Separate explicit operation; never a cached build target |

## Inspection scope

The audit read all **684 tracked files** across the working source trees:
Loom 532, Pale 112 and Loom-Use 40. It parsed every tracked Python source without
executing it, inspected Rust crate/module/include references, and reviewed the
build/generation/test/package/CI paths in detail. The snapshot includes the pending
Home Job-group changes; it is not just the last committed release.

This is a whole-tree **build/dependency audit**, not a claim that every business
logic path has been revalidated. Static imports cannot prove dynamic dependencies.
See the [audit and limitations](AUDIT.md) and [per-file inventory](audit-inventory.json).

## Confirmed reasons for wasted work

1. `loom-core/build.rs` fingerprints all of `rust/` and `loom_use/`. A host/SDK or
   guide edit therefore invalidates core and its consumers, beyond Cargo's declared
   package graph. The script also embeds build identity into core.
2. Every numbered snapshot/release changes build IDs/timestamps before compilation.
   All six crates also inherit the changing release version. Compile identity and
   distribution identity are conflated.
3. `tests/run.py` has no target selection and runs suites sequentially.
   `tests/rust/run.py` builds/lists everything and launches every native case.
4. Transitive feature impact maps a UI CSS edit to **18 of 20 blocks / 88 test
   files**. Some Markdown guides instead have **zero mapped impact**. Review
   relationships cannot be used as executable build dependencies.
5. One 4,653-line Rust test file is referenced by 15 feature blocks. The core test
   binary and shared Python browser classes are broad units of invalidation.
6. Release builds and installation/build helpers mix pure artifact work with
   host state, privilege escalation, Docker discovery and side effects.

These are source findings, not inferred from a test count. Precise paths and
exceptions are recorded in [AUDIT.md](AUDIT.md).

## Build-system comparison

| Option | Useful capabilities | Cost/limitation here | Decision |
|---|---|---|---|
| Cargo + BuildKit + existing scripts | Existing compiler/layer caches; quickest first fixes | No unified cross-language test/artifact graph | Keep as baseline and staged fallback |
| Make/Ninja | Lightweight explicit file DAG and scheduling | Does not supply the required hermetic content/result cache by itself; a substantial custom layer would remain | Not the preferred end state |
| Bazel + rules_rust/rules_python | Explicit input graph, target queries, sandboxed actions and build/test caches | Requires dependency/runfile/toolchain cleanup and a real pilot | Preferred candidate |
| sccache / cargo-nextest | Compiler caching / native case scheduling and filtering | Complementary tools; neither is the whole product graph | Optional measured optimizations |

Ninja explicitly operates as a low-level build executor ([manual](https://ninja-build.org/manual.html)).
Bazel documents action-based [remote caching](https://bazel.build/remote/caching)
and input/tool isolation ([hermeticity](https://bazel.build/basics/hermeticity)).
Those capabilities justify evaluating Bazel; they do not fix undeclared inputs or
monolithic targets automatically.

## Implementation order

| Phase | Deliverable | Exit condition |
|---|---|---|
| P0 | Measured baseline and dependency inventory | Timings/rebuild reasons for no-op, CSS, guide, core, host and SDK changes |
| P1 | Pure generators/build entry points; narrow Cargo/BuildKit inputs | No-op does no substantive work; builds never mutate installation state |
| P2 | Build identity, contracts/guides and host protocol boundaries | Guide/SDK edits do not unnecessarily rebuild transactional core/host |
| P3 | Bazel pilot: native Ops + Dashboard generation + native/API tests | Real dependencies, behavior parity and demonstrated cache reuse |
| P4 | Verification graph and CI cutover | Affected runs explain selection; reused evidence has exact valid inputs |
| P5 | Component packaging and installer/release identity | Same-source components reused without reporting false provenance |
| P6 | Apply shared conventions to Pale and Loom-Use | Existing independent installation and source-pin contracts preserved |

Details, file moves, acceptance tests and reversibility are in
[IMPLEMENTATION.md](IMPLEMENTATION.md). The target/API/cache design is in
[ARCHITECTURE.md](ARCHITECTURE.md). [VERIFICATION.md](VERIFICATION.md) defines
independent acceptance blocks, not an instruction to run every check on every edit.

## Boundaries

- No research Runtime, model weights, datasets, credentials or production DBs enter
  the build graph/cache. This graph builds the platform, clients and fixtures.
- Keep Job state, transactional ownership, recovery/resource-return fences,
  canonical hashes and current public contracts intact during extraction.
- Preserve existing feature documentation as the review map; do not replace it
  with BUILD files or turn build targets into user Job gates.
- Compiler cache hits, cached test passes, skipped checks and live acceptance are
  reported separately. A changed binary invalidates its test-result cache.
- No numerical speedup is promised before controlled measurements. The previous
  963-check record totals about 366 seconds in suite time; image building,
  transport, backup and CI wait are additional, separate costs.

## Explicit exclusions from this plan

No framework rewrite, microservice split, operational DB migration, deletion of
compatibility tests merely because they use Python, automatic dependency
installation, shared remote cache service setup or deployment is authorized by
writing this plan. Existing pending functional changes stay separate.
