# Diagnostics — Item/index impact and detailed work

Status: proposed. Impact: **Direct — privacy boundary and safe status**. No runtime change is made by this plan.

## Observed baseline

Native incident JSONL records safe operation/code/status in a private directory; the retained Python collector has a different schema/retention implementation. These are not ordinary producer logs.

Reviewed source:

- [rust/crates/core/src/diagnostics.rs](../../../rust/crates/core/src/diagnostics.rs)
- [dashboard/diagnostics.py](../../../dashboard/diagnostics.py)
- [docs/features/diagnostics.md](../../../docs/features/diagnostics.md)

## Implementation / consumer review

1. Exclude diagnostic files/incident bodies from default inventories and ordinary Job/search text. Stable incident IDs may remain correlation references with existing validation.
2. Record indexer/collector failures as bounded safe codes without raw query text, file contents, secrets, full host paths or provider bodies.
3. Preserve fail-open behavior, private owner/mode, symlink refusal and bounded rotation; do not make diagnostics a Job/index readiness requirement.
4. Correct documentation that describes retained Python retention/envelopes as if it were the native sink. Any future administrator diagnostic search needs a separate reviewed contract.

## Invariants

- Private diagnostic content cannot leak via ordinary search snippets/facets/relations.
- Index diagnostics never upload themselves or expose request bodies.
- Diagnostic failures do not alter execution or source truth.

## Planned acceptance and dependencies

Cases: `AUTH-02`, `TX-04`, `LOG-06`, `VERIFY-01`. Definitions and evidence levels: [VERIFICATION](../VERIFICATION.md).

Review dependencies: [identity](identity.md), [observations](observations.md).
