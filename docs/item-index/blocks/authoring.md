# Authoring — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

The SDK emits bounded producer manifests/progress; registration checks source/build/runtime evidence without executing source. Exact source and build binary locators already exist.

Reviewed source:

- [rust/crates/core/src/authoring.rs](../../../rust/crates/core/src/authoring.rs)
- [rust/crates/core/src/software.rs](../../../rust/crates/core/src/software.rs)
- [rust/crates/sdk/src/lib.rs](../../../rust/crates/sdk/src/lib.rs)
- [rust/contracts/authoring.md](../../../rust/contracts/authoring.md)
- [rust/templates/runner.rs](../../../rust/templates/runner.rs)
- [rust/templates/hybrid.rs](../../../rust/templates/hybrid.rs)

## Implementation / consumer review

1. Extend optional producer metadata with meaningful file roles/content type and inventory descriptor references under a versioned schema, not arbitrary global IDs or host paths.
2. Platform-assigned stable File IDs are returned by registration/publication; producer code cannot mutate the catalog DB or self-approve content evidence.
3. Inventory Rust source/Cargo.lock/build artifacts and explicitly justified Python worker stages without re-running build scripts or importing scientific libraries.
4. Reuse existing Component/Tool/Profile/runtime revisions; file indexing alone never creates a new revision or grants dependency installation.
5. Update generated templates, validation, SDK docs and live guides together; keep old producer schemas executable through adapters.

For new large outputs, version the producer inventory descriptor/chunk contract
and exercise it through actual publication and downstream consumption. Older
SDK/result schemas retain their current bytes and behavior; a server-side index
capability alone does not authorize emitting an unsupported producer schema.

## Invariants

- No compiler, network or scientific module executed in indexing/registration.
- Original accepted source/build/Runtime pins are unchanged.
- Progress/metadata never completes a Job or bypasses publication return proof.

## Planned acceptance and dependencies

Cases: `SW-02`, `FILE-01`, `FILE-07`, `FILE-04`, `TX-01`, `TX-02`, `QUERY-04`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [extensions](extensions.md), [onboarding](onboarding.md), [results](results.md).
