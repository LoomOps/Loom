# Verification review across all feature blocks

This matrix separates code-level regression coverage, installation diagnostics and actual live behavior. All feature ownership/test paths remain authoritative in catalog.json. Passing a test count or a progress receipt is not a deployment acceptance result. Review this matrix whenever the listed boundary changes.

## Unreleased Profile combinations and software attribution

Profile inventory regressions cover component/standalone/configured origins,
multiple declared purposes, observed usage kept separate, exact old Tool pins,
mixed registered/legacy stage environments, non-mutating discovery and input
manifest immutability. Read-only clients can discover combinations without gaining
registration rights. Attribution covers Tool/Component/Profile targets, normal
user versus Built-in, idempotence, maintenance fences and unchanged frozen Jobs.
Browser cases exercise Profiles filters, Runtime backlinks, combination links,
responsible-person labels, escaping, narrow layouts and unrecorded metadata.
Ownership cleanup on an existing installation requires an explicit reviewed plan;
source tests do not mutate production rows or classify research code by name.

## Job business-logic audit (unreleased)

Review the state machine, adversarial API/results boundaries and consumer
projections in separate passes. Recheck findings against the block invariants;
existing PASS records alone do not establish those invariants. The dedicated
`tests/ops/test_job_lifecycle.py` covers cancellation/readiness races, stale Attempt
updates, original-key replay after repair/default changes, immutable reindex,
same-Attempt finalization recovery, per-Attempt failure evidence, input graph
repair, unconfirmed container/hook return, registration timestamps, stage timing, finalizer receipts and online-transfer
grant preservation. Extension tests reject Component/independent Profile identity
collisions in both registration orders.
Browser coverage exercises running recovery, imported paused work, native Activity
and canonical-source deduplication. Both normal and rejected reindex paths remain
covered. No fixture run implies live GPU acceptance or production rollout.

| Block | Automated evidence to keep | Installation / live boundary |
|---|---|---|
| identity | Native bindings, signed identity, expiry, VPN-account change, revocation and Admin matching | Doctor compares native SSO/Admin configuration, bypass flags and live Docker gateway/trusted-proxy agreement. Another-client profile/role verification remains live. |
| api | Six tool groups, HTTP/MCP schema parity including reconnect, independent result-action coverage, protocol errors and version metadata | Doctor queries actual API/tools; smoke submits through MCP. Real third-party clients remain separate. |
| queue | Order, hold/resume, dependent Jobs and preparation overlap | Smoke never reorders or cancels existing work. A busy queue leaves acceptance pending. |
| execution | Frozen profiles/output stores, container boundary, phase retries, return/cancellation | Doctor checks executor/image/GPU exposure; smoke runs prepare and run in containers. GPU computation is not inferred from nvidia-smi. |
| recovery | Leases, timeout, retry limits, lost agent and downstream blocking | Installer never fixes a failed smoke Job by rewriting receipts. Failed Job evidence is retained. |
| results | Required metadata, hashes, provenance, finalization retry, multi-store indexing and downstream input resolution | Shipped smoke includes metrics/provenance/relations and rechecks the registered artifact digest. External-client download is not claimed. |
| extensions | Immutable Component/Tool/Profile/Runtime revisions, permission, source and environment pins | Registered Runtime validation proves its measured scope; research compatibility remains user-selected. Repeat smoke reuses its frozen profile/Job. |
| catalog | Shared queries, preserved history, preview/path boundaries | Doctor compares selected storage config and actual container mounts. All registered stores are RW. Profiles opt into named storage mounts; doctor checks real mount modes/service-user access. |
| observations | Docker and host collector handling, stale/unavailable states | Doctor checks collector timer plus API/container startup; actual sensor availability is hardware-dependent. |
| dashboard | Native entry, theme, responsive workspace and relay behavior | Doctor checks native entry and matching image; browser suites cover presentation. Other-client account switching must be exercised live. |
| onboarding | Guide/schema versions, bundle integrity, current references | Matching guides ship in source/offline kits; step documents and Gateway-first handoff must remain packaged. |
| network | Serve collision checks, tokenless user vs tagged-client boundaries, trusted peers, Sharing state/ETag/grant isolation | Doctor checks private/public routes, public rejection, TLS and policy guard. Real invitation acceptance requires a consenting participant. |
| portability | Preserved identities/history, fencing and import boundaries | Fresh installation does not clone another machine's DB, credentials or identity. Migration uses its separate contract. |
| installation | Default /loom/data, optional RW stores/mounts, persistent storage IDs, identity/storage config, stage failure, missing image recovery, uninstall plan/stop/revoke/purge/resume and packaging | Doctor uses actual state, never a completed-step flag. Full service contract is mandatory; publication resume preserves installed state. |
| testing | Schema/ownership/import/link checks, isolated SQLite/files and mocked external APIs | Test fixtures must not call host Docker/Tailscale or mutate live policy. Reboot/clean-host acceptance cannot be replaced with mocks. |
| ssh | Personal UID bindings, Admin/CSRF boundaries, policy journal ownership, retry/revoke and local-client detection | External personal-client login and existing-session revocation require separate live acceptance. |
| diagnostics | Privacy, bounded retention, concurrent writers, symlink rejection and fail-open collection | Fixture failures do not establish actual disk capacity, delivery or external service availability; diagnostics never upload themselves. |

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

## Independent client boundary

Onboarding owns the exact Loom Use release/checksum and installer handoff.
The pinned client is fetched by deploy/client_release.py before CI. Server tests
exercise it over real fixture HTTP for discovery, authentication, revocation,
status/guide and profile registration. Client unit tests cover package integrity,
connection switching, old credential adoption and existing-app refresh failures.
No server transport test implies active desktop-thread readiness.

Large Runtime inventory regression uses 60 Profiles and 996 package records,
checks the serialized MCP Profile discovery response stays below the installed
client's 2 MiB bound, verifies complete Runtime detail remains available, and
compares stored rows before/after discovery. Dependency previews explicitly report
their full count and completeness; omitted entries do not imply absence.

## Personal SSH review

SSH adds a host privilege boundary, not a Job execution stage. Review Admin session
and CSRF, approved membership, policy ETags/journal ownership, account registry UID
binding, rollback/retry, Sharing revoke, and installation/update/uninstall consumers.
Real fixture HTTP and Unix socket tests supplement pure function tests. A healthy
service or metadata file does not prove SSH login. Keep external personal-client
login and existing-session revocation explicitly separate from mock evidence.

Local diagnostics: verify retention/privacy/fail-open tests, image inclusion and
shared per-instance mount ownership. Sharing regression must distinguish local
PermissionError from a provider outage, and root installer rewrites must retain
service ownership. Historical metadata is not a live file availability test.

## Unreleased source audit

The file inventory extends beyond feature ownership to documentation, build inputs,
licenses, binary provenance and generated assets. Legacy Component-to-Tool copying
checks matching sealed Profile pins and rejects conflicts. Update rollback checks
and restores database file permissions and symlink ownership; resumed updates stop
shared helpers before code replacement. Package/guide handling rejects canonical
path aliases, injected sources and symlink directories, while allowing generated
Python bytecode in the client contract cache. Tests use disposable files/SQLite,
mock Docker/systemd and isolated HTTP/Firefox; no deployment acceptance is implied.
Alpha.45 pins the published alpha.7 package after its exact artifact checksum
is verified. Source-to-source contract validation remains separate.

Fenced mutation regression covers Component source writes/registration, Tool and
standalone Profile registration, and ownership reassignment. Maintenance/readonly
retains database rows and source bytes while discovery/get remains readable.

## GPU runtime and administrator policy correction

Executor tests cover default compute/utility, explicit graphics, invalid/duplicate
capabilities, rejected Job policy overrides, protected environment precedence,
bridge egress and preserved container isolation. Installer tests cover validated
runtime options and matching actual grants. Updater tests preserve custom runtime
configuration and reject unexpected drift. Runtime-doctor tests cover inert
inspection, private receipts, busy GPU, lost start, bounded timeout, owned-ticket
cleanup and exit-zero/missing-evidence failure. Real graphics/scientific acceptance
must be recorded separately on the target runtime after deployment.

## Unreleased boot ordering regression

Boot tests reproduce address assignment later than the original five-second
authority restart window. They cover finite wait deadlines, probe failure, IPv6
readiness, loopback/wildcard fast paths, private-output suppression, final Compose
overrides, shutdown without the readiness helper, valid systemd policy and
identity/state-preserving authority generation. Upgrade/resume/rollback tests
verify both unit adoption and exact original backup restoration. No production
service was restarted, no release was deployed and no real reboot is implied by
these fixture results.

Stack ownership regression tests reproduce an auto-restart while updating,
verify stop-before-Compose/start-after-health ordering, preserve original activation
across failed-update resume, keep inactive units inactive and reject another
instance/project's unit. These are mocked systemd checks; target deployment
records separately establish actual unit and container health.

## Runtime catalog and configured dependency installation

Runtime metadata is owned by a separate host SQLite catalog. Tests exercise exact
revision/hash grants, immutable Profile/Job pins, legacy/unavailable discovery,
dependency-requirement mismatches, and installation-state revision invalidation
without synthetic Jobs. Installer tests use a nondefault directory containing
spaces, fail on missing/mismatching/symlinked configuration, check download hashes
and archive traversal/link rejection, and reject false measured versions.
Validation timeout cleanup, unconfirmed GPU return fences, shared/exclusive locks
and separate-catalog preservation are covered with real SQLite and mocked Docker.
Firefox exercises Runtime dependencies, Profile consumers, failed installation
visibility, escaping and light/dark 1440/500px detail views.

An additional disposable CPU acceptance exercised real Docker build, measured
interpreter validation, catalog registration, private executor resolution, non-root
execution of the exact image ID, exit 0 and container cleanup. It used temporary
configuration/data and the configured custom dependency directory. This is not
Isaac Sim/Lab, GPU, renderer, driver, host reboot or production deployment evidence.

## Independent final review and verifier acceptance

Review the complete source/document/package inventory independently before using
these blocks as evidence. The final source pass follows block execution; findings
require corrected behavior tests, updated block scope and fresh source-bound reports.
The catalog itself is a hashed test input. A changed mapping invalidates prior
evidence. Missing execution, skips, expected failures, unexpected successes, failed
suite summaries and stale source hashes never count as a block PASS. Regressions
exercise those rejection paths and reject a malformed catalog without crashing.

Runtime installer regressions also cover dotted dependency download names,
instance-scoped reconciliation, preserved ready revisions, recovery of committed
registration with a lost ready receipt, and repeated cleanup without new Docker
effects. Catalog, response and installation state must agree without a rebuild.
Credential-bearing source URLs and query/fragment credentials are rejected before
staging because dependency metadata is exposed through authenticated discovery.
These boundaries are installation diagnostics, not new
Job gates or scientific acceptance.

Software classification tests preserve sealed manifests/frozen Jobs, deny reader
writes and unknown revisions/tags, and respect maintenance. Runtime inventory tests
assert CPU-only bounded non-root probes, no research mounts, idempotency, exact
policy/image/metadata bindings and drift rejection. Browser tests cover separate
software lists, role tags in Profile combinations, absence of boilerplate, narrow
landscape navigation and collapsed technical evidence. Live metadata inventory
and agent call-history review remain separate from fixture outcomes.

## Runtimes workspace acceptance

Runtimes hub tests exercise real SQLite curation, family preference conflicts,
archiving, source/Job immutability, scope/maintenance rejection, scoped stage
requirements, dependency representatives and exact-started-Job evidence.
Browser checks cover the single sidebar and four inner views, retained Profile
context, grouped revisions, representative-only display, provenance filters,
read-only preparation/comparison and authenticated editing. HTTP boundary tests
reject arbitrary action forwarding, cross-origin requests and absent credentials.
Installed metadata never establishes runtime invocation or GPU/scientific validity.


Research presentation and logical execution acceptance is covered by
`tests/ops/test_research_workflows.py`: atomic graph rejection, conflicting-key
replay, real dependency readiness, node controls, root event deduplication,
operational isolation and additive curation with unchanged original records.
Private real-catalog replay checks constituent SDG ledgers and acceptance fixtures
without embedding production data in public test fixtures. Browser checks must
cover direct hidden-record links and Research/Operations/All discovery separately.
