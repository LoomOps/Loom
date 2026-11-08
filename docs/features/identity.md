# identity — Network identity, local sessions and ownership

Authoritative implementations are `dashboard/sso.py`, `dashboard/server.py`, `ops/access.py` and `ops/ownership.py`. Follow [Tailscale authentication](../TAILSCALE-SSO.md) for installation and transition.

Local sessions require the current trusted ingress user and either a native binding or a ready Sharing record. Every request compares the cookie with the current network account and approval status. Admin uses a separate exact issuer/subject mapping. Owner denotes responsibility, not an ownership restriction on ordinary Job reads or controls.

Dashboard→Ops uses short-lived HMAC assertions. The issuer is a persistent identity namespace and does not contact an external login service. Preserve existing identity mappings during transitions. Sessions retain fixed expiry, opaque cookies and CSRF protection. Public Gateway receipts are not login identities.

When authentication contracts change, review API principals, ownership overlays, Sharing administration, Gateway probes, new installations and user guides together. catalog.json defines owned files, consumers and tests.

Tests use actual SQLite, sessions and HTTP while mocking only the trusted Serve boundary. They cover account switching, forged proxy headers, revocation, expiry, restarts, native network login and retired paths. Actual external VPN clients and TLS require separate validation.

## Resident agent tokens

Dashboard Admins issue and revoke individual agent tokens in Tailscale → Agent Tokens. Select an existing Loom user and Operator or Read only permissions. Tokens have no expiry and are shown once; only SHA-256 digests are stored. They retain the selected user's attribution and event inbox, and grant no Admin capability. Revocation takes effect on the next request, including existing MCP connections; already running Jobs continue.

Admin-only HTTP routes: `GET /api/admin/tokens` lists metadata and registered users; `POST /api/admin/tokens` accepts `{label, owner, scopes}`; `POST /api/admin/tokens/revoke` accepts `{id}`. Browser writes require the Admin session and CSRF/Origin checks. Ops accepts only the signed Dashboard Admin claim for these routes. Neither ordinary MCP callers nor the installation owner credential can issue tokens. Six operational MCP groups remain unchanged.

Token acquisition is an optional client step after server installation. The installer never auto-issues or renews tokens. Follow the independent Loom Use INSTALL.md from the actual client account; keep credentials outside Git. A tagged machine's network identity is not its individual Linux user's identity.

Local operator issuance of short-lived credentials has been removed. Existing
short-lived rows remain readable only until their original expiry/revocation;
updates never extend them or convert them to permanent tokens. Historical rows
are preserved, while new resident credentials use the Dashboard Admin contract.
