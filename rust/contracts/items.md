# Common Items, files and logs

When `system.describe` advertises these actions, prefer `results.search_items`
to repeated full catalog reads. IDs are unsigned 128-bit decimal **strings**;
never convert them to JavaScript Number or replace an existing Job/Component ID.
A search hit includes its exact `native_ref` and `revision_id`. Use native schemas
for execution and registration. An Item ID alone is not a Job input or permission.

Search by text, kinds, exact Scenario, presentation `view`/`role`, category and
recorded time. Default discovery is Research/main. Use `view=all, role=all` for
technical/support records; this never changes the actual execution queue.
Reuse the identical query with `next_cursor`; restart after `cursor_expired`.
`partial` and `system.search_status` describe projection lag. Empty partial results
do not establish that a record or file does not exist. Exact native reads remain
available while an index is being built.

Use `results.get_item` for compact metadata, `results.item_relations` for explicit
context and `results.item_files` for bounded file pages. The initial direct-relation
preview is capped at 200 edges; `relations_truncated` is not complete graph coverage. Supply `revision_id` when
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
change pins or run a reconciler. Root selection, backfill, rebuild and retention
are explicit administrator operations documented in `docs/ITEM-INDEX.md`.
