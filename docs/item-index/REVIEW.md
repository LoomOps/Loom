# Independent source and cross-block review

Status: planning observations only. Baseline commits and file fingerprints are in
[SOURCE-REVIEW.json](SOURCE-REVIEW.json). This is a targeted source/contract review
of all affected feature boundaries, not a claim to prove every line of the earlier
whole-tree inventory or any operational machine's data.

Method: inspect native schemas/ID generation; follow publication, historical copy,
Runtime, log and read paths; inspect separate client/consumer constraints; then
cross-check all 19 catalog features and their verification blocks. Distinguish a
missing new feature from a confirmed inherited issue. No runtime fix is applied.

| Finding | Source observation | Planned disposition |
|---|---|---|
| R01 — Domain identity is not one registry | `store::id` emits UUIDv4; jobs/results/software/history have different exact keys; `item_owners` is a display overlay | Add common IDs/aliases without renumbering native records; ID-01..04 |
| R02 — File preview identity includes its copy | `media::file_dtos` encodes parent/component/storage/ref | Separate stable logical File ID from location and retain preview adapter; ID-05 |
| R03 — File inventory belongs outside the scheduler hot DB | Native output manifests contain member evidence; one additional core row per source file would scale with byte inventory | Seal ID-bearing chunked manifests, keep core descriptors and index file rows; FILE-06 |
| R04 — Current reads are fragmented/full projections | `results_listing` iterates native/history; `dashboard_catalog` builds a full entity projection; parent loads it before normal workspace data | Add bounded indexed search and a lightweight/lazy UI path; QUERY-01..04, UI-01 |
| R05 — Historical exact snapshot gap | `historical_files` requires snapshot; `history_record` filters source/kind/id/revision then latest row, without snapshot predicate | Exact Item resolution must not use the latest-row helper for a supplied snapshot; add an exact adapter/compatible optional selector; ID-03 |
| R06 — Live log read can delay Executor authority | `ticket_action` captures full Docker logs on an open ticket; `wire::serve` holds the shared Executor mutex around handle | Separate bounded capture from authority lock before adding log search; LOG-02 |
| R07 — Log generation/return ordering needs adoption work | Worker syncs stage logs before `stage_returned`; output filename is worker.log or stage.log, while retries have effect generations | Prompt return evidence, durable asynchronous seal and generation-qualified stream/legacy adapters; LOG-03..06 |
| R08 — Runtime/dependency identity needs exact origin | Runtime authority is a separate host SQLite DB; dependencies aggregate display families by normalized package name; observation freshness is separate | Use origin/runtime-revision/scope/manifest evidence for aliases, not the display family alone; SW-01 |
| R09 — Existing backup classification is insufficient for the new design | Snapshot code currently preserves instance/extension/host-catalog files with no new manifest/index classification | Add authoritative manifest preservation and optional derived-index rebuild rules; MIG-01..03 |
| R10 — Client/consumer compatibility is bounded | Published transport caps replies at 2 MiB; external coordinator checks native Job identity and owns its own receipts/events | Bound encoded responses, preserve native IDs and event semantics, add only discovered capabilities; QUERY-04 |
| R11 — Current spec prose has authority drift | Feature README still describes Python Ops as live extraction target; Scenario feature prose forbids archived exact pins although native pin permits them; diagnostics prose describes retained Python sink as universal | Correct touched specs/verification contracts during I1/I3/I5; do not reproduce obsolete prose as behavior |
| R12 — No search security shortcut | Existing read access is shared workspace scope, ownership is responsibility, events are recipient-specific, diagnostics are private | Reuse typed source access; exclude private/event/credential bodies and check snippets/facets/cursors after revocation; AUTH-01..03 |

## Design counterexamples checked separately

- All files as ordinary core registry/legacy location rows would undo DB separation:
  canonical inventories plus an inventory-backed new-output adapter are required.
- Allocating IDs only in the search DB would change IDs after rebuild: stable IDs
  must be preserved in the authority intent and sealed manifests first.
- Cursor generation/watermark plus in-place upserts can skip updated rows; live
  BM25 statistics can reorder them: retain MVCC document versions and stable rank.
- Reusing ordinary events for projection could duplicate external reports or consume
  recipient ACKs: use a distinct catalog journal and no new research Job/event.
- Treating index backup as a credential-transfer archive changes current portability:
  same-instance preservation and credential-stripping export remain distinct.

## Consequences for the design

Stable identifiers alone are insufficient: preserve their mapping in authoritative
metadata before indexing. A separate index alone is insufficient: replayed changes,
source manifests, generation/checkpoint validation and source permissions are needed.
Hash matching alone is insufficient: it does not establish scientific identity.
Log offsets alone are insufficient across rotation or repeated effect generations.

The design avoids a source-incompatible generic object store, a global integer
allocator, an extra scheduler Task hierarchy and synchronous writes to two DBs.
It adds explicit source adapters, compact authority metadata, reusable inventories
and a disposable retrieval projection. No existing execution gate is expanded.

## Cross-repository boundary

The independent Loom Use checkout was read for Skill guidance, response size and
schema/capability discovery. Its transport can remain generic; Skills/guides and
actual packaged-client acceptance change with the server capability.

A local external coordination adapter was read only for native Job/snapshot/event
compatibility. Its exact Job ID checks and independent task/receipt ownership must
remain valid. New search/item notifications must not duplicate its completion
reports. No coordinator code, installation, thread or operational source was changed.

## Review limitations

No production catalogs, remote conversations, filesystem inventory, provider policy,
network host, installed client or live research Job was inspected or modified.
No FTS, UUIDv7 generator, index implementation, million-file behavior or log latency
acceptance was executed. Official references constrain the proposed implementation;
new dependency/SQLite feature/version behavior must be checked in its actual build.

Implementation must revisit these findings against executable cases. Existing
verification blocks and the native candidate's passing suite remain historical
baseline evidence, not a PASS for the proposed Item/index system.
