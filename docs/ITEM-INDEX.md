# Item index operations

The native development candidate uses core schema 6. Common Item IDs, exact aliases
and revisions are authoritative metadata. File IDs are sealed in chunked manifests.
`runtime/search/catalog.sqlite3` is a derived FTS projection and can be rebuilt.
Never restore that DB as a substitute for lost authority manifests.

## Configuration

The optional `catalog` object in the installation API config accepts absolute
`manifest_directory` and `index_directory`. Defaults are beneath the configured
state directory (`catalog-manifests` and `search`). No dependency/data root is
hardcoded. Use private directories reachable by the service identity. Keep the
manifest directory separate from the derived index; preserve it with the core DB.
The updater's default private snapshot excludes the derived search directory and
preserves authoritative inventories. For packaged Compose updates, overrides must
remain beneath the configured state mount (`/loom/ops` inside the container).
The updater maps that mount to the installation's actual host runtime directory.
An external manifest mount requires a separate preservation plan; the updater
refuses to guess its host path. Offline native commands accept host paths directly.
Portable exports continue stripping credentials.

One background worker adopts compact native metadata in bounded transactions;
a separate SQLite writer projects changes. File inventories project at most one
4-MiB chunk per checkpoint transaction. File entries do not add one row each to
the core Item registry. Search reads use their own read-only connections, not the
execution writer. An index failure does not stop native exact reads or Job returns.

## Additive upgrade

Stop/drain writers using the existing update contract. Keep the old native cutover
migration unchanged. The cumulative `common-item-index-v1` step then applies the
schema 5 -> 6 boundary, preserving old tables and credentials. It does not rerun
Jobs or register another dataset. The schema 6 successor verifies readiness on
subsequent skipped-version updates without changing the old migration checksum.

For an offline explicit migration, use the packaged binary and the installation's
actual roots/configuration; run filesystem indexing with the service identity:

```sh
loom-migrate catalog-install --state "$STATE" --config "$CONFIG" \
  --data "$DATA" --components "$COMPONENTS" --backup "$BACKUP"
loom-migrate catalog-backfill --state "$STATE" --config "$CONFIG" \
  --data "$DATA" --components "$COMPONENTS" --batches 100
```

`catalog-install` preserves original table hashes and writes a private rollback
receipt. `rollback --state "$STATE" --receipt "$RECEIPT"` refuses changes after
the captured boundary, including allocated common IDs. Use forward repair once
new records exist. Never restore an older DB merely to make search look healthy.

## Rebuild and verification

With the writer stopped, `catalog-rebuild` accepts the same roots/config and batch
budget. It retires the derived generation, replays the authority journal and reads
retained manifests. IDs and original results remain unchanged. Old cursors expire.
A batch limit can leave explicit `index_behind` status; resume with backfill.
This operation does not recursively discover unregistered research directories.

Text extraction is limited to registered, verified UTF-8 text formats and 256-KiB
source files; the search text budget is 8192 characters. Binary/scientific payloads
remain searchable by path and recorded metadata. Background verification is capped
at 64 MiB per candidate file; larger files require an explicit offline command:

```sh
loom-migrate catalog-verify-file --state "$STATE" --config "$CONFIG" \
  --data "$DATA" --components "$COMPONENTS" --file-id "$FILE_ID"
```

Verification receipts bind the digest to device/inode/size/mtime/ctime. Registered
ranges recheck the open file's stamp and source grant, never the entire store.
Changed content is rejected. Reconnection uses the existing verified typed binding
APIs; the index never approves a scientific mismatch or changes an old manifest.

`catalog-prune` retires superseded projection versions only beyond the cursor TTL
safety margin. Raw log removal is not automatic. Preserve the Job/effect source
and its generation evidence when administering spool retention; this version does
not infer erased bytes or producer timestamps from search snippets.

Registered log bytes are stored in the service state `log-spool`, outside Job
output mounts. External capture is limited to 256 MiB per physical source generation. The Ops
spool also defaults to 256 MiB; `catalog.log_stream_max_bytes` overrides the spool
budget within the provider limit. Exhaustion preserves the
observed prefix and marks a gap; it never delays process return or deletes bytes.

## Current limits

Search is eventually consistent, with default 50/max 200 results and bounded
file/log byte pages. Common IDs do not replace existing specialized IDs or the Job
input schema. Default discovery filters do not affect queue occupancy. Exact
historical aliases include the original snapshot. Ownership is attribution, not
an additional ACL; current storage/grant checks still apply to byte reads.

The initial implementation indexes registered legacy output inventories and
source pins. A new inventory-backed producer publication/input format, Runtime
payload-tree inventories, automatic log retention and full server-paged replacement
of the legacy Dashboard bootstrap are not activated. Existing producers, inputs,
research records and scientific runtimes retain their existing contracts.
