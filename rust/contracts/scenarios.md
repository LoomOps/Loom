# Scenario context

A Scenario is versioned task/protocol context, not a scheduler or Profile. It can connect datasets, models, experiments and software involved in the same task without changing their identities.

## Create and pin

Read the exact schemas for `results.register_scenario` and `jobs.submit` (or `submit_workflow`). Create a definition with `label` and optional `description`, `protocol`, `references` and `metadata`, using a stable key. Retain the returned ID, revision and digest. Submit `scenario_ref: {"id":"<scenario-id>","revision":1}` when the user's task has explicit known context. Unassigned records remain valid.

`results.update_scenario` appends a definition revision using `expected_revision` and a reason. Existing accepted Jobs retain their original reference. `archive_scenario` changes discovery; exact historical context remains readable. A stale revision or changed key/request is a conflict.

## Link records

Use `results.link_scenario` with an exact Scenario revision, typed target, relation and provenance. Targets include Jobs, native entities, full historical selectors and software revisions. Read `scenario_graph` with pagination to inspect these declarations. `supersede_scenario_link` records a correction while preserving the original link.

A context link does not run a Job, create another dataset/model/experiment, grant file access or alter dependency readiness. It does not prove scientific acceptance. Keep ambiguous historical context explicit rather than inferring a protocol from a name.
