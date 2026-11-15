# Execution — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Container log capture is inside handle under the host Executor mutex. Worker returned-log synchronization precedes stage_returned, and stage log filenames do not encode every hook/finalizer generation.

Reviewed source:

- [rust/crates/ops/src/worker.rs](../../../rust/crates/ops/src/worker.rs)
- [rust/crates/core/src/execution.rs](../../../rust/crates/core/src/execution.rs)
- [rust/crates/host/src/executor.rs](../../../rust/crates/host/src/executor.rs)
- [rust/crates/host/src/wire.rs](../../../rust/crates/host/src/wire.rs)

## Implementation / consumer review

1. Register per-effect/generation/channel LogStream identity before launch; preserve current Job/Attempt/ticket fences.
2. Move long capture/read work outside the host authority mutex with a grant-validated capability and independent bounded collector.
3. Commit actual return promptly, then enqueue durable best-effort log sealing; keep the source container until capture or explicit expiry.
4. Negotiate new collector/stream capabilities and refuse unsupported claims from stale helpers; retain bounded compatible reads outside the lock.
5. Preserve delayed-start cancellation tombstones, unreturned-owner fencing, nonroot isolation and no Docker socket in Ops/Jobs.
6. Measure cancellation/status latency during deliberately slow and large real CPU-container logs.

## Invariants

- Log availability is independent of process return and Job success.
- Different effect generations cannot overwrite each other.
- Indexer/collector cannot launch work, synthesize returns or clear unknown ownership.

## Planned acceptance and dependencies

Cases: `LOG-01`, `LOG-02`, `LOG-03`, `LOG-04`, `LOG-05`, `LOG-06`, `TX-02`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [recovery](recovery.md), [results](results.md), [installation](installation.md).
