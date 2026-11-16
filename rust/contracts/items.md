# Common Items, files and logs

When `system.describe` advertises these actions, prefer `results.search_items`
to repeated full catalog reads. IDs are unsigned 128-bit decimal **strings**;
never convert them to JavaScript Number or replace an existing Job/Component ID.
A search hit includes its exact `native_ref` and `revision_id`. Use native schemas
for execution and registration. An Item ID alone is not a Job input or permission.

Search by text, kinds, exact Scenario, presentation `view`/`role`, category and
recorded time. history=all includes exact recorded revisions; origin_type,
native_kind and related_to narrow source and declared relations. Exact matches
rank before prefix then lexical matches, with stable time/ID ordering within a tier.
at_least is a decimal change sequence; timeout_ms waits at most 1000 ms and
reports at_least_satisfied=false if still behind. Default discovery is Research/main. Use `view=all, role=all` for
technical/support records; this never changes the actual execution queue.
Reuse the identical query with `next_cursor`; restart after `cursor_expired`.
`partial` and `system.search_status` describe projection lag. Empty partial results
do not establish that a record or file does not exist. Exact native reads remain
available while an index is being built.

Use `results.get_item` for compact metadata, `results.item_relations` for explicit
context and `results.item_files` for bounded file pages. Relations default to 50/max 200 edges per page. Follow next_cursor with
identical arguments; source changes expire the cursor. direction=outgoing reads
declarations; direction=both includes indexed reverse software use.
include_support=true includes non-representative Runtime dependencies. Supply `revision_id` when
following an exact revision. `results.resolve_item` accepts the full typed legacy
reference, including historical source/kind/id/revision/snapshot. It allocates no
identity and does not promote historical records to new research results.

Use `results.get_file` before `results.read_file`. File identity, digest and copy
locations are different concepts. Read registered ranges using decimal `offset`
and a maximum 262144-byte `limit`; decode `data_base64` as bytes. Copy reads check
current storage identity, permissions and recorded digest. Changed bytes are an
error. Verification of files above the automatic background budget requires an
administrator verification receipt; retrying a range never triggers a whole-store
scan. Do not guess absolute paths or repair a scientific mismatch automatically.

Use `jobs.log_streams` with an existing Job ID to discover exact Attempt/stage/
effect streams. `jobs.read_log` supports raw byte ranges or bounded `tail`; keep
`source_generation` with subsequent offsets. A source restart, rotation, missing
bytes or a retention gap is evidence, not permission to concatenate replayed bytes.
`jobs.search_logs` searches indexed ranges within a Job or stream. Follow returned
stream/range references for raw bytes. Index coverage may be partial.
Log collection is independent of process return and result publication; it does
not create Jobs, consume event ACKs or generate another completion notification.

Search/index operations never build software, install dependencies, submit work,
change pins or run a reconciler. Root configuration is host administration. Backfill, rebuild and retention
are explicit submit-scope operations documented in `docs/ITEM-INDEX.md`.


## Streamed outputs and inputs

For large outputs use the Rust SDK Producer.register_inventory with sorted unique
owner-relative member paths including the exact primary. It emits
loom-output-inventory/v1 with hashed bounded JSONL chunks; core publication stores
a compact authority handle, not one registry row per file. Inline legacy members
remain supported. Use item_files for inventories above the legacy detail limit.

An input may select {job,entity,member?} or
{artifact:{id,revision},member?}. These are native artifact identities, never Item
IDs or search-result positions. Loom verifies the exact member independently of FTS,
retains producing-Job dependencies and records consumes lineage. Accepted Profiles
must allow the chosen input shape; reuse existing compatible revisions.

## Search and log maintenance

system.rebuild_search returns a durable task; poll system.catalog_task. The active
index remains available until the shadow catches up and atomically replaces it.
Old cursors expire; Item/File IDs remain unchanged. Failure leaves the prior index.
jobs.prune_logs selects exact closed, returned stream generations and a reason.
It records retained gaps before removing raw provider/spool copies and searchable
text. Legacy logs cannot bypass expiry; use exact stream ranges to inspect coverage.
These actions do not acknowledge events, create scientific Jobs or stop containers.

For explicit adoption/import/copy binding, load the `reconciliation` guide.
