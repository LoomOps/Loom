# First Job

This walkthrough uses an existing Profile. It does not install software or change the server. Choose a small workload only when execution is part of the user's request.

## 1. Select the target and recipe

Call `system.status`, then `components.profiles`. Confirm the target, principal/scopes and admission state. Select a compatible exact `kind` from discovery and inspect its params/input/output contract. `components.inspect_profile` can check a proposed parameter object without reserving resources or submitting a Job.

The built-in `cpu.smoke` Profile, when advertised, provides a bounded platform test with `params: {"seconds":0,"fail":false}`. It is not a scientific experiment. Other Profiles have different parameters; use their actual contract.

## 2. Submit

Fetch `system.describe` with `{"group":"jobs","action":"submit"}`. An MCP `tools/call` request wraps an action like this:

```json
{
  "jsonrpc":"2.0", "id":1, "method":"tools/call",
  "params":{
    "name":"jobs",
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

Replace the principal/recipient IDs with permitted identities and confirm the Profile exists. For research, use the requested Profile, parameters, category and presentation instead of copying platform-test labels. The response JSON is in `result.content[0].text`; check `result.isError` before reading it. Retain the returned `id` and original request.

If the response is lost, replay the same key and original arguments. A changed request under the same key conflicts. A different key would create another execution.

## 3. Observe and inspect outputs

Call `jobs.get({id})`. `status` is the public phase; `state` retains the native value (`complete` corresponds to `completed`). Scheduling and recovery are separate objects. Continue legacy `jobs.logs` using `next_offset`, or use `jobs.log_streams/read_log` for exact source generations.

After publication, inspect the Job's result and use `results.get` with an exact returned artifact identity/revision. Read `events.list({kind:"all"})`; ACK records delivery only. A client disconnect does not stop a server Job. If execution or indexing fails, follow `recovery` or `results` rather than creating another Job blindly.

## HTTP equivalent

Send `POST /api/actions` with `{"group":"jobs","action":"submit","arguments":...}`. The operation returns JSON directly; inspect HTTP status/error body. Use the installation's configured Tailscale identity or scoped Bearer credential. Browser cookies and forged identity headers are not client authentication.

Next: `jobs` for queue/storage, `components` for reusable recipes, `workflows` for multiple stages, and `items` for files/logs.
