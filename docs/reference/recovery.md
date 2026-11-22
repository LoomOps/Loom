# recovery

Bounded incident repair

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [claim](#claim)
- [progress](#progress)
- [resume](#resume)
- [defer](#defer)
- [reactivate](#reactivate)

## claim

Claim the failed execution Unit awaiting a response. Find its ID in jobs.get units and pass the returned lease to progress/resume.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `lease` | no | `"string"` | `—` |
| `patch` | no | `"object"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "lease": {
      "type": "string"
    },
    "patch": {
      "type": "object"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<unit-id>"
}
```



### Result and failures

Recovery · state, rounds, agent, deadline, hard_deadline, lease. Includes current and prior troubleshooting records for the selected execution.

A research Job ID returns recovery_unit_required. The waiting deadline can expire or another agent may already hold the Unit lease.

## progress

Report progress with a valid lease to refresh the idle deadline, without extending the round's hard deadline. An optional finding records a verified cause or repair check on the failed Attempt. Omit credentials, raw logs and private paths.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `finding` | no | `"string"` | `—` |
| `id` | yes | `"string"` | `—` |
| `lease` | yes | `"string"` | `—` |
| `patch` | no | `"object"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "finding": {
      "maxLength": 1000,
      "minLength": 1,
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "lease": {
      "type": "string"
    },
    "patch": {
      "type": "object"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "lease"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<unit-id>",
  "lease": "<recovery-lease>"
}
```



### Result and failures

Recovery · state, rounds, agent, deadline, hard_deadline, troubleshooting

Lease mismatch, claimant mismatch or expired recovery deadline.

## resume

Schedule recovery at the reserved queue position using a valid lease. Set preview to true to validate the lease and repair patch without running the Profile or scheduling work. Finalization-only repair preserves the successful execution Attempt.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `lease` | yes | `"string"` | `—` |
| `patch` | no | `"object"` | `—` |
| `preview` | no | `"boolean"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "lease": {
      "type": "string"
    },
    "patch": {
      "type": "object"
    },
    "preview": {
      "type": "boolean"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "lease"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<unit-id>",
  "lease": "<recovery-lease>",
  "patch": {},
  "reason": "Rerun after repairing the cause"
}
```



### Result and failures

Recovery receipt with state=pending, rounds and agent. A preview returns current state, repair mode, failed Attempt, deadlines and candidate Profile, Runtime and storage references without consuming the lease or a recovery round.

Lease mismatch, three recovery rounds exhausted or invalid patched inputs. For finalization-only failure, retry postprocessing without a patch.

## defer

Release the current queue reservation. Dependent Jobs wait while independent Jobs proceed.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `lease` | no | `"string"` | `—` |
| `patch` | no | `"object"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "lease": {
      "type": "string"
    },
    "patch": {
      "type": "object"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<unit-id>"
}
```



### Result and failures

Recovery · state=deferred

No valid reservation. Another user's reservation can be explicitly released; the acting user is recorded.

## reactivate

Return a deferred Job to a new queue position without resetting recovery rounds.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `lease` | no | `"string"` | `—` |
| `patch` | no | `"object"` | `—` |
| `reason` | no | `"string"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "lease": {
      "type": "string"
    },
    "patch": {
      "type": "object"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "id": "<job-id>"
}
```



### Result and failures

Recovery · state=waiting, deadline

Job is not deferred.
