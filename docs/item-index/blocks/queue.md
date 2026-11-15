# Queue — Item/index impact and detailed work

Status: proposed. Impact: **Associated — regression only**. No runtime change is made by this plan.

## Observed baseline

Queue readiness depends on current states, real dependencies, holds, recovery leases and resource ownership. Search or research-presentation curation is not a readiness fact.

Reviewed source:

- [rust/crates/core/src/jobs.rs](../../../rust/crates/core/src/jobs.rs)
- [rust/crates/core/src/query.rs](../../../rust/crates/core/src/query.rs)
- [rust/crates/core/src/execution.rs](../../../rust/crates/core/src/execution.rs)

## Implementation / consumer review

1. Keep queue/status/control reads on existing authoritative paths when search is delayed, rebuilding or unavailable.
2. Do not create scheduler Jobs for scan chunks, index rebuild, text extraction or log sealing. Expose separate maintenance observations.
3. Preserve real workflow membership and internal Step/Attempt counts; Item aliases add navigation only.
4. Exercise cancellation, reorder, dependency admission and all actual occupants under background index load and every presentation filter.

## Invariants

- An indexed available flag cannot admit an input.
- Actual resource consumers remain visible regardless of search filters.
- No new scheduler state or index-completion gate.

## Planned acceptance and dependencies

Cases: `TX-02`, `TX-04`, `LOG-02`, `QUERY-03`, `UI-02`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [execution](execution.md), [catalog](catalog.md).
