# Catalog request walkthroughs

Identify the durable records implied by the requested outcome. The agent selects the registered templates, report contracts and evidence links; Loom's finalization publishes accepted Job outputs. An agent can prepare the requested Dataset, deliver a trained Policy or complete an evaluation without a second instruction to publish it. Questions about existing records start with reads. These are HTTP action envelopes. For MCP, use the group as the tool name and pass `action` and `arguments` as shown in [Job request walkthroughs](job-cases.md). The quoted requests use sample project records. Copy IDs, revisions, hashes and locators from the connected installation.

## "I restored the grasp dataset to data/1. Make that copy available under the existing record."

Look up the exact Dataset.

```json
{"group":"results","action":"search_items","arguments":{"kinds":["dataset"],"text":"Grasp RGB-D v3","view":"all","role":"all","limit":20}}
```

From the matching hit, take `native_ref.identity`. For this native record, read its components and current locations.

```json
{"group":"results","action":"get","arguments":{"id":"grasp-rgbd","revision":3}}
```

Suppose the relevant component has `ref:"manifest.json"`. Use that original ref as `component` and the restored path relative to the registered store as `ref`.

```json
{"group":"results","action":"reconnect","arguments":{"id":"grasp-rgbd","revision":3,"component":"manifest.json","ref":"restored/grasp-rgbd/manifest.json","storage":"data/1"}}
```

Verify `status:"available"` and read revision 3 again to confirm the location. The action verifies the original hash and size. Repeat for other required components as needed. For a historical search hit, use `results.bind_historical_location` with the exact historical selector, following [reconciliation](reconciliation.md).

## "That held-out dataset already exists. Record that we selected its 20 episodes from revision 3 of Grasp RGB-D."

Read both exact revisions with `results.get`. Inspect actual members and the registered evidence component. The two records share a Task, and the membership comparison establishes a derivation. The example assumes a registered digest from `manifest.json`; copy the actual value from the record.

```json
{
  "group":"results","action":"link_dataset","arguments":{
    "target":{"kind":"entity","identity":{"id":"grasp-heldout","revision":1}},
    "parent":{"kind":"entity","identity":{"id":"grasp-rgbd","revision":3}},
    "transformation":{"kind":"subset","details":"Selected 20 held-out episodes","member_count":20},
    "evidence":[{"target":{"kind":"entity","identity":{"id":"grasp-rgbd","revision":3}},"component":"manifest.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","pointer":"/members"}],
    "reason":"Record the verified held-out subset",
    "dry_run":true
  }
}
```

The preview returns `stored:false` and a `digest` for this claim. To apply, send the same request with the three fields shown below and preserve the evidence. Replace the example digest with the preview's value. If the source has changed, verify the evidence and preview again.

```json
{
  "group":"results","action":"link_dataset","arguments":{
    "target":{"kind":"entity","identity":{"id":"grasp-heldout","revision":1}},
    "parent":{"kind":"entity","identity":{"id":"grasp-rgbd","revision":3}},
    "transformation":{"kind":"subset","details":"Selected 20 held-out episodes","member_count":20},
    "evidence":[{"target":{"kind":"entity","identity":{"id":"grasp-rgbd","revision":3}},"component":"manifest.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","pointer":"/members"}],
    "reason":"Record the verified held-out subset",
    "dry_run":false,
    "key":"grasp-heldout-source-r1",
    "expected_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  }
}
```

Check `stored:true`, then read the held-out Dataset and confirm `dataset_lineage.parents` includes `grasp-rgbd` at revision 3. If the request is to **produce** the child, submit the transformation Profile with `parent_dataset` bound to that revision. The producer declares the actual input and transformation as part of its output manifest. Publication records both the new Dataset and its parent edge.

## "Bring this external benchmark into our project so we can use it for evaluation."

Confirm the external source ID, release revision, published checksum and recorded metadata. Here the archive has already been staged and is usable without conversion. Search for an existing import and inspect a comparable Dataset and its exact template. In this example the external release leaves the robot and collection details unrecorded. The fixture store has a single verified archive.

Look up the Task with `results.scenarios` and search its Datasets with
`results.search_items` using `task_id`. For example, Square D0, D1 and D2 share
one `Square` Task. Reuse the exact Task and a published variant if it matches.
For a new variant, record its stable path, such as
`metadata.research.organization.path:[{"id":"d1","label":"D1"}]`. Pin the Dataset's
returned ID and revision in downstream contracts.

For a changed split, membership or other scientific content, publish a verified
candidate that consumes the exact Dataset. When the user requested replacement
of an unused Dataset, call `results.adopt_output_revision` with `base`,
`source`, `mode:"replace_unused"`, `authorization` and a stable `key`. A
successful adoption keeps one Current entry and preserves both exact source
records. If the action returns `dataset_in_use`, keep the candidate as a
separate record. Resolve the new exact input before training or evaluation.

Use `metadata.research.organization.handle` for the short name of a Dataset
variant and `metadata.research.organization.tags` for record-level labels.
For matched conditions, use handles such as `D0 · S1-A` and `D0 · S1-B`. Keep
the full Dataset name in `label` and the train, validation and test counts in
the required `metadata.research.collection.split` field.
Dataset cards show the handle above each full name. Policy and Experiment
lists show the handle and details link to the full Dataset name. Tags omit
spaces and Split is a separate property. Use an exact
ID/revision from discovery when binding an input. Update an existing record's
presentation through `results.organize_dataset` or **Edit metadata** in the
Dashboard. Use `properties` for arbitrary descriptive catalog properties,
including provenance notes, responsibility or review information. Use the typed
research contract for scientific properties. Include `path:[]` to remove a
former subgroup heading when moving to a per-record handle. Record augmentation
using an exact Dataset lineage edge with transformation `augment`. The Dataset
card shows the source Dataset for that transformation.

```json
{"group":"results","action":"get_metadata_template","arguments":{"id":"loom.dataset","revision":1}}
```

Use its returned `id`, `revision` and `sha256` for `template_ref`. Supply the granted installation's machine/storage IDs and the archive's store-relative locator. Replace the example template and archive digests with the retrieved values.

```json
{
  "group":"results","action":"reconcile_plan","arguments":{
    "key":"grasp-benchmark-2026-01",
    "proposal":{"schema":"loom-reconcile/v1","operations":[{
      "operation":"import","schema":"loom-asset/v3",
      "source":{"system":"benchmark-release","id":"grasp","revision":"2026.01"},
      "definition":{
        "kind":"dataset","label":"Grasp benchmark · 2026.01",
        "scenario_ref":{"id":"<task-id>","revision":1},
        "primary":{"machine_id":"fixture","storage_id":"fixture-data","ref":"incoming/grasp-2026-01.tar","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
        "metadata":{
          "template_ref":{"id":"loom.dataset","revision":1,"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
          "research":{"robot":{"status":"unrecorded"},"collection":{"status":"unrecorded","split":{"train":"unrecorded","validation":"unrecorded","test":"unrecorded"}}}
        }
      }
    }]}
  }
}
```

Take the returned `plan_id`. Call `results.reconcile_validate` with it and check `state:"validated"`. Then call `results.reconcile_apply` with the same ID. For example, if the first call returns `plan_id:"f1000000000000000000000000000001"`:

```json
{"group":"results","action":"reconcile_validate","arguments":{"plan_id":"f1000000000000000000000000000001"}}
```

```json
{"group":"results","action":"reconcile_apply","arguments":{"plan_id":"f1000000000000000000000000000001"}}
```

Read the exact `id` and `revision` from `receipt.operations[0]` with `results.get`. Confirm `origin.type:"import"`, the source release and the available component. The receipt reports `created_jobs:0`. If several Jobs need a verification or indexing pass, run it as a standalone Unit and bind the same imported Dataset revision in each Job. If the requested work creates a distinct publishable Dataset, use a research Job and record the derivation.

## "Which of my finished closed-loop evaluations did better?"

Search for both with `results.search_items` and `kinds:["experiment"]`. Suppose the selected hits identify the following exact Experiment revisions. Send one read per record.

```json
{"group":"results","action":"get","arguments":{"id":"grasp-a-closed-loop","revision":2}}
```

```json
{"group":"results","action":"get","arguments":{"id":"grasp-b-closed-loop","revision":1}}
```

Compare protocol, reset cohort, exact Policy and Dataset revisions, valid cases, invalid cases and denominators. Report differences in conditions that make the scores incomparable. Comparison reads those exact records. It requires no catalog mutation. A newly scheduled matched checkpoint sweep can publish an Experiment using the checkpoint-evaluation reporting contract. See [scientific reporting](reporting.md).

## Catalog entries inferred from the requested result

| User request | Calls and resulting records |
|---|---|
| "Generate 140 more grasp episodes." | The agent selects the Dataset template and reporting contract when planning generation. Loom publishes the verified cohort as the Job's Dataset output with membership, splits, observations and variation. Read its catalog ID and revision after finalization. |
| "Train a grasp policy on those episodes." | The agent declares the trained Policy as wire kind `model` and binds the exact Dataset and reporting contract. Finalization links the selected checkpoint, conditioning, action representation and available training reports to the Policy. The training report remains evidence for that Policy. |
| "Build a contact observer we can reuse." | The request calls for reusable software. The agent selects `components.register_tool` for a callable observer or `components.register` for a lifecycle adapter, and links its weights and validation evidence to the software revision. For an observer internal to one execution, keep the code and weights with that execution unless reusable software is an intended result. |
| "Run our evaluation on the simulator version it was designed for." | Discover and reuse a compatible Runtime. When provisioning is part of the work, record the installed Dependencies and bind the validated Runtime revision to the relevant Profile stage. See [Runtimes](runtimes.md). |
| "Can we restore missing metrics for this old run?" | Look up the exact revision and original evidence. Where evidence supports an additive correction, preview and apply through the operation in [metadata](metadata.md) or [reconciliation](reconciliation.md). Keep unrecorded measurements explicit. |
