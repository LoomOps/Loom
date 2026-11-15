# Reconcile policy and evidence

Status: proposed bounded catalog reconciliation. No general autonomous reconciler
or authority to repair operational state is created by an indexer.

## Evidence domains

Keep four facts independent: recorded identity/provenance; expected immutable
content; currently observed copy/availability; derived searchable projection.
Store verification time/method, source/epoch/version, bounded evidence digest and
actor/command receipt. A metadata stat, source name, equal file size, successful
text extraction or indexed document is not a verified content/provenance claim.

| Observation | Automatic derived action | Authority change |
|---|---|---|
| Duplicate journal delivery | Idempotent document upsert/checkpoint | None |
| Missing/corrupt/outdated search generation | Rebuild from exact retained descriptors | None; preserve all IDs |
| Known unchanged registered copy | Refresh observation using valid local cache evidence | None |
| New path with matching verified bytes | Candidate copy proposal | Keyed verified binding, exact expected version and store identity |
| Same path now has different content | Mark observation conflict/unavailable for old content | Never replace old evidence; explicit new revision/source generation |
| Two logical files with the same hash | Share optional hash computation cache | Never merge their Items or lineage |
| Missing mount / permission denial / offline source | Record corresponding unavailable state | Never infer deletion or remap store |
| No entry seen in an incomplete scan | Preserve last observation, mark scope incomplete | No tombstone |
| Complete authorized check proves copy absent | Append missing-copy observation | Logical Item remains; withdrawal is explicit |
| Source history and current live bytes differ | Expose separate candidate evidence | Require original snapshot/verified copy; no automatic promotion |
| Lost authority manifest but surviving search row | Quarantine rebuild for that scope; retain evidence | Restore authoritative backup or reviewed import; never mint replacement IDs |
| Catalog clone with conflicting native origin/alias | Reject conflicting adoption | Explicit origin/import policy with provenance |
| Purged log interval | Remove search body, record retained gap | Never reconstruct bytes or claimed chronology from hits |

## Bounded inspect / propose / apply

Inspect names a typed owner/inventory/store scope, source version and max work
budget. Return issues/candidates with continuation, not a recursive host-wide sweep.
A proposal records the exact Item/revision, locator, expected digest/size, observed
file stamp and why it matches. It is evidence, not a path accepted from a model.

Apply requires existing write/administrator scope appropriate to that operation,
a stable key and unchanged expected version. Hash outside the core writer; recheck
stamp, grants, store marker, cancellation/epoch and immutable expected evidence in
the fenced commit. Replay after moving the already accepted copy returns the
original receipt; changed bytes or a changed request key conflict. Bulk changes
use sealed, hashed proposal/receipt manifests and bounded commit batches.

Index observation writes are separate from authoritative copy bindings. No indexed
`available` flag can make a dependency ready. Job inputs still resolve through the
existing verified result/store contract. Explicit file-to-native input support must
pin a revision at acceptance; a current/latest search result is not such a pin.

Automatic repair is limited to rebuilding derived state and updating truthful
observations. No deleting files, rewriting historical bodies, moving stores,
changing accounts/ACLs, killing Jobs, installing dependencies, relabeling research
or importing a directory as a dataset without a separate authorized command.

## Reconcile invariants

One File Item survives a verified relocation and can retain multiple copies.
Alias/inventory adoption is unique and idempotent. A hash match identifies bytes,
not the experiment that produced them. Current permissions apply at read and apply,
including when a proposal was created under an older grant. Source visibility and
scientific lineage are never inferred from an accessible filesystem path.

This makes future reconciliation more tractable: stable identities and explicit
observations replace repeated filename guessing. It cannot recover lost provenance
or IDs when every authoritative copy/manifest and backup has been destroyed.
