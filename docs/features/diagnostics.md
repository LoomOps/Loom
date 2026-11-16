# diagnostics — Private bounded local error collection; no remote telemetry

## Boundary

Collection failures never block authentication, Jobs or responses. No secret/request/exception-message capture and no automatic upload.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Local incident | `dashboard/diagnostics.py#record` | Fixed component/operation/code and optional exception object | Incident ID or None on suppressed/unwritable output; bounded JSONL | api, network, dashboard, installation |
| Native local incident | `rust/crates/core/src/diagnostics.rs#incident` | Safe operation ID and typed error code/status | Private incident ID or absent diagnostic; no operation-state change | api |

## Sources and contracts

- [dashboard/diagnostics.py](../../dashboard/diagnostics.py) — integration, packaging or documentation.
- [ops/compose.diagnostics.yaml](../../ops/compose.diagnostics.yaml) — integration, packaging or documentation.
- [rust/crates/core/src/diagnostics.rs](../../rust/crates/core/src/diagnostics.rs) — native implementation / contract.

## Change review

Review every caller, image packaging, private volume ownership and independent Loom Use collector schema before changing collection fields.

Related blocks: None..

## Verification

- [tests/dashboard/test_diagnostics.py](../../tests/dashboard/test_diagnostics.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Tests cover filesystem faults and retention. External collection/upload is not implemented; client and server directories remain on their respective machines.
