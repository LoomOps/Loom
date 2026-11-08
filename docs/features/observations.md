# observations — Host, GPU and Docker observations

## Responsibilities and boundaries

Observations are read-only and do not affect queue dispatch. Preserve missing/stale/partial distinctions.

## Inputs, outputs and authoritative sources

- [ops/docker_status.py](../../ops/docker_status.py)
- [ops/host_metrics.py](../../ops/host_metrics.py)

The linked code/schemas define fields, enums and state transitions. This document does not duplicate them.

## Call contracts

| Boundary | Input | Result or failure | Consumers |
|---|---|---|---|
| `ops/docker_status.py#collect` | Configured Docker command/projects/read scope | Read-only container/image snapshot; unavailable facts remain unavailable | dashboard, installation |

## Related features and change impact

Independent boundary. Find consumers through reverse references in catalog.json.

When collector fields change, check host mounts, collection services, API projections and UI rendering.

## Tests and failure boundaries

- [tests/ops/test_docker_status.py](../../tests/ops/test_docker_status.py)
- [tests/ops/test_host_metrics.py](../../tests/ops/test_host_metrics.py)

Test sources define the concrete scenarios. `deploy/test.sh` records individual results as JSON/JUnit.

## Live validation

Actual OS, kernel, driver and Docker socket responses require live verification.
