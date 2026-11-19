# scenarios — Versioned Research Task context and explicit immutable record links

## Boundary

Accepted revisions and execution provenance remain immutable. Source registration never runs user code.

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

Acceptance outside these fixtures:
- Run isolated Rust/HTTP/producer/browser fixtures; operational acceptance requires separately authorized deployment.


## Presentation and exact membership

Public UI terminology is Task; Scenario action and field names remain compatible.
List DTOs include compact task_refs and research_tasks metadata independently of
the bounded bootstrap. Membership comes from accepted pins or active exact links,
never label matching in a projection. Task filters are server-paginated and retain
linked historical selectors. Withdrawn links stop matching; originals remain.
Task metadata cannot hide current queue occupants or grant file access.

The Catalog Task index filters Datasets, Policies and Experiments and defaults to
All, including unlinked historical records. New Dataset, Policy and Experiment
submissions and imports require an exact Task revision for both catalog classes.
General infrastructure evidence stays in Artifacts and Runtime receipts.
Exact Task details and API authoring remain available. Policy and Experiment DTOs
carry resolved exact catalog inputs, including preserved links within one source
snapshot. Dataset and Policy indexes intersect Task, dataset variant and model
family or evaluated Policy. New Policy and Experiment admissions validate typed
input revisions or declared upstream outputs under the pinned Task. Imports use
typed exact relations with the same Task check.
