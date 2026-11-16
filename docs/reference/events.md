# events

Recipient lifecycle events

The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.

## Actions

- [list](#list)
- [ack](#ack)

## list

Read up to 100 unacknowledged recipient events in seq order. Use kind=all to include completion.

Scope: `events`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/query.rs#event_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `after` | no | `"integer"` | `—` |
| `kind` | no | `["errors", "lifecycle", "all"]` | `"errors"` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "after": {
      "minimum": 0,
      "type": "integer"
    },
    "kind": {
      "default": "errors",
      "enum": [
        "errors",
        "lifecycle",
        "all"
      ],
      "type": "string"
    }
  },
  "required": [],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "after": 0,
  "kind": "all"
}
```



### Result and failures

Event[] · seq, job, code, kind, ack, delivery_state

Unsupported kind or invalid cursor.

## ack

Record receipt of an event. This does not claim recovery or mark work complete.

Scope: `events`. Owner: [recovery](../features/recovery.md). Implementation: [rust/crates/core/src/query.rs#event_action](../../rust/crates/core/src/query.rs).

### Inputs

| Field | Required | Type / values | Default |
|---|---|---|---|
| `seq` | yes | `"integer"` | `—` |

<details><summary>Exact input JSON Schema</summary>

```json
{
  "additionalProperties": false,
  "properties": {
    "seq": {
      "type": "integer"
    }
  },
  "required": [
    "seq"
  ],
  "type": "object"
}
```

</details>

### Example arguments

```json
{
  "seq": 123
}
```



### Result and failures

Acknowledgment receipt

Event belongs to another recipient or seq does not exist.
