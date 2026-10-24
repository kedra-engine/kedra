# Programming

For exact interfaces, use the [Rust API guide](../../api/index.md) and
[Luau API overview](../../api/luau/index.md). To change Kedra itself, start with
[Contributing](../../contributing/index.md).

Kedra extends an ordinary Bevy application. Rust components hold data, systems implement
gameplay, and plugins install the services an application chooses to use. Luau is an
optional interpreted layer with explicit, host-owned permissions, not a replacement ECS.

If you come from Unity, separate these familiar concerns before looking for equivalents:

| Concern | Current Kedra mechanism |
| --- | --- |
| Runtime component data and gameplay | Ordinary Bevy components, resources, systems and plugins |
| Data stored in a scene or prefab | Explicit reflected component policies and source compilation |
| Per-object script state and callbacks | An explicitly authorized Luau Behavior attached by Rust |
| A finite coroutine-like operation | A Luau Sequence with owned arguments, result and deadlines |

Each mechanism needs explicit integration. To store a Rust component in a source document,
register its schema; to execute a script, authorize it in the host. A Luau declaration file
helps an editor understand names but grants no authority.

## Choose your starting point

- [Rust components and host integration](rust-components.md): compose the application,
  register authored schemas and keep source state separate from live entities.
- [Luau programs and modules](luau.md): choose a Behavior or Sequence, authorize the
  switch example and admit imports explicitly.
- [Lifetime and capabilities](lifetime-and-capabilities.md): design narrow bindings,
  parameters, event/child ownership, clocks, budgets and failure handling.

Read [scenes and prefabs](../scenes/index.md) first if source documents and instances are
new to you. For a tool that changes those documents, use the
[source-editing and Save protocol](../scenes/source-editing.md), not gameplay World writes.

## What the Rust host owns

The **host** is the native Rust application that constructs the Bevy `App`. It chooses:

1. Native plugins and their ordering, including assets and any presentation adapters.
2. Verified asset sources and the explicit scene/prefab source catalog.
3. Authored component keys, exact schema versions and side-effect-free validators.
4. Startup scene selection and its native mount/hydration function.
5. Script sources, bindings, parameters, allowed events/children and execution limits.
6. The moment execution is authorized and when owners are cancelled or retired.

Install Bevy's `AssetPlugin` before `kedra_script::LuauPlugin`. A host using
`kedra_app::KedraAppPlugin` gets Luau and shared asset services through that plugin;
do not install a second Luau backend. `KedraAppPlugin` itself creates no window or renderer.
Native UI/Transform script adapters remain separate opt-ins.

This **initialization excerpt** from the headless adapter contract uses Bevy directly,
without `KedraAppPlugin`. It installs services but does not authorize any program:

```rust
let mut app = App::new();
app.add_plugins((
    MinimalPlugins,
    AssetPlugin::default(),
    kedra_script::LuauPlugin,
    kedra_script_bevy::ScriptBevyPlugin,
));
```

`App`, `MinimalPlugins` and `AssetPlugin` here use `bevy::prelude::*`.

The repository's ordinary `kedra-test-project` executable demonstrates this composition.
It verifies two deliveries, registers their sources, installs native plugins, registers
schemas, installs `PrefabPlugin`, and configures an explicit startup scene. Its mount
function authorizes the switch script only after the UI projection provides its target.
Use it as a test fixture for host integration, not as a complete game SDK or game template.

## Current execution boundary

Project opening, scene loading and module import are not execution authorization.
There is no automatic discovery or compilation of project Rust code, project extension
loading, editor Play, or automatic loading of project Luau programs. Embedded consumers
provide source explicitly, for example with `include_str!`, then call `start` or `attach`.
Neither source registration nor a debug scene override can choose new native plugins.

Shared launch configuration retains the host's normal startup scene in release.
The `--scene` override and remote diagnostic configuration exist only with debug assertions.
Game-facing Luau can remain in release; `kedra_app::scripting` and its application-control
adapter are debug-only. That adapter runs registered finite programs, not remote source eval.

Capabilities are curated APIs such as “change this owned object's local position,” not a
generic script-visible `World`. Trusted native Rust can still access Bevy directly;
script scopes and budgets do not form a security sandbox around that Rust code.

Headless VM/schedule tests establish runtime contracts. They do not establish real widget
input, renderer readiness or visible frames. A completed Sequence is not evidence
that a physical click was accepted or that a GPU frame has appeared.

## Repository references

For registered diagnostic programs and external supervision, continue to
[Programs and diagnostics](../automation/programs-and-diagnostics.md) and
[Native scenarios](../testing/native-scenarios.md).

For exact integration details, use the [Luau runtime contract][runtime],
[native adapter contract][adapter] and [complete fixture setup][fixture].

[runtime]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md
[adapter]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script_bevy/README.md
[fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
