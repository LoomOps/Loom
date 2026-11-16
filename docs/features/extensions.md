# extensions — Components, Tools, Profiles and immutable Runtime environment contracts

## Boundary

Registering a new component must not require new MCP tools or platform redeployment. Preserve profile revision pins on submitted Jobs.
Runtime metadata is separate from Job state; instance grants and exact revisions are verified, and dependency discovery does not authorize host installation.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Dynamic profile | `rust/crates/core/src/software.rs#software_action` | Actor + profile operation/schema + revision/config | Stored validated profile revision or reference; submitted Job keeps its pin | api, execution |
| Dynamic component | `rust/crates/core/src/software.rs#software_action` | Actor + component operation/metadata | Immutable component revision and namespace; no new MCP server required | api, execution |
| Granted Runtime inventory | `rust/crates/core/src/runtime.rs#runtime_inventory` | Instance executor configuration plus read-only Profile/Job catalog | Granted exact Runtime revisions, validation evidence and actual consumers, or explicit unavailable status | api, dashboard, execution |
| Exact Runtime resolution | `rust/crates/host/src/runtime.rs#detail` | Executor state, instance and exact id/revision with optional sealed digest | Integrity-checked immutable manifest within instance grant; denies missing/changed revisions | execution, installation |
| Profile combinations | `rust/crates/core/src/composition.rs#profile_listing` | Read-only registry and installed config, actor and recorded Jobs | Exact software and stage environment combinations with declared/observed usage and current attribution; no execution or Job gate | api, dashboard, onboarding |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `components.assign_owner`, `components.list`, `components.get`, `components.write`, `components.register`, `components.register_profile`, `components.get_profile`, `components.profiles`, `components.curate`, `components.inspect_profile`, `components.compare_profiles`, `components.annotate`, `components.runtimes`, `components.dependencies`, `components.get_dependency`, `components.get_runtime`, `components.list_tools`, `components.get_tool`, `components.register_tool`.

## Sources and contracts

- [executor/runtime_store.py](../../executor/runtime_store.py) — active administrator Runtime catalog adapter, used by deploy/runtime.py and runtime_inventory.py; native host/Core consume the same catalog.
- [ops/component_store.py](../../ops/component_store.py) — retained Python compatibility reference.
- [ops/dependency_catalog.py](../../ops/dependency_catalog.py) — retained Python compatibility reference.
- [ops/profile_inventory.py](../../ops/profile_inventory.py) — retained Python compatibility reference.
- [ops/profile_store.py](../../ops/profile_store.py) — retained Python compatibility reference.
- [ops/profiles.py](../../ops/profiles.py) — retained Python compatibility reference.
- [ops/runtime_catalog.py](../../ops/runtime_catalog.py) — retained Python compatibility reference.
- [ops/runtime_hub.py](../../ops/runtime_hub.py) — retained Python compatibility reference.
- [ops/software_metadata.py](../../ops/software_metadata.py) — retained Python compatibility reference.
- [ops/tool_store.py](../../ops/tool_store.py) — retained Python compatibility reference.
- [rust/crates/core/src/composition.rs](../../rust/crates/core/src/composition.rs) — native implementation / contract.
- [rust/crates/core/src/runtime.rs](../../rust/crates/core/src/runtime.rs) — native implementation / contract.
- [rust/crates/core/src/software.rs](../../rust/crates/core/src/software.rs) — native implementation / contract.
- [rust/crates/host/src/runtime.rs](../../rust/crates/host/src/runtime.rs) — native implementation / contract.
- [rust/crates/host/src/runtime_files.rs](../../rust/crates/host/src/runtime_files.rs) — native implementation / contract.
- [loom_use/runtimes.md](../../loom_use/runtimes.md) — integration, packaging or documentation.
- [ops/execution.py](../../ops/execution.py) — retained Python compatibility reference.
- [ops/runtime_contract.py](../../ops/runtime_contract.py) — retained Python compatibility reference.

## Change review

When Profiles, Components, Tools or Runtimes change, review stage pins, dependency requirements, executor grants/policy, API/MCP discovery, Dashboard, configured installation directories and preservation.

Related blocks: [identity](identity.md).

## Verification

- [tests/ops/test_components_platform.py](../../tests/ops/test_components_platform.py)
- [tests/ops/test_dependency_catalog.py](../../tests/ops/test_dependency_catalog.py)
- [tests/ops/test_independent_profiles.py](../../tests/ops/test_independent_profiles.py)
- [tests/ops/test_profile_inventory.py](../../tests/ops/test_profile_inventory.py)
- [tests/ops/test_research_tools.py](../../tests/ops/test_research_tools.py)
- [tests/ops/test_runtime_catalog.py](../../tests/ops/test_runtime_catalog.py)
- [tests/ops/test_runtime_hub.py](../../tests/ops/test_runtime_hub.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- The execution-environment owner verifies actual external image dependencies, caches and GPU compatibility.
