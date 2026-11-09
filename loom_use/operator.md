# Administrator installation and operations

The service account is non-login loom:loom without sudo or Docker group membership. The installer creates the account and /loom layout. Ops runs with the service UID; a restricted private host executor performs container execution. Never pass the Docker socket or platform credentials into user Jobs.

## Server layout

- **loom-ops**: MCP/API, Job queue and worker, state, metadata and result registration. One MCP endpoint exposes six tool groups.
- **loom-dashboard**: web UI and authenticated API relay, including Job controls and Admin token management. Ops enforces the operation contracts.
- **host executor**: creates Job containers from approved images and manages resource allocation, termination and return. Platform and Job containers have independent lifetimes.
- **Storage**: instance databases, settings and secrets under `/loom/ops`; registered source/profiles under `/loom/components` and `/loom/profiles`; artifacts on external storage attached through Compose.
- **Host helpers**: observations and configured execution-authority services under `/loom/shared`. Addresses, ports and external storage paths are installation-specific.

Server-specific SSH access, personal development paths, historical archive locations and entry skills belong in separate server documentation. The common Loom package does not install them or require them for entry.

Give users an authorized Tailscale URL and the server's onboarding guide. Servers configured for Tailscale user authentication do not require separate personal Loom tokens. Dashboard SSO provides browser entry. Unattended tagged clients or explicitly scope-limited services use separate service credentials. Downloading a guide does not grant access.

## Tailscale user authentication

Authentication uses Tailscale Serve's `Tailscale-User-Login` and `Tailscale-User-Name`. Explicitly enable the following Ops API configuration. Replace example addresses with the exact Serve proxy and fixed Dashboard addresses.

```json
{"tailscale_auth":{"enabled":true,"trusted_proxies":["127.0.0.1","192.0.2.10"],"users":{"operator@example.com":"owner"},"allow_new_users":false}}
```

- `users` maps Tailscale logins to existing Loom actor IDs, retaining their scopes and event recipient IDs. Ownership does not restrict data access. Do not infer or automatically translate Tailscale organization Owner/Member roles into Loom permissions. Organization role lookup requires separate Users API management credentials and is not part of this connection.
- `allow_new_users:true` assigns each unmapped login a stable `ts-<hash>` ID and read/submit/cancel/events scopes, allowing shared access to all Jobs, results, profiles and history. Event delivery remains recipient-specific. This includes external shared users allowed through Serve; restrict network access through tailnet policy.
- Tagged devices lack user headers and require service tokens. An explicit Bearer token is validated first; invalid tokens cannot fall back to Tailscale authentication.
- Set Dashboard `LOOM_TAILSCALE_TRUSTED_PROXIES` to exact, space-separated Serve peer addresses. Dashboard forwards identity headers only from these peers and skips token entry after `/api/principal` authentication succeeds.
- **Trust boundary:** Serve removes incoming identity headers and replaces them with authenticated values. Do not expose Ops/Dashboard backends directly. Local host processes and services sharing the trusted Docker network are inside this boundary and can spoof headers; untrusted Job containers must not join that network or host networking. `trusted_proxies` checks exact connection peers, not broad CIDRs or forwarded X-Forwarded-For values.
- Disabled by default. `LOOM_DEV_NO_AUTH=1` is for isolated development only and must not become a deployment default. Policy changes take effect on API restart.

## Shared operations and ownership

Authenticated ordinary users can inspect all Jobs, logs, data, models, results and preserved history, and reorder, hold, resume, cancel, recover or correct results. Components, Tools and Profiles are shared, and users may register revisions. Users coordinate scheduling themselves. Explicitly read-only service tokens remain read-only.

Owner identifies the current responsible user. It is stored in user_profiles and item_owners without rewriting original specs, submitters, execution snapshots or history bodies. Obtain profile IDs from `system.users`, then use `jobs.assign_owner` or `results.assign_owner`. Changes are audited. Login and profile names are visible to collaborating users.

Running Jobs remain non-preemptive. Dependencies, recovery leases, retry limits, allowed storage and runtime restrictions still apply. Event retrieval/acknowledgment remains recipient-specific so one agent cannot consume another agent's completion message. sudo, Docker sockets, server deployment and token issuance are not ordinary MCP operations.

## Installation identity

The installing Linux user, service account loom:loom and Loom ownership profiles are independent. A fresh installation generates a random installation_id and a machine_id incorporating it, avoiding collisions even with identical Compose project names. Restarts and updates preserve saved IDs. Existing installations gain installation_id without changing machine_id referenced by historical locators. Distinguish fresh independent installations from restores; never run identical cloned DB/config instances concurrently as separate services.

## MCP transport

One `/mcp` endpoint provides stateless Streamable HTTP. Supported versions are `2025-03-26`, `2025-06-18` and `2025-11-25`, negotiated during initialize. Send `MCP-Protocol-Version` on subsequent requests. Normal responses are JSON; requests selecting SSE receive only their own response before the stream closes. Notifications return 202 with an empty body; GET/DELETE return 405. No sessions, separate `/sse` or `/messages` endpoints, or resident subscriptions are created. Callers remain responsible for Job event consumption. The server does not advertise the 2026-07-28 discovery contract.

Source installation starts at deploy/install.py, host execution attachment at deploy/executor-attach.py, and Compose at deploy/compose.sh. Prepare Docker/Compose and Python 3; GPU execution also requires NVIDIA drivers and Container Toolkit. Read deploy/PLATFORM.md and deploy/README.md from the selected deployment source.

/loom/ops stores instance configuration, DB and secrets. /loom/shared stores executor, authority and observation services. /loom/components and /loom/profiles store registered revisions. /loom/loom_use stores versioned user guides. External data storage is declared in Compose; /dependencies holds external source and installation adjustments.

Updates deploy immutable images with matching guides. Block execution admission and confirm actual worker/lease return before backing up the DB, configuration and image identity and switching versions. Use isolated dev-build for development; never copy its authentication bypass into deployment defaults.

Entry guides should link to system.guide/describe/profiles rather than embed volatile API fields or profile lists. Deployment does not alter existing Job pins or historical results. After deployment, verify MCP initialize/status/guide, basic read/action calls and, where needed, an isolated small Job.

## Dashboard SSO

The Sharing gateway accepts an email address and sends a single-use machine-sharing invitation. After the user accepts in their own tailnet, an administrator verifies the actual recipient and approves access in Dashboard `Tailscale` → Access Request. Approval opens only this machine's Dashboard/MCP ports.

The lower-left profile opens the same tab. Ordinary users see Account. Administrators also see Member and Access Request; Member shows approved sharing recipients and the current administrator. Revoke removes only this machine's share and Loom-managed grants. There is no tailnet joining or device-name input; `tag:loom` is for Hub discovery.

Shared-user Dashboard login uses trusted Serve identity and approval records; only this machine is shared. See `docs/ACCESS-ENROLLMENT.md` for installation, permissions and public boundaries.

Configure session lifetime through `session_seconds` (default and maximum: seven days). Current trusted Tailscale identity plus explicit native binding or Sharing approval creates a local session. Every request checks the current VPN account and approval status. Activity and restarts do not extend expiry. Lifetime changes apply to the next login. Cookies are HttpOnly/Secure; state changes check Origin/CSRF. Existing issuer/subject references remain for ownership continuity.

Ops status authentication.tailscale/dashboard_sso reports actual configuration activation. Distinguish platform implementation from incomplete network enrollment, pending device approval or failed login. Installers use docs/TAILSCALE-SSO.md and deploy/installer/tailnet-plan.example.json.

The public Funnel gateway verifies private Dashboard connectivity and identity through the browser. Successful verification redirects to the private Dashboard. With VPN disconnected, reconnect Tailscale and retry. An approved request alone does not create a browser login. Public intake and administration are separate: `loom-gateway` and the private Dashboard. The public service exposes no operational API.

If a native Admin or approved member reaches access intake, do not submit another
share invitation. `tailscale_identity_required` means trusted network identity
was not received; `machine_access_required` means the verified identity has no
binding/grant. Ask the operator to compare the actual Docker bridge gateway with
Dashboard/Ops trusted proxies and run installation doctor. Existing-account
requests report `access_check_required` and must not be treated as authentication.

## Uninstallation

Machine administrators use `deploy/uninstall.py plan` and then `apply` from the matching source/installation kit; see `docs/UNINSTALL.md`. This is not a public MCP or Dashboard action. The workflow cleans up managed Sharing grants/invitations and local access state, preserves research data and the host Tailscale identity, and retains cleanup evidence on remote failure.

## Resident agent tokens

Dashboard Admins issue and revoke individual agent tokens in Tailscale → Agent Tokens. Select an existing Loom user and Operator or Read only permissions. Tokens have no expiry and are shown once; only SHA-256 digests are stored. They retain the selected user's attribution and event inbox, and grant no Admin capability. Revocation takes effect on the next request, including existing MCP connections; already running Jobs continue.

Admin-only HTTP routes: `GET /api/admin/tokens` lists metadata and registered users; `POST /api/admin/tokens` accepts `{label, owner, scopes}`; `POST /api/admin/tokens/revoke` accepts `{id}`. Browser writes require the Admin session and CSRF/Origin checks. Ops accepts only the signed Dashboard Admin claim for these routes. Neither ordinary MCP callers nor the installation owner credential can issue tokens. Six operational MCP groups remain unchanged.

Token acquisition is an optional client step after server installation. The installer never auto-issues or renews tokens. Follow the independent LoomOps/Loom-Use INSTALL.md from the actual client account; keep credentials outside Git. A tagged machine's network identity is not its individual Linux user's identity.

## Local files and optional SSH

MCP `system.ssh` returns the caller's optional SSH descriptor and host addresses.
On the same machine, use the current OS account's local filesystem permissions;
no SSH registration is needed. For remote file analysis, install the optional
Loom SSH Skill from the pinned Loom Use release, reuse existing authorized SSH,
or ask the Dashboard Admin to enable a personal account. MCP tokens are not SSH
passwords. Component/Tool/Profile drafts are registered via MCP; Jobs stay in the queue.
