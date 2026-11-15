# Api — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

The live registry exposes six groups; native reads use bounded read snapshots. Historical lookup accepts revision but currently selects the latest matching snapshot in history_record.

Reviewed source:

- [rust/crates/core/src/contracts.rs](../../../rust/crates/core/src/contracts.rs)
- [rust/crates/core/src/http.rs](../../../rust/crates/core/src/http.rs)
- [rust/crates/ops/src/lib.rs](../../../rust/crates/ops/src/lib.rs)
- [rust/crates/core/src/onboarding.rs](../../../rust/crates/core/src/onboarding.rs)

## Implementation / consumer review

1. Add the proposed actions in API.md within existing results/jobs/system groups, with strict typed operation-specific schemas and explicit response budgets.
2. Keep specialized get/log/reconnect routes and native IDs; add common Item references without changing event ACK or Job submit replay.
3. Implement keyset/generation/access-bound cursors and explicit index lag/unavailable/expired outcomes.
4. Correct exact historical snapshot selection before Item alias/detail adoption; preserve old latest-selection behavior only when no snapshot is explicitly supplied.
5. Add a lightweight Dashboard summary/bootstrap endpoint so indexed search and direct Item reads do not require the legacy full-catalog payload.
6. Run index/file/log I/O outside the core writer and separately from host execution authority; use bounded reads, range caps and safe error envelopes.

## Invariants

- No public SQL, arbitrary filesystem query, compiler or Docker operation.
- No empty success substituted for unavailable or partial search.
- Published client response limits and existing MCP JSON/SSE semantics remain valid.

## Planned acceptance and dependencies

Cases: `ID-02`, `ID-03`, `QUERY-01`, `QUERY-02`, `QUERY-03`, `QUERY-04`, `AUTH-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [identity](identity.md), [catalog](catalog.md), [onboarding](onboarding.md).
