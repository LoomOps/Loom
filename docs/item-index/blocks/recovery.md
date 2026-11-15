# Recovery — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Recovery uses per-Job leases, shared budgets, accepted request revisions and same-Attempt publication retries. Search maintenance has no corresponding execution authority.

Reviewed source:

- [rust/crates/core/src/jobs.rs](../../../rust/crates/core/src/jobs.rs)
- [rust/crates/core/src/execution.rs](../../../rust/crates/core/src/execution.rs)
- [rust/crates/ops/src/worker.rs](../../../rust/crates/ops/src/worker.rs)

## Implementation / consumer review

1. Give repeated finalizer/hook effects independent log generations while retaining the real Attempt and publication-only retry semantics.
2. Reconcile pending index/seal work with its own stable journal keys; never acquire a Job recovery lease for an index issue.
3. Preserve original submission-key replay, pauses, lease deadlines, cancellation and owner fences during backfill/rebuild.
4. Report unavailable files/logs as source evidence, not a guessed root cause or a new failure/recovery transition.

## Invariants

- Index repair never retrains, increments an Attempt or emits another completion.
- Same-Attempt finalization remains distinguishable from resumed execution.
- Unknown log continuity does not imply unknown process return.

## Planned acceptance and dependencies

Cases: `TX-02`, `TX-03`, `TX-04`, `LOG-03`, `LOG-04`, `LOG-06`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [execution](execution.md), [diagnostics](diagnostics.md).
