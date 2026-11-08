# Loom tests

All regression tests and fixtures live here. Product implementations remain in `ops/`, `dashboard/`, `deploy/` and `executor/`.

```text
tests/
├── run.py                 # Full runner, reports and JUnit output
├── suite.py               # Separate Python process for each suite
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
# Run one suite:
python3 tests/suite.py tests/deploy /tmp/loom-packaging.json
```

Set browser paths through `LOOM_TEST_FIREFOX` and `LOOM_TEST_GECKODRIVER`. `deploy/test.sh` is a compatibility wrapper. Each suite runs in a separate process to isolate product modules with matching names; use `run.py` for the full run.

Maintain [validation scope and limitations](../deploy/TESTING.md) and the [feature-to-test mapping](../docs/features/catalog.json). Tests use temporary SQLite databases, files and HTTP servers without starting actual Jobs or deployments. Passing Docker/external-network mocks is distinct from verification on a real machine.
