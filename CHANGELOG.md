# Changelog

## 0.2.0-alpha.86

- Load Dashboard resource scopes separately and page terminal history in finish order. Defer detailed Runtime usage, registered files and training reports until requested.
- Keep catalog summaries and comparisons tied to exact revision records and refresh the views affected by each change.
- Preserve execution and log evidence during container return, and prioritize live log collection.
- Pin Loom Use alpha.17, which adds optional Agent Workflows for an existing Codex parent conversation.

## 0.2.0-alpha.80

- Claim existing Dataset and Policy records for explicitly authorized Job updates, while active readers retain their exact versions.
- Consolidate a reviewed continuation without changing its original Job, files or exact links.
- Distinguish unsupported and temporarily unavailable completion estimates, and accept progress submitted after execution begins.

## 0.2.0-alpha.79

- Carry the pinned Task into the native runtime and retain diagnostic artifacts when a producer reports that execution failed.
- Record recoverable execution and publication failures in Troubleshooting. Findings and closure keep the original record and append a revision.
- Improve Unit queue alignment and refresh behavior for running Jobs.

## 0.2.0-alpha.78

- Derive compact, whitespace-free Dataset tags such as `split-100/20/20` from recorded counts. Align monospace tags with adjacent catalog fields and document the naming rule in Loom Use.

## 0.2.0-alpha.77

- Show Job Units and Activity loading explicitly, retain loaded details during refresh, and estimate Unit finish times from producer progress samples when available.
- Expose recovery on failed execution Units and verify the Job and Unit schema during managed updates.
- Filter the catalog Task index by current dispositions, show current Task names on catalog groups, and display Dataset split counts as tags while preserving exact revision links. Allow research submit scope to discard and restore catalog outputs.
- Label report chart axes, ticks and series, and pin Loom Use alpha.16.

## 0.2.0-alpha.63

- Group recorded architecture descriptions that name the same recognized policy family, including BC-RNN descriptions with implementation prefixes.
- Preserve an existing operator maintenance fence across a completed update or recovery.

## 0.2.0-alpha.62

- Show identical preserved copies of one Policy or Experiment source revision once within a catalog selection. Keep exact snapshots individually accessible and show differently recorded sources separately.

## 0.2.0-alpha.61

- Navigate the research Catalog by Task. Policies add an independent Model family selection, and catalog rows retain their exact record and lineage links.
- Require an exact Task revision for newly submitted or imported Regular and Reference Datasets, Policies and Experiments. Keep general installation and Runtime evidence in Artifacts and receipts.
- Clarify Standalone Unit history, consolidate scientific record presentation, and synchronize Loom Use guidance with the native API reference.

## 0.2.0-alpha.46

- Discover grouped Job parents once per catalog read and avoid recursive family aggregation for unrelated singleton Jobs. Preserve real group summaries and keep large catalog reads separate from the mutation lane.
- Includes the Inspector placeholder removal and incremental release snapshot fixes from alpha.44/45.

## 0.2.0-alpha.45

- Preserve source modification times when preparing immutable release snapshots, allowing Cargo to reuse unchanged native components across image builds. Keep generated release identities explicit and existing numbered artifacts unchanged.
- Includes the Inspector placeholder removal, incremental build boundaries and Main Job/Activity corrections from alpha.44.

## 0.2.0-alpha.44

- Omit unavailable GPU identity and storage mount text in Inspector instead of displaying a pending-measurement placeholder. Preserve numeric readings, missing/stale indicators and real mount warnings.
- Isolate native contracts, secure host primitives and reference/build assets. Add pinned Bookworm incremental builds, scoped verification and source-verified native kit reuse; retain the protected operational update path.
- Keep Main Jobs visible through related execution activity, show root/group status separately, and count related Activity intervals once per logical Main Job. Publish API v56 and matching guidance.

## 0.2.0-alpha.43

- Resolve unloaded Runtime ID/revision links to one exact immutable alias, including older revisions used by recorded Jobs. Reject conflicting aliases or explicit SHA mismatch; never substitute the latest Runtime.
- Verify Main Job pagination, historical Runtime navigation and back-links with actual recorded metadata on an isolated native service. Publish API v55 and the matching reference.
- Includes workflow discovery, asynchronous Activity, preparation/Runtime usage, Dataset split and System smoke corrections below.

## 0.2.0-alpha.42

- Renew Home Activity when a bootstrap revision changes without a full render. Keep one request per revision/view, ignore late replies, and preserve the last verified rollup until replacement.
- Add a deterministic browser regression that holds an old empty Activity reply across a bootstrap and requires the newer populated response without clicking a filter.
- Includes the workflow discovery correction and prior preparation/Runtime, Dataset split and System smoke fixes below.

## 0.2.0-alpha.41

- Keep logical workflow roots in paged Item discovery after Job presentation curation. Refresh from the workflow authority instead of treating an absent physical Job as a deleted workflow; preserve exact native aliases during a source-backed repair.
- Verify workflow curation through real HTTP pagination and recovery of an earlier derived tombstone, alongside original execution preservation.
- Let the large catalog-read concurrency fixture use its intended 20-second read budget while retaining the separate 2-second mutation-lane assertion.

## 0.2.0-alpha.40

- Give the 100,000-entry inventory correctness fixture a dedicated 120-second process watchdog on shared CI runners while retaining its full input and growth/chunking assertions. Other native cases retain their 60-second watchdog. Record elapsed time and the chosen budget for each native case.
- Includes the preparation history, Runtime consumer, Activity, Dataset split and installation smoke corrections described below.

## 0.2.0-alpha.39 (unpublished intermediate)

- Connect preparation and follow-up Job history to its Main outcome while preserving original workflow membership, execution states, Attempts and accepted software pins. Apply workflow presentation overlays consistently and validate partial curation against the merged parent graph.
- Expose exact Runtime consumers from frozen Job stage pins and started-stage receipts, with source Job/Profile and Main Job navigation.
- Keep Home Activity and default terminal history independent of scientific Outputs filters. Group related preparation history, union actual execution intervals, retain explicit filters and fence stale Activity refresh responses.
- Normalize recorded legacy Dataset split counts without losing zero, Missing or N/A distinctions; preserve conflicting source evidence instead of inventing counts.
- Attribute native installation smoke to System at submission and preserve its notification recipient. Update API v54, generated references, authoring guides and regression coverage.

## 0.2.0-alpha.38

- Preserve native Runtime/Dependency observation retirement during upgrades without treating it as scientific Item deletion. Keep original IDs, native keys, creation identity and immutable revisions exact.
- Permit only monotonic catalog access-epoch advancement while preserving origin and cursor-signing keys; reject epoch regression and scientific Item retirement. Add focused preservation failure regressions.

## 0.2.0-alpha.37

- Add reversible Archive, Restore and deprecation for exact Component, Tool, Profile, Runtime, metadata Template, report Contract and report Definition registrations. Preview dependencies and Job usage, explicitly select related registrations, and apply an audited atomic batch while preserving source files, accepted pins and execution history. Protect Built-ins and enforce management rights.
- Filter archived registrations before Dashboard/search pagination, preserve exact reads and revision history, and expose consistent registry filters and lifecycle menus. Update HTTP/MCP schemas, server guides and generated reference.
- Move Comparison back to the sidebar with a distinct icon. Keep Outputs headers focused on Datasets, Experiments and Policies, and place Template/Contract browsing in Loom Use.
- Bind image OS package observations to their immutable Runtime image, preventing dependency-version oscillation and unbounded search history growth.
- Correct long report/metric layouts, escaped scientific selectors, exact unloaded Item navigation and comparison defaults. Preserve source data and existing metadata/reporting contracts.

## 0.2.0-alpha.36

- Reuse prepared responsibility queries for large Dashboard catalogs and remove a redundant Job ownership lookup.

## 0.2.0-alpha.35

- Reserve System responsibility for Loom platform/install smoke checks. Preserve original namespaces and execution identity; restrict System attribution changes to administrators.
- Hide System records from Dashboard catalog, queue, search, counts and Activity while preserving normal MCP/API discovery. Keep user Runtime development and diagnostic work separate.

## 0.2.0-alpha.34

- Keep report detail tabs compact and horizontal; wrap only as needed on narrow panes.

## 0.2.0-alpha.33

- Register immutable metadata formats, reporting contracts and metric/factor/protocol definitions with shared Item identities. Reuse Built-in coverage, derive evidence-backed exclusions/replacements, or author additional analysis without a platform release.
- Freeze reporter capabilities, exact scientific inputs, analysis schedules and expected values before new Jobs. Verify report bytes, producer/attempt pins, populations, SDG membership and evaluation predicates before publication; preserve accepted legacy execution and repair evidence without rerunning computation.
- Separate Robot DoF, controlled joint DoF and ordered Action dimensions. Retain per-channel held-out MSE with squared units, MAE/RMSE, normalization, instruction assignments and checkpoint/phase learning series.
- Show Policies and typed report detail sections in Outputs. Keep comparison as a derived view, expose Built-in/registered contract browsing, and preserve incomplete historical records with explicit Missing states. Keep reports indexed alongside sealed large-file inventories.
- Prefix output display names with an evidence-backed robot model and support additive source-bound metadata adoption. Fix initial Dashboard Job pagination without a filter change.

## 0.2.0-alpha.31

- Move Dashboard queue/history Task links below Job details and directly above Owner. Use subdued text without badge backgrounds or borders in both themes while retaining keyboard focus and readable contrast.

## 0.2.0-alpha.30

- Preserve exact recovery requests, output-store identity and dependency graphs; resolve nested consolidation by frozen Attempt across Steps, Activity and Profile usage.
- Use bounded verified file-copy alternatives for inputs, reads and search; retain continuation past unavailable content matches and protect derived catalog roots.
- Harden metadata/trust-key reads, Sharing revocation and idle update ownership checks; preserve native API/MCP revisions and complete offline client handoff.
- Escape imported output labels/counts, preserve newer search details after stale errors, and remove obsolete Profile table styles.
- Pin Loom Use alpha.13 with registered File handoff and active-connection Skill preservation. Simplify duplicated verification while retaining failure-boundary coverage.

## 0.2.0-alpha.27

- Recognize equivalent imported Dataset records across publication/catalog envelopes. Preserve scientific metadata, file hashes, publication state and exact source links; ignore only JSON formatting and known generated index bindings.

## 0.2.0-alpha.26

- Fit long exact Dataset-parent links in narrow panes. Use unframed output sections rather than inherited card skins, and keep provenance tables within Sources.
- Keep opaque source hashes and full transform evidence in Sources, prioritize quantity/split facts, and stop interpreting unloaded paged references as unregistered records.

## 0.2.0-alpha.25

- Register exact Dataset derivations from accepted inputs or source-bound additive evidence; reject missing revisions, bad component hashes and cycles. Preserve original records and expose lineage in results, Item graph and Dashboard.
- Group related Dataset variants and collapse only byte-identical imported original records, retaining exact source links and individual counts.
- Replace oversized output sections with compact typed fields and one Sources disclosure. Remove Overview prose; distinguish stored Dataset language, observer inputs and policy conditioning.
- Show recorded BC eligibility/positive coverage, retain false and zero, and correct ambiguous saved observer-source aliases without invalidating unrelated metadata.
- Publish matching server reference/producer guidance and pin Loom Use alpha.12.

## 0.2.0-alpha.24

- Distinguish explicit source verification failures from metadata-only refreshes. Render structured input values without object placeholders.

## 0.2.0-alpha.23

- Persist recovered scientific metadata as exact-source annotations, with preview digests, idempotent apply, immutable original records and current cache invalidation. Include saved facts in every catalog projection.
- Retain verified page/detail facts across metadata-only refreshes of the same generation. New revisions and exact reads can replace them explicitly.
- Use full-width responsive input/observation groups, remove duplicate Model inputs, preserve readable labels in narrow panes and omit empty summary rows.

## 0.2.0-alpha.22

- Preserve units on multi-valued clocks and meaningful precision on small scientific measurements. Apply the same source precedence/conflict handling to condition tables as scalar facts.

## 0.2.0-alpha.21

- Show verified per-condition task outcomes with separate success counts and denominators, including retained RGB/depth comparisons. Reject validation-check counters and impossible success counts as task outcomes.

## 0.2.0-alpha.20

- Build a canonical scientific summary from exact accepted metadata and bounded, registered JSON artifacts verified by size, location and SHA-256. Retain source pointers and conflicting recorded values; preserve original records and scientific acceptance.
- Admit sibling publication context only for a unique Model/Dataset in the same Job/Attempt with explicit matching product hashes. Do not follow arbitrary JSON paths or open model/HDF5 archives.
- Replace generic used/unused input labels and flat highlights with shared scientific cards and detail sections: Dataset collection/physics/annotation, Model input/prediction/training/validation, and Experiment evaluator/population/success conditions.
- Show Observer point errors in millimeters and actual input shapes, preserve zero/false values and distinguish inapplicable observations from missing metadata. Keep offline action error separate from physical task success.

## 0.2.0-alpha.19

- Keep Task as an Outputs filter and move its badges above original titles; remove New Task controls. Preserve high-contrast Task badges in Job history.
- Prioritize research outputs, expose supporting/Observer records through Development filters, and add the additive Development Job category. Separate current supporting queue rows without hiding their resource use.
- Lead result details with recorded collection, model and evaluation metadata. Move ownership to record provenance and remove generic dataset role banners and redundant footer prose.
- Emphasize only an unambiguous declared primary outcome; keep distinct diagnostic metrics and populations expandable rather than manufacturing a headline success rate.
- Move Indexing status to the sidebar and continue host observations on output pages. Keep newer ISO-timestamped host samples when older catalogs arrive.

## 0.2.0-alpha.18

- Read current curated output names consistently in native results, historical listings, exact Item metadata and Dashboard list/detail views. Preserve accepted manifests, historical source bodies, file pins, Task links and scientific revisions.
- Refresh historical search descriptors after display curation. Keep output names independent of parent Job titles.

## 0.2.0-alpha.17

- Group Datasets, Experiments and Models in Outputs, with Comparison inside Experiments. Keep existing deep links and exact record identities.
- Refresh complete loaded catalog pages atomically, retain the visible output anchor and restart superseded cursors without collapsing the list to its bootstrap.
- Keep original output titles and show Task membership as a separate property. Add output naming guidance and an additive Rust SDK label helper for new producers.
- Use matching server-cog icons for Runtimes and hammer icons for Tools.

## 0.2.0-alpha.15

- Group research records with versioned Task filters and exact context links while retaining compatible Scenario API names. Remove standalone Scenario navigation.
- Preserve loaded catalog pages and open details during refresh; reload pages on a changed revision without accepting stale responses.
- Include support research in scoped Job history, collapse preserved Work mirrors, and retain exact completed consolidation boundaries without hiding later Attempts.
- Discover Explorer storage observations without the Inspector and page registered file metadata independently of research lists.
- Wrap long model input descriptions in narrow layouts and retain record ordering within a Task.

## 0.2.0-alpha.14

- Route browser search, Runtime and Scenario actions through the authenticated Dashboard relay at the installed Tailscale ingress. Keep direct browser calls to the native API rejected.
- Render sidebar Docker and NVIDIA driver versions from current host observations instead of incomplete installation snapshots.
- Validate browser request families against the deployment route plan and exercise collected software values in native/browser fixtures.

## 0.2.0-alpha.13

- Preserve account identity and display fields during updates while allowing the authenticated-read profile clock to advance. Keep exact original snapshots and all other account/token checks.
- Include the native Dashboard machine, Inspector, catalog pagination and bounded-read fixes from alpha.11–12.

## 0.2.0-alpha.12

- Queue bounded read bursts instead of immediately rejecting navigation with 503; keep the existing four-reader and two-heavy-reader limits.
- Classify Dashboard catalog actions as heavy reads and keep machine telemetry and status outside that pool. Waiters have a count limit and a shared three-second deadline.

## 0.2.0-alpha.11

- Restore collected machine identity and the Inspector resource envelope in the native Dashboard API, preserving observation freshness.
- Connect Models and Experiments to catalog pagination, and load Troubleshooting records outside the bootstrap.
- Add real native API and Firefox regressions for host observations and multi-page research catalogs.

## 0.2.0-alpha.10

- Separate human documentation, task guides, implementation specifications and historical design records. Link every native action to its owning feature and handler.
- Cover all 92 actions with explicit descriptions and schema-valid examples; add generated HTTP/API reference and documentation inventory checks.
- Compile 15 task guides from one catalog. Refresh guidance for changed reference prose, packaged instructions and client descriptors; include the public Job contract in contract revision identity.
- Pin Loom Use 0.1.0-alpha.11 with concise task routing and verified client documentation. Preserve existing credentials, software revisions, execution pins and storage state.


## 0.2.0-alpha.9

- Add explicit plan/validate/apply reconciliation with immutable source evidence,
  nullable-Job imported assets and verified additive file bindings.
- Complete bounded file inventories, exact member inputs, Runtime file discovery,
  source-qualified search/graphs and online search/log maintenance.
- Page Dashboard catalogs and preserve exact historical/Profile revisions; use
  compact actual-interval Activity instead of loading the entire catalog.
- Preserve installed identities, credentials and original records through the
  native schema 4 → 5 → 6 → 7 upgrade chain. Pin Loom Use alpha.10.
- Check container storage access without Python and resume using the newest
  superseding schema check. Preserve exact Dependency details in paged catalogs.

## Unreleased — native Rust candidate

- Add a native domain/application, scoped host executor, producer SDK and offline migrator; retain web, scientific SDKs and administrator integrations.
- Add immutable Scenario revisions, explicit research links, preserved historical location bindings and Rust-first authoring/templates.
- Preserve actual queue occupants, Job/Step/Attempt evidence and original request keys; fence delayed starts, stale returns, recovery and publication.
- Deliver same-image native binaries with additive schema/preservation receipts, read snapshots, live log ranges and native/browser/package acceptance.
- This development entry does not announce a published release or an operational cutover.

## 0.1.0-alpha.60

- Add a Dependencies catalog within Runtimes, grouped into major software categories with search, exact versions, recorded origins and consuming Runtime/Profile links.
- Expose grant-scoped dependency discovery/detail through MCP without promoting transitive package inventories or rewriting installed Runtime/Job pins.
- Support optional validated dependency catalog/category display metadata in administrator recipes; retain configured installation directories and existing recipes.

## 0.1.0-alpha.59

- Unify the Runtimes sidebar, tab and badges with a shared layered icon, distinct from Models and Docker.
- Use a shared multi-tool icon for Tools, distinct from Troubleshooting and terminal commands.

## 0.1.0-alpha.58

- Namespace installation smoke Components by build so post-upgrade acceptance does not collide with immutable earlier smoke registrations or reuse a previous build's execution receipt.

## 0.1.0-alpha.57

- Record the exact immutable kit build in fresh installation state so the first subsequent upgrade can locate the installed source safely.

## 0.1.0-alpha.56

- Correct fresh-install Gateway diagnostics mount validation while continuing to reject unrelated mounts, incorrect host paths and unusable diagnostics grants.

## 0.1.0-alpha.46

- Define one requested outcome per Job; batching and acceleration remain internal to the pinned runner by default.
- Add bounded producer-reported internal steps and explicit consolidation of completed producer Jobs without rewriting their history. Show the verified canonical Job by default with original execution history available.
- Add a durable operator dispatch pause that fences old workers, survives updates, permits staged continuation admission after update and resumes only with the matching pause key.
- Align server, Loom Use alpha.8 and external coordination guidance around logical Job boundaries and exact output reuse.

## 0.1.0-alpha.45

- Support validated administrator GPU driver capabilities, including graphics, and explicit none/bridge runtime networking without accepting Job Docker-policy overrides.
- Carry runtime options through fresh installation and verify actual grants; preserve executor configuration across update/resume/rollback.
- Add an opt-in bounded runtime doctor using the real broker, execution UID, mounts, read-only root, scratch and driver libraries. Keep runtime acceptance separate from scientific evaluation.
- Include the repository audit corrections and Extensions navigation prepared after alpha.43.


## Source audit included in alpha.45

- Align Extensions documentation across guides and feature specifications while preserving Component/Tool/Profile contracts and the six MCP groups.
- Accept matching sealed Profile source pins when explicitly copying legacy Components to Tools; reject conflicting evidence for Tool/Profile registration.
- Verify database file ownership/modes during state preservation and restore database attributes and symlink ownership on rollback.
- Harden client/guide archive paths and installed file-set checks; stop shared helpers before resuming an update. Correct the removed in-tree client installation reference.
- Enforce maintenance/readonly fencing for Component/Tool/Profile writes and ownership reassignment.
- This section records source review; actual deployment acceptance is recorded separately.

## 0.1.0-alpha.44

- Shorten the combined software navigation and page title to Extensions; retain Components and Tools as type filters.

## 0.1.0-alpha.43

- Combine Components and Tools in one searchable, type/namespace-filtered software inventory with a distinct sidebar icon.
- Add revision browsing, source hashes, execution stages and exact Profile/Tool usage links; preserve historical revision URLs and background-refresh selections.
- Improve light/dark and narrow/split-pane software layouts without browser registration or execution actions.

## 0.1.0-alpha.41

- Separate immutable research Tools from Job lifecycle Components; expose discovery, exact revision copying and pinned read-only execution through existing MCP groups and Dashboard.
- Ship cumulative dependency-ordered administrator migrations with bounded check/apply/verify scripts and durable receipts.
- Add explicit state-preserving container rebuild, private all-database/configuration snapshots, preservation checks and resumable/guarded recovery.
- Require effective storage parity at update/installation readiness, including old installs without a plan, actual mounts, executor and SSH grants, marker IDs, boot dependencies and service-user access.


## 0.1.0-alpha.40

- Apply normalized dataset facts to list rows and detail highlights, not only the Properties panel.
- Show compact input types and recorded splits; keep verbose producer summaries and input descriptions in the detail page.
- Keep fact labels intact while wrapping on narrow screens, with list-route browser regression coverage.

## 0.1.0-alpha.39

- Show consistent dataset properties from the normalized overview and load detailed source metadata on demand. Keep observation counts, missing values and original records distinct.
- Include active preserved catalog models and datasets in result discovery with exact historical detail references; do not imply payload availability or execution readiness.
- Restrict new Agent Token owners to current Admins and approved shared members, on both the Dashboard and issue API. Keep historical owner attribution and explicit token revocation intact.


## 0.1.0-alpha.38

- Preserve service ownership and private modes when atomically replacing the managed-access journal; distinguish local permission/storage errors from upstream failures.
- Collect private local diagnostics across Ops, Dashboard and Gateway with a 1 GiB default retention budget and configurable rotation. Preserve diagnostic correlation in standard MCP error content.
- Support compact group-only MCP action discovery and report current historical-artifact availability separately from preserved imported evidence.
- Pin Loom Use alpha.6 with installation write preflight, actionable refresh status and private client diagnostics.
- Extend packaging, permission, transport and cross-repository regressions without adding Job execution gates.

## 0.1.0-alpha.37

- Pin Loom Use alpha.5: honor explicit Tailscale authentication, verify identity on reconnect, preserve concurrent configuration changes during rollback, and validate MCP response envelopes.
- Stop creating retired agent-operation directories; clarify independent client ownership in installation and developer documentation.
- Validate component source path conflicts before publication and recover identical immutable snapshots after a rolled-back registration.
- Extend component, host-layout, client transport and cross-repository packaging regression coverage.

## 0.1.0-alpha.36

- Pin Loom Use alpha.4: resolve verified Codex socket symlinks, report specific refresh failures, and keep thread ID optional for current-chat verification only.
- Clarify that cwd is the existing invoking project context, not an automatically created or discovered project.

## 0.1.0-alpha.35

- Configure managed SSH after final storage registration and preserve its socket mount across restarts.
- Wait for initial service health; distinguish waiting and unrun smoke checks, and explain server sudo requirements early.
- Pin Loom Use alpha.3 with native WebSocket-over-Unix Codex reconnection and real framing regressions.
- Expose component/profile Owner metadata, explicit Built-in common components and shared discovery without moving revisions.

## 0.1.0-alpha.34

- Add Admin-managed personal host SSH accounts, exact Tailscale policy bindings and per-user MCP connection discovery.
- Keep local agents on local filesystem access; distribute optional Loom SSH with Loom Use alpha.2.
- Preserve homes and Jobs on SSH revocation; integrate protected helper lifecycle, storage ACLs, doctor and uninstall.
- Audit feature blocks against actual executed cases and tested source hashes; reject missing, skipped and stale evidence.
- Verify every Dashboard Docker COPY input against the staged release and build allowlist, including generated build metadata. Alpha.32–33 were unpublished verification candidates.

## 0.1.0-alpha.31

- Split client installation, connection profiles and the Skill into LoomOps/Loom-Use.
- Pin the exact tested client release in server guides and installer handoff.
- Delegate optional client setup to its INSTALL.md for the current agent/user.
- Test the external released client against Loom HTTP/MCP; preserve existing credentials and Skill entry links.
- Use the canonical LoomOps/Loom repository spelling.


## 0.1.0-alpha.30 — 2026-11-09

- Default MCP registration to `loom-use` in client setup, doctor and installation handoff, retaining explicit names and existing-connection conflict checks.

## 0.1.0-alpha.29 — 2026-11-09

- Display Loom Use consistently in Dashboard navigation, MCP discovery, usage guides and installer handoff; preserve connection IDs, paths and tool contracts.

## 0.1.0-alpha.28 — 2026-11-08

- Remove the retired browser token-entry fallback; missing native identity now stays closed with a retry action.

- Align installer defaults and documentation with Gateway 443 / private 8443; keep explicit existing ports.
- Remove obsolete local short-lived credential issuance; preserve existing expiry semantics.
- Support verified Tailscale guide refresh with principal-scoped caches and consistent private credential reads.
- Strip all client token rows on export and restore; reject schema-1 exports that would omit secondary-store payloads.
- Review all tracked files, feature verification boundaries, dev/alpha packaging and current instructions.

## 0.1.0-alpha.27 — 2026-11-08

- Fix sandbox-blocked token revocation with a parent-owned confirmation dialog.
- Remove duplicate connection instructions from token issuance; installer supplies exact private-endpoint setup commands.

## 0.1.0-alpha.26 — 2026-11-08

- Align Agent Token forms/dialogs with the Dashboard theme and collapse revoked history.
- Show verified development network identity without implying Admin rights.

## 0.1.0-alpha.25 — 2026-11-08

- Verify deployed native identity, private relay routes and separate public Gateway during update acceptance.
- Repair Admin, update-check and event relay routing.

## 0.1.0-alpha.24 — 2026-11-08

- Add Dashboard Admin-issued, individually revocable agent tokens without expiration.
  Resident clients retain their selected user identity and operational scopes.
- Add private token storage, Codex Streamable HTTP setup and read-only client doctor.
  Installation offers client setup without automatically issuing credentials.
- API revision: `loom-ops/v38`; six MCP groups and Job contracts remain unchanged.

## 0.1.0-alpha.23 — 2026-11-08

- Guide existing Tailscale connection reuse, initial login, explicit account switching
  and device approval separately from the Loom Admin email and management credential.
- Journal installation and runtime policy/tag ownership, reconcile uncertain writes,
  and restore only owned changes during uninstall. Preserve reused manual invitations
  and shared resources; detect final-device-tag reauthentication before stopping services.
- Add reviewed, policy-only cleanup for removed installations, plus installation
  doctor, offline-kit dependency and regression coverage.
- Include the ownership journal module in the explicit Dashboard build context.
  Alpha.22 failed packaging and was never published or deployed.

## 0.1.0-alpha.21 — 2026-11-08

- Add an administrator-only uninstall CLI that previews and removes an instance's
  managed Sharing grants/invitations, listeners and local access history. Remote
  cleanup is verified before deleting credentials; interrupted cleanup can resume.
  Jobs, artifacts, shared host dependencies and the host's Tailscale identity remain.

## 0.1.0-alpha.20 — 2026-11-08

- Expose artifact location repair as `results.reconnect` through MCP, generated
  reference/bundle and usage guides, matching the existing HTTP operation.
- Add HTTP/MCP parity and tokenless Tailscale MCP regression coverage; document
  instance-specific authentication and Sharing port reconciliation on upgrades.
- API revision: `loom-ops/v37`. Existing credentials, Job pins and data are preserved.

## 0.1.0-alpha.19 — 2026-11-08

- Default new installs to host /loom/data as data/0; prompt for optional RW stores and existing filesystem mounts. Preserve existing data owners and check effective service access.
- Add jobs.submit.output_storage, persistent storage identities, selected Attempt workspaces and result DB locations. Secondary outputs support catalog previews, result reconnects and hash-checked downstream input mounts. Existing Jobs retain their primary output storage.
- Expose store discovery and output selection through the same HTTP/MCP contracts; retain six tool groups. Show Output Storage in native Job properties.
- Offer Loom Use skill installation and user-scoped MCP registration after verified server installation. Client verification is separate from service readiness.
- Extend storage, executor, result, installer and client-handoff regressions; isolate release tests from installed host systemd units.

## 0.1.0-alpha.18 — 2026-11-08

- Pin the Docker bridge gateway to the exact trusted ingress address. Installation doctor compares live container gateways and trusted-proxy configuration, catching healthy services that cannot identify Tailscale users.
- Distinguish a failed private identity probe from a new visitor. Show connection/administrator guidance instead of prompting an existing user to request another invitation.
- Block duplicate Sharing invitations for native Admins/users, approved recipients and existing tailnet members. Membership lookup failures cannot fall through to sending an invitation.
- Ask explicitly for the Tailscale login email of the account that will administer Loom; update installation steps, diagnostics and operator guidance together.
- Refine Gateway acrylic: 34% tint in both themes, 30px card-only backdrop blur, 60px continuous corners and no white edge highlight. Preserve the scene blur and readable controls.
- Recheck all 15 feature boundaries, regression suites and matching installation/usage packaging. Existing installations retain local configuration and need an explicit idle repair if their bridge gateway differs.

## 0.1.0-alpha.17 — 2026-11-08

- Require the complete native Tailscale/Sharing installation: Ops, Dashboard, Gateway, executor and explicit Admin identity. Reject boolean Tailscale/token-only installation plans.
- Split installation guidance into seven steps, with host-based recommendations, per-step rereading and Gateway-first handoff.
- Add read-only stage diagnostics (`doctor`/`verify`) for actual identities, images, mounts, services, observations and public/private ingress. Progress receipts alone never establish readiness.
- Ship a repeatable CPU preparation/execution/result smoke Job and require its verified artifact before a successful handoff.
- Generate machine-bound plans, support additional read-only catalog storage, and grant only explicit native users the private machine port.
- Prepare the Python execution image explicitly and support packaging recovery from a verified immutable build. Source and offline kits include diagnostics and step documents.
- Review all 15 feature boundaries and extend installer, policy and result-contract regression coverage. Existing installations require scoped configuration reconciliation; this release does not automatically update services or rerun fresh installation.

## 0.1.0-alpha.16 — 2026-11-08

- Establish the public GPL-3.0-only source baseline, with license files in source, images and usage bundles.
- Keep one native Tailscale identity and machine Sharing flow. Remove the unused tailnet membership/device approval backend and obsolete login/admin UI.
- Remove personal operator references, one-off research migration/repair scripts, the old Agent application and pre-component producer execution.
- Expose only the six documented MCP groups. Preserve existing database identities, ownership, history and immutable artifacts.
- Keep GitHub Release discovery, explicit idle updates, local overrides and versioned usage guides; no automatic deployment is introduced.
- Extend regression coverage for unsupported routes, preserved historical data, distribution licensing and all-source syntax validation.
