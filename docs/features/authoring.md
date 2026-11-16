# authoring — Rust producer SDK, source/build identities and distributed authoring guidance

## Boundary

Accepted revisions and execution provenance remain immutable. Source registration never runs user code. Local and copied legacy sources use the same bounded regular-file reader before staging; special files and cumulative package limits cannot leave a published revision.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Rust producer SDK, source/build identities and distributed authoring guidance | `rust/crates/core/src/authoring.rs#verify_build` | Authenticated typed action and exact revision identities | Transactional receipt or explicit conflict | extensions, execution, onboarding, installation |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `components.scaffold`, `components.validate_package`.

## Sources and contracts

- [rust/contracts/authoring.md](../../rust/contracts/authoring.md) — native implementation / contract.
- [rust/crates/core/src/authoring.rs](../../rust/crates/core/src/authoring.rs) — native implementation / contract.
- [rust/crates/sdk/Cargo.toml](../../rust/crates/sdk/Cargo.toml) — native implementation / contract.
- [rust/crates/sdk/src/lib.rs](../../rust/crates/sdk/src/lib.rs) — native implementation / contract.
- [rust/crates/sdk/src/main.rs](../../rust/crates/sdk/src/main.rs) — native implementation / contract.
- [rust/templates/hybrid.rs](../../rust/templates/hybrid.rs) — native implementation / contract.
- [rust/templates/runner.rs](../../rust/templates/runner.rs) — native implementation / contract.
- [rust/templates/sdk-Cargo.toml](../../rust/templates/sdk-Cargo.toml) — native implementation / contract.
- [rust/templates/worker.py](../../rust/templates/worker.py) — native implementation / contract.
- [rust/crates/core/src/software.rs](../../rust/crates/core/src/software.rs) — native implementation / contract.
- [rust/crates/core/src/contracts.rs](../../rust/crates/core/src/contracts.rs) — native implementation / contract.

## Change review

Review live schemas, exact bindings, Dashboard, distributed guidance, migration and native acceptance together.

Related blocks: [identity](identity.md) · [api](api.md).

## Verification

- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/sdk.rs](../../tests/rust/sdk.rs)
- [tests/rust/test_authoring.py](../../tests/rust/test_authoring.py)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)

Acceptance outside these fixtures:
- Run isolated Rust/HTTP/producer/browser fixtures; operational acceptance requires separately authorized deployment.
