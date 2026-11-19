# Job input walkthroughs

These examples show how an agent turns a user request into an exact submission, or asks for a missing decision. HTTP action envelopes use illustrative IDs, Profile names and response values. Use the connected server's [submission guidance](jobs.md#resolve-submission-inputs) and live schemas.

## "Run yesterday's RGB evaluation again with the same settings."

The conversation identifies the prior evaluation by its Job ID. Read the accepted request and outputs.

```json
{"group":"jobs","action":"get","arguments":{"id":"a2000000000000000000000000000001"}}
```

The Job's publishing Unit identifies the evaluation Profile, Policy and test Dataset revisions and the seeded protocol. Confirm that the same Profile pin, input files and protocol are still available. Inspect the actual parameters. Submit another execution with a new key and those exact inputs.

```json
{
  "group": "jobs",
  "action": "submit_research",
  "arguments": {
    "key": "rgb-eval-replicate-02",
    "scenario_ref": {"id": "<task-id>", "revision": 1},
    "owner": "owner",
    "notify": "owner",
    "category": "evaluation",
    "outcome": {
      "goal": "Replicate the fixed RGB evaluation",
      "deliverables": [
        {
          "alias": "evaluation",
          "kind": "experiment",
          "from": {
            "unit": "run",
            "entity": "score"
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
            "kind": "grasp.rgb.evaluate.r3",
            "params": {
              "policy": {
                "artifact": {
                  "id": "grasp-rgb-policy",
                  "revision": 3
                }
              },
              "test_dataset": {
                "artifact": {
                  "id": "grasp-heldout",
                  "revision": 2
                }
              },
              "protocol": "closed-loop-test20-r2",
              "seed": 42000
            },
            "execution_mode": "full"
          }
        }
      ]
    }
  }
}
```

Retain the new Job ID and compare the two published Experiment revisions under the recorded protocol.

## "Train on the same split as last time, but stop at 5,000 updates."

The preceding Job pins the Dataset revision and used 20,000 updates. The current request changes the training budget. The Profile exposes `max_updates` and already pins the checkpoint selection schedule appropriate to that budget. Confirm the proposed combination with `components.inspect_profile` before submitting.

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

The new Policy's training report records its actual update count. The earlier Job remains the source for the reused split.

## "Train on the S1 data."

The conversation uses “S1” for two generations of the same task. Search the catalog and open the candidate Dataset revisions.

```json
{"group":"results","action":"search_items","arguments":{"text":"S1","kinds":["dataset"],"view":"all","role":"all","limit":20}}
```

Two compatible hits remain and selecting either would change the training cohort. Ask “Which should I train on, S1v1 r2 with the original starting conditions, or S1v2 r1 with the wider conditions?” The queue receives zero new Jobs while the choice is open. After the reply, pass the selected exact Dataset revision to the inspected training Profile.

## "Evaluate the new policy."

The conversation pins the Policy, but the Task contains both a simulation acceptance protocol and a real-robot protocol. Read the Task revision and find the recorded evaluation conditions. The choice changes the environment, physical operation and primary outcome.

```json
{"group":"results","action":"get_scenario","arguments":{"id":"grasp-and-align","revision":3}}
```

Ask “Should I use the simulation test suite with the existing held-out resets, or the real-robot protocol?” Continue with any available read-only checks while waiting. Build the evaluation submission after the user confirms the intended protocol.

## "Use the exact recipe from the previous run."

The earlier accepted Job pins an RGB-only Profile revision. Inspect the current candidate from the same family. Its input contract requires depth and its conditioning differs.

```json
{"group":"components","action":"inspect_profile","arguments":{"kind":"grasp.rgbd.train.r6"}}
```

The requested recipe is currently unavailable. Ask “The available r6 recipe requires depth and changes the inputs. Should I wait for the original RGB recipe or switch this run to RGB-D?” Inspect the chosen Profile and confirm its pins and input revisions before submission.

## "Show me a quick sample from the agreed generator."

The review-only Profile pins the admitted generator and camera settings. Its live default is `target_count:2`. Inspect an empty candidate parameter object and use the valid draft and its defaults.

```json
{"group":"components","action":"inspect_profile","arguments":{"kind":"grasp.preview.r2","params":{}}}
```

```json
{
  "group": "units",
  "action": "submit",
  "arguments": {
    "key": "grasp-review-sample",
    "kind": "grasp.preview.r2",
    "owner": "owner",
    "notify": "owner",
    "category": "sdg",
    "execution_mode": "pilot",
    "params": {
      "target_count": 2
    },
    "pilot": {
      "preflight_for": "Agreed 140-episode generation",
      "reduced_scope": "Two preview episodes using the accepted generator and cameras"
    }
  }
}
```

Report the observed sample and its receipt. The pinned default resolves the sample count for this bounded preview.

## "Test whether wider starts improve success on the held-out cohort."

The conversation pins a matched baseline, a wider-start Policy, the held-out Dataset and a primary success rate. The matched evaluation Profile accepts both policy revisions. Read the template and report contract, then preview the commitments. Substitute the actual template and report hashes read from the server.

```json
{
  "group":"jobs","action":"preview_outcome","arguments":{
    "outcome":{
      "goal":"Compare wider starts against the pinned baseline",
      "deliverables":[{"alias":"comparison","entity":"score","kind":"experiment"}],
      "science":{
        "deliverables":{"comparison":{
          "activity":"evaluation",
          "template_ref":{"id":"loom.experiment.evaluation","revision":1,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
          "contract_ref":{"id":"loom.report.evaluation","revision":1,"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
          "inputs":{"test_dataset":{"id":"grasp-heldout","revision":2}}
        }},
        "hypothesis":{
          "statement":"Wider starts improve held-out grasp success",
          "analysis":"Compare success under matched resets using both pinned policies",
          "population":"Held-out grasp cohort revision 2",
          "metric":"success_rate",
          "aliases":["comparison"]
        }
      }
    }
  }
}
```

The preview identifies the required reports. Map the accepted deliverable to a publishing Unit in `jobs.submit_research`, bind both accepted Policy revisions and the exact test Dataset in the Profile's input fields, and keep the analysis evidence with the published Experiment. If the request instead leaves two materially different primary metrics open, resolve the metric before committing a scientific outcome.

## "Use the new partner release and reproduce the baseline."

Discover the catalog item and the configured stores. The supplied release name has no matching local catalog entry and the conversation supplies neither a release revision nor a readable locator.

```json
{"group":"results","action":"search_items","arguments":{"text":"partner grasp release","kinds":["dataset"],"view":"all","role":"all","limit":20}}
```

Ask “Which release revision should I use, and where can Loom read it?” After the reply, verify the exact bytes and follow the [external Dataset import walkthrough](catalog-cases.md). Submit a baseline Job after binding the imported revision. This request holds submission until the Dataset can be identified.
