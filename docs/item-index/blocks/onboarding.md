# Onboarding — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Guides/schemas are versioned by the native binary and the client is independently pinned. The current client transport bounds responses at 2 MiB.

Reviewed source:

- [rust/crates/core/src/onboarding.rs](../../../rust/crates/core/src/onboarding.rs)
- [rust/contracts/action-docs.json](../../../rust/contracts/action-docs.json)
- [loom_use/README.md](../../../loom_use/README.md)
- [loom_use/jobs.md](../../../loom_use/jobs.md)
- [loom_use/components.md](../../../loom_use/components.md)
- [loom_use/runtimes.md](../../../loom_use/runtimes.md)
- [loom_use/skills/loom-use/SKILL.md](../../../loom_use/skills/loom-use/SKILL.md)

## Implementation / consumer review

1. Advertise Item/search/file/log capabilities only when the actions are implemented; generate live descriptions from the registry and update server guides.
2. Teach search -> exact revision -> files/log ranges -> native action; no path guessing, broad shell grep or invented lineage when API discovery suffices.
3. Specify common Item ID versus native Job ID, index lag versus source availability, exact input pins and metadata-versus-scientific validation.
4. Update independent client Skills/docs/examples and verify both current published and candidate packaged clients against real native service.
5. Keep the existing published client pin until a real verified independent release exists. No forced new coordinator threads or duplicate user reports.

## Invariants

- Planning docs are not live usage contracts.
- Client refresh cannot grant source/administrator access.
- Search events do not consume recipient events or become Task completion.

## Planned acceptance and dependencies

Cases: `QUERY-04`, `AUTH-02`, `SW-02`, `VERIFY-01`, `UI-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [api](api.md), [authoring](authoring.md).
