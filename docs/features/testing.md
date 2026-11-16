# testing — Feature specifications, impact analysis and isolated test tools

## Boundary

Development validation adds no new gates to Job execution. Distinguish test results from actual deployment state.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Change impact | `deploy/features.py#impact` | Changed repo-relative paths + feature catalog | Direct owners, related consumers, tests/specs; no mandatory edits to every consumer | installation |
| Catalog check | `deploy/features.py#check` | Catalog + repository tree | Missing references, ambiguous owners, unmapped code/tests and broken interface links | installation |

## Sources and contracts

- [deploy/TESTING.md](../../deploy/TESTING.md) — integration, packaging or documentation.
- [deploy/features.py](../../deploy/features.py) — integration, packaging or documentation.
- [tests/README.md](../../tests/README.md) — integration, packaging or documentation.
- [tests/dashboard/mock_ops.py](../../tests/dashboard/mock_ops.py) — integration, packaging or documentation.
- [tests/dashboard/verify_browser.py](../../tests/dashboard/verify_browser.py) — integration, packaging or documentation.
- [tests/dashboard/verify_workspace.py](../../tests/dashboard/verify_workspace.py) — integration, packaging or documentation.
- [tests/ops/test_support.py](../../tests/ops/test_support.py) — integration, packaging or documentation.
- [tests/run.py](../../tests/run.py) — integration, packaging or documentation.
- [tests/rust/fixtures/casefold.json](../../tests/rust/fixtures/casefold.json) — integration, packaging or documentation.
- [tests/rust/fixtures/json-namespaces.json](../../tests/rust/fixtures/json-namespaces.json) — integration, packaging or documentation.
- [tests/rust/run.py](../../tests/rust/run.py) — integration, packaging or documentation.
- [tests/suite.py](../../tests/suite.py) — integration, packaging or documentation.
- [rust/crates/core/examples/catalog_scale.rs](../../rust/crates/core/examples/catalog_scale.rs) — native implementation / contract.
- [deploy/documentation.py](../../deploy/documentation.py) — integration, packaging or documentation.
- [docs/features/VERIFICATION.md](../../docs/features/VERIFICATION.md) — integration, packaging or documentation.
- [docs/features/catalog.json](../../docs/features/catalog.json) — integration, packaging or documentation.
- [docs/features/catalog.schema.json](../../docs/features/catalog.schema.json) — integration, packaging or documentation.

## Change review

Register each new feature's unique ID, owned files, inputs/outputs, dependencies, tests and live-validation limitations. Test behavior and failure boundaries rather than adding generic assertions.

Related blocks: None..

## Verification

- [tests/deploy/test_distribution.py](../../tests/deploy/test_distribution.py)
- [tests/deploy/test_features.py](../../tests/deploy/test_features.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_item_image.py](../../tests/rust/test_item_image.py)
- [tests/deploy/test_documentation.py](../../tests/deploy/test_documentation.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Passing all tests does not prove successful authentication with actual external services.
