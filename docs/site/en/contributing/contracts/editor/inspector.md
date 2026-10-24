# Read-only Inspector contract

Inspector presents source values read-only under
[ADR 0015](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0015-inspector-panel.md),
[ADR 0011](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0011-universal-reflected-inspector.md)
and
[ADR 0009](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0009-property-editing-and-inspector.md).
The same architecture is intended to support editing. See the
[Manual](../../../manual/editor/inspector.md) for inspection, search, lock and default-view
instructions. Interactive editing,
history, component commands, pickers, Save and project execution remain unavailable, including
inside built-in custom presentations. Future Begin/Propose/Accept/Cancel is proposed semantics,
not a write protocol exposed by this implementation.

## Owners and layering

| Owner | Responsibility |
| --- | --- |
| `kedra_ui::fields` | Shared retained rows, labels, value surfaces, diagnostics, native read-only interaction and responsive layout |
| `kedra_inspector` | Reflected discovery, bounded presentation, dispatch, default/custom composition and retained value views |
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
hierarchy or rendering components. `Workspace::open_compiled` retains immutable admission
compilations; existing `ReflectComponent` access reads effective overridden values. Origin
metadata stays separate, and computed UI layout is not substituted for authored layout.
Dependencies whose working revision differs from the admitted revision invalidate inspection,
rather than returning stale compiled values.

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
IDs alone. Current panel disclosure forwarding covers top-level component trees only.
Recursive custom consumers must forward their own inner-tree actions; shipped flat fixture
specializations do not establish general nested-disclosure acceptance.

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
examples, not fixed schemas; rotation remains native until an Euler policy is specified.

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
custom failure/retention/default fallback and shared limits. Source tests own effective
overrides, provenance, revision/scope retirement and reference limitations; panel tests own
lock/retained controls without a viewport or source writes. Run repository checks and:

```sh
cargo xtask test -- features --only inspector,inspector-fixture --output target/test/inspector
```

Both real consumers require native frames/input, keyboard access, scrolling, retention,
host movement and unchanged source/history snapshots. The runner builds a verified delivery
and isolated editor processes. The source scenario copies checked `tests/fixtures/test_project`
and reads existing `Inspector/` sources; it generates no project/prefab data. The independent
consumer selects `editor://fixtures/inspector.scene.ron` through debug `--scene`, using authored
composition and native reflected values. `inspector.presentation` Luau drives refresh/default
input; Rust independently asserts precision, specialization, fallback and retention. Native
adapters retain registration, read-only refresh and disclosure behavior; independence is from
document authority, not a runtime-to-editor dependency.

The scenarios inject Bevy input and synchronize GPU readbacks; physical OS input needs a
separate check. Temporary readbacks are discarded, so reports contain no PNGs or screenshot
goldens. Store generated evidence under `target/`. Offline references must come from Figma,
not application captures. Native verification of this revision remains pending.
