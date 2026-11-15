# Rust producer authoring

Native candidate authority: `rust/crates/core/src/authoring.rs#verify_build`.
Retained Python paths below are compatibility references or administrator/web
integrations. See [the implementation map](../rust-port/IMPLEMENTATION.md) for
current process boundaries and acceptance limits.

New orchestration defaults to Rust. Scientific SDK/framework integration may use
explicit Python worker stages with package, stage, Runtime and a concrete reason.
Accepted historical Python source/Profile pins remain unchanged.

The public `components` group discovers templates and validates packages. Source
registration does not run Cargo, build scripts, import Python, access the network
or install a compiler. The producer SDK has a bounded request/progress/manifest
protocol without scheduler or credential access. Source identity and a compiled
artifact/build receipt are separate. `@producer` binds a verified ELF artifact
from a registered store in an exact granted builder Runtime; execution rechecks
its hash and mounts it read-only in the selected container Runtime.

Keep build execution in an explicitly authorized builder stage or administrator
workflow. Use the configured dependency directory; an API restart/client update
cannot install dependencies or backfill accepted execution pins. Tool presence
means availability to the runner, not proof that an algorithm was called.

Verify actual generated Cargo projects and hybrid worker failure/cancellation,
not only template strings. Tests belong under `tests/rust/`.
