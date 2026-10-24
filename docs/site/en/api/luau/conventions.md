# Context and value conventions

Use these conventions to read signatures in the [runtime](runtime.md) and host APIs.
For installation, see host integration.

Luau type identifiers use descriptive names without a project-brand prefix.

## Context shape

`ctx` is passed to entrypoints, not published as a global. Its outer table is readonly.
The common fields are `params`, `log` and, for attached programs, `entity`.
Only Behaviors receive `events` and `sequences`; host bindings add other fields.
Bindings must freeze their own nested capability tables.

Context annotations describe a shape, not authorization. A declaration file, module import,
scene load or project open does not install capabilities or start a program. See
[host integration](integration.md) for the Rust boundary and declaration inputs.

## Reading signatures and failures

Signatures retain the declaration identifiers. `T?` means an optional/nil value;
`Nullable<T>` means `T | JsonNull`. Serialized null is `json.null`, not an absent
field. `ctx.app.node` is the important missing-target exception: it returns nil.
Opaque entity handles, observation ID strings and persisted source IDs are different types.

Bindings raise Luau errors on invalid arguments, denied authority or stale lifetimes.
An uncaught error terminates that run. Native effects already applied are not rolled back.
See [JSON-domain bounds](runtime.md#json-domain-values) and
[structured error fields](runtime.md#logging-and-structured-errors).

## Isolation boundary

Cooperative budgets and restricted capabilities are **not** a hostile-code security sandbox
or process-isolation guarantee. Native work still needs external supervision when appropriate.
See [runtime limits](runtime.md#lifetime-and-resource-limits) for the distinction between
callback-time interrupts and allocation failures, and the
[lifetime guide](../../manual/programming/lifetime-and-capabilities.md) for the authority model.
