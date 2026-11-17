# Verification blocks

The feature catalog maps every block to its executable cases. This matrix records the boundary to assess; actual JSON/JUnit results belong to a specific source revision. A previous PASS, completed installation step or count of cases is not current acceptance. Native runner acceptance requires one executed, passing case; ignored, unmatched and timed-out cases cannot become passes through a zero process exit code. Aggregate summaries omit duplicate source hashes; per-suite reports retain the hashes used by the feature audit.

| Block | Automated evidence to keep | Installation / live boundary |
|---|---|---|
| identity | Native bindings, signed identity, expiry, VPN-account change, revocation and Admin matching | Doctor compares native SSO/Admin configuration, bypass flags and live Docker gateway/trusted-proxy agreement. Another-client profile/role verification remains live. |
| api | Six tool groups, HTTP/MCP schema parity including reconnect, independent result-action coverage, protocol errors and version metadata | Doctor queries actual API/tools; smoke submits through MCP. Real third-party clients remain separate. |
| queue | Order, hold/resume, dependent Jobs, workflow readiness reasons and preparation overlap | Smoke never reorders or cancels existing work. A busy queue leaves acceptance pending. |
| execution | Frozen profiles/output stores, container boundary, phase retries, return/cancellation | Doctor checks executor/image/GPU exposure; smoke runs prepare and run in containers. GPU computation is not inferred from nvidia-smi. |
| recovery | Leases, timeout, retry limits, lost agent, repaired-input dependency cycles and pinned storage | Installer never fixes a failed smoke Job by rewriting receipts. Failed Job evidence is retained. |
| results | Required metadata, hashes, provenance, finalization retry, multi-store indexing and downstream input resolution | Shipped smoke includes metrics/provenance/relations and rechecks the registered artifact digest. External-client download is not claimed. |
| extensions | Immutable Component/Tool/Profile/Runtime revisions, permission, source and environment pins | Registered Runtime validation proves its measured scope; research compatibility remains user-selected. Repeat smoke reuses its frozen profile/Job. |
| registry-lifecycle | Exact registration lifecycle, owner/admin rights, Built-in protection, reverse references/dependencies, atomic preview/apply and audit | Archive is a discovery overlay. Files, accepted pins and running/past Jobs remain valid; related targets require explicit selection. Test stale/mixed batches, restart, cursor invalidation and Dashboard restore on isolated resources. |
| catalog | Shared queries, preserved history, exact-boundary relation pagination, stage activity, alternate verified copies and preview/path boundaries | Doctor compares selected storage config and actual container mounts. All registered stores are RW. Profiles opt into named storage mounts; doctor checks real mount modes/service-user access. |
| observations | Docker and host collector handling, stale/unavailable states | Doctor checks collector timer plus API/container startup; actual sensor availability is hardware-dependent. |
| dashboard | Native entry, theme, responsive workspace and relay behavior | Doctor checks native entry and matching image; browser suites cover presentation. Other-client account switching must be exercised live. |
| onboarding | Guide/schema versions, bundle integrity, current references | Matching guides ship in source/offline kits; step documents and Gateway-first handoff must remain packaged. |
| network | Serve collision checks, tokenless user vs tagged-client boundaries, trusted peers, Sharing state/ETag/grant isolation | Doctor checks private/public routes, public rejection, TLS and policy guard. Real invitation acceptance requires a consenting participant. |
| portability | Preserved identities/history, fencing and import boundaries | Fresh installation does not clone another machine's DB, credentials or identity. Migration uses its separate contract. |
| installation | Default /loom/data, optional RW stores/mounts, persistent storage IDs, identity/storage config, stage failure, missing image recovery, uninstall plan/stop/revoke/purge/resume and packaging | Doctor uses actual state, never a completed-step flag. Full service contract is mandatory; publication resume preserves installed state. |
| testing | Schema/ownership/import/link checks, isolated SQLite/files and mocked external APIs | Default infrastructure fixtures mock external services. Explicit native CPU Docker acceptance uses disposable containers; no operational installation or live Tailscale policy is changed. Reboot/clean-host acceptance remains separate. |
| ssh | Personal UID bindings, Admin/CSRF boundaries, policy journal ownership, retry/revoke and local-client detection | External personal-client login and existing-session revocation require separate live acceptance. |
| diagnostics | Privacy, bounded retention, concurrent writers, symlink rejection and fail-open collection | Fixture failures do not establish actual disk capacity, delivery or external service availability; diagnostics never upload themselves. |
| scenarios | Native exact revision pins, archived context, explicit link provenance and graph pagination | No automatic protocol inference or scientific acceptance. |
| authoring | Native source draft compilation, SDK receipts, path bounds, Python integration and exact target/build checks | Registration is not a build; GPU/SDK scientific behavior is separate. |

## Review and run

1. Read the handler, consumers and invariant independently of the existing tests. Assess whether the tested behavior is the intended behavior.
2. Run `python3 deploy/features.py check` and `impact <changed paths> --transitive`; update only affected contracts.
3. Run the mapped native and retained suites. Use `python3 tests/run.py --browser --output <evidence-directory>` for the complete fixture boundary.
4. Run `python3 deploy/features.py audit --evidence <evidence-directory>` to bind blocks to actual cases and source hashes. Missing, skipped, failed or stale evidence is not a pass.
5. For documentation changes, run `deploy/documentation.py check --binary target/debug/loom-ops`, Loom-Use tests and native discovery/workflow fixtures. Action examples must satisfy their exact schema; behavioral examples must also meet handler preconditions.

Guide revisions cover guide contents/titles, package metadata, reference prose/examples, the compatibility Skill and exact client release descriptor. Contract revisions cover native action schemas and the public Job contract. Tests must show that guidance-only edits invalidate cached guidance while an unrelated build marker does not force a schema change.

Installation uses actual doctor results and the selected step's exit criteria. Ordinary Job use does not inherit an installation checklist. Live Tailscale consent, external clients, reboot, large-store performance, GPU/Sim and scientific semantics require separate scoped evidence.

Historical release-specific notes are preserved in [VERIFICATION-HISTORY.md](VERIFICATION-HISTORY.md); they are not another current checklist.
