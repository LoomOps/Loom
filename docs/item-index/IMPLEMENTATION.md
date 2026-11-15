# Initial native implementation ledger

This is the first implementation of the approved design, not completion of every
phase in ROADMAP. No operational installation, research data, account, token or
external agent was changed. No release was published or deployed.

## Implemented boundaries

- UUIDv7 common IDs as lossless decimal strings; native identities remain exact.
  Registry/revision/alias changes and the independent journal share the domain
  transaction. Historical snapshot selectors preserve original rows.
- Stable File IDs in sealed, resumable 4-MiB manifest chunks; source digests reject
  changed replay prefixes. Core registry size follows owners/revisions, not every
  file. Derived SQLite FTS projects bounded chunks and has separate checkpoints.
- Literal Unicode/trigram discovery, structured filters, exact descriptors,
  revision-pinned file pages and registered byte reads. Archived Scenarios and
  declared historical/parent presentation keep their existing discovery semantics.
  HMAC cursors pin immutable
  projection versions and bind query, generation, actor, credentials and access
  epoch. Source changes/rebuilds expire invalid cursors.
- Granted current-copy reads, digest/stat verification receipts, bounded UTF-8
  extraction, and offline verification of larger registered files. Reconnection
  continues using existing verified typed binding APIs.
- Per-effect/Attempt/stage log streams with raw byte offsets and explicit source
  generations. Spool bytes live outside producer mounts. Container log I/O releases
  the Executor authority mutex; process return precedes asynchronous log copying.
  Raw ranges and text search preserve byte references, gaps and partial coverage.
  Only committed spool offsets are indexed/read; an interrupted append cannot
  publish a transient tail. Independent reaping seals returned captures and a
  provider file-size ceiling bounds each physical source generation.
- Additive schema 5 -> 6 migration, original-table preservation checks, guarded
  rollback, explicit startup schema/identity fences, bounded backfill, index
  retirement/rebuild and projection pruning. Disposable index paths cannot overlap
  authority/payload/log roots or redirect through linked ancestors. Packaged
  snapshots map configured paths under the actual state mount and refuse unknown
  external preservation mappings.
  Private snapshots preserve authority manifests/spool; portable exports preserve
  manifests while stripping credentials. Existing numbered cutover code is unchanged.
- MCP actions stay in six groups. The Dashboard's existing search launcher uses
  server-backed discovery when advertised, with lazy Item/file/log details.
  Native guides and the independent client Skill explain lossless IDs and limits.

## Verification and acceptance boundary

Native tests cover transaction rollback, exact equal-definition revisions, reversed
backfill order, Unicode/punctuation, current access/cursor checks, index deletion
with stable File IDs, digest-vs-identity, manifest corruption, 100,000 streamed file
entries without per-file core rows, and >10-MiB raw log byte/range fixtures.
Independent tests exercise real native HTTP/MCP, actual CPU Docker execution,
slow log capture versus cancellation, collector reaping/budget exhaustion,
uncommitted append visibility, cumulative migration/preservation, the
packaged client and Firefox through the retained authenticated relay.

Actual run receipts remain outside distributable source. Feature audits bind test
cases to the final source hashes; earlier design/baseline PASS results are not
reassigned to this feature. Real host rollout and scientific GPU/Sim validation
are outside this implementation task.

The final isolated run passed **841 Loom cases**: 87 Rust, 30 native protocol/image,
339 retained Ops, 84 Dashboard, 244 packaging/migration and 57 Firefox cases.
The independent client passed 60 cases. All 19 feature blocks passed the
source-qualified audit; strict workspace Clippy and formatting checks passed.
The Python-free verification image matched the captured Rust/guide sources.
These results accept the implemented boundaries above, not the open target work.

## Open target-design work

| Boundary | Status / reason it is not advertised |
|---|---|
| Inventory-backed producer publication and downstream input format | Not activated. Legacy output/input contracts remain unchanged; the new manifest engine can stream inventories but does not yet replace producer verification/publication with bounded inventory handles. |
| Runtime/dependency payload-tree inventories | Runtime descriptors are adopted from protected observations; full payload/source tree adapters and separate dependency Items are pending. |
| Complete graph traversal and revision/history search | Initial relations expose declared Scenario/producing Job/workflow context. Full software combinations, all historical native revisions and graph pagination remain pending. |
| Ranking tiers, origin/relation filters and at-least waits | Initial ordering uses recorded time/ID and lexical filters. These additional query operators are not advertised. |
| Log retention and global disk quotas | Per-stream external capture budget preserves a prefix/gap; raw deletion and administrator retention manifests are pending. |
| Entire Dashboard bootstrap pagination | Search/files/logs are lazy and bounded. Existing catalog/queue bootstrap remains for compatibility. |
| Atomic generation switch with concurrent readers | Offline rebuild retires the old directory under the writer lock. Online shadow generation switch is pending. |
| Full scale/image acceptance | 100k-file native fixtures are included; the proposed constrained-envelope latency/RSS benchmark, million-file/10m-log-document run and release-kit acceptance remain separate. A Python-free, read-only native verification image is exercised on private CPU fixtures; this is not an operational release. |

These gaps keep I2/I3/I4/I5/I6 partially open. The implementation must not be
reported as the complete I1–I6 plan or deployed as fully accepted search migration.
See [operations](../ITEM-INDEX.md) for current configuration, commands and limits.
