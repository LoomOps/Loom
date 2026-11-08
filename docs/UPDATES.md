# Installation, overrides, updates and rollback

## Source and host state

The repository contains deployable source. Do not symlink `/loom` into it or include `/loom` in Docker images. Installation creates/copies initial configuration, Compose and host helpers, then bind-mounts databases, secrets, user components/profiles and external data.

| Scope | Owner | Update behavior |
|---|---|---|
| Images and `/loom/distribution/<version>` | Release | New versioned copy |
| `/loom/ops/config`, secrets, runtime | Installation instance | Preserve values; apply only explicit migrations |
| `/loom/ops/override.env`, compose.override.yaml | Machine operator | Applied last; never overwritten |
| `/loom/components`, `/loom/profiles` | Users/agents | Preserve revisions; new registrations are immediately available through API/MCP |
| Code under `/loom/shared/executor`, shared/services | Release | Replace code after idle verification; retain settings/keys |
| `/loom/loom_use/<version>` | Installed guide copies | Generate from the new image, preserve versions, switch `current` after health checks; restore the previous pointer on rollback |
| Data stores (default `/loom/data`, optional other host paths), `/dependencies` | Operator | Contents/locations preserved; Job storage grants remain explicit |

Compose order is base → executor/observations/data → dev → generated network/tailnet/admission → **user compose.override.yaml**. override.env also applies last. Overrides are operating settings, not credentials. Operators must review changed ports/mounts. Validate the final combination with `compose.sh config --quiet` when changing versions.

## New installation from a Git checkout

Pin the repository/tag, then run `sudo python3 deploy/source_install.py 0.1.0-alpha.N --destination /absolute/new-staging`. It builds images from a source snapshot and bundles matching installers/guides without Docker image tar export. Follow installer/INSTALL.md and its numbered steps: inspect/recommend → full identity/Gateway plan → host/images → apply → per-stage doctor → packaged smoke → full doctor → Gateway-first handoff. Boolean Tailscale/token-only installation is unsupported. Builds may download base images.

For offline delivery, `install-kit.py` includes matching image tar files. An external guide bundle must have the release's build_id. Both paths are for new installations and refuse to overwrite existing data directories.

## Updating an existing installation

1. Prepare new source/images with `release.py VERSION`; this does not affect running services.
2. Run `sudo python3 deploy/update.py plan --instance /loom/ops --project loom --release /loom/builds/alpha/VERSION`.
3. Run `apply` at the chosen update time. It refuses active Jobs, result processing, pending callbacks or unreturned executor tickets. Idle verification and dispatch blocking share one DB transaction.
4. Preserve DB, previous image selection and host helper copies under `/loom/updates/<id>`. Start services, verify API/web health, then resume dispatch.
5. Deploying alpha on another machine is a separate operation. Apply new network/Sharing configuration explicitly with `tailnet.py`/`access_setup.py`; image updates do not clone another instance's settings.

Failures retain a recovery record and maintenance state. Use `sudo python3 deploy/update.py rollback --record /loom/updates/<id>/record.json` to restore previous code, configuration and DB. **Automatic restoration refuses databases containing new post-update records**, preventing loss of new Jobs/artifacts. Review a data-preserving migration in that case. Updates are not automatically scheduled and do not force-stop active Jobs.

Do not use the release updater for development source mounts. Validate dev source changes, then pin a dev image snapshot. Temporary-filesystem/Docker mock tests are distinct from actual fresh-host, reboot and multi-client acceptance.

Host guides and the registered usage skill are consolidated under `loom_use`. Installation and updates do not create `/loom/agents` or machine entry skills. Machine access instructions and personal agent operations belong to external operators; Loom does not overwrite instructions in user homes.

## New release discovery

Ops checks the **published GitHub Releases**, not branch commits or unassociated tags.
The default repository is `LoomOps/loom`; checks run on startup when due and every six
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
  --repository LoomOps/loom --channel alpha --credential
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
