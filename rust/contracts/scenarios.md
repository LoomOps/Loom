# Research Task context

A Research Task is versioned task/protocol context, not a scheduler or Profile. It can connect datasets, models, experiments and software involved in the same task without changing their identities.

## Create and pin

Read the exact schemas for `results.register_scenario` and `jobs.submit` (or `submit_workflow`). Create a definition with `label` and optional `description`, `protocol`, `references` and `metadata`, using a stable key. Retain the returned ID, revision and digest. Submit `scenario_ref: {"id":"<scenario-id>","revision":1}` when the user's task has explicit known context. Unassigned records remain valid.

`results.update_scenario` appends a definition revision using `expected_revision` and a reason. Existing accepted Jobs retain their original reference. `archive_scenario` changes discovery; exact historical context remains readable. A stale revision or changed key/request is a conflict.

## Link records

Use `results.link_scenario` with an exact Task revision, typed target, relation and provenance. Targets include Jobs, native entities, full historical selectors and software revisions. Read `scenario_graph` with pagination to inspect these declarations. `supersede_scenario_link` records a correction while preserving the original link.

A context link does not run a Job, create another dataset/model/experiment, grant file access or alter dependency readiness. It does not prove scientific acceptance. Keep ambiguous historical context explicit rather than inferring a protocol from a name.


## Dashboard grouping

The UI calls this context **Task**. A Task describes a shared research goal, not
one execution, one dataset, or an external coordinator task. Store baseline
Reference demos in definition metadata/references; keep version-specific
conditions, splits and result identities separate. Sibling versions and planner
extensions can share one Task when provenance supports that relationship.
Do not group unrelated scenes merely because they cite the same model/demo.

The compatible API names remain scenarios, scenario_ref and link_scenario.
Dashboard records expose task_refs (id, revision, label), including exact
additive links even when the Task is outside the current catalog page.
dashboard_page/search_items task_id filters span Task revisions. The exact\nscenario_ref filter selects one revision. Both match accepted pins and active
typed links. An explicitly linked historical snapshot remains selectable even
when another snapshot is newer; link corrections expire context-filtered cursors.
Unassigned records remain visible in All tasks. The Dashboard has no standalone
Scenario navigation tab; Tasks are filters and links on research lists.
