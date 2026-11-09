# Local diagnostics

Loom writes `loom-diagnostic/v1` JSONL into `<instance>/diagnostics` (normally
`/loom/ops/diagnostics`; dev builds use their own instance directory). Ops,
Dashboard and Gateway share that private bind mount at `/var/lib/loom-diagnostics`.
The directory is owned by the instance service UID/GID, mode 0700; files are 0600.
The installer creates it and the Compose entry script prepares it for upgrades.
The base Dashboard runs with that same instance identity. No host-wide chmod is used.

Each record includes an incident UUID, UTC epoch timestamp, component, operation,
allowlisted error code, build identity when available, exception class/errno and
up to 12 code frames (basename/function/line only). It does not serialize exception
messages, source lines, locals, tokens, headers, emails, bodies, URLs or task data.
Do not pass user input as component, operation, code or build.

The sink uses a nonblocking process lock and a thread lock, suppresses identical
categories for 30 seconds per process, and rotates at 64 MiB with fifteen backups
(about 1 GiB total). It is best-effort incident evidence, not an exhaustive audit
log. Disk-full, permission, symlink and contention failures return without blocking
Jobs or changing their outcome. Existing durable Job/events and enrollment journals
remain authoritative. HTTP/MCP unexpected errors, invalid MCP calls, Sharing
reconciliation, public enrollment errors and uncaught process/thread/handler failures
are collected. Python process kills, browser-side exceptions and GPU driver crashes
are not claimed as captured by this collector.

Loom Use keeps the same envelope under `$CODEX_HOME/loom-use/diagnostics` on the
client machine, including CLI failures and pending runtime refresh. It does not
need access to the server filesystem. Operators can collect these files explicitly;
there is no automatic upload or network sender. A future development-server sender
can consume this versioned envelope separately, with explicit configuration and a
persistent delivery cursor, without entering the Job execution path.

Tests cover safe fields, retention, contention, concurrent writes, symlink refusal
and fail-open disk errors. Validate actual container permissions separately after a
Compose upgrade. Do not use diagnostics presence as a Job admission gate.

Set `LOOM_DIAGNOSTICS_SEGMENT_MIB` and `LOOM_DIAGNOSTICS_BACKUPS` to adjust retention.
For Compose, put them in the instance `override.env` and recreate the services.
Invalid values fall back to defaults. No disk space is preallocated.

MCP tool errors expose `structuredContent: {error: "tool_error", incident_id}`
in addition to the existing text and version metadata. The legacy top-level
`incident_id` remains for raw clients. Loom Use forwards the response unchanged.
External consumers should validate the UUID before relaying it across their own
API/MCP boundaries. No error body is needed for correlation. An absent ID is valid
when diagnostics are unavailable or suppressed; it never changes execution policy.
