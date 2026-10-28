# kedra_inspector_luau

Editor-layer adapter for explicitly selected Luau Inspector drawers. It composes
`kedra_inspector::ValueEditor` with `kedra_script` and `kedra_ui::fields`. It owns
neither a VM implementation nor source documents, transactions, Save or project trust.
The editor's embedded UiCaption drawer is its production consumer.

## Host contract

Install the existing `LuauPlugin` and `InspectorLuauPlugin`, then explicitly register
a `LuauEditor` by native type or field through the ordinary `EditorRegistry`.
Construct it with a diagnostic name, canonical source path and owned Luau source.
Optional modules use the runtime's existing `Sources` snapshot. No filesystem search,
code discovery, source evaluation on project Open or hot reload is installed.
Mounting a registered presentation explicitly authorizes one finite composition run.

```lua
function run(ctx, args)
    ctx.fields.add("text", "Text")
    ctx.fields.add("justify", "Alignment")
    ctx.fields.add("wrap", "Wrap")
end
```

`ctx.fields.add(native_path, label)` mounts the existing property row/value controls
inside this run's owned body. Paths are Bevy reflected paths, not labels, entity IDs,
source locators or a script-defined schema. Root/nested component chrome and collapse
remain base-Inspector-owned. Scripts have no mutable World/source access, input buffer,
accept/save function, renderer handles or game bindings. Field values refresh natively
after composition; no per-frame Luau callbacks or arbitrary layout language are added.

The supported leaves are exact String, bool and native all-unit enums. Strings are
single-line, at most 4096 Unicode scalar values. Booleans accept `true`/`false`; enum
input accepts exact native variant names. Other shapes, missing paths and multiline
values require the explicit default view, not conversion through strings or floats.
These native leaf semantics live in `kedra_inspector::fields`, not this script adapter.

## Admission, bounds and ownership

The existing bounded reflected discovery admits at most 32 supported leaf candidates.
The drawer reserves at most 32 controls from the host's shared rendering budget.
Paths are at most 256 bytes, labels at most 128 bytes without control characters;
duplicate native access sequences reject. Runtime source/module/VM/time/resume limits
remain enforced by `kedra_script`; native mounting separately obeys these field bounds.
No complete unbounded reflected value is cloned into the script or adapter.

Fields remain read-only until the complete composition succeeds. The host consumes
`ReflectedField` metadata only after `ready` and separately validates source access,
identity, exact working revision and typed authoring policy. Native input uses the
existing TextField acknowledgment. `DrawerChanged` tells hosts to refresh their retained
TreeViews after composition/failure. It is not a source-edit request or event bus.

An error leaves partial controls read-only with a custom-editor diagnostic and default
view access. There is no retry loop or silent fallback hiding the script failure.
Retirement revokes the existing script owner and releases only its native subtree.
Hide/float retains the presentation; switching to default retires it and returning
authorizes a fresh composition. Source history is host-owned and survives drawer VM
retirement. This is neither general project extension loading nor a security sandbox
for arbitrary native bindings.

## Verification

`tests/contracts.rs` owns native-path binding, exact typed proposals, retained refresh,
scope retirement, partial-failure access and default-view availability with real Luau
and CPU kit fixtures. Editor native acceptance lives in the existing Inspector scenario:
Unicode text, enum rejection, wrapping, document history and UI-only/mixed projections.
CPU tests do not establish physical input, IME, clipboard or screen-reader behavior.
