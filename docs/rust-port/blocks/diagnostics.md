# Private diagnostics — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D1, D6, D7.

## Responsibility and model

**Authority:** Rust safe error/incident adapter; retained Python collector for retained services.

**Typed model:** IncidentId, ComponentCode, OperationCode, Severity, DiagnosticRevision.

**Operations:** RecordBoundedIncident and read authorized opaque diagnostic references; no automatic upload.

**Persistence:** Private bounded per-service JSONL/rotation files, never platform state or release source.

## Reference source and consumers

- [dashboard/diagnostics.py](../../../dashboard/diagnostics.py)
- [ops/compose.diagnostics.yaml](../../../ops/compose.diagnostics.yaml)

Existing feature dependencies: none declared.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Preserve the shared diagnostic envelope and UUID correlation across HTTP/MCP/external-coordinator/Loom Use.

2. Only fixed safe codes and allowlisted metadata are recorded. Do not capture raw params, credentials, scientific records, arbitrary upstream bodies or exception messages.

3. Make record/rotate failures non-blocking with bounded buffers, dedup and explicit retention. Files/directories preserve service ownership/private modes.

4. Keep client and server collection on their respective machines; no hidden telemetry or evidence copying on connection.

5. Classify internal Rust panics/storage faults into safe typed errors with private incident IDs; avoid leaking backtraces into public errors.

## Transaction and effect boundary

Diagnostics are best-effort outside authoritative state commits. Failure to record an incident cannot roll back a successful Job command or create one.

## Retained code and compatibility

Existing diagnostics implementation for web/client services and retention knobs. One cross-language envelope, not one forced process.

Keep existing paths/retention and verify actual mounts on fresh install/upgrade. Private evidence directories never enter source bundles.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| DIAG-01 | Unwritable/full diagnostics directory | Operation outcome unchanged; no credential leakage. |
| DIAG-02 | Malformed upstream incident string | No arbitrary text reflected as a locator. |
| DIAG-03 | Concurrent rotation and duplicate errors | Bounded valid files with private ownership. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/dashboard/test_diagnostics.py](../../../tests/dashboard/test_diagnostics.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
