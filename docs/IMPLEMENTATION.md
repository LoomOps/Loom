# Implementation boundaries

| Concern | Owner | Entry / consumer boundary |
|---|---|---|
| Pure state transitions and execution vocabulary | `rust/crates/domain` | Typed aggregate/events; no HTTP or filesystem |
| Serialized transactional commands | `rust/crates/core/src/store.rs#execute` | Authenticate, validate action scope/schema and call the feature handler |
| Action inputs | `rust/crates/core/src/contracts.rs`, `rust/contracts/legacy-actions.json` | Native registry combines retained input shapes and typed native additions |
| HTTP and grouped MCP | `rust/crates/core/src/http.rs`, `rust/crates/ops` | Same application operations; bounded transport, query workers and errors |
| Queue / recovery / events | `jobs.rs`, `execution.rs`, `query.rs` in core | Durable eligibility, lease and execution evidence; domain owns transitions |
| Software / Profile combinations | `software.rs`, `composition.rs`, `authoring.rs` in core | Sealed source and build receipts; registration does not execute code |
| Scenario context | `scenario.rs` in core | Accepted revisions and explicit immutable links |
| Result publication / historical bindings | `results.rs`, `history.rs`, `storage.rs` in core | Verified bytes, typed output and additive copy availability |
| Items / files / search / logs | `items.rs`, `inventory.rs`, `file_set.rs`, `index_store.rs`, `graph.rs`, `log_index.rs`, `maintenance.rs` in core | Compact authority, sealed inventories, separate disposable projection and bounded maintenance |
| Process/container and resource authority | `rust/crates/host` | Protected host service, executor tickets, return evidence and Runtime grants |
| Native process / background workers | `rust/crates/ops` | Service composition, independent query/index/log collection; no second Job state machine |
| Producer protocol and templates | `rust/crates/sdk`, `rust/templates` | User producer library, progress, results and streamed output inventories |
| Additive migration | `rust/crates/migrate`, `deploy/migrations` | Preserve accepted authority, versions, backups and idempotent receipts |
| Dashboard and identity-provider integrations | `dashboard/` | Python SSO/Sharing/relay and existing HTML/CSS/JS; no Ops DB mount |
| Administrator installation, updates and recipes | `deploy/` | Retained Python/shell host drivers; explicit target and immutable release |
| Agent guidance | `loom_use/catalog.json`, `onboarding.rs` | Compiled task guides, reference prose, exact schemas and revision markers |
| Personal client | Separate Loom-Use repository | Connections, credentials, Skill routing and current Codex refresh |

Paths without a crate prefix in the table are under `rust/crates/core/src`. Each action's generated reference includes its actual owning entry point and feature block. The [feature catalog](features/catalog.json) maps all source, contracts, consumers and tests; the [API reference](reference/README.md) maps caller operations.

## Retained code

`ops/*.py` and `executor/*.py` include the prior implementation and compatibility fixtures. They remain useful for preserved formats and comparisons; they are not the runtime authority of the native service. Active Docker/Compose, host-administrator helpers and web integrations remain in those trees where explicitly consumed by packaging. In particular, executor/runtime_store.py serves retained administrator Runtime installers, runtime_policy.py validates administrator inputs, ops/host_layout.py generates installation configuration, ops/dispatch_control.py serves the host queue-control CLI, and ops/remote_fence.py retains the remote authority protocol. Native service handlers remain responsible for live Job state. Check an executable entry point and its feature ownership before changing it. Directory name or language alone does not prove a file is active or removable.

Current behavior belongs in feature specs, live guides and action descriptions. The Rust redesign and Item-index planning trees preserve original decisions and audit evidence; they do not override these current boundaries. No web rewrite or external scientific SDK port is implied by native core ownership.
