# Task guide index

Call `system.status` to confirm the target, principal and admission state. `system.guide` returns this binary's document index; request one guide by `id`. `system.describe` with a group returns its action index; with group and action it returns the exact input schema, scope and example.

| Task | Guide ID |
|---|---|
| Submit a first Job | `quickstart` |
| Submission, queue, Attempts and output storage | `jobs` |
| Internal Steps, workflows and consolidation | `workflows` |
| Reuse/create Components, Tools and Profiles | `components` |
| Rust SDK, package and build receipts | `authoring` |
| Runtime selection and dependency installation | `runtimes` |
| Result publication, lineage and reconnect | `results` |
| Search Items, registered files and log streams | `items` |
| Bounded asset reconciliation | `reconciliation` |
| Events and error recovery | `recovery` |
| Task/protocol context | `scenarios` |
| Server administration / version updates | `operator` / `updates` |
| What the bundle contains | `inventory` |

Use the IDs returned by this server's index; older engines may expose fewer topics.

MCP exposes jobs, recovery, events, components, results and system at `/mcp`. HTTP uses the same operations at `POST /api/actions`. A client connects using the URL and authentication method supplied for that installation; setup belongs to the independent [Loom-Use installer](https://github.com/LoomOps/Loom-Use/blob/main/INSTALL.md).

The response's onboarding identity distinguishes contract and guide changes. Refresh affected schemas/guides when it changes; keep accepted Job IDs, original request keys and execution pins. New documentation does not prove that another installation has the same features. No resident agent hierarchy is required.

For a new scientific producer or Profile, read `metadata` and `reporting` before
submission. Reuse a suitable contract, justify inapplicable requirements with
evidence, and register missing analysis coverage rather than omit it.
