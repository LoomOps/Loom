# identity — Network identity, local sessions and ownership

Authoritative implementations are `dashboard/sso.py`, `dashboard/server.py`, `ops/access.py` and `ops/ownership.py`. Follow [Tailscale authentication](../TAILSCALE-SSO.md) for installation and transition.

Local sessions require the current trusted ingress user and either a native binding or a ready Sharing record. Every request compares the cookie with the current network account and approval status. Admin uses a separate exact issuer/subject mapping. Owner denotes responsibility, not an ownership restriction on ordinary Job reads or controls.

Dashboard→Ops uses short-lived HMAC assertions. The issuer is a persistent identity namespace and does not contact an external login service. Preserve existing identity mappings during transitions. Sessions retain fixed expiry, opaque cookies and CSRF protection. Public Gateway receipts are not login identities.

When authentication contracts change, review API principals, ownership overlays, Sharing administration, Gateway probes, new installations and user guides together. catalog.json defines owned files, consumers and tests.

Tests use actual SQLite, sessions and HTTP while mocking only the trusted Serve boundary. They cover account switching, forged proxy headers, revocation, expiry, restarts, native network login and retired paths. Actual external VPN clients and TLS require separate validation.
