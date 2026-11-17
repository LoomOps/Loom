# Registry lifecycle

## Contract and scope

Archive/Restore applies to exact registered Component, Tool, Runtime, metadata
Template, report Contract and report Definition revisions, and exact Profile
kinds. Profiles already identify an immutable revision by kind; their lifecycle
revision selector is zero. Dependencies are observations of installed packages,
not independently deletable registrations. Job/output/Task history keeps its
existing lifecycle and is not cascaded by these actions.

States are `active`, `archived` and `deprecated`. Archive hides normal Dashboard
discovery; an Archived/All filter and exact links retain access. Deprecated is
advisory and remains visible. Neither state changes execution admission, accepted
pins, resource leases, source hashes, files or historical manifests. MCP/API exact
reads and ordinary discovery retain lifecycle metadata. Agents prefer active
resources; explicit existing pins remain valid.

There is no physical deletion, package uninstall or automatic cascade. Related
registrations can be explicitly selected for one atomic batch. Past and running
Jobs remain visible and keep their original ownership, state and accepted pins.
Built-in resources are immutable to these operations, including for administrators.

## Application/API

`components.inspect_lifecycle` accepts targets and desired state. It returns exact
current states, management rights, registered reverse references and dependencies, recorded Job
usage and a digest. Responses bound related registrations and Jobs to 100 each, expose totals/truncation, and hash the full reference/usage set. Reference coverage includes registered definitions and frozen
Job pins; payload file contents are not recursively scanned. Absence of a listed
reference is never evidence that physical deletion is safe.

`components.set_lifecycle` accepts the same explicit targets/state plus reason,
request key and expected digest. It rechecks rights, references and states under
an immediate SQLite transaction, writes all selected changes or none, and stores
an audit event and idempotent receipt. A stale preview returns a conflict. Only
an item's author/current assigned owner or the authenticated installation admin
may change it. A submit grant alone does not confer management rights.

Reuse `runtime_curation` for lifecycle overlays, preserving other curation fields
and compatibility with existing Profile/Runtime archive operations. Legacy `curate` state changes enforce the same management rights and Built-in protection, and write the same audit; they do not offer a preview digest. Never update
sealed registration bodies. Dashboard/search visibility is checked against current
authority before pagination; visibility-bound cursors expire after lifecycle changes.
Archived metadata registries are filtered before offset/limit and totals.

## Consumers and verification

Rust Core owns identity resolution, authorization, reverse references, atomic
state changes, projection and paging. Dashboard owns filters, preview/selection
and apply feedback. Parent relay admits only the dedicated inspect/apply actions;
credentials stay outside the iframe. Server guides, generated API reference and
Loom Use explain exact targets and the preservation boundary.

Verify owner/admin/outsider permissions; Built-in protection; missing/stale/mixed
batches; idempotency and restart; shared references and active Job preservation;
pre-pagination filtering and cursor invalidation; legacy curation compatibility;
registry browser filters, restore, selected-related batching and narrow themes.
No production records are archived as implementation acceptance.
