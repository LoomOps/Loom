# onboarding — User guides, version identification and client entry

## Boundary

The running server is authoritative for guides. Updates do not change personal instructions or active Job pins.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Guide package | `rust/crates/core/src/onboarding.rs#bundle` | Authenticated actor, compile-time guide catalog, current action registry and documentation | Version-bound guide package and update identity; no automatic credential issuance | api, installation |

Public actions and input schemas are in the generated [API reference](../reference/README.md). This block owns: `system.guide`, `system.contract`, `system.describe`.

## Sources and contracts

- [dashboard/build_version.py](../../dashboard/build_version.py) — integration, packaging or documentation.
- [deploy/client_release.py](../../deploy/client_release.py) — integration, packaging or documentation.
- [loom_use/client-release.json](../../loom_use/client-release.json) — integration, packaging or documentation.
- [loom_use/runtimes.md](../../loom_use/runtimes.md) — integration, packaging or documentation.
- [ops/build_version.py](../../ops/build_version.py) — retained Python compatibility reference.
- [ops/onboarding.py](../../ops/onboarding.py) — retained Python compatibility reference.
- [rust/contracts/action-docs.json](../../rust/contracts/action-docs.json) — native implementation / contract.
- [rust/crates/core/src/onboarding.rs](../../rust/crates/core/src/onboarding.rs) — native implementation / contract.
- [tests/ops/client_source.py](../../tests/ops/client_source.py) — integration, packaging or documentation.
- [loom_use/catalog.json](../../loom_use/catalog.json) — integration, packaging or documentation.
- [rust/crates/core/src/query.rs](../../rust/crates/core/src/query.rs) — native implementation / contract.
- [loom_use/entry.md](../../loom_use/entry.md) — integration, packaging or documentation.

## Change review

When guidance changes, update the guide catalog, packaged instruction/reference hashes and relevant native action descriptions. Review the independent Loom-Use Skill and exact published client pin separately; run native discovery/workflow and pinned/candidate client acceptance. Guide refresh does not install a client or change Job pins.

Related blocks: [identity](identity.md) · [api](api.md).

## Verification

- [tests/deploy/test_client_release.py](../../tests/deploy/test_client_release.py)
- [tests/ops/test_client_setup.py](../../tests/ops/test_client_setup.py)
- [tests/ops/test_onboarding.py](../../tests/ops/test_onboarding.py)
- [tests/rust/application.rs](../../tests/rust/application.rs)
- [tests/rust/test_service.py](../../tests/rust/test_service.py)
- [tests/rust/test_item_image.py](../../tests/rust/test_item_image.py)
- [tests/rust/documentation.rs](../../tests/rust/documentation.rs)
- [tests/rust/test_documentation.py](../../tests/rust/test_documentation.py)

Acceptance outside these fixtures:
- Verify actual client reconnection and guide refresh separately.

Native tool-call success and tool-error replies retain operation JSON/error in the
first content block and append a short version footer. `_meta.loom` and HTTP
`X-Loom-Revision` carry the full identities. The immutable binary identity is
computed once per process; ordinary responses do not rehash all guide/schema text.

Immutable guide/template payloads and release identity live in `loom-reference`; executable composition installs `ReferenceAssets` before serving. Core retains scoped serving, schema/guide hashing and bundle assembly. Guidance changes invalidate that payload and its binary consumers without recompiling transactional Core/Host.
