# Game scripting

Write a Behavior for persistent instance state, a finite Sequence for work that can wait,
and ordinary modules for shared code. The game's Rust host authorizes each run and chooses
its capabilities. There is no built-in universal `ctx.game`.

## Choose an operation

| I need to… | Reference |
| --- | --- |
| Declare callbacks, read parameters or wait | [Shared runtime](runtime.md) |
| React to an admitted subsystem event | [Subscriptions](runtime.md#subsystem-subscriptions) |
| Start, inspect or cancel an authorized child | [Behavior-owned children](runtime.md#behavior-owned-children) |
| Read or change my attached object's local position | [Local Transform](bevy.md#local-transform) |
| Change my attached UI object's background | [Retained UI background](bevy.md#retained-ui-background) |

The runtime and live-object adapter are release-capable. Object operations require the
host to install their bindings; they do not expose generic World access. For a complete
explanation of host setup and the switch, use the
[programming manual](../../manual/programming/luau.md), rather than debug automation examples.

## Real game authorization: switch

Sources: `tests/fixtures/test_project/src/{main.rs,scripting.rs}` and
`scripts/Native/{switch_behavior.luau,switch_activate.luau,switch.luau}` in that project.
The project is a fixture and example consumer, not a shipped game SDK.

1. The host installs `KedraAppPlugin` (shared Luau included) and `ScriptBevyPlugin`.
2. `scripting::bind` finds the already mounted `game.switch` element; no match is a no-op.
3. Rust constructs a Behavior at `scripts/Native/switch_behavior.luau`, admits only
   `activate`, configures `{color=[30,100,190,255]}`, validates exactly one `color` field
   containing four unsigned bytes, and admits `scripts/Native/switch_activate.luau`.
4. `attach(world,target,program,Value::Null)` explicitly authorizes execution on that target.
5. The parent receives event and child-sequence operations, but no UI setter. The host calls
   `install_ui` only for the child. The child has its own VM and empty default parameters;
   it receives the color in `args`.

The [UI reference](bevy.md#retained-ui-background) contains the child's function body;
the [runtime example](runtime.md#modules-and-real-example) contains the parent's callback.
The parent cancels the previous child on repeated activation. Despawn/revocation clears
authority; already applied color remains. This game-facing route is not guarded by
`debug_assertions`.

## Where the game API ends

Project-defined operations are only those the project's Rust initializer installs. Scene
loading, UI mounting, LSP declarations and a module named `@game` grant neither execution
nor World access. In-editor Play and automatic project-code discovery are not implemented.
For native capability obligations, use [Rust host integration](integration.md).

The fixture's `scene.remote` and `scene.switch` registrations in `src/main.rs` are
debug-only application automation. A remote `game.state` method there reports renderer
identity but is **not** a Luau `ctx.game.state` binding. Its docking projection commands
belong to the [test-fixture reference](testing.md#fixture-only-projection-api), not this
game-scripting API. Headless contracts and native input/frame acceptance remain separate.
