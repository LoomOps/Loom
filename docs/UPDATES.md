# Installation, overrides, updates and rollback

## Native authority migration

The Rust release applies the additive schema 5 -> 6 -> 7 chain and delivers `loom-ops`, `loom-host`, `loom-sdk`
and `loom-migrate` from the same immutable Ops image. The cumulative migration
requires old writers, executions, callbacks and Runtime validation owners to drain.
Upgrades verify schema 7 and schema 8 directly. Schema 8 additionally requires the
Job and Unit tables and the native writer fence.
It preserves old rows/configuration/storage identities and records a private
backup/intent/receipt. Resume verifies unchanged before/after hashes; rollback
refuses new post-cutover records. Never start a legacy writer on a migrated native database. See ITEM-INDEX.md
for exact alias/file preservation, explicit reconciliation and online index maintenance.
See [native migration verification](features/VERIFICATION.md) and the [original implementation ledger](rust-port/IMPLEMENTATION.md).

## Source and host state

The repository contains deployable source. Do not symlink `/loom` into it or include `/loom` in Docker images. Installation creates/copies initial configuration, Compose and host helpers, then bind-mounts databases, secrets, user Component/Tool/Profile revisions and external data.

| Scope | Owner | Update behavior |
|---|---|---|
| Images and `/loom/distribution/<version>` | Release | New versioned copy |
| `/loom/ops/config`, secrets, runtime | Installation instance | Preserve values; apply only explicit migrations |
| `/loom/ops/override.env`, compose.override.yaml | Machine operator | Applied last; never overwritten |
| `/loom/components`, `/loom/profiles` | Users/agents | Preserve Component/Tool/Profile revisions (Tools are under components/.tools); new registrations are immediately available through API/MCP |
| Code under `/loom/shared/executor`, shared/services | Release | Replace code after idle verification; retain settings/keys |
| `/loom/loom_use/<version>` | Installed guide copies | Generate from the new image, preserve versions, switch `current` after health checks; restore the previous pointer on rollback |
| Data stores (default `/loom/data`, optional other host paths), configured dependency directory | Operator | Contents/locations preserved; Job storage grants remain explicit; use the saved installation choice rather than a fixed dependency path |

Compose order is base → executor/observations/data → dev → generated network/tailnet/admission → **user compose.override.yaml**. override.env also applies last. Overrides are operating settings, not credentials. Operators must review changed ports/mounts. Validate the final combination with `compose.sh config --quiet` when changing versions.

## New installation from a Git checkout

Pin the repository/tag, then run `sudo python3 deploy/source_install.py <published-version> --destination /absolute/new-staging`. It builds images from a source snapshot and bundles matching installers/guides without Docker image tar export. Follow installer/INSTALL.md and its numbered steps: inspect/recommend → full identity/Gateway plan → host/images → apply → per-stage doctor → packaged smoke → full doctor → Gateway-first handoff. Boolean Tailscale/token-only installation is unsupported. Builds may download base images.

For offline delivery, `install-kit.py` includes matching image tar files. An external guide bundle must have the release's build_id. Both paths are for new installations and refuse to overwrite existing data directories.

## Updating an existing installation

For cumulative migration plans, resumable scripts and state-preserving container
reinstallation, read [Administrator upgrade handoff](UPGRADE.md). `plan` includes
the required migration chain; `--mode rebuild` is explicit and preserves identity.


1. Prepare new source/images with `release.py VERSION`; this does not affect running services.
2. Run `sudo python3 deploy/update.py plan --instance /loom/ops --project loom --release /loom/builds/alpha/VERSION`.
3. Run `apply` at the chosen update time. It refuses active Jobs, result processing, pending callbacks or unreturned executor tickets. Idle verification and dispatch blocking share one DB transaction.
4. Preserve DB, previous images and host helper copies under `/loom/updates/<id>`. Start services and verify API/web health, native identity and Gateway entry. Readiness checks compare the session endpoint, Gateway installation, private browser relay routes and Funnel isolation. On success the updater restores admission for instances that entered active. A pre-existing maintenance fence and dispatch pause are retained for the operator.
5. Deploying alpha on another machine is a separate operation. Apply new network/Sharing configuration explicitly with `tailnet.py`/`access_setup.py`; image updates do not clone another instance's settings.

Failures retain a recovery record and maintenance state. Use `sudo python3 deploy/update.py rollback --record /loom/updates/<id>/record.json` to restore previous code, configuration and DB. **Automatic restoration refuses databases containing new post-update records**, preventing loss of new Jobs/artifacts. Review a data-preserving migration in that case. Updates are not automatically scheduled and do not force-stop active Jobs.

If preflight fails before backup, admission returns to its previous mode for each
instance already checked. A completed rollback also restores the previous
admission mode and preserves the saved dispatch pause.

Do not use the release updater for development source mounts. Validate dev source changes, then pin a dev image snapshot. Temporary-filesystem/Docker mock tests are distinct from actual fresh-host, reboot and multi-client acceptance.

Host guides remain under `loom_use`; client Skills are distributed by the independently pinned Loom Use release. Installation and updates do not create `/loom/agents` or machine entry skills. Machine access instructions and personal agent operations belong to external operators; Loom does not overwrite instructions in user homes.

## New release discovery

Ops checks the **published GitHub Releases**, not branch commits or unassociated tags.
The default repository is `LoomOps/Loom`; checks run on startup when due and every six
hours in a separate API-process thread. There is no Job, DB lock, GPU allocation,
download, pause or installation involved. Worker restarts do not create another checker.

- `GET /api/updates` / MCP `system.updates`: cached status, installed image version,
  latest eligible release/link, check timestamps, error and stale flag.
- `POST /api/updates/check` with `{}` / MCP `system.check_updates`: queues a refresh,
  returning `accepted`, `reason`, `updates` immediately. The same `read` scope applies;
  requests coalesce with a 60-second cooldown and GitHub rate-limit backoff.
- `system.status.updates` embeds the same snapshot. Dashboard exposes status beside
  its build label, with a release link and Check for updates action.

Stable installations consider stable releases only. Alpha accepts alpha, beta, rc
and stable; beta/rc exclude less mature stages. Numeric SemVer comparison puts
`alpha.10` after `alpha.9`. Drafts and unpublished releases are ignored. Development
builds expose the latest release but report `unversioned`, not a false update verdict.
The default channel follows the installed image version; it can be pinned explicitly.

ETags and successful pages persist in the instance runtime `release-check.json`.
All pages are checked (bounded to 1,000 releases); reaching the bound reports
`release_list_incomplete`, never a false up-to-date result. Offline, permission and
rate-limit failures preserve the last success and mark it stale. A missing/private
repository is `repository_unavailable`; a private repo requires GitHub Contents:read.
Tokens and upstream error bodies are never returned to browsers, MCP, or the cache.

### Configure an installed instance

```bash
sudo python3 deploy/release_check_setup.py --root /loom/ops \
  --repository LoomOps/Loom --channel alpha --credential
```

The token prompt is hidden. Use a repository-scoped read-only token. `--token-stdin`
accepts a secret pipe for unattended provisioning; never put the value in command
arguments. Omit credential options for public repositories or to preserve an existing
token. `--disable` supports offline installations; `--interval` accepts 300–604800 seconds.

The helper writes `release-check.env` and `secrets/releases/github-token` (0600,
owned by the installed runtime identity). Compose mounts only this credential directory
read-only at `/etc/loom/releases`; it does not copy credentials into an image. Final
`override.env` still wins. The installer creates the empty credential directory;
upgrades preserve it and `release-check.env`. Token rotation is read at the next check;
configuration changes need the normal idle Ops restart/recreate. The helper never
restarts a service. On a private installation, configure the token before its first
start, or at the next safe restart. No administrator Tailscale credential is reused.

The checker **does not install** releases. Select an immutable release, review its
notes and follow the existing idle `deploy/update.py` procedure above. New-source
workspaces do not become releases until explicitly tagged and published.

GitHub contract: [List releases](https://docs.github.com/en/rest/releases/releases#list-releases),
[conditional requests and rate limits](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api).

New installations register all selected stores RW. Existing generated storage overlays and executor grants are preserved by updates; an old read-only overlay must be reconciled explicitly before using `runner.storage`. The base Compose data mounts are RW, while final user overrides remain authoritative.

### Verify authentication on the selected instance

An image update preserves each instance's authentication settings. A working
Tailscale-authenticated development endpoint does not prove that the alpha endpoint
accepts the same identity. For tokenless user access, enable `tailscale_auth` on
that instance, trust only its actual Serve ingress peer, preserve explicit actor
bindings, and verify the deployed HTTPS `/mcp` using an authorized user device.
Sharing approval must grant that exact HTTPS port; keep the public Funnel
request-only. Apply the same private-port list to existing approval grants and
the admission worker so future approvals/revocations remain consistent. Do not
grant all shared users access before approval or turn on development auth bypass.
Tagged or unidentified clients still require their own service credentials.

When moving a user-facing entry from development to alpha, migrate its native
identity namespace, Admin bindings, sessions, approval database and managed-resource
journal explicitly. Run one approval writer. Keep development on a separate private
port, and update Gateway destinations, Serve routes and exact policy ports together.
A public Gateway on 443 and private Dashboard/MCP on 8443 is supported; port numbers
are installation settings, not a release-wide constant. Never expose operational
API routes on the public Gateway port. Preserve unrelated Tailnet/SSH policies.

## Independent Loom Use

Loom pins the independently published client in loom_use/client-release.json.
Operational guides remain server-owned; the client installer and Skill are in
https://github.com/LoomOps/Loom-Use. Server installation offers a handoff to that
release's INSTALL.md for the current agent/user and already-known private endpoint.
The server does not write personal client settings during a service update. Existing
server-installed Skill links keep a compatibility entry; the client adopts verified
exact-endpoint credentials when the user proceeds with the independent setup.

Publish the client artifact first, update the server pin, run feature checks and
the real HTTP contract tests against that artifact, then publish the server. See
docs/features/onboarding.md. Never substitute a sibling working tree in release CI.

Managed SSH is optional on existing installations. `ssh_setup.py install` installs
its host service and Compose overlay at an idle maintenance point; ordinary image
updates do not issue accounts. Later updates preserve accounts/homes and refresh
the protected helper with rollback backups. Apply data-group ACLs only to the
explicitly registered data stores. See [SSH](SSH.md).

## Boot address readiness

The boot-order correction is applied during an explicit idle update, not by
release discovery. The updater refreshes the installed stack and optional authority
unit policy and retains both original units for rollback. The new Compose wrapper
waits for exact published host IPs before up/start/restart; it does not block stop
or configuration/log queries. Each address probe is bounded to 120 seconds and
managed units retry failures after 15 seconds. Existing longer operator timeouts
and machine drop-ins remain authoritative. Neither update nor this probe logs in
to Tailscale, changes listener exposure, initializes authority state or resumes a
research Job. After deployment, inspect effective systemd policy and perform a
separately authorized idle reboot acceptance; a current health response does not
prove boot ordering.

The updater stops the matching managed stack unit before running Compose, cancelling
pending systemd retries that could race container replacement. It checks the unit's
effective instance and project environment first. A previously active/retrying unit
is started again only after readiness and preservation checks; failed updates leave
it stopped for explicit recovery. Resume retains the original activation decision.

Managed Runtime revisions are persistent host state in a separate executor
runtimes.sqlite3. Updates ship the catalog helper, preserve existing Runtime/Job
pins and take a private catalog backup alongside the instance snapshot. Runtime
installation and platform update share an exclusive executor lock; unconfirmed
validation return blocks update. Rollback does not replace the Runtime catalog
with older metadata or erase later environment registrations. Dependency trees
and Docker images remain in place and require their own host backup policy.

On a partial startup failure, the updater stops instance writers before recording
failure DB fingerprints, so additive startup schema cannot race the resume guard.
Preservation still compares every identity, grant, state and token column. Only
`updated`/`next_attempt` scheduler clocks in admission share_requests may progress;
the raw backup retains their original values. No authorizations or Job data are
relaxed by this narrow operational-clock comparison.

Preservation readers use a single SQLite read transaction and close connections
before WAL cleanup. A database fingerprint error retains the primary failure and
names the failed evidence in the update record. Incomplete failure evidence never
permits resume or successful admission reopening; an operator must inspect and
capture verified recovery evidence first. This does not automatically restore or
repair application data.


## Native common Item schema

The native Item index adds a separate schema 5 -> 6 migration after the unchanged
native cutover step. Existing schema 4/5 writers require the explicit drained
upgrade and a private verified backup; starting a newer binary does not implicitly
assign replacement IDs. Missing core Item identity metadata blocks startup rather
than reconstructing new IDs from a surviving search cache. See
[Item index operations](ITEM-INDEX.md) and the
[implementation boundary](item-index/IMPLEMENTATION.md) before preparing a release.

## Reporting contract cutover

Alpha.32 adds report contracts without rewriting installed Profiles, accepted Jobs
or original outputs. Inventory `components.profiles` before enabling new research
submissions: `reporting_policy: loom-reporting/v1` and exact `reporting_pins`
identify ready revisions. Legacy scientific Profiles require an explicitly authored
new revision for new Jobs; their already accepted execution remains compatible.
Read `system.guide` with `id: reporting`, select coverage, declare reporter roles
and expected schedules, then register the new Profile. Do not edit user Profile
files silently or rerun old computation to manufacture missing reports.

New Jobs also freeze reporting policy when output kinds were formerly dynamic;
declaring a scientific output only at publication cannot bypass the new boundary.
Read-only historical results may retain Missing. Contract registration and Metadata
adoption are separate from scientific verification and do not fabricate measurements.

Registry lifecycle adds a `registry_lifecycle_events` audit table on native schema
open. Existing `runtime_curation` Profile/Runtime states remain authoritative and
are not rewritten. New Component/Tool/Template/Contract/Definition lifecycle rows
use the same overlay table. Upgrade and restore preserve both tables, registry
source bodies, accepted Job pins and files. No host resource is archived by upgrade.

Native update preservation permits Runtime/Dependency observation retirement and
monotonic catalog access-epoch advancement. Their original IDs, native keys,
creation identity, immutable revisions and aliases remain preserved. Scientific
Item retirement, origin/cursor-key changes and access-epoch regression are rejected.


### Job and Unit compatibility boundary

The first accepted typed Job or Unit raises the instance's SQLite authority to schema 8 in the acceptance transaction. A release that only supports schema 7 will reject that database. Keep the drained checkpoint for recovery. After accepting typed work, prefer forward repair or a verified data-preserving conversion before changing writer versions. An older checkpoint omits submissions and publications made after its capture.
