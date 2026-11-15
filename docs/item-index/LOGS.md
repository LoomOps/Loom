# Log streams, incremental retrieval and retention

Status: proposed. Execution logs and private platform diagnostics remain distinct.

## Stream identity and lifecycle

Register a stream for `(installation origin, Job, Attempt, stage, effect ID,
effect generation, channel)`. Stable stream IDs are recorded before opening the
source. Use `combined` where the existing container source does not preserve stdout
versus stderr ordering; do not fabricate channel separation or wall-clock timestamps.
A finalizer/hook retry has another effect generation even if its Attempt is unchanged.

Lifecycle: `registered -> open -> sealing -> closed`, with independent `gap`,
`unavailable`, `partially_retained` and `purged` evidence. Container/process return
is not log-delivery success; log capture/index failure cannot retain or reclaim a
returned execution resource. Raw-byte and text-index high watermarks are separate.

Each segment has stream ID, source generation, start/end byte offsets, sealed digest,
byte count, locator, capture time and any recorded timestamp range. Segments are
search documents, not global dashboard Items. Chunk/line number is not execution
Step or Attempt count. Repeated Docker reads/restarts cannot append duplicate bytes.

## Required change to current host behavior

Current `ticket_action(log_range)` invokes full `docker logs` capture for an open
container. `wire::serve` holds the Executor mutex during `handle`, so a large log
read can delay status/cancel. Current worker sync also precedes `stage_returned`.
These are source-observed paths, not measured production latency claims.

Introduce an independently bounded collector/read path. Under a short authority
lock, validate instance/peer/grant/ticket/effect and obtain an immutable read
capability. Release that lock before Docker/log filesystem work. Per-stream locks
serialize append/seal only; they do not block unrelated tickets or cancellation.
All reads revalidate scope and prevent stale capability reuse after revocation.

Where the source provides an exact append cursor, checkpoint byte intervals and
collect only new bytes. Where Docker timestamp replay cannot prove byte continuity,
retain bounded snapshots/overlap evidence and mark uncertainty/gaps. Do not treat
`--since` timestamps as unique event offsets. A bounded full-snapshot compatibility
fallback runs outside the authority mutex, is rate/size limited and is not the
advertised incremental path. Negotiate host capabilities before native adoption.

Report actual process/container return promptly using existing fences. A separate
durable seal task preserves pending logs, preventing premature removal of their
source container until collected or explicitly expired by retention policy.
This task is an internal maintenance activity, not another research Job or user
completion event. Failure remains visible as log availability, not Job failure.

## Retrieval and text search

Proposed `jobs.log_streams`, `jobs.read_log` and `jobs.search_logs` accept common
stream IDs and optional exact native Job/Attempt/stage filters. Read caps at 256 KiB
raw bytes per page initially; encoded reply budgets must stay below the client bound.
Use a signed cursor binding stream generation, offsets, query and access context.
Return `next_cursor`, raw total/high watermark, truncation/retention/gap evidence,
source check time and `tail_available`. Preserve arbitrary bytes with base64;
UTF-8 decoding across segment boundaries is an explicit presentation operation.

Legacy `jobs.logs(id,attempt,offset,limit)` remains an adapter. It must not silently
concatenate different retries, change old offsets or claim an observed EOF is a
closed stream. Bounded tail is polling in v1; no unbounded MCP subscription is needed.
Search hits identify exact byte ranges and return only authorized bounded context.
Line boundaries/CRLF, multibyte UTF-8, incomplete final lines and equal timestamps
need fixtures. A matching line proves only text presence, not a diagnosed root cause.

## Retention and privacy

Source retention and searchable text retention are explicit installation settings:
byte/time budgets, hot/archive tiers, per-class rules and legal/operational hold if
later introduced. This plan does not select a new default or purge existing logs.
Sealed old segments can move with verified locators and preserved stream IDs.

On expiry, commit a retention receipt/tombstone, remove indexed body/snippets and
rotate index generation/cursors as necessary. Retain minimal permitted identity/gap
metadata. Search caches cannot keep deleted source content indefinitely. Purged
bytes remain unavailable even if their old hash is known.

Private diagnostics, tokens, request bodies and recipient-specific messages are
excluded from ordinary Job-log indexing. Do not promise automatic secret removal
from arbitrary producer output; use existing source permissions and explicit
retention, with separate reviewed diagnostics support if ever requested.
