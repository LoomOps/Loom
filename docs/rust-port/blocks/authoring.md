# Rust package and agent authoring — detailed integration block

Status: new coordinated development block. Phases D1/D2/D6/D7.
Read the full [authoring contract](../AUTHORING.md) for workflow and package fields.

## Ownership and operations

`loom-sdk` owns producer request/progress/result types; `loom-author` owns pure draft
scaffolding/validation; `loom-software` and the app own registration/build evidence.
The protected build/Runtime driver executes compiler/build scripts in isolation.
The Loom Use Skill directs agents to reuse and Rust-first generation but cannot
install a compiler or grant host permissions on connection.

Draft operations are side-effect-free archive/manifest generation. Build execution
is a separately authorized bounded build-runtime action/ordinary internal work
unit. Register seals exact source/build refs and Profile binds exact stage/runtime
pins. Never embed `cargo build` in a registration handler or trust a mutable local
executable path as an immutable package.

## Concrete work packages

1. Specify backward-readable request/result/progress envelopes and Rust/Python
   compatibility fixtures, cancellation behavior and safe locator handling.
2. Create Cargo templates for Tool, Component lifecycle and hybrid external-SDK
   worker; pin template/SDK versions and avoid per-Job hard-coded roots.
3. Package source/Cargo.lock/build receipt/target/entrypoints. Validate resource,
   path, source/hash and Runtime compatibility before publication.
4. Supply release-built Rust bounded CPU/installation-smoke packages through the
   normal producer/Profile/Job/result route. Preserve old smoke API and histories.
5. Update server jobs/components/runtimes guides and distributed Loom Use/SSH Skills
   with the same policy and capability checks. Keep old clients schema-transparent.
6. Test representative agent workflow inputs: reuse existing Profile for seed/input
   changes, generate Rust for pure orchestration, isolate justified Python SDK code,
   and submit one outcome with true internal stages and optional Scenario context.

## Transaction and compatibility

No build/import/network request runs in a registration transaction. A build receipt
is validated against exact source and Runtime/target and committed under the same
admission/epoch guard as its revision. Source-level language metadata does not
change access rights. Existing Python/legacy revisions remain intact/executable;
new implementations are new immutable revisions. Compiler/network caches live in
configured dependency/build roots, not hard-coded /dependencies or data paths.

| Case | Trigger | Required result |
|---|---|---|
| AUTH-01 | New pure batching/orchestration runner requested | Rust source/SDK path with exact reusable profile discovery |
| AUTH-02 | Cargo build script in registration input | Validated as source only; never executed by API |
| AUTH-03 | SDK-heavy Isaac/PyTorch integration | Explicit pinned Python worker, Rust orchestration where useful |
| AUTH-04 | New params/seed only | Reuse compatible exact Profile/Tool, no task-named copies |
| AUTH-05 | Wrong target/hash/runtime artifact | Reject before publication/launch; no fallback to host executable |
| AUTH-06 | Existing Python Job during update | Original source/Profile/Runtime pins and execution remain unchanged |

Independent SDK/package/protocol tests and packaged Skill workflow fixtures are
required. Documentation mentions alone are not evidence agents follow the policy.
