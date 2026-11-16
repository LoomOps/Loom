# Loom Ops

The native `loom-ops` binary owns admission, execution, cancellation, durable state and publication. Its process composition is in `rust/crates/ops`; typed domain rules and transactional handlers are in `rust/crates/domain` and `rust/crates/core`.

This directory contains image/Compose definitions and retained Python compatibility references. Python fixtures do not make the old dispatcher the runtime authority. Read [implementation boundaries](../docs/IMPLEMENTATION.md) before changing or removing an adapter.

- [API and MCP reference](../docs/reference/README.md)
- [Job and software task guides](../loom_use/entry.md)
- [Platform architecture](../deploy/PLATFORM.md)
- [Installation and updates](../docs/UPDATES.md)
- [Remote handoff](REMOTE-FENCING.md)
- [Feature ownership and tests](../docs/features/README.md)

Use the isolated dev-build selected by `LOOM_INSTANCE` and `LOOM_PROJECT`; dev changes do not update operational installations. Persistent DBs, tokens, registered sources and storage are installation state, not repository content. The native immutable history adapter is `rust/crates/core/src/history.rs`; preserved records never enter execution merely because they are readable.
