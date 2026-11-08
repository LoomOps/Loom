# Verification review across all feature blocks

This matrix separates code-level regression coverage, installation diagnostics and actual live behavior. All feature ownership/test paths remain authoritative in catalog.json. Passing a test count or a progress receipt is not a deployment acceptance result. Review this matrix whenever the listed boundary changes.

| Block | Automated evidence to keep | Installation / live boundary |
|---|---|---|
| identity | Native bindings, signed identity, expiry, VPN-account change, revocation and Admin matching | Doctor compares native SSO/Admin configuration, bypass flags and live Docker gateway/trusted-proxy agreement. Another-client profile/role verification remains live. |
| api | Six tool groups, HTTP/MCP schema parity including reconnect, independent result-action coverage, protocol errors and version metadata | Doctor queries actual API/tools; smoke submits through MCP. Real third-party clients remain separate. |
| queue | Order, hold/resume, dependent Jobs and preparation overlap | Smoke never reorders or cancels existing work. A busy queue leaves acceptance pending. |
| execution | Frozen profiles/output stores, container boundary, phase retries, return/cancellation | Doctor checks executor/image/GPU exposure; smoke runs prepare and run in containers. GPU computation is not inferred from nvidia-smi. |
| recovery | Leases, timeout, retry limits, lost agent and downstream blocking | Installer never fixes a failed smoke Job by rewriting receipts. Failed Job evidence is retained. |
| results | Required metadata, hashes, provenance, finalization retry, multi-store indexing and downstream input resolution | Shipped smoke includes metrics/provenance/relations and rechecks the registered artifact digest. External-client download is not claimed. |
| extensions | Immutable component/profile revisions, permission and source pins | Installer registers only its smoke component; research dependencies remain user-selected. Repeat smoke reuses its frozen profile/Job. |
| catalog | Shared queries, preserved history, preview/path boundaries | Doctor compares selected storage config and actual container mounts. All registered stores are RW. Profiles opt into named storage mounts; doctor checks real mount modes/service-user access. |
| observations | Docker and host collector handling, stale/unavailable states | Doctor checks collector timer plus API/container startup; actual sensor availability is hardware-dependent. |
| dashboard | Native entry, theme, responsive workspace and relay behavior | Doctor checks native entry and matching image; browser suites cover presentation. Other-client account switching must be exercised live. |
| onboarding | Guide/schema versions, bundle integrity, current references | Matching guides ship in source/offline kits; step documents and Gateway-first handoff must remain packaged. |
| network | Serve collision checks, tokenless user vs tagged-client boundaries, trusted peers, Sharing state/ETag/grant isolation | Doctor checks private/public routes, public rejection, TLS and policy guard. Real invitation acceptance requires a consenting participant. |
| portability | Preserved identities/history, fencing and import boundaries | Fresh installation does not clone another machine's DB, credentials or identity. Migration uses its separate contract. |
| installation | Default /loom/data, optional RW stores/mounts, persistent storage IDs, identity/storage config, stage failure, missing image recovery, uninstall plan/stop/revoke/purge/resume and packaging | Doctor uses actual state, never a completed-step flag. Full service contract is mandatory; publication resume preserves installed state. |
| testing | Schema/ownership/import/link checks, isolated SQLite/files and mocked external APIs | Test fixtures must not call host Docker/Tailscale or mutate live policy. Reboot/clean-host acceptance cannot be replaced with mocks. |

## Change review protocol

1. Run `python3 deploy/features.py check` and `python3 deploy/features.py impact <changed paths> --transitive`.
2. Inspect affected interfaces and downstream consumers, including installation and Loom Use; update only contracts actually changed.
3. Run the mapped regression suites. For entry/theme/relay changes include browser verification.
4. Record the tested revision and JSON/JUnit evidence. Keep unperformed live checks explicit.
5. During installation, reread INSTALL.md after every numbered step and compare doctor output with the step exit criteria. Do not turn this installation checklist into an extra Job execution gate.

The installation regression addressed here is the boolean tailscale plan that disabled SSO while verification still passed with an owner token. New plans must fail that path. Additional regressions cover a missing Gateway, mismatched Admin, auth bypass, wrong service image, missing storage, exposed private routes, and absent smoke result evidence, and a Docker-selected gateway differing from trusted proxies. Sharing regressions cover native Admins, approved recipients, existing tailnet members and unavailable membership lookup: none may cause a new invitation.


## Alpha.21 review

Uninstall cleanup is administrator CLI-only. Its tests cover API failure/retry,
confirmed remote deletion before credential purge, unexpected routes/group
extensions, uncertain invitations, wrong Compose identity, active Jobs and
unreturned executor tickets, including rechecking these conditions on resume.
Temporary SQLite/files are real; Docker/systemd/Tailscale are mocked. Uninstall is
not executed on the development host as a deployment acceptance step. This
workflow is included in source/installer snapshots; upgrades preserve installed
state and never invoke it.

## Managed access review

Connection selection covers missing client/login, current account reuse, explicit
switch/disconnection acknowledgment, wrong Tailnet/account, tagged identity and
device approval wait. Ownership tests cover installer/runtime deltas, preexisting
state, shared tags, changed references, ETag conflicts, uncertain write recovery
and reviewed orphan policy. These use isolated journals and mocked provider APIs;
no current user's login or live remote policy is changed by tests.

## Repository-wide review (alpha.28)

The review includes every tracked source, test, document, configuration, license
and generated artifact, beyond the catalog's executable-file ownership checks.
Evidence records each path, digest, feature/role and comparison with the previous
alpha snapshot. Generated HTML is checked against its source builder; image
imports and source/installer snapshots are reviewed separately from live mounts.

Corrections and regression boundaries:

- Identity: remove the local TTL issuance CLI/function; keep read-only compatibility
  for existing expiring tokens. New permanent credentials remain Admin-only.
- Onboarding: tokenless Tailscale refresh verifies the current principal and isolates
  scoped caches across accounts. Stdio and native header helpers reject linked or
  publicly readable credential files consistently.
- Installation/network: new Gateway defaults to 443, private Dashboard/MCP to 8443.
  Existing explicit 10000 configurations still pass dedicated compatibility tests.
- Portability: real SQLite export/restore strips resident and temporary credentials,
  including old archives. Single-store schema 1 must reject secondary payloads
  before fencing; this does not claim a new multi-store migration implementation.
- Dashboard/docs: release native identity is mandatory; the relay supports its
  explicit write allowlist. Development is optional/separate, with no public MCP.
- Packaging/testing: dependency documentation, changelog and removed-source
  ownership are synchronized. Root tests remain the authoritative regression suite.

The historical primary-storage alias and persistent database fields are retained
for compatibility, not silently renamed. Real invitation acceptance, clean-host
installation/reboot, multi-machine transfer and GPU numerical behavior still need
separate live acceptance; source-wide scans do not prove those outcomes.

Browser fallback audit: no password/token entry form is shipped. Native identity
or an explicit development instance is required; a missing identity displays
connection retry instead of reviving the obsolete token-only installation path.
The read-only browser acceptance harness follows the same native entry contract.
