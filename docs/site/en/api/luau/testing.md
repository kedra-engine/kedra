# Test fixtures and diagnostic programs

Use this reference when maintaining an existing native test scenario, not when writing
ordinary game behavior. These APIs operate on explicitly mounted test compositions.
For scenario setup and evaluation of results, use the
[testing manual](../../manual/testing/native-scenarios.md); for invocation, use
[automation programs](../../manual/automation/programs-and-diagnostics.md).

## Choose the fixture

| Task | Context | Required host setup |
| --- | --- | --- |
| Reset or vary the editor docking specimen | `DockContext` | Matching editor diagnostic composition |
| Exercise project-defined docking projection | `NativeProjectionContext` | Canonical test project's mounted projection lab |
| Prepare a control specimen or inspect retained fields | `AppContext` | Host-mounted catalog or Inspector specimen |

All these registrations are debug-only. Common waits, values and logging remain in the
[shared runtime](runtime.md); input and captures remain in [application automation](application.md).
Test-only scope markers and bindings in runtime regression suites are not shipped APIs.

## Named docking diagnostics only

Declarations:
[editor.d.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/types/editor.d.luau).
Bindings: `crates/kedra_editor/src/workspace/fixture.rs::script_api`.

`DockContext` is an alternative `AppContext & {dock: DockApi}`, not an
extension automatically present on `EditorContext`:

```luau
ctx.dock.state: () -> DockDiagnostic
ctx.dock.reset: () -> ()
ctx.dock.drafts: () -> ()
ctx.dock.overflow: () -> ()
```

`state` returns `{ready=false}` or `{ready=true,generation=number}` for the diagnostic
fixture, **not** the full workspace. `reset` resets the fixture; `drafts` toggles
fixture draft visibility; `overflow` applies the overflow layout defined in the fixture sources.
They need the matching mounted diagnostic composition, can raise setup/readiness errors, and are not
generic project docking commands. Earlier synchronous setup is not rolled back on error.

The declaration is a minimal diagnostic shape, not an exhaustive wire schema.
`crates/kedra_editor/src/workspace/fixture.rs::describe` also returns `error` when unready
and additional readiness/identity/renderer fields when ready. Its additional `host` field
and `fields` array entries are decimal entity-ID strings, not Luau numbers or entity
handles. Schema, generation and counters remain numeric. These extra observations do not
expand the declared diagnostic capability or provide generic editor commands.

`script_api` installs these fields only for `dock.reset`, `dock.drafts`, `dock.overflow`.
Their sources import the explicitly admitted `src/workspace/fixture/helpers.luau`.
Registration alone neither mounts nor executes them.

## Fixture-only projection API

Declaration:
[projection.d.luau](https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/types/projection.d.luau).
Project binding implementation:
[scripting.rs](https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/src/docking/scripting.rs).
Bindings: `tests/fixtures/test_project/src/docking/scripting.rs::bindings`.
This project-defined, debug-only scope combines `AppContext` and
`{projection: NativeProjectionApi}`. It is not shared runtime or editor authority.

```luau
ctx.projection.state: () -> NativeProjectionState
ctx.projection.move: (name: string, target: string,
    edge: ("left" | "right" | "top" | "bottom")?, index: number,
    fraction: number?) -> ()
ctx.projection.maximize: (name: string?) -> ()
ctx.projection.visibility: (name: string, target: string, visible: boolean) -> ()
ctx.projection.restore: () -> ()
```

| Function | Meaning and errors |
| --- | --- |
| `state` | Returns `{mounted=false,projected=false}` while loading; source/startup/UI failure raises |
| `move` | Resolves unique live view kinds; target must be visible; model validates command |
| `maximize` | Maximizes the visible named view's group; nil restores the full host tree |
| `visibility` | Hides named view, or shows it at index 0 in visible target's group |
| `restore` | Restores authored topology, retaining registered content/drafts; marks projection pending |

Tab placement requires nil edge/fraction and uses a native zero-based `usize` index, not
a Lua array offset. Edge placement requires a supported edge and finite fraction in `(0,1)`;
`index` remains required by the binding but is unused for edge placement. Unknown or
ambiguous names, hidden targets, missing mounting and rejected model commands raise.
When hiding, `target` is still a required string but is not resolved. Operations synchronously
change the model; `projected` becomes true only when its revision has been projected.
No command result alone proves native input or rendering.

`NativeProjectionState` when mounted has `mounted=true`, `projected`, `snapshot`, `views`.
`snapshot` contains `version`, `hosts`, `nodes`, `views`, `hidden`, with Group/Split
topology shapes like [editor observations](editor.md#workspace-shape).
Each `NativeProjectionView` contains `id`, opaque `content` entity string, nullable `tab`
UI identity, nullable `group`, and `hidden`. These are observations, not entity handles.
`NativeProjectionAction` declares `reorder`, `split`, `activate`, `maximize`, `visibility`,
`restore`; it is not a public `ctx.projection.action` function.

Rust registers `projection.exercise` and one Sequence per action, each importing the
explicitly admitted `scripts/Native/projection.luau` via `@game/Native/projection`.
The project's admitted `.luaurc` maps `@game` to `scripts`; no filesystem discovery occurs.
See that module for the full exercise, not a generic production game UI API.

## Other editor-owned programs and boundaries

- `catalog.prepare` (`src/catalog/prepare.luau`) receives only `AppContext`;
  its host mounts the specimen. There is no universal `ctx.catalog`.
- `inspector.presentation` (`src/diagnostics/values.luau`) also receives only application
  bindings. It validates fixture field prefixes and exercises native retained controls,
  not a generic writable Inspector API.
- `editor.startup.forms` uses `src/scripting/startup.luau` with
  [editor observations](editor.md).

The paths above are relative to `crates/kedra_editor`. Program names are native
registrations, not automatically discovered project files. The
[editor manual](../../manual/editor/index.md) describes current read-only tools.

## Evidence and limits

The canonical project is a fixture and example consumer, not a shipped game SDK.
Its runtime-only dependencies demonstrate separation from the editor; its custom debug
projection API is not promised to every game. See the
[fixture contract](https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md).
Its Kedra-authored geometry/textures are not imported private game assets; the included
Inter font retains its OFL notice. This reference does not replace those source obligations.

Runtime and Bevy owned-object contracts test revocation and the event → child mutation
route without a renderer. Projection test bodies in `src/docking/scripting.rs` distinguish
unmounted readiness from source/UI failure. These headless tests cannot verify physical
input or rendered frames; run the debug fixture scenarios for those checks. The fixture
provides no editor plugin, release diagnostic transport, Save, Play or hostile-code sandbox.
The [geometry caveat](limitations.md#fixture-geometry)
also applies to the fixture's remote diagnostic example.
