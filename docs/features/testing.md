# testing — Feature specifications, impact analysis and isolated test tools

## Responsibilities and boundaries

Development validation adds no new gates to Job execution. Distinguish test results from actual deployment state.

Tests and fixtures live in [`tests/`](../../tests/README.md). Use `python3 tests/run.py --browser` for the full suite; `deploy/test.sh` is a compatibility wrapper.

## Inputs, outputs and authoritative sources

- [docs/features/catalog.schema.json](../../docs/features/catalog.schema.json)
- [deploy/features.py](../../deploy/features.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `deploy/features.py#impact` | Changed repo-relative paths + feature catalog | Direct owners, related consumers, tests/specs; no mandatory edits to every consumer | installation |
| `deploy/features.py#check` | Catalog + repository tree | Missing references, ambiguous owners, unmapped code/tests and broken interface links | installation |

## Related features and change impact

Independent boundary. Find consumers through reverse references in catalog.json.

Register each new feature's unique ID, owned files, inputs/outputs, dependencies, tests and live-validation limitations. Test behavior and failure boundaries rather than adding generic assertions.

## Tests and failure boundaries

- [tests/deploy/test_features.py](../../tests/deploy/test_features.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Passing all tests does not prove successful authentication with actual external services.
