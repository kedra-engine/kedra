# Shared runtime API

The common foundation for game scripts, editor diagnostics and automation. For your
host's additional operations, return to the [task chooser](index.md). Context notation
and error conventions are defined [once for all scopes](conventions.md).

Declarations:
[runtime.d.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/types/runtime.d.luau).
Runtime initialization:
[vm.rs](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/src/vm.rs).
Bindings: `crates/kedra_script/src/{vm.rs,parameters.rs,diagnostics.rs,jobs.rs,events.rs}`.
Wait/value implementations: `src/{wait.luau,values.luau,values.rs,vm/sequence.rs}` in that crate.
This scope is shared by editor and game hosts and is not debug-only.

## Entrypoints and common context

```luau
start(self, ctx)
update(self, ctx, dt)
fixed_update(self, ctx, dt)
stop(self, ctx, reason)
run(ctx, args)
```

A Behavior declares at least one of the first four top-level functions. It has its own
mutable `self`; callbacks and event handlers cannot yield. `start` runs once before the
first update/fixed callback. `dt` is Bevy game delta in seconds. Pausing virtual time
leaves update callbacks active with zero delta; fixed callbacks follow actual FixedUpdate.
No ordering between instances is promised.

A Sequence declares `run`, may yield using waits, and returns one owned JSON-domain value.
No return/nil becomes JSON null. It has no Behavior callbacks or `self`.
Entry evaluation cannot yield or perform Scope effects, including through `require`.

`RuntimeContext` fields:

| Field | Meaning |
| --- | --- |
| `entity: EntityHandle?` | Opaque attached live target; absent when unattached |
| `params: {[string]: JsonValue}` | Host-validated, deeply readonly parameter snapshot |
| `log: Log` | Owned structured telemetry, including during load/stop |

`ctx.params` is separate from Sequence `args` and Behavior `self`. The default host
validator accepts only an empty object. Parameters are validated before allocation or
replacement; they are not an automatic scene property schema or serialization API.

`BehaviorContext.entity` is optional too. Operations requiring an attached target
must narrow it or use a host context that guarantees attachment; see
[attachment annotations](limitations.md#behavior-attachment-annotation).

## Clocks and waits

```luau
runtime.elapsed_ms: () -> number
wait.updates: (count: number) -> ()
wait.fixed: (count: number) -> ()
wait.game_seconds: (seconds: number) -> ()
wait.real_seconds: (seconds: number) -> ()
wait.until_true: (label: string, predicate: () -> boolean, timeout_ms: number) -> ()
```

- `elapsed_ms` is monotonic milliseconds since admission, including suspension.
- `updates` / `fixed` require integral counts in `1..=1000000` and count future schedule
  executions, not rendered frames. Fixed waits do not run the continuation in FixedUpdate.
- Seconds must be finite, positive and at most `1000000`. Game seconds use Bevy virtual
  time and need that clock installed; real seconds still require updates to resume.
- `until_true` requires a string label, function and timeout in `(0, 600000]` milliseconds.
  It polls immediately and then once per future update, returning no value on success.
  The implementation accepts a truthy predicate result; the declaration asks for boolean.
  Timeout raises an error. The deadline is checked between calls, not preemptively inside
  a yielding predicate's nested waits. Never retain native borrows across suspension.

Sequence resumption is in Last at `ScriptSystems::Run`. Module initialization required
by a live Sequence can suspend under its existing wait/budget/cancellation rules;
Behavior callbacks and entry evaluation still cannot suspend.

## JSON-domain values

```luau
json.null: JsonNull
type Nullable<T> = T | JsonNull
type JsonValue = boolean | number | string | JsonNull
    | {JsonValue} | {[string]: JsonValue}
```

Arguments, results, parameters, event payloads and structured fields cross an owned-value
boundary: at most 64 KiB serialized JSON, depth 32, integers in
`-9007199254740991..=9007199254740991`. Cyclic tables, unsupported values and nonfinite
numbers are rejected. Empty tables decode as objects by default. Native identities that
need full integer width must be opaque strings or handles, never coerced to Luau numbers.
Changing a returned observation table does not mutate native state.

## Logging and structured errors

```luau
ctx.log.info: (message: string, fields: JsonValue?) -> ()
ctx.log.warn: (message: string, fields: JsonValue?) -> ()
ctx.log.error: (message: string, fields: JsonValue?) -> ()
```

Logs record program/path/phase and source/line when available. Messages and available
tracebacks are bounded to 8 KiB. The host diagnostics sink retains the newest 256 records
in order and counts overwritten entries. Logging grants no World access.

`ScriptError` has `kind`, `message`, `path`, `phase`; the last two use
`Nullable<string>`. Codes are `invalid_input`, `capacity`, `load`, `runtime`, `budget`,
`permission`, `stale`. Child outcomes expose these fields; thrown errors need not be tables.

## Behavior-owned children

```luau
ctx.sequences.start: (path: string, args: JsonValue?) -> SequenceHandle
ctx.sequences.status: (handle: SequenceHandle) -> SequenceOutcome
ctx.sequences.cancel: (handle: SequenceHandle) -> ()
```

`start` selects only a finite program in Rust's `Program.sequences`, using standard
relative/alias module resolution without evaluating its entry. Omitted args become null.
Admission occurs in Last after events, before Sequence resumption. Permission/path/capacity
errors can be synchronous; deferred admission failures appear in the outcome.

`status` returns a fresh snapshot: `{state="queued"}`, `{state="running"}`,
`{state="completed", result=...}`, `{state="cancelled"}`, or
`{state="failed", error=...}`. `cancel` is idempotent while the owning scope is live;
foreign handles and revoked owners fail. Cancellation does not undo earlier effects.

Each child has its own VM, parameters, modules, bindings and budget. It inherits the
target/lifetime, **not** parent capabilities. Sequences cannot spawn children. Parent
cancellation, replacement, failure or despawn revokes pending/active children. Completed
children release native owners; their handles retain outcomes. A Behavior retains at most
32 handles, including referenced terminal ones; unreferenced terminal handles are reclaimed
on subsequent starts. Global Sequence capacity applies separately.

## Subsystem subscriptions

```luau
ctx.events.on: (target: EntityHandle, name: string,
    callback: (payload: JsonValue) -> ()) -> Connection
connection:disconnect()
```

Only Behaviors receive this API. Target must be the run's own attached live entity and
name must be in `Program.events`. Hosts enqueue owned data through Rust `events::notify`;
scripts cannot publish to a global event bus. The opt-in Bevy adapter forwards `activate`.
Callbacks run without yielding, under their Behavior's authority and callback budget.

Disconnect removes the subscription and its queued deliveries; repeated disconnect is
safe in a live scope. Dropping the handle alone does not disconnect. Revocation clears all
subscriptions; notifications sent before subscribing are lost. Limits per owner are
32 subscriptions, 128 queued deliveries / 64 KiB, and 16 deliveries per application update.
Overflow fails the owner before the next batch. Order is per owner; reentrant deliveries
wait until the next update. No cross-owner ordering is guaranteed.

## Lifetime and resource limits

Started Behaviors receive one best-effort stop with `StopReason`: `cancelled`,
`retired`, `replaced`, `despawned`, `failed`, `budget_exhausted`. Scope effects are already
revoked in stop; telemetry can remain available. Never-started Behaviors get no stop.
Application shutdown after schedules cease does not guarantee hook delivery.

Default limits: 16 MiB Lua heap, 100 ms per load/dispatch; Sequences additionally have
45 seconds total and 100000 resumes. Host capacity defaults to 32 Sequences / 256 Behaviors.
Admission requires finite Sequence limits, total at most 600 seconds. Native allocation,
compilation and host callbacks are not all preempted or covered by Lua heap limits.
The cooperative callback-time interrupt is terminal even through `pcall`: it cannot be
turned into successful continuation. Uncaught allocation-limit failures become terminal
`budget` errors; no sticky-termination guarantee covers every caught allocation failure.
Total-time, resume and event-queue limits have independent native checks. Prior effects
remain applied. These controls are not hostile-code isolation; retain external supervision.

Rust cancellation revokes effects immediately; retirement also removes the owner.
Parent despawn revokes access before cleanup at a scripting dispatch boundary.
Validated replacement creates fresh globals/state/module cache, not state migration.
See [lifetime guide](../../manual/programming/lifetime-and-capabilities.md).

## Modules and real example

Standard `require("./helpers")`, `require("../shared/helpers")` and configured aliases
operate only on `Program.sources`: at most 256 files / 8 MiB including entry/config,
64 KiB per file. Canonical project-relative source paths are not source-qualified asset
paths such as `editor://prefabs/button.prefab.ron`. No disk search or ambient `.luaurc`
loading occurs. Each VM has its own module cache; importing grants no execution authority.

This Behavior excerpt is from the test game's `scripts/Native/switch_behavior.luau`.
Its host admits `activate` and the child path; it is not automatic project discovery.

```luau
type Context = BehaviorContext & {entity: EntityHandle, params: {color: {number}}}
type State = {activation: SequenceHandle?}

function start(self: State, ctx: Context)
    ctx.events.on(ctx.entity, "activate", function(_payload)
        if self.activation then
            ctx.sequences.cancel(self.activation)
        end
        self.activation = ctx.sequences.start("./switch_activate", {color = ctx.params.color})
    end)
end
```

`Context` narrows `entity` because this host uses `attach`; the annotation does not attach
anything. The Rust authorization is in the [complete example](../../manual/programming/luau.md).
Contract tests for these operations are in
`crates/kedra_script/tests/contracts/{behaviors,parameters,children,
events,modules,budgets,lifecycle,diagnostics}.rs`.
