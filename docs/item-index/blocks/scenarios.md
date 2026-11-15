# Scenarios — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Scenario revisions/typed target links and accepted Job pins are separate from scheduling. Exact archived context remains usable in code; the current feature prose incorrectly forbids new pins to it.

Reviewed source:

- [rust/crates/core/src/scenario.rs](../../../rust/crates/core/src/scenario.rs)
- [rust/crates/core/src/results.rs](../../../rust/crates/core/src/results.rs)
- [rust/crates/core/src/catalog.rs](../../../rust/crates/core/src/catalog.rs)
- [dashboard/workspace/scenarios.js](../../../dashboard/workspace/scenarios.js)
- [docs/features/scenarios.md](../../../docs/features/scenarios.md)

## Implementation / consumer review

1. Map Scenario and exact revisions to common Items while keeping existing IDs, typed links, supersession and immutable protocol payloads.
2. Resolve graph/search links through exact kind/origin/revision/hash/native aliases, including full historical snapshots and installation Profiles.
3. Index explicit Scenario relations and frozen Job context only; do not infer assignments or scientific provenance from names or nearby files.
4. Show related files/logs through their real parent revisions and Attempts; no duplicate dataset/model/Job created by a link.
5. Correct archived-context documentation to match the accepted native behavior: archive affects discovery, not explicit exact-revision execution context.

## Invariants

- Scenario and search are context, not execution/permission gates.
- Unassigned/ambiguous history remains explicit.
- Equal labels or numeric IDs across kinds do not connect unrelated records.

## Planned acceptance and dependencies

Cases: `SCN-01`, `SCN-02`, `ID-03`, `FILE-01`, `UI-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [results](results.md), [catalog](catalog.md), [dashboard](dashboard.md).
