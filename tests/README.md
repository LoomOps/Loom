# Loom tests

All regression tests and fixtures live here. Native product implementations live in `rust/`; retained integrations and compatibility references live in `ops/`, `dashboard/`, `deploy/` and `executor/`.

```text
tests/
├── run.py                 # Full runner, reports and JUnit output
├── suite.py               # Separate Python process for each suite
├── rust/                  # Native rules, HTTP/MCP, migration and opt-in CPU Docker acceptance
├── ops/                   # Jobs, queue, execution, MCP/API and result contracts
├── dashboard/             # Authentication, Sharing, Gateway and HTTP fixtures
│   └── browser/           # Firefox UI integration tests
├── deploy/                # Installation, updates, packaging and feature specifications
└── evidence/              # Run results; excluded from Git and releases
```

Run from the repository root. The Python environment requires `jsonschema==4.23.0` and `PyYAML==6.0.2`.

```bash
python3 tests/run.py
python3 tests/run.py --browser --output /tmp/loom-tests
# Run one suite/file or one native package:
python3 tests/run.py --suite packaging --python-pattern test_build_inputs.py
python3 tests/run.py --suite native --rust-package loom-host
```

Set browser paths through `LOOM_TEST_FIREFOX` and `LOOM_TEST_GECKODRIVER`. `deploy/test.sh` is a compatibility wrapper. Each suite runs in a separate process to isolate product modules with matching names; use `run.py` for the full run.

Maintain [validation scope and limitations](../deploy/TESTING.md) and the [feature-to-test mapping](../docs/features/catalog.json). Tests use temporary SQLite databases, files and HTTP servers. Native acceptance runs real fixture Jobs and CPU producers; optional Docker cases use disposable containers. No operational installation or research Job is changed. Passing external-service mocks does not verify those services.

## Native Rust acceptance

Use the pinned Rust toolchain/Cargo.lock. `tests/run.py` compiles and executes the
actual native tests and independent protocol cases before the retained suites.
Set `LOOM_NATIVE_DOCKER_TEST=1` with a cached `python:3.12-slim` image for disposable
CPU containers. Docker skips cannot count as feature readiness. `--reference-only`
explicitly omits native acceptance. `LOOM_USE_CANDIDATE` may select a separate
candidate client checkout in addition to the independently pinned published client.
No operational instance, GPU Job or installed Skill is changed by this runner.

The [incremental build guide](../docs/BUILD.md) describes the pinned native graph, independent scale target, input/output boundaries and uncached local acceptance.
