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

File access is separate from the Outputs summary. A remote MCP client with the
existing read grant can discover registered files and read their actual bytes;
local filesystem or SSH access is not required for this API path. This is bounded
inspection, not a bulk archive download or analysis service. TAR/ZIP members are
not individually searchable unless the producer registered them in an inventory.
Use a compatible Loom Job with accepted file inputs for large-data processing, or
an existing separately authorized transfer route. No download URL is implied.

Use `results.get_file` before `results.read_file`. File identity, digest and copy
locations are different concepts. Read registered ranges using decimal `offset`
and a maximum 262144-byte `limit`; decode `data_base64` as bytes. Copy reads check
current storage identity, permissions and recorded digest. Changed bytes are an
error. Verification of files above the automatic background budget requires an
administrator verification receipt; an available copy may still return
`verification_pending` until verified. Each range is at most 256 KiB (base64 on
the wire), so reconstructing GB archives through MCP is inefficient; retrying a range never triggers a whole-store
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

## Local/SSH file analysis

Use MCP for File identity, current copy locations and expected hashes; keep large
file bytes on the owning host. For one `storage` copy from `results.get_file`:

1. Match its `machine_id` to the selected connection's `system.status.machine_id`.
2. Match its `storage_id` to a selector in `system.status.data_stores`.
3. Use that selector's host root from `system.ssh.data_paths` plus the copy's
   nonescaping relative `ref`. Do not use a container mount path as a host path.

`system.ssh` also provides the current caller's optional SSH descriptor. Same-host
clients use the current OS account. Remote clients need an enabled personal grant
or an existing explicitly authorized manual route. An MCP token is not an SSH
credential; metadata resolution does not prove login, file or interpreter access.
A copy on another machine requires its own selected connection; never guess a host
from a historical machine label or forward this connection's token to it.

The independent Loom Use client offers `ssh resolve` and optional read-only probes;
see its [SSH how-to](https://github.com/LoomOps/Loom-Use/blob/main/docs/ssh.md).
Older clients can compose the metadata above without downloading the file through
MCP. Preserve exact File/revision identity, copy locator, size and SHA-256. Check
actual account permissions, storage identity and expected bytes before relying on
an analysis. An SSH-side hash check does not create a server verification receipt.

Use an existing suitable **host** interpreter for short CPU inspection. A registered
container Runtime is not proof that its Python path or libraries are available on
the host or readable by that SSH account. Select compatible Runtime/Profiles and
Loom Jobs for GPU or long-running work; do not bypass the queue with SSH processes.
Do not install packages merely because the host default Python lacks a reader.

Inspect TAR/ZIP contents in place when practical and write derived working files
in the personal workspace. An archive-member hint does not create a registered
File or prove that member's contents. Follow explicit manifests, current historical
bindings or preserved reference indexes for unregistered legacy working files.
Search can be ready while `file_content_coverage_bounded` still marks file queries
partial. Zero hits in such a query do not establish filesystem absence, and a
single file question does not require whole-store reconciliation.

A bounded file-content page may contain no readable matches and still return a
`next_cursor`. Follow that cursor to continue the same search snapshot; stop when
it is null. Unreadable content matches are excluded from the returned Items.
