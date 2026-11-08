# Loom Dashboard

The API-only console presents Jobs, results, resources, Docker and MCP Reference.
It has no Ops database, Docker socket or research runtime mounts. Release native Tailscale authentication
uses an instance-local session SQLite database and a private relay key.
The main UI is the Loom workspace.

## Development and release

From the Loom checkout run `bash deploy/compose.sh up -d`. The default isolated
`loom-dev-build` project uses the ports in its instance configuration. Source
mounts rebuild/reload UI edits; Ops source changes need a safe Ops restart. Dev
explicitly enables tokenless owner access. `/session` declares that mode.

`bash deploy/runtime.sh` selects the separate, authenticated alpha deployment and
is used only for an authorized release. Images are immutable numbered builds;
source changes do not automatically update alpha. Compose supplies the internal
Ops origin; Dashboard publishes only its configured loopback port. Tailscale
Serve provides the external HTTPS origin. For new hosts start with the root [INSTALL.md](../INSTALL.md); the low-level installer alone is not a complete installation.

Build the workspace from its hash-pinned synthetic source:

```sh
python3 dashboard/workspace/build.py dashboard/workspace/template.html
```

The synthetic template's SHA is checked. The deployed output starts with an empty
catalog and obtains live records through the API. The opaque iframe receives scoped data
from the parent; only the parent holds the page-memory bearer and network access.
Shared links carry routes without credentials. `/session` and `/api/status`
distinguish image baseline and live source; DEV BUILD is development-only.

## Queue and data

Current queue order, hold flags, waiting reasons and caller capabilities come
from the same backend scheduling function used by dispatch. Authenticated users share Job reads and queue operations; Owner indicates responsibility.
Running Jobs
are never interrupted. HTTP POST `/api/jobs/queue-actions` forwards only these three
actions. Other explicitly supported writes include Job cancellation, Admin token issue/revoke and update checks; arbitrary API writes are rejected. Use Ops MCP/API for other operations.
Same-origin and native session/CSRF checks apply to browser operations; explicit token access is limited to the configured non-SSO path. Job detail shows
Attempts, recovery, actual resource return, artifacts and queue-change history.
Termination history starts at 10 rows with expansion/collapse. No aggregate
Execution log or separate Loom Jobs queue is added.

The catalog is revision-checked every 30 seconds while visible; manual refresh
is available. Queue changes refresh the catalog immediately. Resource data has
its separate visible-screen refresh. Historical payload availability, missing
files and unverified data are not inferred to be successful outputs.

Docker observation is read-only, with running/stopped containers and row-click
details. Release hides dev-build containers unless its observation configuration
explicitly permits them. Host collectors supply machine/OS/kernel/driver metrics;
missing, warming, partial and stale values remain distinct. Observations never
control dispatch. Sim/Cyclo are external optional Job runtimes, not core services.

MCP Reference renders system.describe with six groups, action schemas, nested
contracts, examples and response/error notes. It does not execute copied examples.
Unknown saved views resolve to Dashboard.

## Validation

Run `bash deploy/test.sh --browser` from the Loom checkout. Temporary Ops fixtures
and real Firefox exercise API failures, permissions, queue controls, themes,
responsive layouts, Docker/MCP reference, sharing and previews. See
[TESTING.md](../deploy/TESTING.md) for limits. Historical evidence in `evidence/`
is retained for provenance, not a current installation command.

## Browser SSO

When LOOM_SSO_CONFIG is set, native browser sessions are required even in dev mode.
Each request validates current trusted Tailscale identity and native binding or
Sharing approval. Direct Ops MCP/API retains separate
authentication. See [Tailscale/SSO](../docs/TAILSCALE-SSO.md).

The authenticated profile sits below sidebar software information. Clicking it
opens Tailscale: Account for ordinary users, plus Member and Access Request for
administrators. No logout button is displayed. Only display identity crosses into
the workspace iframe; session/CSRF state remains in the parent.
