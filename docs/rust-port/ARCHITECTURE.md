# Rust application architecture

This design is implemented as six crates with internal ownership modules; see
[the concrete mapping](IMPLEMENTATION.md). Dependencies point inward:
transport and repositories call application operations; pure domain types never
import SQLite, Tokio, HTTP, Docker, Python or UI code. File names in the legacy
repository do not determine crate boundaries.

## Crates and dependency direction

| Logical boundary | Owns | Permitted dependencies / exclusions |
|---|---|---|
| `loom-domain` | Typed identities, Job/execution states, readiness, recovery and Scenario policies | Standard types and domain errors; no I/O or generic JSON state model |
| `loom-contracts` | Versioned public/private DTOs and one action/schema registry | Domain IDs; JSON only for metadata, schemas and compatibility envelopes |
| `loom-app` | Authorized commands, consistency checks, transaction/effect plans | Domain + repository/effect ports; no HTTP handler or Docker client |
| `loom-store` | Existing SQLite codecs, migrations, transaction unit of work | Repository port implementation; no externally callable SQL/helper interface |
| `loom-query` | Queue/history/catalog/Scenario/software projections | Read repositories; cannot mutate or launch work |
| `loom-storage` | Marker-verified locators, safe file publication and hashing | Filesystem ports; no arbitrary shell or scientific metadata guessing |
| `loom-software` | Package validation, pins, composition and build receipts | Application/domain ports; no compiler invocation in registration transaction |
| `loom-api` | HTTP/MCP/auth context, bounded decoding, streaming and TLS | Contracts + app/query; no copied domain rules |
| `loom-ops` | Per-instance service composition and supervised worker/effect loops | Wiring only; exposes no internal mutation RPC to Python |
| `loom-executor` | Privileged host launch/cancel/return authority and Runtime registrar | Protected socket, durable ticket DB, exact grants; no public API credential |
| `loom-observer` | Host/Docker observations under existing read-only privileges | Bounded collectors, no scheduling or registration ownership |
| `loom-migrate` | Offline schema/codec checks and explicit cumulative migrations | No worker startup or credential issuance |
| `loom-sdk` / `loom-author` | Rust producer protocol, draft templates and package validation | Same contract crate; no scheduler DB access or extra daemon |

These are module/ownership boundaries, not a requirement for thirteen processes.
Keep crates small enough to enforce trust and testability; combine internal modules
when a separate package would only add plumbing. No Wasm, plugin ABI or event-sourced
rewrite is required for the first release.

## Processes and authority

Retain the installation's existing trust layout: per-instance Ops, a protected
host Executor, web/Gateway services, read-only observers and administrator helpers.
The current `ops/serve.py` supervises separate Python API and worker processes
inside one Ops container. The target Rust composition combines the application
service within that same container/trust envelope and replaces this supervisor. A worker lock and installation epoch fence enforce one execution
coordinator. All application mutations use the same transaction discipline even
when a request arrives on another thread. No old/new worker runs concurrently.

Inside Ops, Tokio owns bounded network/timer/effect tasks. A dedicated bounded
SQLite write lane executes synchronous transactions with rusqlite; handlers await
a reply instead of blocking async runtime threads. Read projections use bounded
read connections/snapshots. A full command queue returns a typed unavailable/backoff
response before acceptance; accepted durable work is not silently dropped.

Transactions read current grants, admission/epoch, Job/Attempt/revision and intent,
apply a pure decision, write state plus outbox/receipt, then commit. Filesystem,
Docker, network and long hashing/build work run outside the transaction. A second
transaction verifies the same identity/version/epoch and records its evidence.
A dropped future or timeout is not cancellation evidence. In particular, started
blocking tasks cannot be stopped by Tokio abort; use dedicated bounded workers
and explicit process/container reconciliation. [Tokio behavior](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html).

## Closed ports

Repository ports expose `load_job`, `save_transition(expected_version, ...)`,
`load_receipt`, `append_effect` and scoped query operations; they do not accept SQL
strings or arbitrary callbacks. Effect ports expose typed launch/cancel/stat/hash/
publish/probe operations with stable identities and evidence types. Python adapters
cannot select a next Job state, mark a ticket returned or manufacture result ACKs.

Private Executor envelopes remain compatible initially. Version negotiation adds
capabilities before any new operation. A stale helper cannot silently accept an
unsupported launch plan. Ops has no Docker socket; Jobs receive no platform or
native-control socket, credentials or ability to register arbitrary Runtimes.

## Retained integration code

Retain HTML/JS/CSS, iframe/build/template pipeline, existing Python browser SSO/
Sharing/Gateway integrations and administrator installation drivers initially.
They use bounded public or protected typed interfaces and retain their current
privileges. The Python Loom Use client stays generic transport/setup/refresh.
Scientific Python lives in explicitly pinned Job Runtimes or narrow worker
processes, never inside the Ops mutation authority.

## Dependencies and build decision

Locked implementation stack: Tokio for I/O/timers, axum/tower for HTTP, serde for wire
DTOs, rusqlite for the existing SQLite boundary, and a Draft 2020-12-capable schema
validator. Exact versions/MSRV and the TLS provider are locked in Cargo.lock and
rust-toolchain.toml; the native kit includes dependency licenses/SBOM. `latest` is never an installed artifact pin.
Use a standards-tested MCP transport implementation only if it preserves current
JSON/SSE/error/notification behavior; a framework choice does not change schemas.

[Axum's documented responsibility](https://docs.rs/axum/latest/axum/) is routing and
request handling. [Rust JSON Schema validation](https://docs.rs/jsonschema/latest/jsonschema/)
requires explicit parity checks, especially arbitrary precision, formats, local
references and draft compliance. Disable external schema fetching. Do not adopt a
validator's defaults as new Job admission behavior.

Pin toolchain/Cargo.lock, build immutable per-target artifacts in CI, embed source
fingerprints and feature/guide revisions, and deliver prebuilt operator binaries.
[Cargo locked builds](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
fix dependency resolution; offline builds additionally require a verified cache.
No Cargo installation or compilation occurs merely by connecting Loom Use.
