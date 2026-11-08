# Online remote handoff

The authority is a single durable controller scoped to one relocation. It
stores the archive manifest digest, the currently authorized role, epoch
and worker session. A role credential is bound to an explicit machine and
storage identity. It is issued independently of the archive and mounted
only in the worker. API/user credentials remain separate.

1. Prepare the source worker with an authority grant before execution.
   Stop it normally and verify the local worker lock and process return.
   Export from the source to fence admission. Seal the source against the
   exact manifest hash from the source operator environment.
2. Transfer the archive separately. Restore using target-local binding.
   The target is read-only and can inspect its own restore receipt.
   Activate from the target using the manifest hash and target grant.
3. Start one worker with compose.loom.fence.yaml. The worker generates
   a random in-memory session, claims its epoch, and verifies the grant
   before every dispatch. Copying the credential or the restored DB does
   not give a second worker the session. A duplicate claimant is denied.
4. For rollback stop the target worker and return its grant. Only then
   activate the source with the original transfer identity. Neither
   handoff command reads the other node's DB, lock or path.

Use remote-handoff.py with the local instance root, action, role,
authority origin, local machine credential file and manifest SHA-256.
Use remote_fence.py --initialize only to create a new authority ledger.
Normal start requires that exact durable ledger. Never restore a stale
authority DB, start a second authority for the same logical node or copy
a running worker's in-memory session. Production transport requires a
validated HTTPS origin, for example a separately configured TLS reverse
proxy in front of the single authority. That deployment was not tested. Plain HTTP is supported for the loopback or
named private verification bridge. The tests used the private bridge.

If the authority becomes unreachable, an already-started job can finish,
but the worker cannot dispatch another. The authority retains its session
across service restart. It never grants a different owner while that
session is held, and does not expire the lease on a timeout. A worker
crash or lost release may require external fencing and operator
reconciliation. The implementation provides no automatic force-release
or offline failover. A separately started worker or restored
authority copy outside this contract requires operator fencing. There is
no distributed consensus or cross-host production test in this result.

The verification ran source and target commands in separate containers
with only their own node filesystem mounted. The authority occupied its
own container and disk binding. An actual CPU Work ran on the source,
another after target activation, a queued Work waited through authority
outage and ran after its recovery, and a final Work ran on the source
after rollback. Stale activation, duplicate worker claims using the same
machine credential and authority reset were denied. The data copies
were independent. All verification processes were stopped.

## Operator command shape

Initialize the authority once with its own mode-0600 policy file. The
policy maps source and target to the SHA-256 digest of separate machine
credentials and a binding object containing machine_id and storage_id.
Keep the plaintext machine token in the respective node's secrets
directory. Install the source worker configuration with authority URL,
epoch 0, binding, and token_path /etc/loom/fence-token before the source
worker runs.

    python3 -B remote_fence.py AUTHORITY_DB POLICY_JSON --initialize
    python3 -B remote_fence.py AUTHORITY_DB POLICY_JSON --port 8080
    python3 -B remote-handoff.py SOURCE seal --role source --authority AUTHORITY_URL --token-file SOURCE_TOKEN --manifest-sha256 ARCHIVE_SHA
    python3 -B remote-handoff.py TARGET activate --role target --authority AUTHORITY_URL --token-file TARGET_TOKEN --manifest-sha256 ARCHIVE_SHA
    python3 -B remote-handoff.py TARGET return --role target --authority AUTHORITY_URL --token-file TARGET_TOKEN --manifest-sha256 ARCHIVE_SHA
    python3 -B remote-handoff.py SOURCE activate --role source --authority AUTHORITY_URL --token-file SOURCE_TOKEN --manifest-sha256 ARCHIVE_SHA

Set OPS_FENCE_TOKEN and OPS_FENCE_NETWORK for the worker Compose override.
Stop the worker before local handoff commands and recreate the service
after local configuration changes. The authority transport network or
HTTPS route is a deployment input.
