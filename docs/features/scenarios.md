# scenarios — Versioned Research Task context and explicit immutable record links

## Boundary

Accepted revisions and execution provenance remain immutable. Source registration never runs user code.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Versioned research context and explicit immutable record links | `rust/crates/core/src/scenario.rs#create_scenario` | Authenticated typed action and exact revision identities | Transactional receipt or explicit conflict | api, catalog, dashboard, onboarding |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `results.scenarios`, `results.get_scenario`, `results.scenario_graph`, `results.register_scenario`, `results.update_scenario`, `results.archive_scenario`, `results.link_scenario`, `results.supersede_scenario_link`.

## Sources and contracts

- [dashboard/workspace/scenarios.js](../../dashboard/workspace/scenarios.js) — integration, packaging or documentation.
- [rust/contracts/scenarios.md](../../rust/contracts/scenarios.md) — native implementation / contract.
- [rust/crates/core/src/scenario.rs](../../rust/crates/core/src/scenario.rs) — native implementation / contract.
- [rust/crates/core/src/contracts.rs](../../rust/crates/core/src/contracts.rs) — native implementation / contract.

## Change review

Review live schemas, exact bindings, Dashboard, distributed guidance, migration and native acceptance together.

Related blocks: [identity](identity.md) · [api](api.md).

## Verification

- [tests/dashboard/browser/test_native_workspace.py](../../tests/dashboard/browser/test_native_workspace.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Run isolated Rust/HTTP/producer/browser fixtures; operational acceptance requires separately authorized deployment.


## Presentation and exact membership

Public UI terminology is Task; Scenario action and field names remain compatible.
List DTOs include compact task_refs and research_tasks metadata independently of
the bounded bootstrap. Membership comes from accepted pins or active exact links,
never label matching in a projection. Task filters are server-paginated and retain
linked historical selectors. Withdrawn links stop matching; originals remain.
Task metadata cannot hide current queue occupants or grant file access.
