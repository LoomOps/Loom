# Rust authoring

Discover and reuse exact Profiles/Tools before authoring. Parameter, input and seed
changes normally reuse their compatible Profile. New Components, Tools and Job
orchestration default to Rust. Use components.scaffold and validate_package when
advertised. Rust packages use Cargo.lock and a source/build/target receipt; build
in an explicitly authorized pinned builder Runtime, never inside registration.
Python is allowed for a concrete external SDK/library integration such as Isaac
or a scientific model API. Keep it in a pinned worker stage; do not port the
external ecosystem or wrap all pure orchestration in Python. Preserve accepted
legacy Python revisions. Dependency/build roots come from the installation plan.
Submit one requested outcome as one Job with internal batching or an execution
DAG. Current queue occupants are visible regardless of research/operations purpose.
