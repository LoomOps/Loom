# Native implementation and acceptance

Status: historical implementation ledger for `codex/loom-rust`.
See [implementation boundaries](../IMPLEMENTATION.md) and the [generated API reference](../reference/README.md) for the current source.
The frozen design/audit remains in this directory. This document records what is
implemented, where authority resides, and which acceptance boundaries are separate.
D1–D7 implementation is present. D8 requires separate source-qualified and
per-installation operational receipts; this implementation ledger is not a
deployment receipt. Public publication, client refresh and target acceptance
remain distinct actions.

## Concrete ownership

The thirteen logical boundaries in ARCHITECTURE are implemented in six crates.
They are not thirteen services or a Python compatibility RPC bridge.

| Crate | Responsibility | Boundaries |
|---|---|---|
| `loom-domain` | Typed IDs, lifecycle decisions, DAG readiness, recovery leases/budget, interval unions | Standard library only; no SQLite/HTTP/Docker/JSON state machine |
| `loom-core` | Authorized application commands, one action registry, transactional repositories, exact legacy codecs, storage/software/Scenario rules and read projections | Internal modules implement the contracts/app/store/query/storage/software boundaries; no public SQL or generic mutation callback |
| `loom-sdk` | Producer request/progress/result contract, bounded worker process groups, native CPU/installation producers | No scheduler DB, API credentials or Docker access |
| `loom-host` | Protected Unix executor, durable scoped tickets, Runtime registrar, host observations | Docker authority stays outside Ops and Job containers; peer UID plus credential/grant validation |
| `loom-ops` | HTTP/MCP/TLS, bounded application write lane, read snapshots, effect/observer/update loops and remote ownership client | One serialized writer; hashing, files, network and Docker work outside the transaction/lane where material |
| `loom-migrate` | Offline inspect/apply/rollback, additive schema and preservation receipts | Same writer lock, drained owners, private backups; never starts a worker |

Web assets, native browser identity/Sharing/Gateway integrations, personal SSH and
administrator installation/runtime-build drivers are retained. Python scientific
SDKs remain explicit isolated stages. The generic Loom Use transport remains
Python; authoring instructions and generated producer projects prefer Rust.

## Implemented behavior

- Job submission preserves original key/request independently of later recovery,
  Profile or Scenario changes. DAG nodes are real execution membership; producer
  Steps are observations. Attempts reserve separately from actual process starts.
  Consolidation retains original source histories without fabricating execution.
- One immutable effect intent precedes external I/O. Epoch/Job revision/Attempt
  fences reject stale returns. Unknown launch/resource ownership is retained.
  A cancellation tombstone prevents a delayed start from launching after cancel.
- Recovery distinguishes retrying execution from retrying publication/finalization.
  Lease expiry, operator pause and exhausted budget remain separate. Cancellation
  closes failed recovery reservations; callbacks cannot resurrect terminal Jobs.
- Results publish only after process return and exact file evidence. Rename-before-
  commit is reconciled; changed bytes cannot overwrite old evidence. Historical
  bindings add verified current copies without rewriting recorded availability.
- Scenario revisions are immutable optional research context, with exact Job pins,
  typed many-to-many links and supersession. Archiving hides default discovery;
  it does not invalidate an explicit accepted/reused revision or gate execution.
- Queue queries include every actual occupant regardless of Research/Operations
  presentation. History defaults to All with independent selectors. Main/support/
  test curation never changes scientific records or scheduler eligibility.
- Profiles bind exact Component/Tool/source/Runtime composition. Representative
  dependencies are separate from complete transitive package inventory. Runtime
  metadata remains in its separate host catalog with configured dependency roots.
  Usage counts distinguish recorded references, accepted execution and logical
  reuse; metadata presence is not an invocation or scientific validation claim.
- Rust SDK templates cover Tool, Component and hybrid workers. Registration never
  compiles/imports/runs source. Python source in new packages requires a concrete
  external package, stage and reason. Existing accepted Python revisions persist.
- HTTP/MCP shared one schema/action registry, versioned guides,
  JSON/SSE and safe errors. Existing published and candidate clients are exercised
  against the native service. Native logs support bounded ranges during execution;
  returned logs preserve complete bytes beyond control-reply limits.
- Read projections use bounded read-only SQLite snapshots. The snapshot cannot
  become mutation authority even with a submit credential. Large file verification
  runs outside the writer and rechecks file stamps at commit. Catalog revision
  caching changes for domain facts, not authentication heartbeat-only writes.
- The native release image contains prebuilt Ops/host/SDK/migrator binaries and no
  Python interpreter. Native installer smoke uses the release-built Rust producer
  through ordinary prepare/run/result publication, with no host compiler.

## Migration and preservation

Schema 4 is accepted only after drain. Schema 5 adds native authority/effect/
receipt/Scenario/binding/projection facts; existing tables and rows are preserved.
Schema 0 is initialized only when empty. Legacy Python writers reject schema 5.
A shared worker lock and epoch protect local ownership; the existing remote-fence
protocol is used where configured, with no automatic lease expiry/failover.

`loom-migrate apply` checks configured store markers, roots, identities and drained
host/Runtime owners. It creates a private SQLite/config backup and durable intent
before cutover. An interrupted receipt write can resume only when the before or
independently reconstructed additive after-state matches. New records make old-DB
rollback unsafe and are rejected rather than discarded. Updates deliver matching
native helper binaries and preserve administrator configuration and user overrides.

The cumulative migration is introduced at `0.2.0-alpha.1`. That identifier is a
migration/release boundary, not a claim that this candidate was published. Existing
numbered releases are immutable. The published Loom Use alpha.9 pin stays unchanged
until the independent alpha.10 candidate has actually been published and verified.

## Verification entry points

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 deploy/features.py check
LOOM_NATIVE_DOCKER_TEST=1 python3 tests/run.py --browser --output /tmp/loom-native-check
python3 deploy/features.py audit --evidence /tmp/loom-native-check
python3 deploy/rust_build.py --output /tmp/new-native-kit
```

Use the CLI's `--help` for accepted audit arguments. Test reports stay outside
release source. Reports record per-case execution and source fingerprints;
missing/skipped/stale checks cannot satisfy feature readiness. The current source
inventory is [IMPLEMENTATION-REVIEW.json](IMPLEMENTATION-REVIEW.json); it is separate
from frozen [FILE-REVIEW.json](FILE-REVIEW.json) and from feature block mappings.

| Boundary | Executable evidence | Limit |
|---|---|---|
| Pure decisions / transactions / exact codecs | `tests/rust/domain.rs`, `application.rs`, `codec.rs` | Temporary real SQLite/files, not scientific data |
| Launch uncertainty / scopes / cancel | `tests/rust/host.rs`; `test_executor.py` real CPU Docker | No GPU driver or physical power-loss acceptance |
| API/MCP/client/restart/fencing | `tests/rust/test_service.py` | Loopback fixtures; provider authority fixture is independent |
| Authoring SDK/templates | `tests/rust/test_authoring.py`, `sdk.rs` | Actual generated Cargo build/run; external scientific worker fixture only |
| Migration / rollback / resume | `tests/rust/test_installation.py`, deployment regressions | Private disposable DB/config; systemd/provider operations mocked |
| Browser / Scenario / retained layout | `tests/dashboard/browser/test_native_workspace.py` and retained Firefox suites | Actual native Ops plus retained relay; synthetic records |
| Immutable artifact / smoke | `ops/Dockerfile.rust`, native artifact/installer tests and separately recorded image acceptance | Same-image binaries, actual CPU prepare/run/output; no operational installation |
| Performance | Separately recorded equal-resource HTTP Docker baseline at 100/10k/100k synthetic records | Control plane only; no Isaac/ML/hash/build speed claim |

## Independent correction cycles

Independent semantic review preceded feature-block verification. Subsequent full
suite passes were followed by another review and regression corrections:

1. Original-key replay after recovery, current-Attempt/epoch fencing, uncertain
   launch ownership, cancellation/publication ordering and actual start evidence.
2. Preserved imported history/current bindings, full returned logs, real generated
   Cargo packages, remote handoff ownership, Scenario browser save and package pins.
3. Large-history query shape and writer isolation, catalog cache invalidation,
   workflow queue movement, failed-recovery cancellation, late-start cancellation,
   archive/context independence, live logs, crash-safe migration receipt resume
   private diagnostics that reject redirected directories, and exact typed Scenario
   record links that cannot appear on unrelated records.

Each correction has an executable fixture in the boundaries above. Whole-tree
inventory/parse/hash inspection is not a mathematical proof of every branch; it
is cross-checked against actual native tests and separately executed feature audit.
Passing old Python suites alone cannot establish native behavior.

## External acceptance still separate

No operational database/configuration, installed Skill, remote service, account,
share, ACL, active research Job or GPU was changed by this implementation task.
Clean-host systemd installation/reboot, real multi-machine/TLS/provider access,
physical power loss, long-running operational load and actual GPU/Sim/model
numerical behavior need target-specific acceptance. Unknown ownership requires
explicit reconciliation; it is never cleared just to obtain a green health result.
A general autonomous reconciler and frontend rewrite remain outside this scope.
