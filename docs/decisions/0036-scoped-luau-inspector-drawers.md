# ADR 0036: Scoped Luau Inspector composition

Status: implemented narrow profile; owner review pending. General editable reflected
fields, project extension loading and complete settlement in ADR 0011 remain proposed.

## Decision

Use the existing `EditorRegistry`/`ValueEditor` protocol for Luau presentations, not a
second inspector or a new UI/schema language. `kedra_inspector_luau` is an editor-layer
adapter with the editor's embedded UiCaption drawer as a real consumer. Its independent
responsibility is runtime-to-presentation composition and lifetime, not editor workspace
authority. Keeping it in the host would couple reusable drawers to document/docking code;
adding scripting to the native Inspector would couple all value consumers to a VM.
The adapter introduces no new third-party dependencies or runtime-to-editor edge.

The host explicitly selects owned source and modules, registers the native type/path,
and authorizes one finite `kedra_script` Sequence when mounting the drawer. Scripts call
the bounded `ctx.fields.add(native_path, label)` capability on their attached UI owner.
They cannot write sources, submit authoring transactions, mutate native/game entities,
choose component schemas or reach filesystem/remote APIs. Field composition reuses the
shared property kit. Base Inspector owns headers, disclosure, search and default view.

The ordinary bounded Bevy reflection traversal supplies leaf candidates. Exact String,
bool and all-unit-enum proposal semantics live in native `kedra_inspector::fields`.
Scripts do not parse native values or serialize source edits. Rust binds emitted fields
to source identity/revision and reuses the same staged authoring acceptance, history,
dirty/close policy and native acknowledgment as Transform. Caption rendering updates
the existing passive Text/TextLayout and invalidates UI/spatial readback epochs.

## Limits and alternatives

Complete composition must succeed before fields become editable. Script failure retains
diagnostics, disables partial controls and leaves an explicit default view. Retirement
uses existing runtime scopes and ordinary Bevy hierarchy; no owner or interpreter is
retained through a new global registry. Field count and strings/native paths have explicit
bounds in addition to the existing VM/source budgets. No complete value clone is needed.

A handwritten Rust UiCaption drawer would not exercise the extension boundary. A custom
Lua VM would duplicate existing execution, module and budget guarantees. A UI descriptor
DSL would duplicate authored controls/reflection. Direct script mutation would bypass
source validation and accepted-history ownership. These alternatives are not used.

This profile composes fields once and refreshes values natively. Dynamic scripted layout,
per-field Lua validation callbacks, general number/collection widgets, multiline editing,
project file discovery, hot reload, Save and Play are not implemented by it. Booleans and
enum variants use current native text surfaces, not new checkbox/picker designs.

## Verification ownership

The adapter owns scope/composition/failure contracts in its public suite; the native
Inspector owns exact leaf proposal semantics. Editor/native scenarios own validated
acceptance, source Undo/Redo, retained controls, passive caption reprojection and a new
readback frame in mixed/UI-only scenes. Existing runtime suites continue to own generic
VM/module/budget behavior. Native OS IME, clipboard failures and other platforms remain
separate acceptance boundaries, not conclusions from headless script tests.
