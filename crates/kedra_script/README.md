# kedra_script

This runtime-layer crate runs **interpreted Luau** using BMS 0.23.0 and the workspace's
pinned Bevy. Games and tools install the same `LuauPlugin` and provide their own narrow
capabilities. The crate has no editor, authoring, importer or QA dependency.
The API is unstable. Project-code discovery and automatic loading are not implemented.

## Three source contracts

- **Behavior:** persistent `start(self, ctx)`, `update(self, ctx, dt)`,
  `fixed_update(self, ctx, dt)`, `stop(self, ctx, reason)` functions. Callbacks are
  optional, but at least one must be declared. Each authorized instance has its own
  VM, mutable `self` and readonly capability context. Lifecycle functions cannot
  yield; use a sequence for work involving waits.
- **Sequence:** one finite, yielding `run(ctx, args)` invocation, with bounded owned
  JSON arguments and result. Each invocation has an independent VM and coroutine.
- **Module:** an ordinary Luau file returning a value, imported through standard
  `require("./module")`, `require("../shared/module")` or `require("@game/lib/module")`.
  Importing a module does not register a behavior or authorize another program.

Lifecycle functions are declared at the top level. Host capabilities are fields
of `ctx`. Lua permits unused parameters; the host passes the documented tuple.

```lua
function start(self, ctx)
    self.elapsed = 0
end

function update(self, ctx, dt)
    self.elapsed += dt
end
```

## Host integration

Install Bevy's ordinary AssetPlugin before LuauPlugin. Construct a `Program` with
an explicit kind, diagnostic name, canonical project-relative entry path, owned
source and trusted Rust `Bindings` initializer. `Program::sequence` supplies finite
default budgets; `Program::behavior` supplies persistent defaults.

Calling `start` is execution authorization; registration, imports, scene loading
and project open are not. `attach` additionally ties lifetime to a live parent using
ordinary Bevy hierarchy and supplies `ctx.entity`. Unattached programs have no entity.
Loading/replacement inside native script callbacks is rejected; schedule it after
dispatch to avoid re-entering BMS registries.

```rust,no_run
use bevy::prelude::*;
use kedra_script::{Program, Scope, lua::{self, Lua}};

#[derive(Resource)]
struct Score(u32);

fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let game = lua.create_table()?;
    game.set("score", lua.create_function(move |_, ()| {
        scope.with_world(|world| Ok(world.resource::<Score>().0))
    })?)?;
    game.set_readonly(true);
    ctx.set("game", game)
}

fn authorize(world: &mut World) -> Result<kedra_script::Run, kedra_script::ScriptError> {
    kedra_script::start(world,
        Program::sequence("example", "scripts/example.luau",
            "function run(ctx, args) return ctx.game.score() end", bindings),
        serde_json::Value::Null)
}
```

The version-aligned `lua` module is the real mlua API, not a reflection registry.
Bindings add capabilities to the supplied `ctx`, not globals. Freeze nested tables
explicitly; the runtime freezes the outer context after initialization. Host
functions bound their own native work and return owned values. Capture Scope,
not component borrows or strong Lua handles. `Scope::with_world` reacquires current
exclusive ECS access and rejects inactive, foreign or retired owners.

`ctx.entity` is an opaque `EntityHandle` userdata, not a numeric identity or entity
API. Trusted bindings can borrow that type and call `Scope::resolve_entity` inside
`with_world` to enforce the run's own target. `EntityHandle::resolve` only checks
World/liveness; broader domain authorization remains host-owned.
Scene/prefab source identities and paths do not become runtime entity handles.

## Modules and LSP

`Program::sources` is a bounded, explicitly admitted `Sources` snapshot of module
files and `.luaurc` configuration. Paths correspond to the project's actual source
layout. Each file is at most 64 KiB; the complete snapshot, including the entry,
is at most 256 files / 8 MiB. Duplicate and noncanonical paths are rejected.

```json
{
    "aliases": {
        "game": "./scripts"
    }
}
```

Include this same `.luaurc` in the runtime snapshot. Aliases are relative to their
configuration file; nested configurations can override parent aliases. Imports
omit `.luau`/`.lua`; directory `init` modules work, and ambiguous file/extension
matches fail. Standard Luau owns resolution/configuration/cache behavior through
mlua's public `Require` interface. Kedra only supplies the admitted virtual tree;
the upstream filesystem `TextRequirer` is deliberately not installed.

There is no disk search, absolute-path alias access or ambient filesystem config.
Snapshots are source input, not an asset registry. Published project loading
must use existing source authority; it is not implemented here. Embedded native
consumers explicitly supply `include_str!` sources. Snapshot memory/compilation are
native work, not all covered by Lua allocator limits.

Modules cache within one VM; different runs never share mutable module state.
Use module constructors for distinct state and pass `ctx` explicitly to operations.
Imports grant no extra capabilities. Replacement creates a fresh module cache.
Entry evaluation cannot use Scope effects or yield; modules loaded during a live
callback execute with that callback's existing authority and budget. Standard Luau
also permits module initialization to suspend a requiring sequence: the same wait
tokens, total deadline and cancellation apply. Entry loading and behavior callbacks
still cannot yield, including through require. Prefer exporting functions rather
than hiding asynchronous gameplay work in module initialization.

luau-lsp understands these imports without a Kedra resolver extension.
Use explicit context/state annotations for lifecycle callbacks; the language server
does not infer host-injected parameters. See the [editing guide](../../tools/luau/README.md).

## Scheduling and waits

Behavior start runs once immediately before the first update or fixed callback,
including dynamically attached instances. It is not Bevy's one-shot Startup.
Update callbacks run in Update at `ScriptSystems::Update`; fixed callbacks run
once per actual FixedUpdate at `ScriptSystems::FixedUpdate`. Hosts order domain
systems around these sets; no inter-instance order is promised. A paused virtual
clock leaves update callbacks active with zero game delta; automatic fixed steps
follow Bevy's clock policy. There is no automatic disable/enable or scene-ready API.

```lua
function run(ctx, args)
    wait.updates(3)
    wait.fixed(2)
    wait.game_seconds(0.25)
    wait.real_seconds(0.01)
    wait.until_true("positive score", function() return ctx.game.score() > 0 end, 5000)
    return {name = "Кедра", ready = true}
end
```

Sequences resume in Last at `ScriptSystems::Run`, after BMS queued callbacks.
Update/fixed waits count future Bevy schedule executions, not GPU frames. Waiting
for fixed steps does not move a sequence into FixedUpdate. Game seconds use virtual
time and honor pause; real seconds use monotonic time but still need updates to resume.
`wait.until_true` polls immediately and then on updates, with a finite positive
deadline; a yielding predicate must not hold a borrow across suspension. Its deadline
is checked between predicate calls, not as a preemptive deadline for nested waits.

## Ownership and failure

`status` distinguishes running, completed, cancelled and failed. Errors are terminal,
not retried; prior synchronous effects are not rolled back. This is not an authoring
transaction or Save API. Source edits must use real document authority.

`cancel` immediately revokes Scope effects and releases VM handles, retaining terminal
evidence. `retire` additionally removes the owner. Parent despawn revokes access
immediately; VM cleanup and stop notification settle at the next scripting dispatch
boundary. Inside callbacks cleanup is deferred until BMS dispatch returns.

Started behaviors receive one best-effort `stop` notification. Reasons are
`cancelled`, `retired`, `replaced`, `despawned`, `failed`, or
`budget_exhausted`. Ordinary Scope effects are already revoked in stop; explicit
effect-free host telemetry can remain available. Failed/yielding/timed-out stop
callbacks report diagnostics but cannot prevent native cleanup. Never-started
behaviors receive no stop. Stopping the entire Bevy application does not promise
delivery of user hooks after schedules have ceased; native owners still drop.

`replace` validates a fresh candidate before retiring the old generation; failure
preserves the old run and success returns a new handle with fresh globals, state
and module cache. Replacement works at full capacity without requiring a spare
slot. No automatic watching, stack/state migration or implicit source execution.

## Parameters, children and subsystem events

`Program::params` supplies a deeply readonly object as `ctx.params`, independent of
sequence arguments and mutable `self`. The host's `validate_params` function validates
ordinary JSON/domain types before allocation or replacement. The default accepts only
an empty object. There is no second property schema or automatic scene serialization.

A Behavior may start only the finite programs listed in its host-owned `sequences`.
`ctx.sequences.start("./child", args)` uses standard Luau relative/alias resolution
without evaluating the entry and returns an opaque owner-bound handle. Child admission
runs in Last outside VM dispatch, after event delivery and before sequence resumption.
Each child has its own VM, bindings, parameters, module snapshot and budget; it inherits
only the attached target and lifetime owner, never parent capabilities. A child cannot
spawn more children. No program discovery or arbitrary source evaluation occurs.

`ctx.sequences.status(handle)` returns a fresh owned outcome: `queued`, `running`,
`completed` with `result`, `cancelled`, or `failed` with a structured `error`.
`ctx.sequences.cancel(handle)` is idempotent and immediately revokes active effects.
Behavior cancellation, failure, replacement or despawn revokes pending/active children.
Completed children release their native owners; terminal outcomes remain in handles.
Each Behavior retains at most 32 handles, including terminal handles still referenced
by Lua. Unreferenced terminal handles are reclaimed on subsequent starts. Global
Sequence capacity still applies; deferred admission failures appear in the outcome.

`ctx.events.on(ctx.entity, "activate", callback)` subscribes to a host-approved
subsystem event named in `Program::events`. Native subsystems call `events::notify`
with owned bounded JSON, never enter a VM recursively and never expose a script
publisher/global bus. Callbacks receive the payload, cannot yield and use their
Behavior's authority/budget. A returned connection's `:disconnect()` is idempotent
and removes queued deliveries too; dropping the connection alone does not disconnect.
Owner revocation clears all subscriptions. Notifications before subscription are lost.

Per owner, there are at most 32 subscriptions, 128 queued deliveries / 64 KiB, and
16 deliveries per application update. Queue overflow fails the owner before the next
batch; it does not silently drop individual events. Deliveries preserve enqueue order
within an owner; no inter-owner order is promised. Reentrant notifications wait until
the next update, including notifications for an owner later in the current batch.

The opt-in [`kedra_script_bevy`](../kedra_script_bevy/README.md) adapter owns local
Transform and retained UI operations and forwards real Bevy widget `Activate` events.
The canonical game's switch is a real consumer, not a generic World/component API.

## Structured diagnostics

Admission/control APIs and `Status::Failed` use `ScriptError`: a stable `ErrorKind`,
bounded message/available traceback, and program/path/phase where known. Codes are
`invalid_input`, `capacity`, `load`, `runtime`, `budget`, `permission`, `stale`.
Synchronous effects before failure remain applied. Script-side child failures use
the same lowercase codes, without requiring message parsing.

`ctx.log.info/warn/error(message, fields?)` records owned bounded JSON fields and
source/line when available. `Diagnostics::drain` consumes the newest 256 records in
order; `dropped()` counts overwritten entries. Logs/failures also emit stderr output.
Messages/tracebacks are at most 8 KiB; JSON fields use the ordinary value limits.
Hosts can consume this sink without an editor console dependency. Logging does not
grant World access; it is available during load/stop as effect-free telemetry.

## Budgets and limits

Default VM limits are 16 MiB Lua heap and 100 ms per load/dispatch. Sequence defaults
add 45 seconds total and 100,000 resumes; admission requires finite sequence limits,
with at most 600 seconds total. Behaviors have no total deadline/resume ceiling by
default, but each callback is bounded. Initial start and its first update share a
dispatch budget. Host-owned `Capacity` defaults to 32 sequences and 256 behaviors;
these are admission bounds, not performance guarantees or aggregate frame budgets.

Arguments/results are JSON-compatible owned data: at most 64 KiB, depth 32, and
integers within the exact double range. `json.null` represents explicit null rather
than a missing table field. Full-width observation IDs use opaque strings, not
Luau numbers. Diagnostic names are at most 128 bytes without control characters.

Interrupt exhaustion yields through pcall and is terminal; it cannot be caught and
turned into successful continuation. These are cooperative execution controls,
**not hostile-code or process isolation**. Compilation, native callbacks/allocation
and panics are not preempted. A hung process needs an external watchdog. Play
isolation, browser/mobile certification, debugging, generated bindings and broad
gameplay subsystem APIs remain outside this API.

Contracts live in `tests/contracts/` and use real Bevy schedules/VMs; dangerous probes
have an external process deadline. Scoped
game/tool probes are test-owned, not the editor's document authority. Native input
and renderer acceptance remain separate; see
[testing](../../docs/site/en/contributing/contracts/testing.md),
[ADR 0029](../../docs/decisions/0029-luau-runtime-evaluation.md),
[ADR 0032](../../docs/decisions/0032-script-entrypoints-and-modules.md) and
[automation](../../docs/site/en/contributing/contracts/automation.md).
