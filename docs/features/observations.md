# observations — Host, GPU and Docker observations

## Boundary

Observations are read-only and do not affect queue dispatch. Preserve missing/stale/partial distinctions.

## Interfaces

| Boundary | Owning entry point | Input | Result / failure | Consumers |
|---|---|---|---|---|
| Docker observation | `ops/docker_status.py#collect` | Configured Docker command/projects/read scope | Read-only container/image snapshot; unavailable facts remain unavailable | dashboard, installation |

## Sources and contracts

- [deploy/collect-dashboard-gpus.py](../../deploy/collect-dashboard-gpus.py) — integration, packaging or documentation.
- [deploy/collect-observations.py](../../deploy/collect-observations.py) — integration, packaging or documentation.
- [ops/docker_status.py](../../ops/docker_status.py) — active administrator observation collector dependency.
- [ops/host_metrics.py](../../ops/host_metrics.py) — active administrator host-metric collector dependency.
- [rust/crates/host/src/observe.rs](../../rust/crates/host/src/observe.rs) — native implementation / contract.

## Change review

When collector fields change, check host mounts, collection services, API projections and UI rendering. Review the full installation doctor and step exit criteria when this feature changes; a local health response does not establish user-facing readiness.

## Verification

- [tests/ops/test_docker_status.py](../../tests/ops/test_docker_status.py)
- [tests/ops/test_host_metrics.py](../../tests/ops/test_host_metrics.py)

Acceptance outside these fixtures:
- Actual OS, kernel, driver and Docker socket responses require live verification.

Sidebar software versions come from current Docker engine and GPU driver
observations. Installation-time inventory is not a live version source; missing
or stale measurements must not fall back to an old installation value.
