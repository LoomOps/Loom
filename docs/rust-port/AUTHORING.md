# Rust-first authoring, built-ins and Loom Use

This policy governs new source produced for Loom, not automatic conversion of
existing user revisions. The control plane, authoring SDK and producer runtime are
separate products. Changing API implementation language alone does not make an
agent's generated runner Rust.

## Authoring policy

| Work | Default | Python exception |
|---|---|---|
| New Component lifecycle orchestration | Rust binary using `loom-sdk` | Narrow external SDK stage with documented integration |
| New Tool / planner / reusable policy glue | Rust executable/library with stable command protocol | Library-dependent algorithm or external framework with a concrete import/ABI need |
| New Job runner, fan-out/batching, progress and manifest writing | Rust orchestration in an exact Profile/Runtime | Python scientific stage remains separate and pinned |
| Loom-owned bounded smoke / file verification / producer examples | Rust package built by the release | No Python wrapper merely around pure control logic |
| Isaac Sim/Lab, PyTorch, Python model/dataset APIs | Rust caller + isolated Python worker where useful | Keep framework code and dependencies; do not reimplement these ecosystems |
| Existing sealed Python package/Profile/Job | Preserve exact accepted revision | New conversion is an explicit new revision, not a mutation |

Preference is communicated in server guides, templates and Loom Use Skills, with
build/source metadata to make it inspectable. Language is not a new authorization
scope or a blanket ban on Python. Do not force FFI/PyO3 into every hybrid package:
a subprocess boundary with versioned stdin/files and explicit cancellation is the
default. In-process FFI requires measured need, audited interpreter/GIL/thread
ownership and a separate lifecycle design; it cannot import Python into Ops state.

## Producer SDK contract

`loom-sdk` supplies typed request-file decoding, safe configured input/output
locators, stable Step IDs/sequences, stage timing, output manifest writing, bounded
structured errors and cancellation cooperation. Preserve existing request/manifest
versions through codecs. It does not contain scheduler access, credentials, Docker
socket control, arbitrary retries or scientific acceptance policy.

Step progress records identity (Job, execution, Attempt, Step), producer sequence,
phase/status and recorded timestamps. Publish atomically under the granted output
root. Duplicate observations are deduplicated; absent/out-of-order progress remains
unknown or is rejected without falsifying completion. Final result publication
stays the platform's responsibility after verified process/container return.

## Draft -> build -> publish -> submit

1. Discover current guide/actions and reusable exact Profiles/Components/Tools.
   Parameter/seed/input changes reuse a compatible Profile; do not create a copy
   named after every task. Inspect Scenario context without changing permissions.
2. Generate a draft Cargo project from a versioned template. Keep draft files in
   an authorized workspace, not a mutable registered revision. Templates cover
   a Tool, a lifecycle Component and a Rust-orchestrated Python SDK worker.
3. Validate source inventory, interfaces and declared Runtime requirements. Build
   in a pinned builder Runtime or explicitly authorized development environment;
   compiler/network access is not inherited from client setup or registration.
4. Produce an immutable build receipt: source/package hash, Cargo.lock hash,
   toolchain, target/libc, build Runtime/image digest, executable hashes and exact
   entrypoints. Reject path escape, symlink tricks or inconsistent artifact hashes.
5. Register source plus a verified artifact locator/reference through the versioned
   package contract. Registration seals bytes/pins; it never runs `build.rs`, fetches
   crates, imports a module or installs dependencies inside an API transaction.
6. Register/reuse a Profile binding the Component, Tools, stage commands and exact
   Runtime revision. Submit one Job/outcome with a stable key; parallel build/run
   units belong inside a supported plan, not a clutter of unrequested top-level Jobs.

Builds and Cargo build scripts are untrusted package execution. Run in isolated,
non-root, resource-bounded builder containers with explicit source/output grants.
Dependency cache location comes from the installed Runtime/dependency plan. Frozen
lockfiles and verified cache support reproducibility; offline is not presumed if
crates are missing. Compiler images are not mounted into ordinary runtime Jobs.

## Package fields (advertised by the native candidate)

A new optional versioned `implementation` manifest describes entrypoints, source
languages, build receipt and Python integrations. Each integration lists stage,
external package/SDK, pinned Runtime dependency and concrete reason. Do not use
README text as execution policy. Old missing metadata is `unrecorded`, not a guessed
Rust/Python classification or a reason to reject a previously accepted package.

Binary files use verified registered locators, not UTF-8 `files` strings or a giant
base64 MCP message. Exact source files can retain the current text-file API. An
artifact receipt binds the binary to the same source revision/target; no mutable
`target/release` path is an installed executable. Build caches are not result data.

## Built-in migration

The native release supplies `loom-sdk` CPU smoke and installation producers.
`deploy/installer/smoke.py` selects the release-built producer when the kit contains
its native artifact manifest; the retained branch remains for old Python kits.
Both native producers use the ordinary Job/result path. Preserve the
public bounded smoke parameters, old Jobs and output semantics through adapters.
Expose Built-in as origin, not a claim to ownership of user research code.
Version package IDs/receipts by immutable source; never overwrite a registered
smoke Component when upgrading or require a host compiler to accept installation.
No unnecessary demo research software ships as a platform built-in.

## Loom Use changes (future coordinated release)

Update `skills/loom-use/SKILL.md`, `skills/loom-ssh/SKILL.md`, README, SSH/verification
instructions and packaged examples together. They must explicitly say: discover
reuse first; prefer Rust for new packages/runners; use Python only for justified
external integration; use the configured dependency root and exact Runtime pins;
submit one requested outcome and preserve true Steps/Attempts/Scenario lineage.
The client remains schema-transparent; it never generates undocumented fields,
installs a compiler or runs migration/build code on connection/guide refresh.

Prefer server-owned examples/SDK compatibility metadata to copied schema snippets
in Skills. Publish a new immutable client package only if distributed Skill bytes
change, then pin it from Loom and test installed-client/guide refresh. Existing
connections, credentials and running pins remain unchanged. Test behavioral
fixtures and generated packages, not merely presence of the word Rust in a Skill.
