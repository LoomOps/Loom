# Verification architecture and independent audit — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D0–D8.

## Responsibility and model

**Authority:** Rust domain/application/fault suites plus independent Python/JS/browser/package harnesses.

**Typed model:** CaseId, SourceFingerprint, EvidenceLevel, BoundaryOutcome, PerformanceBaseline and UnverifiedLimit.

**Operations:** Run isolated suites, compare frozen valid contracts and intended corrections, audit feature mappings and source-qualified evidence.

**Persistence:** Synthetic fixtures and ignored private test evidence; never operational DBs/tokens/data in release source.

## Reference source and consumers

- [tests/dashboard/mock_ops.py](../../../tests/dashboard/mock_ops.py)
- [tests/dashboard/verify_browser.py](../../../tests/dashboard/verify_browser.py)
- [tests/dashboard/verify_workspace.py](../../../tests/dashboard/verify_workspace.py)
- [deploy/features.py](../../../deploy/features.py)
- [tests/suite.py](../../../tests/suite.py)
- [tests/run.py](../../../tests/run.py)
- [tests/ops/test_support.py](../../../tests/ops/test_support.py)
- [tests/README.md](../../../tests/README.md)
- [deploy/TESTING.md](../../../deploy/TESTING.md)

Existing feature dependencies: none declared.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Keep all files inventoried independently of the feature catalog. Review mutation/effect boundaries and then cross-check all 17 existing plus three new design blocks.

2. Rust tests own transition matrices, typed validation, SQLite transactions, publication fences, concurrency, actual kill/restart and uncertain external-effect reconciliation.

3. Python/JS retain independent HTTP/MCP/external-coordinator/Loom Use/browser/install clients. Replace Python internal imports/patches for moved domains with real process/protocol behavior; do not create fake SQL/helper production APIs.

4. Add two reference expectations: preserved compatibility fixtures and explicit corrected outcomes for known bugs. A differential test must not require cancellation/data-loss bugs to stay compatible.

5. Close test engines/servers and reset process-local caches per test. Trigger faults on the actual connection/process, not a separate Python fixture connection.

6. Test current pinned client and candidate packaged Skills/examples, actual source-qualified wheel/binary/image paths and skipped-version/fresh installs. Track all skips/stale evidence explicitly.

7. Benchmark control-plane CPU/memory/latency on the same workload/durability/auth settings; do not claim GPU/Sim speedups or use performance checks as user Job gates.

## Transaction and effect boundary

Tests use disposable state and bounded children. Shadow comparison has no external mutation authority. Fault hooks are test-only and absent from distributed production binaries.

## Retained code and compatibility

Independent Python regression harnesses and Firefox; language statistics are not a test migration target.

Re-register moved/new source and tests in feature catalog; add Scenario/authoring/presentation ownership and Rust paths when implementation exists. Planning matrices are proposed cases, never reported execution passes.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| TEST-01 | Kill between write and commit, and after external accept | No partial state, no duplicate effect and preserved unknown owner. |
| TEST-02 | Rust decoder differs from old valid schema/hash | Parity fixture identifies change; no silent new admission semantics. |
| TEST-03 | All feature blocks pass but unmapped source changes | Independent ledger/impact review identifies uncovered boundary. |
| TEST-04 | Mock-only checks pass | GPU/provider/native/reboot acceptance stays explicitly unverified. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/deploy/test_features.py](../../../tests/deploy/test_features.py)
- [tests/deploy/test_distribution.py](../../../tests/deploy/test_distribution.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
