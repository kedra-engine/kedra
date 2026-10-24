# Lifetime and capabilities

For exact lifecycle and capability calls, see the [Luau runtime](../../api/luau/runtime.md)
and [Bevy binding reference](../../api/luau/bevy.md).

A **capability** is a specific operation the host chooses to expose, not permission to
search every component. The host defines the operation; the runtime's `Scope` checks
whether this run may still perform it. Keep these two responsibilities separate.

## Bind a narrow operation

To expose a score, return an owned value rather than a resource borrow. This **excerpt**
from the runtime's host-integration example assumes a native `Score(u32)` resource and the
`kedra_script::{Scope, lua::{self, Lua}}` imports. `ctx.game` is example host API, not built-in.

```rust
fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let game = lua.create_table()?;
    game.set("score", lua.create_function(move |_, ()| {
        scope.with_world(|world| Ok(world.resource::<Score>().0))
    })?)?;
    game.set_readonly(true);
    ctx.set("game", game)
}
```

Capture `Scope`, not an ECS borrow or strong Lua handle. `with_world` reacquires exclusive
access only during this run's live dispatch and returns owned data; a borrow cannot cross
a yield. It rejects foreign execution contexts, inactive/retired owners and load-time calls.
Freeze nested API tables yourself; the runtime freezes the outer context after initialization.
Bound native work separately: cooperative Lua budgets cannot preempt a hung Rust callback.

Attached programs receive opaque `ctx.entity`, not a number or general entity API.
Use `Scope::resolve_entity` inside `with_world` to restrict an adapter to this run's target.
`EntityHandle::resolve` alone checks World/liveness, not domain authorization.
Trusted Rust receives World access; scripts do not receive a generic World binding.

The opt-in Transform adapter reads/writes local position only, preserving rotation/scale.
It requires an existing `Transform` and finite f32 coordinates; there is no parenting,
physics synchronization or world-space conversion. The UI adapter writes an existing
`BackgroundColor` from exactly four integral sRGB bytes; it does not construct widgets.
Foreign/stale targets, invalid values and missing components fail before mutation. Tests
also assert that both the unauthorized target and the caller's target remain unchanged.

## Parameters, events and child ownership

`Program::params` becomes deeply read-only `ctx.params`. A host `validate_params` function
checks ordinary JSON/domain types before VM allocation or replacement. The default accepts
only an empty object. Parameters are distinct from mutable Behavior `self` and Sequence
`args`; there is no automatic scene serialization or second property-schema system.

A Behavior may subscribe only to names in `Program::events`, on its own attached target.
Native subsystems publish bounded owned JSON through `events::notify`; scripts get no
publisher or global event bus. Delivery is deferred, never recursive VM entry. Handlers
cannot yield and use the Behavior's authority and budget. Notifications before subscription
are lost. `connection:disconnect()` is idempotent and clears queued deliveries too;
dropping the connection does not disconnect it. Revocation clears all subscriptions.

Only Behaviors may start entries in their host-owned `sequences` list. Child admission runs
in `Last`, after event delivery and before Sequence resumption. Each child has its own
bindings, parameters, snapshot and budgets; it inherits only target and owner lifetime.
Children cannot start further children. A module's presence is not child authorization.

`ctx.sequences.status(handle)` returns an owned outcome: `queued`, `running`, `completed`
with `result`, `cancelled`, or `failed` with structured `error`. Deferred capacity/load
failure is visible there. Child cancellation is idempotent and revokes active effects.
Terminal child handles retain outcomes after native owners are released.

Per Behavior, the runtime bounds retained child handles to 32, including terminal handles
still referenced by Luau. Unreferenced terminal handles are reclaimed on later starts.
Per owner, events allow 32 subscriptions, 128 queued deliveries / 64 KiB, and 16 deliveries
per application update. Overflow fails the owner before the next batch rather than silently
dropping one event. Enqueue order is preserved within one owner; no inter-owner order is
promised. Reentrant notifications wait until the next update.

## Revocation is not rollback

`cancel` immediately revokes Scope effects and releases VM handles, retaining terminal
evidence. `retire` also removes the owner. Parent despawn revokes access immediately;
VM cleanup/stop settle at a scripting dispatch boundary. Cleanup requested inside a native
callback waits for dispatch to return. Cancellation, failure, replacement and despawn also
revoke pending/active children. Source assets and other instances are not lifetime children.

Started Behaviors receive one best-effort `stop` with `cancelled`, `retired`, `replaced`,
`despawned`, `failed` or `budget_exhausted`. Ordinary effects are already revoked;
effect-free telemetry can remain. A yielding/failing/timed-out stop cannot block cleanup.
Never-started Behaviors receive no stop. Application shutdown does not promise hooks after
schedules cease, although native owners still drop.

`replace` validates a fresh candidate before retiring the old generation. Failure preserves
the old run, including at full capacity; success returns a new handle with fresh VM state.
It does not migrate stacks or undo applied effects. Runtime errors are terminal and are not
retried. If a script changes a value and then fails, that earlier change remains.

## Clocks and execution budgets

Behavior updates run in `Update`, fixed callbacks once per actual `FixedUpdate`.
Paused virtual time leaves updates active with zero game delta; automatic fixed stepping
follows Bevy's clock policy. There is no automatic enable/disable or scene-ready callback.
`wait.updates`/`wait.fixed` count future schedule executions, not GPU frames.
`wait.game_seconds` honors virtual-time pause; `wait.real_seconds` uses monotonic time
but still needs application updates to resume. `wait.until_true` polls immediately, then
on updates, with a finite positive millisecond deadline. Its deadline is checked between
predicate calls, not as preemption of a nested wait; never retain a borrow across suspension.

Defaults per VM are 16 MiB Lua heap and 100 ms per load/dispatch. Sequences additionally
have 45 seconds total and 100,000 resumes; finite limits are mandatory, with a maximum
600-second total. Behaviors default to no total deadline/resume ceiling, but every callback
remains bounded. Initial start and its first update share one dispatch budget.
Host `Capacity` defaults to 32 Sequences / 256 Behaviors, not an aggregate frame-time promise.
Interrupt exhaustion is terminal even through `pcall`.

Arguments/results are owned JSON-compatible values: at most 64 KiB, depth 32 and integers
in the exact double range. Use `json.null` for explicit null, opaque strings for full-width
observation IDs. Native allocation/compilation is not wholly covered by Lua heap limits.
These controls are not hostile-code/process isolation; keep an external watchdog for hangs.

## Diagnose the boundary that failed

Admission/control errors and `Status::Failed` carry `ScriptError`: `kind`, bounded message,
available traceback and program/path/phase where known. Codes are `invalid_input`,
`capacity`, `load`, `runtime`, `budget`, `permission`, `stale`; child errors use the same codes.
Check parameters/path/budgets for admission errors, admitted snapshots for load errors,
installed bindings for missing APIs, and ownership/revocation for permission/stale errors.
Do not parse prose messages to choose recovery.

`ctx.log.info/warn/error(message, fields?)` records bounded JSON plus source/line when
available. Host `Diagnostics::drain` consumes the newest 256 records in order; `dropped()`
counts overwritten records. Messages/tracebacks are at most 8 KiB. Logs/failures also go to
stderr. Logging remains effect-free during load/stop and needs no editor console.

Read `status` before `retire` if the host needs the final outcome. A successful result
proves only the program's contract, not physical input or rendered output. See
[Luau examples](luau.md) and [source/live separation](rust-components.md).

## Repository references

Use the [runtime contract][runtime] for scope and lifetime details and the
[native adapter contract][adapter] for target permissions.
For the distinction between capability, scope and revocation, see the [Glossary](../glossary.md).

[runtime]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md
[adapter]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script_bevy/README.md
