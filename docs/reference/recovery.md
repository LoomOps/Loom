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

Claim a failed Job awaiting a response. Pass the returned lease to progress/resume.

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

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Recovery · state, rounds, agent, deadline, hard_deadline, lease

Waiting deadline expired or another agent already claimed recovery.

## progress

Report progress with a valid lease to refresh the idle deadline, without extending the round's hard deadline.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `lease` | yes | `"string"` | `—` |
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
  "id": "<job-id>",
  "lease": "<recovery-lease>"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Recovery · state, rounds, agent, deadline, hard_deadline

Lease mismatch, claimant mismatch or expired recovery deadline.

## resume

Schedule recovery at the reserved queue position using a valid lease. Inspect the live patch schema; finalization-only repair preserves the successful execution Attempt.

Scope: `submit`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/jobs.rs#recover](../../rust/crates/core/src/jobs.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `id` | yes | `"string"` | `—` |
| `lease` | yes | `"string"` | `—` |
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
  "id": "<job-id>",
  "lease": "<recovery-lease>",
  "patch": {},
  "reason": "Rerun after repairing the cause"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Recovery · state=pending, rounds, agent

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
  "id": "<job-id>"
}
```

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

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

Illustrative arguments. Replace symbolic identities, hashes and values with discovered records; check handler preconditions before a mutation.

### Result and failures

Recovery · state=waiting, deadline

Job is not deferred.
