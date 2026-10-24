# Luau programs and modules

Use a **Behavior** for persistent per-instance state and non-yielding callbacks. Use a
**Sequence** for one finite operation that may wait and return owned data. Use a **Module**
to share ordinary Luau code. Importing a module does not start another program.

Start with [Rust host ownership](index.md) and
[scene loading and hydration](../scenes/lifecycle.md) for the authorization boundary;
opening a document in the editor does not execute these programs.

Each authorized run has an independent VM. Behaviors own mutable `self`; the capability
context `ctx` is read-only. Declare lifecycle functions at the top level, not as fields of
a returned table. A Behavior needs at least one of `start`, `update`, `fixed_update`, `stop`.
Their signatures are `start(self, ctx)`, `update(self, ctx, dt)`,
`fixed_update(self, ctx, dt)` and `stop(self, ctx, reason)`.

`start` runs once immediately before the first update or fixed callback, even for a
dynamically attached Behavior; it is not Bevy's one-shot `Startup`. Callbacks cannot yield.
A Sequence instead declares `run(ctx, args)`. It has no Behavior state or lifecycle hooks.
Unused parameters may be omitted, but the host still supplies the documented tuple.

## A real switch: Rust authorizes, Luau reacts

The canonical test game's Rust mount finds the projected element `game.switch` and
authorizes the following **excerpt** from `src/scripting.rs`. Here `world` and `target`
already belong to the mounted native UI. This is not a standalone setup function.

```rust
let mut program = Program::behavior(
    "game.switch",
    "scripts/Native/switch_behavior.luau",
    include_str!("../scripts/Native/switch_behavior.luau"),
    |_, _, _| Ok(()),
);
program.params = json!({"color":[30,100,190,255]});
program.validate_params = validate;
program.events = &["activate"];
program.sequences.push(Program::sequence(
    "game.switch.activate",
    "scripts/Native/switch_activate.luau",
    include_str!("../scripts/Native/switch_activate.luau"),
    |lua: &lua::Lua, scope, ctx| kedra_script_bevy::install_ui(lua, scope, ctx),
));
kedra_script::attach(world, target, program, Value::Null)?;
```

Imports in that source are `kedra_script::{Program, ScriptError, lua}` and
`serde_json::{Value, json}`. Its `validate` function accepts exactly one `color` field
containing four unsigned integral bytes in `0..=255`; other values fail before allocation.
The host installs `LuauPlugin` through `KedraAppPlugin`, plus `ScriptBevyPlugin` for native
`Activate` forwarding. The parent's empty bindings deliberately grant no UI mutation API.

This is the game's complete Behavior source, except for its lint directive:

```lua
-- The native switch host authorizes this Behavior only through attach.
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

The separately admitted child source is:

```lua
function run(ctx: UiContext, args: {color: {number}})
    ctx.ui.set_background(ctx.entity, args.color)
    ctx.log.info("Switch activated")
end
```

The Behavior subscribes on its own attached object, then starts only the approved child.
The child gets the same target and lifetime owner but its own VM and UI-only bindings.
It does not inherit the parent's events or capabilities. Repeated activation cancels the
previous child; already-applied color changes are not undone.
The headless adapter test covers this interaction between programs. It does not test
physical clicks or rendered frames.

## Wait inside a Sequence

The following runtime example uses a host-defined `ctx.game.score()` binding; it is not
a built-in gameplay service. The host must supply that binding when authorizing the source.

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

Waits use Bevy schedule executions and clocks, not rendered frame counts. Sequences resume
in `Last`; waiting for fixed steps does not run subsequent code inside `FixedUpdate`.
See [clocks and limits](lifetime-and-capabilities.md#clocks-and-execution-budgets) before
using waits for a readiness condition or a pause-sensitive gameplay timer.

## Admit modules instead of searching the disk

`Program::sources` holds an explicitly admitted `Sources` snapshot. The entry path is
canonical and project-relative, such as `scripts/entry.luau`; it is not a qualified scene
asset path. Ordinary imports include `require("./lib/counter")`,
`require("../shared/module")` and `require("@game/lib/counter")`.

For the alias example, include this same `.luaurc` in the runtime snapshot:

```json
{"aliases":{"game":"./scripts"}}
```

A **Rust excerpt** from the module contract adds admitted files to an existing `program`:

```rust
program.sources.insert(".luaurc", r#"{"aliases":{"game":"./scripts"}}"#)?;
program.sources.insert("scripts/lib/counter.luau", "return {value = 1}")?;
```

Relative and alias imports of that counter share one cache per VM, as the module contract
test verifies. A second run starts with fresh module state. Constructors are useful
when one VM needs separate objects; pass `ctx` explicitly to operations that need authority.
Directory `init` modules work; imports omit `.luau`/`.lua`. Ambiguous extension matches,
missing modules, noncanonical/duplicate paths and external filesystem aliases fail.

Each file is at most 64 KiB. The snapshot, including its entry, is at most 256 files / 8 MiB.
There is no ambient disk/config search. Aliases are relative to their configuration file;
nested `.luaurc` files may override parent aliases. Import does not grant extra permissions.

Entry evaluation cannot perform Scope effects or yield. A module required during a live
callback uses that callback's authority and budget. A requiring Sequence may suspend during
module initialization, but entry loading and Behavior callbacks still cannot yield.
Prefer exported functions over hidden asynchronous module initialization.

Use the same source layout/configuration with luau-lsp. Type declarations describe the
host's API; explicit context/state annotations help the language server understand injected
parameters. They do not install bindings. There is no automatic watcher or state migration;
successful replacement creates fresh globals, `self` and module cache.

## Repository references

For exact calls, see the [runtime reference](../../api/luau/runtime.md),
[Bevy bindings](../../api/luau/bevy.md) and
[game-host switch context](../../api/luau/game-host.md).

Read the [complete switch host][switch] with the [fixture setup][fixture] and
[runtime contract][runtime] before adapting the excerpts.

[switch]:
  <https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/src/scripting.rs>
[fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
[runtime]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md
