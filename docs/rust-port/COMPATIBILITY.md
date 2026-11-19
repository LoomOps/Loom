# Compatibility contract for a new Rust design

Status: historical Rust migration design. See the [generated API reference](../reference/README.md) for the current action groups.

Compatibility protects accepted work, not the old internal architecture. Preserve
valid interfaces and evidence; correct documented bugs with explicit cases in
[REVIEW](REVIEW.md). Do not reproduce a broken result solely for differential parity.

## Exact representations

Keep Job/Attempt/node/Step/ticket/entity/Scenario/storage/installation/user/credential
IDs as separate string newtypes, accepting historical formats. Keep Unix timestamp
meaning/precision, nullable fields, zero vs N/A, ordering, Unicode and integer
precision. Preserve raw sealed/historical payloads beside typed semantic views.

| Namespace | Fixture requirement |
|---|---|
| Ops request/dump and submit-key comparison | Python sorted finite JSON, old requested-vs-resolved semantics and exact replay |
| Executor request / launch fingerprint | Existing JSON whitespace/escaping and Docker argv bytes |
| Runtime digest | Sorted compact finite JSON with its current ASCII escaping |
| Component/Tool/Profile publication | Exact source bytes and each path's legacy sealing rules |
| Historical snapshots | Original encoded JSON and identity/revision/snapshot selectors |
| Guides/build manifests | Exact selected-source file set, ordering, hash and independent version markers |

Do not collapse these into a default serde serializer. New versioned hashes are
possible only with coexistence/readability and an explicit migration. New strict
input DTOs do not delete accepted unknown historical fields.

## Authority and external effects

There is one state/receipt authority per instance and one privileged Executor
registrar. Preserve worker/schema/update locks, epoch/remote fences, current-Attempt
checks, ticket/container labels, returned receipts, finalizer receipts, result
outbox, callback keys, reservation deadlines and operator pause keys. A queued
Task report/ACK in an external coordinator is never a Loom execution prerequisite.

Unknown launch/cancel/build/publication outcomes remain resource-owned until
matching evidence arrives. Rust futures or RAII scope exit do not prove process
return. No generic SQL/internal-helper RPC is exposed to a retained Python driver.
No second implementation decides next states during a shadow comparison.

## Preserved integrations and explicit changes

Preserve the original HTTP/MCP entry points, JSON/SSE/errors, scope-filtered discovery,
trusted dashboard assertions, Origin/CSRF, cookie invalidation and iframe isolation.
Existing Loom Use and external coordination clients remain acceptance clients. New Scenario and authoring
capabilities are additive and discovered live; guide/client/package changes are
versioned independently. Legacy Python research revisions remain valid.

Deliberate changes: new authoring defaults to Rust, Scenario adds explicit context,
queue occupancy cannot be hidden by research filtering, history defaults to All,
and documented lifecycle races are prohibited. None permits renaming old evidence,
rewriting an accepted plan or automatically running work during migration.

## Evidence and limits

Structural inventory is coverage evidence, not a semantic proof. Rust domain/fault
tests, independent protocol/browser/client tests, package/CPU execution and actual
GPU/provider/native/reboot acceptance have distinct scopes. Missing/skipped/stale
checks are not passes. Performance targets require measurements, not language counts.
