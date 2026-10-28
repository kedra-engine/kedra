# Inspector source interaction contract

Inspector presents source values under
[ADR 0015](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0015-inspector-panel.md),
[ADR 0011](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0011-universal-reflected-inspector.md)
and
[ADR 0009](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0009-property-editing-and-inspector.md).
The same architecture is intended to support editing. See the
[Manual](../../../manual/editor/inspector.md) for inspection, search, lock and default-view
instructions. Native `scene.transform` and the embedded Luau `ui.caption` drawer support
in-memory editing and source history. Primitive dimensions, Surface color/metallic/roughness
and Sun color/illuminance also use the native typed source adapter.
Supported scalar UI values also use typed source editing and passive refresh. Bool/unit
enums use native checkboxes/menus; payload enums require registered native defaults or
explicit domain constructors. Default view remains read-only. Classes, image densities
and shadows use immediate validated field edits and complete structural proposals,
without Apply/Cancel. Typed resource wells expose recorded logical paths, searchable
choices and Asset Browser drops with typed admission and source history.
Component commands and project execution remain unavailable.
Save of an existing active source is
host-owned and refuses unsettled drafts. The complete interaction/settlement protocol
is incomplete beyond the supported live fields and structural proposal profile.

## Owners and layering

| Owner | Responsibility |
| --- | --- |
| `kedra_ui::fields` | Shared retained rows, labels, value surfaces, diagnostics, native read-only interaction and responsive layout |
| `kedra_ui::popup` | Native anchored surface placement and size/focus constraints; consumers own actions/dismissal |
| `kedra_inspector` | Reflected discovery, bounded presentation, dispatch, default/custom composition and retained value views |
| `kedra_inspector_luau` | Scoped finite Luau composition, native field capability bounds and runtime owner retirement |
| `kedra_editor::inspector` | Source target following/locking, provenance and dockable lifecycle |
| `kedra_editor::documents` | Admitted workspace, active document and persistent scoped selection |
| Existing source compiler | Effective typed values and reference resolution |

Runtime UI cannot depend on inspector, authoring or editor. `kedra_inspector` is editor-layer,
with only Bevy/`kedra_ui` production dependencies, not authoring/desktop/editor. The architecture
gate checks its core/runtime-only workspace edges. The ordinary editor also hosts a non-document
reflected-value consumer of the same controls. Shared widgets belong in `kedra_ui::fields`,
not private panel copies. Feature models are ordinary values; entities stay in UI adapters.

Reusable prefab/CSS primitives use the existing delivery. Rust controls receive explicit
compiled templates/theme context: they do not discover editor UUIDs or read editor resources.
Selection identity and source context remain host composition.

## Binding, borrowing and source authority

A source target includes source-scope identity, document ID and full scoped `NodeAddress`.
Component key/schema version and native represented type/Bevy reflected path identify values;
labels, translations, display order and renderer entities never bind data. Source inspection
works without viewport success or a renderer delivery.

The adapter enumerates authored components of the composed node, not generated `Name`,
hierarchy or rendering components. `Workspace::open_compiled` supplies the initial immutable
compilations. The document adapter pins the complete working revision set and recompiles the
current working catalog on change, using the admitted symbolic asset inventory without loading
assets or writing sources. Only a complete successful compilation replaces the cache. Existing
`ReflectComponent` access reads effective overridden values; origin metadata stays separate and
computed UI layout never substitutes for authored layout. A stale target or dependency revision
rejects inspection until refresh succeeds, never returning an old value under a new revision.

Inspector notices successful source refresh independently of selection and lock changes, updates
retained controls and retires components removed from the source. Idle refresh reuses the current
compilation. Native Transform acceptance/Undo/Redo updates that cache and opened graphs;
Scene View recompiles renderer bindings, validates the unchanged topology/other components,
and updates retained native transforms/passive UI values, owned primitive meshes/materials and
scene lights with a fresh readback epoch. Spatial asset lookups and derived bounds are
preflighted before any native replacement; handles, entities and camera stay retained. Camera
state remains intact. General cross-view settlement and new-document publication are not implemented.

Exact native integers/floats never pass through a universal f64. Vec3 axis proposals
replace only the intended axis in the complete native vector. SDR color fields preserve
native sRGB/linear representation and exact channels on no-op acceptance. Hex input is
explicitly sRGB with optional alpha; HDR/other spaces reject into default-view diagnostics,
never implicit clamping. Generic leaf presentation does not grant a write to unsupported
or source-reference values. Color swatches expand into exact native RGBA channels,
storage-space selection and named presets; presets retain alpha and representation.
Native enum proposals construct
complete values, retaining the
current payload exactly. Primitive constructors belong to the editor's domain adapter.

Discrete `ControlEvent` values retain the original `Arc<ControlBinding>` identity,
target/path and revision set. Neither a replaced control nor a stale menu may borrow
new source authority. The editor checks access, visible ownership and pending drafts,
then stages complete typed compilation before synchronous publication. Checkboxes do
not optimistically mutate their checked state before source refresh.

`PropertySession` lifts indexed/compound edits and edits beneath existing parent or
component overrides into exact complete component replacement. Native construction
and `Reflect::set` avoid collection `try_apply` resize semantics. Symbolic source
references and effective sibling values are preserved; indices are not persistent IDs.

`PassiveView::prepare_refresh` stages unchanged-topology UI updates without interactive
hydration. Publication checks the original instance/source identity. Only changed
authored fields update native components: unrelated edits must not reset stylesheet
layout. Missing presentation/dependencies reject before changing surviving UI nodes.

`UiLayout` composes semantic groups over the original native Node. Selected-field
discovery excludes unrelated subtrees before spending the bounded row allowance;
it never manufactures a parallel reflected value. Nested TreeViews delegate disclosure
ownership and filtering through `ValueEditorView`. Native lengths propose complete Val
values, keeping amount/unit on one row and preserving exact units during numeric edits.

## Immediate collections

`kedra_inspector::collections` retains native list presentation, transient stable item
identities and selection. Item fields use ordinary full-path `SourceField`/`SourceControl`
bindings. Only add/remove/reorder propose a complete native list. These proposals run
after Bevy's native editing phase and pin their original binding/revision; validation
and publication belong to the existing source adapter. No optimistic local source
mutation or separate Apply/Cancel protocol exists. Accepted local structural changes
retain surviving controls and update their indexed native paths.
The host associates value-free item mappings, selection and disclosure with the exact
source `HistoryId`. Undo/Redo requests that explicit attachment rather than matching
list values; focus is requested after source-field rebinding and only for eligible views.
Attachments never execute Undo or publish source. Outside retained mappings,
same-length refresh keeps positional controls; a length change resets identities
and selection rather than guessing which item survived.

Invalid/unaccepted item text, queued actions and pending proposals remain unsettled
while hidden and block source history, Save, target following and Close. A structural
proposal excludes only itself from settlement, never item text or another queued action.
Nested Color/Val editors receive native values. Add uses a valid unique class default
or a native item constructor; image density insertion copies an existing valid resource.
An empty required-image list opens a typed picker when matching delivery choices
exist. Only the selected, admitted and loaded image constructs its first native item;
cancel/failure leaves the list empty. No choices means Add is disabled. This constructor
does not relax `UiImage`'s source validation: an invalid empty `UiImage` document is
still refused on admission and removing its final required image remains invalid.

Bottom Add/Remove controls, grip dragging and Alt+Up/Down propose structural edits.
Hover, outside drop, Escape, access loss and window transfer publish nothing. Losing
the captured binding/window/access retires drag authority rather than leaving a gesture
that can resume when access returns. Native interaction covers narrow/floating layouts
and selection/focus through source history within the retained mapping window.
Long-list autoscroll and cross-window drag acceptance remain incomplete.

## Typed resource references

`CompiledPrefab::dependency_path` reads exact recorded source/subasset paths; ambiguous
or foreign identities return no path. Copyable wells never expose an editable UUID.
`ReferenceEvent` retains the original native binding and requests an exact logical path
or optional Clear. Search/highlight and drag hover are presentation only.

The editor checks source revisions, native resource type and membership in a matching
verified delivery before requesting loading, then repeats admission before source
publication. Loading readiness does not grant authority. Pending requests block Save
and navigation; cancellation, failure and timeout do not publish. A validated same-path
or empty-optional no-op requests no loading and preserves history/Redo. Native reflection
cloning retains strong handle ownership before falling back to dynamic reconstruction.
Native resource interaction covers picker filtering/Escape, optional Clear, same-path
no-ops, image/model drops, incompatible-drop rejection, drag cancellation and Undo/Redo.
Cross-window drops and controlled pending-load failure/timeout acceptance remain incomplete.
Environment refresh retains the camera and respects the current studio-lighting mode.
Model replacement stages audited native descendants off-view, checks ownership/budget
and bounds, then replaces only the recorded imported forest. Abandoned/failed staging
leaves the serving forest intact; foreign descendants refuse replacement. GLTF picker
paths come from loaded native scene handles. Model assignment replaces the native
forest through picker or Browser drop and source Undo/Redo without resetting the camera.
An Asset Browser parent path can resolve to its single advertised native subasset;
multiple scenes require an explicit picker choice rather than an invented Scene0.

## Transform acceptance and history

The shared Rust drawer emits typed `TransformField` presentation metadata. The editor
binds it to the original source scope/node/component and exact working revisions, using
the shared TextField buffer/protocol. A finite native f32 proposal retains untouched
axes; rotation uses YXZ degree projection and quaternion storage. An unchanged rotation
preserves its original quaternion, avoiding conversion-only edits.

The document adapter stages `PropertySession` acceptance on a cloned working set with
complete typed catalog validation and admitted symbolic asset checks. Native acknowledgment
must succeed before the synchronous working/cache swap; failed validation/acknowledgment
cannot mutate sources/history. No-op acceptance preserves redo. The original admission
snapshot/files/Save baselines are not written or acknowledged as saved.

Supported scalar fields opt into `LiveTextField`: settled native changes propose
immediately, with complete source validation before acknowledgment/publication.
Invalid text stays visible without changing source. An explicit `EditGroup` joins
consecutive accepted values from the same retained input/gesture into one source
Undo entry while each changed value advances source revisions. Enter, Escape and
focus loss end gestures. Save acknowledgment ends the active source history group;
Save remains separate. Echoing an input's own accepted source revision retains its
native numeric spelling, including `1.`, instead of canonicalizing during typing.
Foreign source revisions still invalidate drafts, never silently rebase them.

Drafts survive refresh, hiding and dock transfer. Following another target, changing
default mode or collapsing is deferred while a draft exists. Source revision changes
mark existing drafts stale rather than rebase them. Enter requests explicit acceptance;
Escape cancels unaccepted text, not already accepted source changes. Source Undo/Redo
refuses unsettled drafts, including hidden Inspector views. A settled live field routes
its history shortcut to source Undo/Redo; invalid/incomplete text keeps local Undo.
Presentation actions wait for queued native focus/selection work and recheck their
original binding token and every draft afterward. Queue completion does not accept
text, permit a stale disclosure or bypass collection settlement.
Main-window Close is vetoed while changes remain; explicit all-source discard restores
the admission snapshot, clears history and retires the old source-scope identity.

## Scoped Luau composition

`kedra_inspector_luau` implements the existing `ValueEditor` interface with the existing
`kedra_script` runtime. The host authorizes one finite program on mount; registration
and project Open grant no project execution. `ctx.fields.add(native_path, label)` mounts
the shared kit within the owned body, with base-owned component headers/disclosures.
The embedded UiCaption script selects `text`, `justify` and `wrap`; Rust supplies exact
String proposals through `ReflectedField` and native bool/unit-enum proposals through
shared discrete controls, not a duplicated caption schema. Every scripted choice has
a distinct retained scope. All controls remain read-only until composition succeeds.

Native bounded discovery admits at most 32 supported leaf candidates. The drawer reserves
at most 32 controls from the shared rendering allowance; paths/labels/native text are
separately bounded. Fields stay read-only until the complete composition succeeds.
Failures disable partial controls and surface the existing diagnostic/default-view path.
Retirement revokes the runtime owner; no raw World/source/renderer/Save binding is
installed in Luau.
Source acceptance uses the same staged property adapter and history as Transform.
Caption reprojection updates retained passive Text/TextLayout and invalidates the mixed
UI canvas and spatial target epochs, not the camera. Dynamic Luau layout, per-field Lua
validation and project discovery/hot reload remain outside this profile.
Source Save remains host-owned. See
[ADR 0036](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0036-scoped-luau-inspector-drawers.md).

The generic `ValueBinding` receives an immutable reflected value, opaque host token,
access/reason and metadata, never mutable source/gameplay World. Callback World arguments
refer only to presentation. The host owns snapshots through the entire borrow; callbacks
must not retain `ValueBinding`, discovery rows or borrowed fields after mount/update returns
or after replacing the snapshot. Scope retirement/deletion yields Target unavailable, never
a same-named replacement. Lock pins identity, not frozen values.

References need host-supplied symbolic context, not ephemeral entity numbers. The source
adapter explicitly reports representation limitations for `Entity` and `Option<Entity>`;
a lowered unbound port must not be displayed as explicitly authored `None`. When exact intent
cannot be recovered through the existing API, show that limit instead of a second decoder.

## Discovery and deterministic specialization

Bevy `TypeRegistry`, represented `TypeInfo`, `PartialReflect` and native paths own discovery.
Rows are disposable projections, not another serialized schema, type registry or universal
value representation. Primitive/string values preserve exact type/precision; structs, tuples,
tuple structs, active enum variants and bounded collections expose structure. Opaque/missing
capabilities retain type and reason. Do not normalize all numbers through `f64` or serialize
values through strings as an intermediate model. Exact numerics use round-trip formatting.

Ordinary Rust registration resolves explicit field editor (native owner type plus parsed
Bevy path), then exact-type editor, structural default and unsupported presentation.
Duplicate same-key/precedence registration fails without replacing the registry entry;
registration order, short names and namespaces do not determine dispatch.

Custom editors own mount/update/retirement, compose shared controls and may recurse with
the same budget. Equal-target refresh updates retained views; represented-type/editor
changes retire only the affected subtree. Mount errors must clean callback allocations;
errors remain visible, with an explicit default view reachable. Default mode bypasses
specializations throughout that selected subtree, including descendants. Both paths share
the immutable binding, native values, access and diagnostics; specialization cannot grant
writes or silently hide unsupported content. Built-in registration selects neither dynamic
project-code loading nor a stable extension ABI.

Actual exact-type and nested-field fixture presentations exercise controls, not dispatch
IDs alone. The panel routes disclosures owned by its component TreeViews, including base-owned
nested blocks. Custom consumers that mount additional recursive TreeViews must still forward
those separate owners' inner-tree actions.

`kedra_inspector::builtin::editors` installs native Vec2/Vec3/Vec4, Quat and Transform
presentations in the source panel and independent value consumer. Math uses the shared authored
`kedra_ui::fields::VectorView`; Transform composes Position / Rotation / Scale rows at full
property width. Block drawers receive a body inside the base Inspector's section, not ownership
of component chrome. Headers, collapse/expand, retained controls and collapse state across
default/custom modes use the same base path as ordinary reflected groups. Root components reuse
their existing header. Built-in title/layout/search terms never change binding identity.
Rotation displays degrees in Bevy YXZ order (Y yaw, X pitch, Z roll), not Unity's coordinate
convention. Only a display copy within the source unit tolerance is normalized; the quaternion
source is never rewritten. Invalid rotations report a diagnostic with default view available.
Angle editing is enabled only by the source Transform adapter. Continuity across accepted
gestures and retained full-turn intent are not implemented. These drawers never grant
source write authority on their own.

## Retention, budgets and native text

Expansion, search, default mode and lock are view-local. Equal-target refresh preserves
controls, expansion, native selection and scroll. Search retains target and ancestor/component
context; filtering/collapse never changes document selection. Tool focus does not activate
another document. Hide/Reopen, Float/Redock and maximize reuse the retained tool and shared
docking/actions.

The shared rebuild ceilings are reflected depth 32, 128 collection entries, 2048 discovered
rows and 4096 Unicode scalar values per preview. Expansion/search/custom recursion share
the allowance; filtering cannot initiate unbounded traversal. Every exhausted budget has a
visible indication; truncated previews are not complete copyable values and never modify
sources. Collections expose a first bounded segment, not pagination. Map keys/set membership
have no invented positional field identity; Bevy-opaque types such as `BTreeSet` need a custom
presentation.

Read-only text uses native selection/copy where supported, not editable fields with ignored
writes or a competing text editor. Search is not a source edit session. OS clipboard must
be independently verified before claiming copy/paste support. `kedra_ui/system-fonts` opts
native hosts into Bevy/Parley OS fallback discovery, without bundling proprietary fonts.
Installed CJK fallback availability varies by host; upstream ICU complex-script segmentation
warnings remain. This is not IME or universal glyph certification.

## Visual authority and acceptance

Canonical [UI-kit provenance](../ui/design-provenance.md#inspector-and-shared-controls):
Flat compact A `131:20213` (320px)
and `131:20388` (480px). Identity is 40px, headers 24px, controls 22px including borders;
related rows use 2px gaps and groups/sections 4px. The Window glyph fixture stack is 394px
with unchanged values/units, not a universal schema height. Scroll-host padding is 8px
vertical / 12px horizontal, not repeated by nested groups. Typography is 12/16, icons 16px;
below a 296px property body labels stack above values. Native layout, semantic tokens and
exact exports/shared authored assets remain authoritative. Transform/Mesh/Material are
examples, not fixed schemas. Math/Transform drawers use the rotation projection specified above;
raw/default inspection retains the native representation.

Do not expose unsupported Add component, Reset, Apply/Revert or asset-location actions.
No selection, unavailable/locked targets, empty components, missing capabilities, custom
failure, limits, no matches and narrow/floating appearances need owner review before shipping;
the baseline fixture is not acceptance evidence for them. Missing required compositions
need design-owner review. Physical input, OS clipboard/IME, screen readers, pixel equivalence
and diagnostic/narrow owner acceptance remain incomplete.

## Verification

Production APIs are in
[`kedra_inspector`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_inspector/src/lib.rs),
[`documents/inspection.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/documents/inspection.rs)
and
[`inspector`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/inspector/mod.rs).
Inspector public contracts cover native precision/structure, unsupported types, dispatch,
custom failure/retention/default fallback, shared limits, collection draft isolation,
complete proposals and native item-buffer settlement. Source tests own effective
overrides, provenance, current-revision refresh through property acceptance/Undo/Redo, failed
refresh isolation, revision/scope retirement and reference limitations; panel tests own
lock/retained controls, complete collection source admission and stale/rejected Apply
without a viewport or source writes. Run repository checks and:

```sh
cargo xtask test -- features --only inspector,inspector-ui,inspector-collections,inspector-fixture --output target/test/inspector
```

Both real consumers require native frames/input, keyboard access, scrolling, retention,
host movement and unchanged source/history snapshots. `inspector` owns spatial/caption
interaction and Save; `inspector-ui` separately owns grouped Layout and local collection
Apply/Cancel, hidden-draft guards and collection Save.
`inspector-collections` owns image density and shadow draft interaction, resource
preservation, preview/history, narrow details and existing-document Save.
The runner builds a verified delivery
and isolated editor processes. The source scenario copies checked `tests/fixtures/test_project`
and reads existing `Inspector/` sources; it generates no project/prefab data. The independent
consumer selects `editor://fixtures/inspector.scene.ron` through debug `--scene`, using authored
composition and native reflected values. `inspector.presentation` Luau drives refresh/default
input; Rust independently asserts precision, specialization, fallback and retention. Native
adapters retain registration, read-only refresh and disclosure behavior; independence is from
document authority, not a runtime-to-editor dependency.

The scenarios inject Bevy input and synchronize GPU readbacks; physical OS input needs a
separate check. Temporary readbacks are discarded; explicit review captures stay in the
private native session, not public reports or screenshot goldens. Store generated evidence
under `target/`. Offline references must come from Figma,
not application captures. Native verification of this revision remains pending.
