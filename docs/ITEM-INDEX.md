# Common Items, file inventories and reconciliation

The native service stores compact Item/revision/alias authority in the execution
database. Sealed file manifests and source verification evidence live in private
state directories; the separate SQLite/FTS index is disposable. Native IDs and
historical source/kind/id/revision/snapshot selectors stay intact.

## Installation and preservation

Existing installations use the cumulative updater in UPDATES.md. The explicit
schema chain is 4 -> 5 native authority -> 6 common Items -> 7 reconciliation and
retention. Drain old writers, process owners, callbacks and Runtime validation
before cutover. Each boundary records private backups and preservation receipts.
Startup refuses a legacy writer or an incomplete authority. The numbered previous
migration scripts remain immutable; the schema 7 step supersedes their readiness
checks, not their application order.

Backups preserve catalog manifests, file-sources, log-spool and protected host
runtime-files alongside the original databases and user software. Search generations
are rebuilt rather than restored over authority. Rollback rejects new or changed
Item, binding, import, plan and retention records. Forward repair preserves them.
Portable archives carry hashed manifests/source evidence and stream additional
payload members through a sealed JSONL index. Streamed relocation preserves logical
machine/store IDs. A clone or multi-store mapping needs an explicit reviewed copy
binding plan; the single-store archive tool refuses silent member loss.

## Discovery and files

Discover schemas through system.describe and read the items guide. Item, revision,
File and stream IDs are decimal strings. Search defaults to Research/main and the
latest historical record for each source identity; history=all retains exact older
snapshots. Ranking uses exact, prefix and lexical tiers, then stable time/ID order.
Origin, Research Task, relation, parent, kind and time filters are supported. Cursor
queries preserve the same arguments and expire after source/access/generation
changes. at_least waits at most one second and reports unsatisfied consistency
without making a Job wait for an index.

Large Rust producers use Producer.register_inventory: sorted owner-relative JSONL
members, hashed chunks and an exact primary. Publication verifies selected bytes
off the execution writer and stores a compact source pin. Small legacy outputs
remain compatible. Exact {job,entity,member?} and
{artifact:{id,revision},member?} inputs resolve through authority independently of
FTS, preserve producing-Job dependencies and verify bytes before execution.

File ranges open granted current copies without following links. Accepted inventory,
import and binding proofs are rechecked against the open file's current stamp;
large verified files do not need a whole-file hash for every range. A bounded copy
candidate search can use a valid alternate copy when another is missing/corrupt.
Changed bytes are rejected. Legacy files without a current proof require explicit
verification, or the optional background policy. Ownership is attribution, not an
additional ACL; storage and protected Runtime grants still apply.

## Reconciliation

Agents supply meaning and original evidence; the server enforces the current
contract. Use results.reconcile_contract, reconcile_inspect, reconcile_plan,
reconcile_validate, reconcile_apply and reconcile_get. Plans have stable keys, at
most 200 operations and 256 KiB of proposal metadata. Hash only selected candidates;
apply rechecks proofs, identities, grants and contract context in a bounded commit.

adopt indexes an existing exact native identity. bind_file and bind_historical add
verified current copies while preserving original availability and source rows.
import explicitly creates an external typed asset with original provenance, without
creating Jobs, Attempts or scientific completion events. A known source.native_ref
must be adopted/bound rather than imported as a duplicate. Structural/byte validation
never proves episode counts, tensor semantics, simulation correctness or scientific
acceptance. Unknown schemas/conflicting source reuse fail; v1 and current v2 asset
adapters are discoverable. Replay returns the original applied receipt.

This workflow does not perform an unregistered directory sweep, guess a Research Task,
move all research data, change accounts, install dependencies or resume work.
Runtime inventories require a host administrator and explicit package roots under
the configured dependency directory; missing inventory is not an empty tree.

## Maintenance and quotas

system.rebuild_search creates a durable shadow-generation task; poll
system.catalog_task. Catch-up is independent of execution and the active index stays
readable until an atomic switch. IDs survive rebuild and old cursors expire.
A failed task retains progress; retry_catalog_task resumes it with unchanged roots.
An exhausted batch budget needs a new explicit rebuild request. prune_search removes
retired shadow generations at least ten minutes old and preserves the active one.
Offline catalog-backfill/catalog-rebuild remain available under the writer lock.

jobs.prune_logs selects exact returned, closed stream generations. Authority records
a gap before raw provider/spool copies and searchable text are removed. Legacy log
reads cannot resurrect expired generations. Producer containers and their separate
Docker log retention policy remain intact; no Job or completion event is created.

Catalog settings are optional under api.json.catalog:

| Setting | Default | Boundary |
|---|---:|---|
| index_max_bytes | 4 GiB | Active/shadow/retired private search tree |
| minimum_free_bytes | 64 MiB | Reserved free space for metadata workers |
| log_total_max_bytes | 2 GiB | Private Ops raw spool |
| log_stream_max_bytes | 256 MiB | Per source generation, within host capture ceiling |
| read_copy_candidates | 8 | Granted copies checked per file range (max 32) |
| verify_registered_files | false | Explicit opt-in background payload hashing |

The host provider has a 2-GiB default global capture budget; its administrator can
set LOOM_HOST_LOG_MAX_BYTES. Capture exhaustion preserves a prefix/gap and never
holds process return. Large payload verification is opt-in; an upgrade does not
rehash a research store. Exact offline verification uses:

~~~sh
loom-migrate catalog-verify-file --state "$STATE" --config "$CONFIG" \
  --data "$DATA" --components "$COMPONENTS" --file-id "$FILE_ID"
~~~

UTF-8 extraction is bounded to selected verified text formats, 256-KiB source files
and 8192 characters. Binary/scientific payloads remain metadata-searchable.
Ranges are bounded to 256 KiB. Default index/storage quotas are capacity guards,
not claims that any corpus fits them; configure capacity before a scale migration.

## Dashboard and acceptance

The browser bootstraps compact authority metadata, queue occupants and a separate
KST activity rollup. Catalog pages, exact source details, files, relations and logs
are loaded as requested. Revision-qualified reads preserve historical snapshots
and accepted software definitions. The legacy catalog route remains for compatible
clients; new clients do not need it for bootstrap.

Feature checks and source-qualified test audits are development tools, not Job
execution gates. The opt-in synthetic scale example requires a new fixture root
and reports raw sizes, overlap, disk, RSS and latency; it never scans operational
data. Representative operational acceptance must state actual sample sizes and
separate structural import from GPU/Sim/scientific validation.
