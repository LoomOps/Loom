# Completion, errors and recovery

An agent reads its events through events.list(kind=all). ack confirms receipt, not repair ownership or execution completion. Unacknowledged events remain queryable after reconnecting. Preserve the event cursor and relevant Job IDs.

For a research Job, call `jobs.get` and read `units[].state`, `recovery_state`,
and `recovery_reason`. Target the exact failed Unit's `id` with recovery
actions. A Job that owns Units returns `recovery_unit_required` when used as the
recovery target. Standalone Units and legacy physical executions use their own IDs.

On failure, the Job reserves its existing position in the execution lane for 15 minutes. This affects later run stages in the same lane, not independent Jobs moved ahead of it or CPU preparation. An ordinary hold does not reserve a lane. The responsible agent calls recovery.claim and uses the returned lease for progress/resume. The reservation expires after 15 minutes without progress or a maximum 30-minute round. Resume after correcting the cause; at most three recovery rounds are available. Opt-in automatic retries consume the same budget. The profile's max_attempts is an additional automatic-retry cap, not extra recovery budget.

If no agent arrives or the limit is reached, defer the failed Job and its dependents while independent work proceeds. recovery.defer explicitly releases the reservation. A later reactivate assigns a new queue position without resetting recovery rounds. Never give another Job overlapping GPU ownership while the original process has not returned.

## Recover execution after a startup failure

1. Read the exact failed Unit and Attempt, including the failed stage, exit code,
   and process return. Use `jobs.log_streams` with the Unit, Attempt and stage,
   then `jobs.read_log` for the bounded tail. Report an unavailable log separately
   from a confirmed execution failure.
2. Confirm one recovery owner and claim the Unit. Repair the failing Component
   in a new revision and pin the matching Profile. Verify the actual startup
   entry point with the accepted Runtime, paths and input pins. Run the
   initialization calls before expensive execution. Use the Profile's
   `prepare` stage for checks that fit that stage. Any separate hardware smoke
   must stay within the user's authorized scope.
3. If the connected `recovery.resume` schema advertises `preview`, pass
   `preview: true` with the lease and proposed patch. Inspect the Profile
   revision and digest, Runtime, storage, repair mode and presence of a preparation
   stage. The server validates the repair contract without scheduling or
   consuming a round. Preview does not run startup checks or reserve resources.
4. Resume the same Unit without `preview` once the repair is verified. Read its
   stage and new Attempt and retain the original failure evidence. If startup
   fails again, diagnose the new error before trying another revision. Defer
   when the cause or scope remains unresolved. If the resume reply is lost,
   read current state or replay the exact request with the same lease.

## Repair publication

For a Unit with `state=result_failed`, read `units.get.result.publication_error`
for a rejected publication. If the finalizer process failed, read the exact
Attempt's `result.stage`, `result.exit_code` and its logs. The original run
exit code is recorded separately in `result.execution`.
Correct the failure and resume finalization for the same Attempt using the
recovery lease. Use `results.reindex` for a supported publication or index
repair. Read back the same Attempt and its registered outputs after recovery.
On a lost publication reply, check the Unit and exact Attempt before retrying.

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

## Troubleshooting records

Claim the exact failed Unit and read the returned `troubleshooting` records before repair. They contain the original Attempt, failure stage, pinned Profile and available Runtime, and exact log arguments. Use `jobs.logs` with that execution and Attempt. Use `results.search_items` with `kinds: ["issue"]` to find related incidents by Profile or error code. Compare pins and authority before reusing a repair.

After checking the exact failure or validating the repair, pass a short `finding` with `recovery.progress` and the current lease. Findings are visible to Loom readers. Include verified conclusions and omit credentials, raw logs, and private paths. A subsequent successful publication for the same execution links the verifying Attempt on its incident records.
