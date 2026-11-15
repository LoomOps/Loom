# Host and Docker observations — detailed design

Status: implemented development candidate; operational deployment not performed.
Concrete ownership, test boundaries and external limits: [IMPLEMENTATION](../IMPLEMENTATION.md). Phases: D3, D5.

## Responsibility and model

**Authority:** loom-observer bounded read-only host services and loom-query projections.

**Typed model:** ObservationTime, SourceInstance, ResourceSample, ContainerObservation, Staleness and ObservationError.

**Operations:** CollectMetrics/CollectDockerStatus under existing read-only grants; queries consume snapshots with timestamps and source identity.

**Persistence:** Existing host observation files/receipts and configured locations; observation failure is not execution return evidence.

## Reference source and consumers

- [deploy/collect-dashboard-gpus.py](../../../deploy/collect-dashboard-gpus.py)
- [deploy/collect-observations.py](../../../deploy/collect-observations.py)
- [ops/docker_status.py](../../../ops/docker_status.py)
- [ops/host_metrics.py](../../../ops/host_metrics.py)

Existing feature dependencies: none declared.
The references establish old behavior, not target Rust module boundaries.

## Implementation work

1. Preserve protected host vs unprivileged Ops split; no Docker socket or new GPU permission in Ops or Jobs.

2. Use bounded collection intervals/process timeouts, atomic sample publication and backpressure; do not let a slow collector block Job transactions.

3. Distinguish unavailable, stale, zero and permission denied. Never report zero usage solely because a collector failed.

4. Keep observation ownership/instance labels so foreign containers are not adopted or stopped.

5. Migrate stdlib/CLI collectors to Rust when implementing the bounded host binary; retain vendor-specific probes only behind narrow adapters where their SDK is required.

## Transaction and effect boundary

Samples are independently published/read snapshots; they cannot release a ticket, reassign resources or modify queue order. Durable execution return remains the Executor authority.

## Retained code and compatibility

Vendor/driver tools and existing read-only access patterns. Do not rewrite scientific/GPU libraries or turn monitoring into a scheduler.

Keep existing collection paths and ownership until matching service packaging is ready. Include new binary in helper inventory/doctor and retain consumer DTOs.

## Acceptance cases

| Case | Trigger | Required result |
|---|---|---|
| OBS-01 | Collector times out or loses permissions | Bounded stale/unavailable sample; Jobs remain independent. |
| OBS-02 | Foreign container with similar name | Excluded or read-only described; never adopted. |
| OBS-03 | Partial snapshot write/restart | Last complete sample or explicit unavailable, no malformed success. |

Reference regression modules (adapt moved internals to actual Rust boundaries):

- [tests/ops/test_docker_status.py](../../../tests/ops/test_docker_status.py)
- [tests/ops/test_host_metrics.py](../../../tests/ops/test_host_metrics.py)

## Exit review

Validate the cases above plus the shared [domain](../DOMAIN.md), [API](../API-CHANGES.md),
[compatibility](../COMPATIBILITY.md) and [migration](../MIGRATION.md) contracts.
Record exact source/artifact hashes and which external boundaries remain unverified.
A passing old Python suite or compiling crate alone does not close this block.
