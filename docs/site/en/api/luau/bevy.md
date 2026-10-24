# Live-object operations

Use these game-runtime operations to read or change the run's own attached object's local
position or retained UI background. Debug input and test-fixture commands use separate APIs.
For callbacks and waits, use the [shared runtime](runtime.md).

Declarations:
[bevy.d.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script_bevy/types/bevy.d.luau).
Native implementation:
[lib.rs](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script_bevy/src/lib.rs).
Bindings: `crates/kedra_script_bevy/src/lib.rs`, specifically `install_transform`,
`install_ui` and `ScriptBevyPlugin`. These are separate opt-in capabilities, not fields
provided automatically by `LuauPlugin`, `KedraAppPlugin` or an editor context.

## Availability and target authority

The host must explicitly install each capability; see [Rust integration](integration.md).
Use an attached live target. Operations reacquire World access synchronously and resolve
only that run's target. A handle from another entity is not permission to operate on it.
Foreign, stale and unowned targets fail before mutation. Revocation also rejects access.

`TransformContext` and `UiContext` describe separate context shapes, with
`entity`, `params`, `log` and their respective capability. A host may compose both;
the annotations themselves install nothing. Neither table is a reflected component API.

## Local Transform

```luau
type Position = {x: number, y: number, z: number}
ctx.transform.get_position: (target: EntityHandle) -> Position
ctx.transform.set_position: (target: EntityHandle, position: Position) -> ()
```

`get_position` copies the attached entity's local `Transform.translation` into a new
`{x,y,z}` table. Modifying that table changes no component.

`set_position` requires all three coordinates, finite and representable as finite `f32`.
It replaces only local translation, preserving rotation and scale. A missing `Transform`
raises `invalid_input`; there is no automatic component insertion. Conversion errors and
nonfinite/out-of-range coordinates fail without applying a translation.

No world-space conversion, `GlobalTransform`, parenting, physics synchronization, query,
arbitrary entity lookup or scene/prefab source editing is supplied.

This example is adapted from the owned-object contract, whose Rust host installs
Transform bindings and attaches a target with `Transform`:

```luau
function run(ctx: TransformContext, args)
    local position = ctx.transform.get_position(ctx.entity)
    position.x = 99
    ctx.transform.set_position(ctx.entity, {x = 1, y = 2, z = -3})
end
```

The local table edit alone has no native effect; the explicit setter performs the mutation.
This is not an automatically exposed game or editor function.

## Retained UI background

```luau
ctx.ui.set_background: (target: EntityHandle, rgba: {number}) -> ()
```

`rgba` must have exactly four entries at indices `1..4`: integral sRGB bytes in `0..=255`,
ordered red, green, blue, alpha. Extra entries, fractional/out-of-range/nonfinite channels,
wrong shapes and missing `BackgroundColor` fail before mutation. The operation updates an
existing Bevy `BackgroundColor`; it does not create a widget, save authored style or grant
access to other fields. The readonly capability table is separate from the mutable input.

The test-game child `tests/fixtures/test_project/scripts/Native/switch_activate.luau`
uses this capability:

```luau
function run(ctx: UiContext, args: {color: {number}})
    ctx.ui.set_background(ctx.entity, args.color)
    ctx.log.info("Switch activated")
end
```

Its parent supplies `{30,100,190,255}` through a host-validated parameter; the child
receives it as args, not inherited parameters. See [game host authorization](game-host.md).

## Native Activate forwarding

`ScriptBevyPlugin` observes real `bevy::ui_widgets::Activate`, then queues an owned empty
object payload under `"activate"` for subscribed Behaviors. The host must separately admit
that event name in `Program.events`; installing the plugin does not subscribe programs.

Use [runtime events](runtime.md#subsystem-subscriptions):

```luau
ctx.events.on(ctx.entity, "activate", function(_payload)
    ctx.sequences.start("./switch_activate", {color = ctx.params.color})
end)
```

The example is an excerpt inside the real switch Behavior's `start`. Event handlers cannot
yield; start an authorized finite child for yielding work. Delivery occurs at the runtime's
Last boundary, not recursively in a native observer. Native widget lifecycle remains
Bevy-owned; this adapter is not a second UI event system.

## Failure and verification boundary

Each successful setter is synchronous. Later callback failure or cancellation does not
roll it back. Use source-owned authoring APIs for persistent document edits, not these
live component operations. Stop callbacks cannot use revoked Scope effects.

`crates/kedra_script_bevy/tests/contracts/owned_objects.rs` checks translation preservation,
owned copies, foreign/stale denial, malformed colors, missing components and the actual
Activate → Behavior → child route. These headless contracts cannot check physical pointer
delivery or rendered color; those require native UI tests.
