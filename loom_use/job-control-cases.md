# Job control and follow-up walkthroughs

These HTTP examples start with an existing Job, Attempt or published Policy. The IDs are illustrative. [Jobs](jobs.md) and [recovery](recovery.md) describe current scopes and state transitions.

## "The client timed out. Did the training request go through?"

The agent retained the complete original request and its key. Replay that exact request. This is the stored request from this example.

```json
{
  "group": "jobs",
  "action": "submit_research",
  "arguments": {
    "key": "grasp-rgb-5000",
    "scenario_ref": {"id": "<task-id>", "revision": 1},
    "owner": "owner",
    "notify": "owner",
    "category": "fine_tuning",
    "outcome": {
      "goal": "Train the requested 5,000-update policy",
      "deliverables": [
        {
          "alias": "policy",
          "kind": "model",
          "from": {
            "unit": "run",
            "entity": "checkpoint"
          }
        }
      ]
    },
    "plan": {
      "sealed": true,
      "units": [
        {
          "id": "run",
          "label": "Run",
          "execution": {
            "kind": "grasp.rgb.train.r4",
            "params": {
              "dataset": {
                "artifact": {
                  "id": "grasp-train-split",
                  "revision": 2
                }
              },
              "max_updates": 5000
            },
            "execution_mode": "full"
          }
        }
      ]
    }
  }
}
```

The response identifies the existing accepted Job or the single acceptance if the first request never arrived. Read its state using the returned ID.

## "Why hasn't the evaluation started?"

Read the exact evaluation Job and its prerequisite.

```json
{"group":"jobs","action":"get","arguments":{"id":"a3000000000000000000000000000001"}}
```

The evaluation Unit’s `scheduling.waiting_reason` points to a dependency. Check the training Job and its published checkpoint. Tell the user the evaluation is waiting for that checkpoint. The existing queue remains the source for this execution.

## "Keep the queued training from starting until I check the input."

The instruction identifies a waiting Job. Confirm its pending stage and hold it.

```json
{"group":"jobs","action":"hold","arguments":{"id":"a3000000000000000000000000000002"}}
```

Read the queue and verify the hold. When the user says “The input is correct, let it start,” release the hold on the same ID.

```json
{"group":"jobs","action":"resume","arguments":{"id":"a3000000000000000000000000000002"}}
```

The existing execution keeps its queue position and eligibility depends on its prerequisites and lane.

Job holds and direct Unit holds are independent. Releasing one does not change
the other. Hold a Job to let its active work finish and prevent later Units from
starting. Inspect the exact Unit to distinguish an inherited hold from a direct hold.

## "Pause training at a checkpoint and later continue it."

Discover `units.pause` and `units.resume_checkpoint` on the connected server.
Read the Unit and pass its exact current Attempt with a reason. Use the pause
action only for a Profile that supports checkpoint pause. Wait until the Unit
reports `paused` and the executor has returned its resources. The resume action
takes the paused Attempt. It queues a new Attempt on the same Unit with the
retained checkpoint. Ordinary queue resume releases a hold. For an unsupported
Profile, a Job hold lets the current Unit finish and stops later Units.

## "Run the waiting evaluation before my other waiting work."

Read the current queue and identify the exact waiting evaluation Unit and reference Unit.

```json
{"group":"jobs","action":"queue","arguments":{}}
```

```json
{"group":"units","action":"reorder","arguments":{"id":"a3000000000000000000000000000003","position":"before","reference":"a3000000000000000000000000000004"}}
```

Read the updated order and any dependencies. The order determines priority when both Units are eligible.

## "Stop the training that's running now."

Confirm the exact active Job and Attempt, then submit cancellation.

```json
{"group":"jobs","action":"cancel","arguments":{"id":"a3000000000000000000000000000005"}}
```

Check the public status and execution receipts until the process and its resource allocation have returned. Report any checkpoint that is already committed and independently verified.

## "Fix the input mount and continue my failed run."

The process returned and the Job is awaiting recovery. The approved input revision and Profile remain usable after the mount is repaired. Claim this Job's recovery reservation.

```json
{"group":"recovery","action":"claim","arguments":{"id":"a3000000000000000000000000000006"}}
```

Use the lease returned by the claim. In this example the repair is external to the execution parameters, so the patch is empty.

If `system.describe` advertises `preview` on `recovery.resume`, validate this
request with `preview: true` first. Confirm the exact candidate and then send the
execution request below. For code or Runtime startup failures, also verify the
repaired entry point as described in [recovery](recovery.md).

```json
{"group":"recovery","action":"resume","arguments":{"id":"a3000000000000000000000000000006","lease":"example-lease","patch":{},"reason":"Verified the pinned input at its repaired mount"}}
```

Read the same Job as it starts a new Attempt and retain the previous Attempt's failure evidence.

## "Training finished, but the Policy isn't in the catalog. Recover it."

The Job is `finalizing_failed`. Its run receipt confirms resource return, and the retained manifest and checkpoint are complete. After repairing the publication fault, verify the same Attempt and request reindexing.

```json
{"group":"results","action":"reindex","arguments":{"id":"a3000000000000000000000000000007","attempt":"b3000000000000000000000000000007"}}
```

Read the same Job and its published Policy. The training Attempt remains the producer of the checkpoint.

## "Check which of my three saved checkpoints works best on the agreed validation cohort."

The catalog contains all three checkpoints but no matched Experiment. Inspect a sweep Profile that accepts exact checkpoint revisions, uses one simulator allocation, and uses the built-in best-success-rate selection against validation cases. The Profile below exposes these settings as inputs.

```json
{
  "group": "jobs",
  "action": "submit_research",
  "arguments": {
    "key": "xvla-matched-checkpoint-sweep",
    "scenario_ref": {"id": "<task-id>", "revision": 1},
    "owner": "owner",
    "notify": "owner",
    "category": "evaluation",
    "outcome": {
      "goal": "Select the best saved checkpoint using matched resets",
      "deliverables": [
        {
          "alias": "evaluation",
          "kind": "experiment",
          "from": {
            "unit": "run",
            "entity": "sweep"
          }
        }
      ]
    },
    "plan": {
      "sealed": true,
      "units": [
        {
          "id": "run",
          "label": "Run",
          "execution": {
            "kind": "grasp.checkpoint-sweep.r2",
            "params": {
              "checkpoints": [
                {
                  "artifact": {
                    "id": "grasp-xvla-step5000",
                    "revision": 1
                  }
                },
                {
                  "artifact": {
                    "id": "grasp-xvla-step10000",
                    "revision": 1
                  }
                },
                {
                  "artifact": {
                    "id": "grasp-xvla-step20000",
                    "revision": 1
                  }
                }
              ],
              "validation_dataset": {
                "artifact": {
                  "id": "grasp-validation",
                  "revision": 2
                }
              },
              "checkpoint_selection": {
                "split": "validation",
                "rule": "best_success_rate",
                "selected_step": null
              }
            },
            "execution_mode": "full"
          }
        }
      ]
    }
  }
}
```

Read the Experiment's per-checkpoint evidence, cohort and selection rule. The accepted Job delivers one Experiment. With matched valid cases, a unique winner becomes the representative checkpoint. A tie leaves the matching best candidates visible and the representative unset.
