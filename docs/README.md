# Loom documentation

Loom schedules reproducible Jobs, records their outputs, and connects research records to versioned task context. This tree describes the native Rust service and its retained web and administrator integrations. It does not describe the state of a particular installation.

| Need | Start here |
|---|---|
| Understand the objects and their relationships | [Concepts](CONCEPTS.md) |
| Submit and inspect a first Unit | [First Unit](../loom_use/quickstart.md) |
| Perform a specific operation | [Task guide index](../loom_use/entry.md) |
| Look up an API action, schema, scope or example | [API reference](reference/README.md) |
| Find the implementation to change | [Implementation boundaries](IMPLEMENTATION.md), then [feature catalog](features/README.md) |
| Install a server | [Installer entry](../INSTALL.md) |
| Upgrade, restore or remove an installation | [Updates](UPDATES.md), [upgrade](UPGRADE.md), [uninstall](UNINSTALL.md) |
| Connect an agent or maintain its local client | [Loom-Use](https://github.com/LoomOps/Loom-Use), using the running server's exact client release descriptor |
| Review tests and their limits | [Testing](../deploy/TESTING.md), [verification blocks](features/VERIFICATION.md) |
| Write or review documentation | [Documentation contract](DOCUMENTATION.md), [complete inventory](documentation-catalog.json) |

For a connected installation, `system.guide` supplies that binary's task guides and `system.describe` supplies its action schemas. The source reference here describes this checkout; neither it nor a newer client proves that a feature is deployed.

## Design and migration records

The [scientific Output and Experimental Setup proposal](metadata-templates/OUTPUT-EVIDENCE.md) defines analysis coverage, evidence ownership and Sim–Real comparison.

The [current Job and Unit boundaries](jobs/BOUNDARIES.md) describe research outcomes, scheduling and catalog publication. The [original design and rollout record](jobs/JOB-UNIT-DESIGN.md) explains how those boundaries were chosen.

[Rust redesign](rust-port/README.md) and [Item index design](item-index/README.md) retain the plans, decisions and implementation ledgers used to build the native service. The implementation is present in `rust/`. Planning language in a historical document is not a current installation or agent instruction. Use the current reference and feature catalog for changes; use the design records to understand the rationale.
