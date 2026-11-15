# Extensions — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Software revisions are immutable; Runtime authority is a separate protected host catalog. Dependency display families currently aggregate by normalized package name. Runtime observations become stale after their freshness window.

Reviewed source:

- [rust/crates/core/src/software.rs](../../../rust/crates/core/src/software.rs)
- [rust/crates/core/src/composition.rs](../../../rust/crates/core/src/composition.rs)
- [rust/crates/core/src/runtime.rs](../../../rust/crates/core/src/runtime.rs)
- [rust/crates/host/src/runtime.rs](../../../rust/crates/host/src/runtime.rs)

## Implementation / consumer review

1. Add exact Item/revision/alias adapters for Component, Tool, standalone/installation Profile, Runtime and declared dependency units.
2. Use sidecars to inventory pinned source/build files without adding files inside a sealed user revision. Profiles link their reusable composition instead of copying source inventories.
3. Retain exact host catalog origin, Runtime revision/hash, installation provenance, grant and package scope; name-grouped dependency families stay display projections.
4. Capture accepted external catalog descriptors for rebuild and separately display current observation/grant freshness. Missing access is not evidence of uninstallation.
5. Keep transitive package inventories and archive members nested; no automatic top-level Item or dashboard promotion.
6. Use configured dependency directory/grants; indexing does not install, load, import or validate a scientific runtime.

## Invariants

- Existing Python/Rust revisions and accepted Job pins remain byte-identical.
- Shared use or Built-in origin is not an ownership/ACL rewrite.
- A package-name/version match does not prove identical installation or actual use.

## Planned acceptance and dependencies

Cases: `SW-01`, `SW-02`, `FILE-01`, `ID-03`, `ID-04`, `AUTH-03`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [authoring](authoring.md), [identity](identity.md), [installation](installation.md).
