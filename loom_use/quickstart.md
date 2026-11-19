# First Unit

This walkthrough runs a small CPU check using the existing `cpu.smoke` Profile. Use it when the requested work includes a smoke test. For a research deliverable, see [worked Job requests](job-cases.md).

## 1. Select the target and recipe

Call `system.status` to confirm the target, principal and admission state. Use `components.profiles` to find `cpu.smoke`. `components.inspect_profile` checks parameters without reserving resources.

The built-in `cpu.smoke` Profile, when advertised, provides a bounded platform test with `params: {"seconds":0,"fail":false}`. It is not a scientific experiment. Other Profiles have different parameters; use their actual contract.

## 2. Submit

Fetch `system.describe` with `{"group":"units","action":"submit"}`. An MCP `tools/call` request wraps an action like this:

```json
{
  "jsonrpc":"2.0", "id":1, "method":"tools/call",
  "params":{
    "name":"units",
    "arguments":{
      "action":"submit",
      "arguments":{
        "key":"first-platform-check-001",
        "kind":"cpu.smoke",
        "owner":"<principal-id>",
        "notify":"<recipient-id>",
        "params":{"seconds":0,"fail":false},
        "execution_mode":"smoke",
        "presentation":{"scope":"operations","role":"test"}
      }
    }
  }
}
```

Replace the principal/recipient IDs with permitted identities and confirm the Profile exists. For a research deliverable, use [the research plan examples](job-cases.md). The response JSON is in `result.content[0].text`; check `result.isError` before reading it. Retain the returned `id` and original request.

If the response is lost, replay the same key and original arguments. A changed request under the same key conflicts. A different key would create another execution.

Installation administrators submitting installation/platform acceptance checks
use `owner:"system"` at submission and keep `notify` as the actual recipient.
System attribution requires administrator authority and hides the check in the
Dashboard while retaining MCP/API evidence. Using the built-in Profile alone does
not make a user's Job System-owned; user research/preparation remains user work.

## 3. Observe and inspect outputs

Call `units.get({id})`. `status` is the public phase; `state` retains the native value (`complete` corresponds to `completed`). Scheduling and recovery are separate objects. Continue legacy `jobs.logs` using `next_offset`, or use `jobs.log_streams/read_log` for exact source generations.

After publication, use `results.get` with the exact artifact ID and revision. Read `events.list({kind:"all"})` for notifications. ACK records delivery. The Unit continues if the client disconnects. Use [recovery](recovery.md) for a failed execution or publication.

## HTTP equivalent

Send `POST /api/actions` with `{"group":"units","action":"submit","arguments":...}`. The operation returns JSON directly; inspect HTTP status/error body. Use the installation's configured Tailscale identity or scoped Bearer credential. Browser cookies and forged identity headers are not client authentication.

Next: `jobs` for queue/storage, `components` for reusable recipes, `workflows` for multiple stages, and `items` for files/logs.
