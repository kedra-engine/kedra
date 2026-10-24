# Rust host integration

Install or extend Luau from Rust by admitting programs and installing their capabilities.
For setup and examples, use the [programming manual](../../manual/programming/index.md).
Use the [Rust API guide](../index.md) for local rustdoc.

## Admission and installation

`kedra_script::LuauPlugin` supplies the shared runtime. Rust `start` authorizes a run;
`attach` also supplies a live target and binds lifetime to it. Installing plugins, loading
scenes, opening projects, importing modules or loading declarations does not authorize
execution. No generic World API or arbitrary remote source evaluation is exposed.

| Capability | Native installation | Reference |
| --- | --- | --- |
| Shared globals and context | `LuauPlugin` and explicit admission | [Runtime](runtime.md) |
| Local Transform / UI background | `install_transform` / `install_ui` | [Live objects](bevy.md) |
| Debug application operations | `install_app_api` | [Automation](application.md) |
| Editor observations | Editor-owned binding initializer | [Editor diagnostics](editor.md) |
| Project-defined fields | Project-owned initializer and authorization | Project contract, not universal `ctx.game` |

## Custom bindings: host obligations

A host initializer receives the real `Lua`, revocable `Scope` and context table. It chooses
API names and domain semantics, validates arguments, freezes nested capability tables and
returns owned bounded observations. Capture Scope, not component borrows or strong Lua
handles; reacquire synchronous World access through `Scope::with_world`.

For owned-object operations, resolve the attached target through `Scope::resolve_entity`.
`EntityHandle::resolve` alone checks World/liveness, not domain permission. Script type
annotations cannot replace these checks. Loading another program inside an active native
callback is rejected; schedule admission after dispatch. Children need their own bindings,
params, sources and budgets; they do not inherit a parent's API.

Native scene/prefab lookup uses full source-qualified logical paths such as
`editor://prefabs/button.prefab.ron` through the existing loaded source catalog, membership
and published revision checks. Those are not module entry paths like
`scripts/Native/switch_behavior.luau`. Do not introduce hardcoded native UUID lookups or
convert persisted source identity into an entity capability.

## Declaration inputs and editing

| File | Declared contract |
| --- | --- |
| `crates/kedra_script/types/runtime.d.luau` | Globals, values, contexts, children, events |
| `crates/kedra_script_bevy/types/bevy.d.luau` | Owned Transform/UI operations |
| `crates/kedra_app/types/application.d.luau` | Application observation and automation |
| `crates/kedra_editor/types/editor.d.luau` | Editor observations and named dock diagnostics |
| `tests/fixtures/test_project/types/projection.d.luau` | Test project's debug-only custom API |

These files are type inputs, not generated runtime bindings or permission manifests.
LSP sees disk sources; the runtime sees only an explicitly admitted source snapshot.
Annotate injected `ctx` and `self` parameters: the LSP cannot infer Rust authorization.
Use the [editing guide](https://github.com/kedra-engine/kedra/blob/main/tools/luau/README.md)
for profile selection and definition ordering. Repository-wide editor completions can
describe capabilities that a narrower application or game program does not have.
Check [implementation limitations](limitations.md) before relying on a declaration's shape.

## Application private bootstrap

`NativeApp` declares internal `submit`, `receipt`, `window`, `node`. Its factory
captures these during installation; public `ctx.app` has **no** `submit` or `receipt`.
`Receipt` describes the internal pending/succeeded/failed operation states,
`request_hash` and nullable/result/error `detail`; it is not a public general method API.
Only fixed `app.input`, `app.resize`, `app.capture` submissions are admitted internally.
Effect session/id identities are native-owned, not supplied by scripts. Sources and public
helpers are linked from [application automation](application.md).

## Source ownership

The
[runtime crate contract](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md)
defines Rust lifetimes and limits. Each capability reference links its declarations and
binding implementations. For changes to bindings or their documentation, follow the
[contributor guide](../../contributing/index.md). Test runtime behavior, native input and
rendering separately from reviewing these sources.
