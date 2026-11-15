# Scenarios

Native candidate authority: `rust/crates/core/src/scenario.rs#create_scenario`.
Retained Python paths below are compatibility references or administrator/web
integrations. See [the implementation map](../rust-port/IMPLEMENTATION.md) for
current process boundaries and acceptance limits.

Scenario is versioned research context, independent of Job execution identity,
software ownership and permission grants. Its immutable definition can describe a
task, protocol and references. Jobs explicitly pin a revision; updates do not move
old pins. Explicit links reuse an existing dataset/model/experiment/software item
without copying it or recording new scientific work.

Contracts: `rust/crates/core/src/contracts.rs`, `scenario.rs`, and
`rust/contracts/redesign-schema.sql`. The `results` MCP group exposes the live
schemas. Link targets must name exact native or historical identities. Withdrawal
and supersession preserve original evidence. An archived Scenario remains readable
but cannot receive newly accepted Job pins. Existing records may stay unassigned.

Dashboard: Scenarios list, immutable revision selection, Jobs and linked records.
The authenticated relay allowlists Scenario actions; the iframe holds no credential.
Use native SQLite/API fixtures and Firefox; screenshots alone do not prove Job pins.
