# Editor diagnostics

Use this path to observe editor startup and workspace state in named debug programs.
For UI actions, continue to [application automation](application.md). This is not an
editor plugin API: project inspectors, windows, Save and Play are not exposed here.
Fixture setup commands have a separate [test reference](testing.md).

Declarations:
[editor.d.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/types/editor.d.luau).
Scope installation:
[scripting.rs](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/scripting.rs).
Bindings: `crates/kedra_editor/src/scripting.rs`; serialized observations:
`crates/kedra_editor/src/remote.rs` and `src/workspace/`.
The editor installs this scope for `editor.startup.forms` in debug diagnostic services,
not for project scripts.

## Observation functions

```luau
ctx.editor.state: () -> EditorState
ctx.editor.workspace: () -> Workspace
```

`EditorContext` combines `AppContext` and `{editor: EditorApi}`.
Both functions synchronously return owned JSON-domain snapshots through native methods.
They require a live Scope and are revoked with the program.
The readonly editor table has no `project`, `save`, `play`, source-eval or arbitrary
command function. `editor.project` is a separate native remote method, not `ctx.editor`.
Use installed `ctx.app` input helpers for the actual UI route; observation is not mutation.

## Startup state

`EditorState` fields:

| Field | Contract |
| --- | --- |
| `phase` | `"boot" \| "projects" \| "loading" \| "editor"` |
| `generation` | Project manager generation, not a script handle |
| `preferences` | Nullable native history/preferences locator; observation only |
| `busy`, `ready` | Pending project job / model readiness |
| `error`, `notice` | Nullable loading error and current notice string |
| `project` | Nullable `Project`: `name`, native `root`, `format` |
| `entry` | Nullable `Entry`: `mode`, `name`, `directory`, `error` |

`Entry.mode` is `"create" | "open"`; form strings are current drafts and validation
diagnostics, not saved project edits. Nullable fields use `json.null`; narrow before access.
Native filesystem locators here are protocol observations, not suitable editor display
labels. Editor UI should display project-relative asset paths, not expose absolute paths.

`src/scripting/startup.luau` uses these functions to inspect project-manager
forms and native input. This shortened excerpt assumes isolated startup with no form open:

```luau
function run(ctx: EditorContext, args)
    wait.until_true("project manager", function()
        return ctx.editor.state().phase == "projects"
    end, 40000)
    assert(ctx.editor.state().preferences == args.preferences, "Refusing personal editor preferences")
    assert(ctx.editor.workspace().bound == false, "Startup invented a bound workspace")
    ctx.app.click("projects.new")
    wait.until_true("create form", function()
        return ctx.editor.state().entry ~= json.null
    end, 40000)
    ctx.app.field("entry.name", "QA Кедра")
    ctx.app.click("entry.cancel")
end
```

This is an adaptation, not an arbitrary remotely loadable script. The complete registered
program also checks modal focus, validation, geometry and captures; external Rust QA owns
isolated preferences, setup and evidence acceptance.

## Workspace shape

`Workspace` is `{bound: false}` before workspace binding, otherwise:

| Field | Contract |
| --- | --- |
| `bound`, `native_alive` | Binding exists; root entity and native window remain live |
| `main_host`, `revision` | Logical docking host identity and layout revision |
| `layout` | `DockLayout` snapshot, not a mutable command object |
| `documents` | Read-only document session observations |
| `scene_views` | Map from view identity to `SceneView` |
| `navigation_input` | Native gesture/measurement/input diagnostics |

`DockLayout` contains `version`, `hosts`, `nodes`, `views`, `hidden`:

- Hosts map to nullable `root` and `maximized` node identities.
- Nodes are `{Group={tabs: {string}, active: string}}` or
  `{Split={axis: "Horizontal" | "Vertical", fraction: number, first: string, second: string}}`.
- Views contain `kind` and nullable `restore_key`.
- Hidden views contain `host`, `group`, `index` (native topology index, not a Lua array index).

`Documents` has nullable `active`, `pending`, boolean `read_only`, and `documents`.
Each document record has `document`, `path`, `view` and nullable `selection`.
`NodeAddress` selection contains `instances: {string}` and `node: string`; these
persisted identities do not become runtime entity handles. This is not document Save.

`SceneView` is `{ready: false}` or a ready-shape observation with `spatial`, nullable
`translation`, `rotation`, `pivot`, `speed`, and booleans `orthographic`, `overlays`,
`editor_lighting`, `aligning`, plus `ready`. Readiness/geometry observations are not proof
of native frame quality or input acceptance.

`NavigationInput` contains `gesture`, `pending_host`, `menu_nodes`, `measuring`,
nullable `rendered`, `revision`, nullable `capture={view,window}`, and `viewports`.
Each viewport has `view`, nullable logical `rect`, `eligible_windows`, `pointer_windows`.
Both window arrays contain `WindowId` decimal strings, not Luau numbers.
`crates/kedra_editor/src/workspace/scene_input.rs::diagnostics` supplies these strings;
`crates/kedra_editor/types/editor.d.luau` declares both arrays as `{WindowId}`.
Keep them opaque; they are observations, not entity handles. Rectangle coordinates,
revisions and counts remain numeric.

## Verification boundary

Startup, docking, input and rendering require native tests; snapshots alone cannot
verify them. The
[editor manual](../../manual/editor/index.md) describes current read-only tools;
[automation programs](../../manual/automation/programs-and-diagnostics.md) describes invocation.
