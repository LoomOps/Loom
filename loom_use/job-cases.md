# Worked Job and Unit requests

Replace symbolic identities and sample Profile kinds using live discovery. Read the closest case. See [Jobs and Units](jobs.md) and [publication](results.md) for the rules.

## Check the current run

“Is the evaluation still running?”

Confirm the installation, then use the accepted Job ID.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "jobs",
    "arguments": {
      "action": "get",
      "arguments": {"id":"<evaluation-job-id>"}
    }
  }
}
```

Read the active Unit and Attempt with `units.get` when the question concerns execution progress.

## Generate 140 demonstrations, train X-VLA, then evaluate

“Regenerate 140 demos under the accepted recipe, train X-VLA, and evaluate it on the held-out cohort.”

Submit one Job per independent deliverable. The Dataset Job can schedule CPU preparation and use a vectorized generator for its full cohort. Training uses the accepted Dataset alias. Evaluation binds the selected Policy and the pinned held-out Dataset or cohort locator through its own Profile.


```json
{
  "group": "jobs",
  "action": "submit_research",
  "arguments": {
    "key": "grasp-v3-dataset",
    "scenario_ref": {"id": "<task-id>", "revision": 1},
    "owner": "<owner-id>",
    "notify": "<recipient-id>",
    "outcome": {
      "goal": "Generate 140 grasp demonstrations",
      "deliverables": [
        {
          "alias": "dataset",
          "kind": "dataset",
          "from": {
            "unit": "generate",
            "entity": "dataset"
          }
        }
      ]
    },
    "plan": {
      "sealed": true,
      "units": [
        {
          "phase": "prepare",
          "id": "prepare",
          "label": "Prepare",
          "execution": {
            "kind": "grasp.scene.check.r1",
            "params": {}
          }
        },
        {
          "depends_on": [
            "prepare"
          ],
          "id": "generate",
          "label": "Generate",
          "execution": {
            "kind": "grasp.collect.r3",
            "params": {
              "episodes": 140
            },
            "execution_mode": "full"
          }
        }
      ]
    }
  }
}
```


```json
{
  "group": "jobs",
  "action": "submit_research",
  "arguments": {
    "key": "grasp-v3-policy",
    "scenario_ref": {"id": "<task-id>", "revision": 1},
    "owner": "<owner-id>",
    "notify": "<recipient-id>",
    "outcome": {
      "goal": "Train the grasp policy",
      "deliverables": [
        {
          "alias": "policy",
          "kind": "model",
          "from": {
            "unit": "train",
            "entity": "checkpoint"
          }
        }
      ]
    },
    "plan": {
      "sealed": true,
      "units": [
        {
          "id": "train",
          "label": "Train",
          "execution": {
            "kind": "grasp.xvla.train.r4",
            "params": {
              "dataset": {
                "job": "<dataset-job-id>",
                "entity": "dataset"
              }
            },
            "execution_mode": "full",
            "depends_on": [
              "<dataset-job-id>"
            ]
          }
        }
      ]
    }
  }
}
```


```json
{
  "group": "jobs",
  "action": "submit_research",
  "arguments": {
    "key": "grasp-v3-evaluation",
    "scenario_ref": {"id": "<task-id>", "revision": 1},
    "owner": "<owner-id>",
    "notify": "<recipient-id>",
    "outcome": {
      "goal": "Evaluate the grasp policy on the held-out cohort",
      "deliverables": [
        {
          "alias": "evaluation",
          "kind": "experiment",
          "from": {
            "unit": "evaluate",
            "entity": "evaluation"
          }
        }
      ]
    },
    "plan": {
      "sealed": true,
      "units": [
        {
          "id": "evaluate",
          "label": "Evaluate",
          "execution": {
            "kind": "grasp.closed-loop.eval.r2",
            "params": {
              "policy": {
                "job": "<policy-job-id>",
                "entity": "policy"
              }
            },
            "execution_mode": "full",
            "depends_on": [
              "<policy-job-id>"
            ]
          }
        }
      ]
    }
  }
}
```


## Set up a reusable Observer

“Get the RGB-D Observer ready for later trials.”

Submit a standalone Unit. Register the validated Observer in the software registry. Optional output evidence is Reference. A later Job pins the exact Observer revision.


```json
{
  "group": "units",
  "action": "submit",
  "arguments": {
    "key": "observer-rgbd-preflight",
    "owner": "<owner-id>",
    "notify": "<recipient-id>",
    "kind": "observer.validate.r1",
    "params": {}
  }
}
```


## Compare checkpoints under matched conditions

“Check the candidate checkpoints on the same validation run and pick the best supported one.”

Use an Experiment Job whose producer evaluates the pinned checkpoint set on one frozen cohort and reports each checkpoint's outcomes. Selection by success rate requires identical, complete development or validation cohorts. A tie leaves the selection unset. To compare completed runs, read their exact published results.

## Keep preparing while training

“While training runs, start getting the next two evaluations ready.”

Use separate CPU preparation Units within those evaluation Jobs. Gate their GPU Units on their preparation and Policy inputs. Set shared or exclusive GPU access using the actual resource requirements. Read both queues while execution proceeds.

## Resume interrupted generation

“Finish the missing demonstrations from the interrupted run.”

Inspect the original Unit and exact Attempt receipts. Verify the checkpoint manifest against the accepted seeds, episode IDs and hashes. Recover that Unit using the advertised recovery contract. If successful execution has returned and publication alone failed, repair publication from the preserved Attempt.
