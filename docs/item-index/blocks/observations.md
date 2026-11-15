# Observations — Item/index impact and detailed work

Status: proposed. Impact: **Associated — safe status integration**. No runtime change is made by this plan.

## Observed baseline

Host/resource observations and Runtime freshness are separate from execution state. A sensor or collector observation does not grant mutation authority.

Reviewed source:

- [rust/crates/host/src/observe.rs](../../../rust/crates/host/src/observe.rs)
- [rust/crates/core/src/catalog.rs](../../../rust/crates/core/src/catalog.rs)
- [rust/crates/ops/src/worker.rs](../../../rust/crates/ops/src/worker.rs)

## Implementation / consumer review

1. Expose bounded safe index/collector observations: generation, lag, covered scopes, last check, queue depth and storage budget states.
2. Keep source availability, collected-byte and indexed-text high watermarks independent. Missing observations remain unavailable/stale.
3. Do not add index readiness to host/GPU readiness or mark a Runtime ready because its files are searchable.
4. Run observation polling with independent budgets; it cannot recursively discover files or collect private diagnostic content.

## Invariants

- Observations never schedule, recover or terminate Jobs.
- Unknown freshness is not zero usage or successful capture.
- No credentials/host-private paths in general status.

## Planned acceptance and dependencies

Cases: `TX-04`, `FILE-03`, `LOG-01`, `QUERY-03`, `SW-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [catalog](catalog.md), [execution](execution.md).
