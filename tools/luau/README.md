# Luau editing

Install **Luau Language Server** (`JohnnyMorganz.luau-lsp`) in VS Code. The repository
recommends it through `.vscode/extensions.json`; no separate game editor is needed.
Open the repository folder and reload the language server after installing it.
The server executable is bundled with the extension. Definitions use the syntax
supported by luau-lsp 1.70.1.

Repository `.vscode/settings.json` selects standard Luau, disables platform-specific sourcemaps,
enables standard `require`/automatic imports and loads the current runtime,
application and editor definitions in dependency order. No personal VS Code settings
or installed extensions are changed automatically.

## Definitions and scopes

| Definition | Available API |
| --- | --- |
| `crates/kedra_script/types/runtime.d.luau` | `runtime`, `json`, `wait`, JSON-domain types |
| `crates/kedra_script_bevy/types/bevy.d.luau` | Opt-in owned-object Transform/UI contexts |
| `crates/kedra_app/types/application.d.luau` | `AppContext`, `ctx.app`, input variants/keys, nodes, renderer frames |
| `crates/kedra_editor/types/editor.d.luau` | `EditorContext` / `DockContext`, editor/workspace observations |
| `crates/kedra_script/tests/scopes/types/game.d.luau` | Scope regression's test-owned `TestGameContext` and global markers |
| `crates/kedra_script/tests/scopes/types/game-only.d.luau` | Game-only API absence probes |
| `crates/kedra_script/tests/scopes/types/tool.d.luau` | Scope regression's test-owned `TestToolContext` |

Default repository editing uses the production editor API superset. Types do not
grant runtime permissions. For a narrower authoring scope, open
`runtime.code-workspace` or `application.code-workspace` in this directory instead.
The embedded product regression scripts in `crates/kedra_script/tests/scopes` have
their own `game.code-workspace` and `tool.code-workspace`; their test-owned bindings
must not be mixed with production editor definitions. Diagnostics
apply to the profile selected for the open file, not an inferred runtime permission.

Native entity/window IDs are decimal strings. Input coordinates and UI rectangles
are logical pixels; renderer dimensions are physical pixels. Luau cannot statically
enforce array lengths, positive durations, lossless integer bounds or lifecycle
readiness; runtime checks remain authoritative.

Serialized null is `json.null`, not nil. Missing `ctx.app.node` targets return nil, while
nullable record fields use `Nullable<T>`. Narrow a nullable record before access:

```luau
function run(ctx: EditorContext, args)
    local entry = ctx.editor.state().entry
    if type(entry) == "table" then
        print(entry.name)
    end
end
```

Bootstrap-only `NativeApp` describes the internal adapter captured by
`native.luau`; submit/receipt are not public ctx.app methods. The
definitions describe current bindings, not an SDK for arbitrary reflected Bevy
operations, filesystem access, Save or script evaluation. Keep declaration changes
with the corresponding host binding/serialized observation changes.

## Standard modules

Use `require("./helpers")` or `require("../shared/math")` relative to the importing
file, without a `.luau` extension. Each project's `.luaurc` defines aliases; the canonical
test project's own `.luaurc` maps `@game` to its `scripts` directory:

```luau
local projection = require("@game/Native/projection")
```

The same file layout and configuration bytes must be admitted in Program.sources
at runtime. LSP sees source files on disk, whereas runtime sees only that explicit
snapshot; neither a completion nor an alias grants runtime loading permission.
Kedra uses mlua/Luau's standard resolver, not custom URI syntax or a filesystem loader.
Luau owns module type inference. Annotate host-injected ctx/self parameters explicitly
to get typed callback completions; global host API tables are not declared.

Behavior uses named lifecycle functions rather than a returned descriptor table;
Sequence uses `run(ctx, args)`; Module returns its explicit exports. See the
[runtime contract](../../crates/kedra_script/README.md) for scheduling, ownership
and source limits. An imported module does not receive ctx implicitly.

With a locally installed luau-lsp binary, actual static diagnostics can be checked
without an editor (add host-specific definitions in the same dependency order):

```sh
luau-lsp analyze --platform=standard --no-flags-enabled \
   --definitions=@01-runtime=crates/kedra_script/types/runtime.d.luau \
   --definitions=@02-application=crates/kedra_app/types/application.d.luau \
  crates/kedra_app/src/scripting/native.luau
```

Named definitions keep dependency order explicit; add the opt-in Bevy definitions
after runtime when checking object-capability scripts.
This optional tool is not a required build dependency. Static checks do not establish
native input/frame acceptance or permissions at runtime.
