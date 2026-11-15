# Testing — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Feature verification matches actual case execution and source hashes. The existing native candidate tests do not prove the new Item/index behavior.

Reviewed source:

- [deploy/features.py](../../../deploy/features.py)
- [docs/features/catalog.json](../../../docs/features/catalog.json)
- [docs/features/VERIFICATION.md](../../../docs/features/VERIFICATION.md)
- [tests/rust/run.py](../../../tests/rust/run.py)
- [tests/run.py](../../../tests/run.py)
- [tests/rust/application.rs](../../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../../tests/rust/test_service.py)
- [tests/rust/test_executor.py](../../../tests/rust/test_executor.py)
- [tests/rust/test_installation.py](../../../tests/rust/test_installation.py)
- [tests/dashboard/browser/test_native_workspace.py](../../../tests/dashboard/browser/test_native_workspace.py)

## Implementation / consumer review

1. Implement planned cases from VERIFICATION.md with real typed rules, SQLite, manifests and source bytes; keep independent protocol/browser/install harnesses.
2. Add actual crash/response-loss/generation-switch and live slow-log cancellation tests, not mirrors of implementation calculations.
3. Assign new source ownership once and review all 19 blocks; a new search feature entry may own index-specific files.
4. Record current source/image/test case fingerprints; reject skips, stale source, missing client execution or reference-only acceptance.
5. Run independent design/source review before block verification, then counterexample review after tests. Update both test scope and behavior for findings.
6. Benchmark Items/files/log text under identical budgets; separate numerical/scientific/provider acceptance from local source results.

## Invariants

- No operational DB, private catalog or machine credentials in fixtures/release source.
- Feature audit is development evidence, never a Job gate.
- A test name or old PASS is not acceptance of a planned action.

## Planned acceptance and dependencies

Cases: `VERIFY-01`, `TX-01`, `TX-02`, `TX-03`, `TX-04`, `TX-05`, `TX-06`, `LOG-02`, `QUERY-04`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [api](api.md), [catalog](catalog.md), [installation](installation.md).
