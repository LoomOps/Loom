# Identity — Item/index impact and detailed work

Status: proposed. Impact: **Direct**. No runtime change is made by this plan.

## Observed baseline

Domain IDs are strings; generated native IDs currently use UUIDv4. Scope validation and responsibility overlays are independent. There is no common Item alias registry.

Reviewed source:

- [rust/crates/core/src/auth.rs](../../../rust/crates/core/src/auth.rs)
- [rust/crates/core/src/store.rs](../../../rust/crates/core/src/store.rs)
- [rust/crates/core/src/composition.rs](../../../rust/crates/core/src/composition.rs)

## Implementation / consumer review

1. Add distinct 128-bit Item/revision/file/stream types and lossless decimal-string codecs; keep native IDs and aliases unchanged.
2. Resolve exact origin/kind/native reference, including full historical snapshot. Reads resolve existing mappings and never allocate IDs.
3. Reuse current source authorization. Materialize permissions only as hints; revalidate current grants before titles, snippets, facets, counts and cursors leave the service.
4. Preserve ownership as responsibility, not a new access gate. Exclude credentials/private identity attributes from ordinary indexing.
5. Test restored/cloned origins and duplicate native keys without creating a global counter or interpreting IDs as commit order.

## Invariants

- No credential/account or existing native ID rewrite.
- Item ID alone is not a permission or a file locator.
- 128-bit IDs never pass through JSON Number or SQLite INTEGER.

## Planned acceptance and dependencies

Cases: `ID-01`, `ID-02`, `ID-03`, `ID-04`, `AUTH-01`, `AUTH-02`, `AUTH-03`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [api](api.md), [portability](portability.md).
