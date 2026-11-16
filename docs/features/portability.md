# portability — Preserved data, Job migration and remote ownership

## Boundary

Distinguish original preservation from current presentation and retain installation identifiers. Cloned installations running concurrently require distinct identities.

The current native service is implemented in `rust/`. Python files in this block may be retained web/administrator integrations or compatibility references; they do not supersede the native transactional handlers. See [implementation boundaries](../IMPLEMENTATION.md).

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Copy integrity | `ops/portability.py#copy_verified` | Source/destination + expected file identity | Verified copy or explicit failure; does not reinterpret historical recipes | installation |
| Remote fence | `ops/remote_fence.py#apply` | DB, actor, action and ownership request | Ownership transition/receipt or refusal; controls real execution ownership | execution |

## Sources and contracts

- [deploy/assign-owner.py](../../deploy/assign-owner.py) — integration, packaging or documentation.
- [ops/portability.py](../../ops/portability.py) — retained archive/copy utility; native authority migration is loom-migrate.
- [ops/remote-handoff.py](../../ops/remote-handoff.py) — retained Python compatibility reference.
- [ops/remote_fence.py](../../ops/remote_fence.py) — retained remote authority service/protocol; native worker client is rust/crates/ops/src/ownership.rs.
- [rust/contracts/legacy-schema.sql](../../rust/contracts/legacy-schema.sql) — native implementation / contract.
- [rust/contracts/redesign-schema.sql](../../rust/contracts/redesign-schema.sql) — native implementation / contract.
- [rust/crates/migrate/Cargo.toml](../../rust/crates/migrate/Cargo.toml) — native implementation / contract.
- [rust/crates/migrate/src/main.rs](../../rust/crates/migrate/src/main.rs) — native implementation / contract.
- [rust/crates/ops/src/ownership.rs](../../rust/crates/ops/src/ownership.rs) — native implementation / contract.

## Change review

When migration changes, check locator/storage IDs, original hashes, ownership overlays, selective exclusions and rerun idempotency.

Related blocks: [identity](identity.md) · [catalog](catalog.md) · [results](results.md) · [execution](execution.md).

## Verification

- [tests/deploy/test_boot_readiness.py](../../tests/deploy/test_boot_readiness.py)
- [tests/ops/test_remote_ownership.py](../../tests/ops/test_remote_ownership.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)
- [tests/rust/test_installation.py](../../tests/rust/test_installation.py)
- [tests/rust/test_item_image.py](../../tests/rust/test_item_image.py)

Run the affected suites and `deploy/features.py check`; see [the verification matrix](VERIFICATION.md). Passing retained compatibility fixtures is distinct from exercising the native handler.

Acceptance outside these fixtures:
- Verify actual cross-machine TLS/handoff and access to restored data.
