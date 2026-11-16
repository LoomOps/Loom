# Author a producer

## Select the reusable boundary

Discover `components.profiles` and inspect compatible exact kinds first. New inputs, cases and seeds normally reuse a Profile. Create a revision when source, interfaces, stage commands, Tool pins or Runtime pins change. Components implement Job lifecycle integration; Tools provide callable research software. A hybrid runner can be a Tool.

Use Rust for new producer control, batching, progress and result orchestration. A concrete external SDK/library integration may remain a pinned Python worker. Preserve accepted legacy Python revisions; a language change creates a new revision and never repins an accepted Job.

## Scaffold and validate source

1. Read `system.describe` for `components.scaffold`. Choose `template: tool`, `component` or `hybrid` and a package name.
2. Save the returned `files` in the authorized workspace. The draft includes a vendored compatible `loom-sdk`; it does not fetch or execute code. Replace the algorithm and Runtime placeholders. A hybrid draft must name its actual external dependency and integration reason.
3. Generate and retain `Cargo.lock`, build with the selected pinned builder Runtime and target, then add the lockfile to the source package. Compiler caches and dependency/build roots come from the installation configuration.
4. Call `components.validate_package` with `manifest` and `files`. It validates source paths, package bounds and Python integration declarations without compiling, importing or installing anything.

The scaffold's Tool draft has no lifecycle Profiles. Component/hybrid drafts include a draft runner. Registration can accept source files directly; `components.write` is an editable staging operation, not a revision update.

## Producer protocol

The runner receives a request JSON path as its final argument. With the generated SDK:

```rust
use loom_sdk::{Request, Producer};
fn main() -> loom_sdk::Result<()> {
    let request = Request::from_args()?;
    let mut producer = Producer::new(request)?;
    producer.progress("run", "running")?;
    producer.write_artifact("receipt", "receipt.txt", b"Example completed\n")?;
    producer.set_entity_label("receipt", "Producer · Technical receipt")?;
    producer.finish()?;
    Ok(())
}
```

This emits a technical receipt, not a fabricated scientific result. Implement requested semantics explicitly. Write under `request.output_dir`; use the accepted Profile for provenance. `Producer.register_inventory` supports large sorted output inventories; see `items`. Step reports are display progress, not execution-success or resource-return evidence.

## Bind a build and register

Source identity and executable identity are separate. A `loom-producer-build/v1` receipt names `producer_protocol: loom-producer/v1`, the accepted source hash, exact granted builder Runtime, target, and binary locator/hash/size. The verifier checks executable bytes, permission, ELF architecture and source identity. Current supported receipt targets are `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`; a matching target still needs a compatible Runtime/libc.

A runner command beginning `@producer` requires this verified receipt and an explicit container Runtime. Loom binds the verified executable as `/inputs/loom-producer`. The registration path never invokes Cargo or imports the submitted source.

Use `components.register` / `register_tool` and, when needed, `register_profile` with exact Component/Tool/Runtime references. Inspect the resulting revision and Profile. Submit the requested work through `jobs.submit` or one workflow, keeping batching internal. Package validation and registration do not establish GPU/SDK/scientific acceptance; perform the relevant bounded test only within the requested execution scope.

New producer labels follow the Output names section of the results guide:
subject · variant · purpose · revision/selected step, omitting unknown parts.
Use `set_entity_label` after `register_entity`/`register_inventory`/`write_artifact` and
before finish. This edits only the pending manifest's label; it never renames a
registered result or its files. Keep Task links and scientific metadata separate.
