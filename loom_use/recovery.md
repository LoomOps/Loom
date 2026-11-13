# Completion, errors and recovery

An agent reads its events through events.list(kind=all). ack confirms receipt, not repair ownership or execution completion. Unacknowledged events remain queryable after reconnecting. Preserve the event cursor and relevant Job IDs.

On failure, the Job reserves its existing position in the execution lane for 15 minutes. This affects later run stages in the same lane, not independent Jobs moved ahead of it or CPU preparation. An ordinary hold does not reserve a lane. The responsible agent calls recovery.claim and uses the returned lease for progress/resume. The reservation expires after 15 minutes without progress or a maximum 30-minute round. Resume after correcting the cause; at most three recovery rounds are available. Opt-in automatic retries consume the same budget. The profile's max_attempts is an additional automatic-retry cap, not extra recovery budget.

If no agent arrives or the limit is reached, defer the failed Job and its dependents while independent work proceeds. recovery.defer explicitly releases the reservation. A later reactivate assigns a new queue position without resetting recovery rounds. Never give another Job overlapping GPU ownership while the original process has not returned.

finalizing_failed is distinct from run failure. Use results.reindex for publication problems, or the appropriate postprocessing recovery for finalization. Do not rerun training merely to repair result presentation.

`recovery.state=executing` means a recovery round has started. Inspect the Job's
public `status` for its actual preparing/running/finalizing phase; executing is not
a stalled-recovery phase. Claim/progress do not consume an execution round.
Finalization-only resume keeps the same Attempt and frozen inputs without
readmitting execution. An execution repair refreshes inferred input dependencies,
preserves explicit dependencies and rejects cycles or obsolete raw Attempt locators.

If executor start/stop has an unconfirmed outcome, Loom keeps ownership unresolved
and stops dispatch until restart reconciliation confirms return. Do not mark the
Job failed or resubmit another Job merely because an RPC response was lost.
The same boundary applies to finalizers and lifecycle callbacks. Callback retries
retain a stable event id; callback implementations must remain idempotent.

Resident agents, scheduling, callbacks and multi-agent routing are caller choices. Loom provides events and recovery leases; it does not launch a particular client or mandate one supervision policy. A disconnected agent is not automatically assigned repair work without a caller action.
