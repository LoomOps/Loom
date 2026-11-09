# Loom development validation

Run from a Loom checkout. Tests and fixtures live under root `tests/`; see [layout and commands](../tests/README.md).

```bash
# Unit, mock, HTTP integration and packaging checks
python3 tests/run.py

# Include actual Firefox UI verification
python3 tests/run.py --browser

# Explicit CI output directory
python3 tests/run.py --browser --output /tmp/loom-test-results
```

The runner uses standard-library `unittest`. Schema/packaging tests require `jsonschema==4.23.0` and `PyYAML==6.0.2`; authentication runtime uses only the Python standard library. Install the pinned schema dependency in the test environment.

Browser checks use existing Firefox/geckodriver. The default Firefox path is `/snap/firefox/current/usr/lib/firefox`. Override installations with:

```bash
LOOM_TEST_FIREFOX=/path/to/firefox \
LOOM_TEST_GECKODRIVER=/path/to/geckodriver \
  python3 tests/run.py --browser
```

Missing executables are failures when `--browser` was requested. Runs omitting browser checks report `browser_requested=false`.

## Coverage

| Suite or contract | Behavior covered |
|---|---|
| Ops unit/integration | Submission deduplication, input validation, ownership/permissions, admission, cancellation |
| Job classification | Property retention, historical request compatibility, invalid enums, filters/pagination/permissions, API/MCP schema agreement |
| Authentication/delivery | Token expiry/revocation/scopes, receipt deduplication and recipient boundaries |
| Execution/result mocks | Indexing after process return, indexing retry, repair without reexecution, no publication during cancellation |
| Queue pipelines | Dependent execution from verified artifacts while indexing is pending; predecessor failure blocks and notifies |
| Artifacts | Missing files, path escape/symlinks, storage identity, invalid metrics, typed-series order/units |
| Job migration | Full Step/Attempt coverage, stale-summary rejection, deduplication, original states/pauses preserved |
| Actual HTTP API | Tokens/scopes, explicit dev mode, Origin, Job queries, MCP tools and compatible calls |
| Dashboard relay | Read routes and explicit queue/Admin writes, history/MCP forwarding, MIME/size/response/authentication boundaries |
| Installation | Mandatory native identity/Gateway, step doctor, explicit storage, actual image/config checks, missing runtime image recovery, packaged smoke contract and Gateway-first handoff |
| Packaging | Python dependency modules included in Dockerfiles; runtime state excluded from snapshots |
| Browser mocks | Unified queue/history expansion, title/menu alignment, Job properties, share/new-window controls, Docker/MCP, viewport/themes, API failure recovery |

SQLite and file validation use real temporary directories. Process lifecycle tests also execute small local subprocesses; Docker, GPU and external infrastructure boundaries are mocked. Browsers connect to a mock Ops API and actual Dashboard server on ephemeral loopback ports. Tests do not mutate production databases, images, containers, GPUs or queues, and do not depend on historical production IDs, counts or tokens.

Build generated UI into a temporary file and compare with `dashboard/public/workspace.html`. Stale output fails validation; tests never overwrite the running UI.

## Results

Default output: `tests/evidence/<KST-date-time>/`.

- `summary.json`: suite outcome, counts, duration and whether browser tests ran.
- `<suite>.json`, `<suite>.xml`: individual results and JUnit XML.
- `<suite>.log`: output including failure stack traces.
- `screenshots/`: mock Job captures by theme/viewport (`--browser`).

Failures return exit code 1. Evidence is excluded from source snapshots. Developers invoke tests explicitly; they add no automatic queue-dispatch, monitoring or mid-experiment gates.

## Tests versus deployment acceptance

Passing tests validates source regressions, not whether a running image contains that source or whether simulation/training numerical quality is correct.

Before deployment, build numbered images and verify startup, API/MCP and storage connections using the intended images and Compose settings in an isolated environment, then switch the intended installation. Never infer deployment authentication from internal tokenless settings. GPU generation/training/evaluation, cross-machine handoff/TLS and actual process recovery after failure require separate execution.

Use the current test suites and target-specific acceptance scripts. Preserved historical evidence is not an executable installation procedure.

## Shared queue and coverage boundaries

Regression coverage includes FIFO/front/back/relative ordering, duplicate submission, shared cross-user reads/controls with explicit read-only credential restrictions, hold/resume persistence, concurrent reorder, SQLite claim contention, recovery reservation overtaking/expiry/dependent skipping, same-Attempt resume after preparation hold, fresh Attempts for retries requiring preparation, and ordinary resume versus recovery. Compare HTTP/MCP/catalog behavior and verify controls/permission errors in actual Firefox.

Host executor tests use temporary config, SQLite and Unix sockets, mocking Docker calls only. They cover cross-instance GPU exclusion, retained ownership until actual return, foreign-instance cancellation rejection, runtime/path grants, ticket idempotency and created-ticket restart. They do not prove real GPU-driver or Docker-daemon failure behavior.

Unit/mock limitations include clean-host installation, external networking/TLS/Tailscale ACLs, resource accumulation over hours/days, GPU/Sim numerical quality, abrupt power loss between Docker create and ticket persistence, and duplicate external callback effects. Apply acceptance testing appropriate to each deployment change; do not turn this list into per-Job gates.

Executor create recovery regression injects failure after Docker create/before inspect, ticket INSERT failure and a create timeout after the Docker effect. Replay must reuse one container, execute once, retain GPU exclusion and reject foreign, changed or already-started unrecorded containers without deletion. The 2026-11-06 acceptance also exercised two interruption points with real Docker CPU containers and an isolated executor DB/socket, including module restart, returned-ticket replay and collection. Actual machine power loss remains outside that acceptance scope.

## Feature specifications and authentication tests

`docs/features/catalog.json` maps source, contracts and related tests. Packaging detects unregistered files, broken references and duplicate ownership.

Native authentication tests cover trusted ingress, account changes, revocation, fixed expiry, persistent sessions and native network login. Only ingress is mocked; SQLite/HTTP use real implementations. Tailscale installer tests mock CLI/API calls and use temporary instances for approval waits, wrong-tailnet rejection, tag preservation, Serve conflicts, configuration reruns and secret separation. Firefox verifies login, refresh and account-switch UI with local sessions and mocked trusted ingress. Actual Tailscale approval remains separate.
