# Proposed MCP/HTTP/query contract

Status: historical Item index proposal. See the [generated API reference](../reference/README.md) for implemented actions and routes.
Clients discover the action groups, scopes and schemas from the connected server.

## Surfaces

| Proposed action | HTTP equivalent | Scope / result |
|---|---|---|
| `results.search_items` | `POST /api/items/search` | `read`; bounded authorized mixed-kind hits |
| `results.get_item` | `GET /api/items/{item_id}` | `read`; exact typed descriptor/native reference |
| `results.resolve_item` | `POST /api/items/resolve` | `read`; full typed legacy alias -> known Item, no allocation |
| `results.item_relations` | `GET /api/items/{item_id}/relations` | `read`; bounded declared edges; no inferred relationships |
| `results.item_files` | `POST /api/items/{item_id}/files` | `read`; bounded exact-revision file pages |
| `results.get_file` | `GET /api/files/{file_item_id}` | `read`; metadata/copies and verification observations |
| `results.read_file` | `POST /api/files/{file_item_id}/read` | `read`; registered range/preview, current source checks |
| `jobs.log_streams` | `POST /api/logs/streams` | `read`; exact Job/Attempt/stage/generation inventory |
| `jobs.read_log` | `POST /api/logs/read` | `read`; raw ranges or bounded tail; closed/gap/retention state |
| `jobs.search_logs` | `POST /api/logs/search` | `read`; scoped text hits with stream/range references |
| `system.dashboard_summary` | `GET /api/dashboard/summary` | `read`; compact navigation/capabilities/authorized summaries; no file/log inventory array |
| `system.search_status` | `GET /api/search/status` | `read`; safe index generation/lag/capabilities only |
| Existing verified reconnect/binding commands | Existing routes | Existing write scopes; add stable File/Item references when advertised |

Rebuild, broad root scans, index location/retention changes and bulk migration are
administrator CLI operations initially. A bounded reconcile apply command can be
added later only with a strict typed schema and existing privilege rules; `read`
never gains filesystem repair, mounting, copying, Docker or arbitrary SQL powers.
Existing `jobs.get/logs`, `results.get/historical/reconnect`, Profile/Runtime APIs
and preview adapters remain available. Searches are not a substitute for exact
input schemas or runtime readiness. File IDs are not accepted as Job inputs until
an explicit adapter resolves an exact revision and verified locator at acceptance.

## Search model

`SearchItems` has text, kinds, exact Scenario/ref, relation selector, category,
presentation scope/role, source origin, current/history/revision selection,
availability filter, recorded/observed time range, sort, cursor and limit.
The default result revision is current; historical/include-all is explicit and
returns exact source/snapshot refs. `view=all` concerns research presentation,
not permission. Queue scope is never affected by search/default curation.

Structured filters plus lexical matching are v1. No natural-language planner,
embeddings, fuzzy scientific equivalence, arbitrary regex/FTS syntax or remote URL
fetch is implied. Bind FTS terms as data, cap tokens/complexity and provide stable
exact-ID/path handling before ranked text. Preserve original case-sensitive paths;
normalize a separate search field. Korean, Unicode and punctuation need fixtures.
Use Unicode-token FTS for words and a reviewed trigram field for >=3-character
substring queries. Shorter queries require exact/prefix filters or a bounded
narrow scope; never fall back to an unrestricted filesystem/SQL scan.
[SQLite FTS5](https://www.sqlite.org/fts5.html).

Initial limits, to freeze against I0 measurements: 50 results default/200 max,
<=1 MiB encoded JSON per response including text envelope overhead, <=64 KiB
metadata body per detail, <=512-byte search snippet, and separate paginated file,
relation and log bodies. Raw log/file pages cap at 256 KiB before encoding.
If a single definition exceeds the detail budget, return a typed descriptor plus
bounded document read, not silently clipped metadata or an oversized MCP response.

## Common envelope

Return `items`, `next_cursor`, `partial`, `warnings` with safe codes, plus:

- `consistency`: origin, authority epoch/head, index generation/indexed-through,
  source observation time and extraction version. Do not label stale text current.
- Each hit: `item_id` as decimal string, kind, exact revision, label, concise match
  basis, `native_ref`, relation/context refs and safe availability observations.
- File hits: parent revision/entry key, content evidence and candidate copy refs.
- Log hits: stream/effect generation and byte ranges; producer timestamps only
  when actually recorded. Navigation does not synthesize a Job.

Example intent: search Evaluation Jobs linked to an exact Scenario, then list the
chosen result's checkpoint files and read the matching Attempt's error range.
Each step uses the previous response's typed IDs; no directory guessing is needed.

New sequence/byte-offset/high-watermark fields use decimal strings or opaque
cursors as well; legacy numeric offset APIs preserve their existing contract and
return an explicit cursor-required error if a value cannot be represented safely.

## Consistency and cursors

Use keyset cursors, not growing OFFSET scans. Bind the cursor to normalized query,
sort/rank, origin, index generation, source high watermark, schema and current
access context. Order by deterministic match tier/rank plus recorded time and ID.
Pin a generation and MVCC sequence S for a bounded TTL or return `cursor_expired`
on rebuild/history expiry. Use immutable versioned projection rows, not only a
watermark over in-place upserts. Initial ranking avoids mutable global corpus
statistics; see [ARCHITECTURE](ARCHITECTURE.md).
Concurrent metadata updates after S appear in a new search; pages return exact
as-of versions. Permission/purge changes invalidate the cursor rather than silently
skipping or exposing a hit. No duplicate or silently skipped hit across pages. Never use UUID order as scientific chronology.

The index worker is eventually consistent. Direct authoritative native reads stay
available. `at_least` may wait up to a bounded timeout for a known core sequence;
otherwise return `index_behind`/partial explicitly, never block execution. With no
usable generation, indexed actions return `index_unavailable`; exact known
Item/manifest descriptor reads can still use authority without global scans.

## Authorization and errors

Derive scope/visibility predicates from existing typed adapters. Ownership remains
responsibility metadata, not a new ACL. Recheck current credentials, source epoch,
tombstones and relevant grants before emitting labels, snippets, counts, facets,
relations or continuation hints. Stale index access annotations cannot override
revocation. Do not reveal hidden matches through aggregate counts or error shape.
Diagnostics/event recipients do not inherit ordinary `read` merely from Item IDs.

Planned safe error codes include `unknown_item`, `ambiguous_legacy_ref`,
`index_unavailable`, `index_behind`, `cursor_expired`, `query_too_broad`,
`source_unavailable`, `content_changed`, `retention_gap`, `paging_required` and
`conflicting_identity`.
Keep HTTP/MCP error-envelope compatibility; an action error is not transport success
with an invented empty list. Incident details never contain query bodies or secrets.
