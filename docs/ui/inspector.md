# Inspector foundation design

## Scope and status

Read-only implementation profile for [ADR 0015](../decisions/0015-inspector-panel.md),
[ADR 0011](../decisions/0011-universal-reflected-inspector.md) and
[ADR 0009](../decisions/0009-property-editing-and-inspector.md).
The shared field kit, reflected presentations and source-bound panel are implemented.
Native acceptance is incomplete: physical input, OS clipboard/IME, screen readers,
pixel equivalence and owner review of diagnostic/narrow states are not certified.
This delivery is read-only, including built-in custom presentations; it is
not a disposable implementation preceding a different editing architecture.

An author selects a source node through Hierarchy or Scene View and inspects its
registered components, nested values, types and source context. Inspection does
not require a successful viewport frame or a verified renderer delivery.

## Ownership

| Owner | Responsibility |
| --- | --- |
| `kedra_ui::fields` | Reusable retained property rows, value surfaces, labels, diagnostics, native read-only interaction and responsive layout |
| `kedra_inspector` | Bevy-reflected discovery, bounded presentation, editor dispatch, default/custom composition and retained value views |
| `kedra_editor::inspector` | Source bindings, target following/locking, provenance and dockable panel lifecycle |
| `kedra_editor::documents` | Existing admitted workspace, active document and persistent node selection |
| Existing source compiler | Typed effective source values and reference resolution |

The runtime UI kit cannot depend on inspector, authoring or editor crates. The
inspector is an editor-layer crate depending on Bevy and `kedra_ui`, not authoring,
desktop or editor. A non-document value consumer in the ordinary editor executable
exercises the same controls without passing authoring state to the inspector.
Shared widgets must not be hidden in the inspector crate or copied into the panel.
Feature models are ordinary Rust values; entity IDs belong to UI adapters only.

Authored prefab/CSS primitives remain reusable assets in the existing delivery
mechanism. The shared Rust controls receive explicit compiled UI templates and
theme context; they do not discover editor asset UUIDs or read editor resources.
Editor-specific selection identity and source context remain panel composition.

## Binding and source authority

The editor resolves a target using its project/source-scope generation, document
identity and scoped `NodeAddress`. Component keys/schema versions identify source
components; native represented types and Bevy reflected paths identify values.
Labels, translated names, display order and renderer entities are never bindings.

Reuse immutable compiled source values and existing `ReflectComponent` access.
The adapter must enumerate authored components from the composed node, not include
compiler-generated `Name`, hierarchy or render components accidentally. Effective
field overrides must already be applied by the existing compiler. Preserve origin
metadata separately; computed UI layout is not authored layout.

The generic inspector receives immutable reflected values, an opaque host binding
token, access/reason and presentation metadata. It receives no mutable source or
gameplay World. UI adapter callbacks may operate on their presentation World;
that World is not the inspected data authority. The source adapter owns snapshots
for the complete duration of all reflected borrows. Never retain borrowed fields
after replacing their owning snapshot.

Reference display uses host-supplied symbolic source/asset context, never ephemeral
entity numbers as persistent references. Optional ports must not be mislabeled as
explicitly authored `None` merely because compilation lowered an unbound port.
If exact authored reference intent cannot be recovered through an existing API,
show that limitation rather than inventing a value or a second decoder.

Read-only capability is explicit throughout this delivery. No source mutation,
history, Save, component commands, pickers or project execution are enabled.
Do not invent a nonfunctional write protocol simply to reserve names; future
Begin/Propose/Accept/Cancel behavior remains subject to ADR 0009 and field gates.

## Default and custom presentations

Bevy `TypeRegistry`, represented `TypeInfo`, `PartialReflect` and native paths own
discovery. Presentation rows are disposable projections, not a serialized schema,
parallel type registry or alternate universal value representation.

The default presentation handles exact primitive/string values, named structs,
tuples/tuple structs, active enum variants and bounded collection structure.
Unimplemented shapes, opaque values and missing reflection capabilities retain
their type and an explicit reason. No conversion of all numbers through `f64` or
serialization through strings as an intermediate value model is allowed.

Ordinary Rust registration supports exact-type editors and explicit field editors
identified by native containing type and parsed Bevy field path. Selection order
is explicit field editor, exact-type editor, structural default, unsupported.
Duplicate registrations at the same key/precedence are errors, independent of
registration order. There is no implicit short-name or namespace matching.

A custom editor has mount, update and retirement responsibilities; it composes
shared controls and recursive child editors. Equal target refreshes update retained
views rather than remounting them. Changing its represented type/editor selection
retires only the affected subtree. Errors are visible and an explicit default view
remains reachable. Default mode bypasses specialization for that selected subtree,
including nested editors, so users can inspect its actual reflected structure.

The foundation must exercise a real exact-type presentation and a nested field
presentation with a plain application-style fixture, not just test dispatch IDs.
Both default and custom paths consume the same immutable binding, native values,
access and diagnostics. Specialized layouts cannot authorize writes or silently
hide unsupported content. No dynamic project-code loading or stable extension ABI
is selected by ordinary built-in registration.

The current panel routes disclosures for top-level component trees. A recursive
custom consumer must forward its inner trees' disclosure actions itself; standard
panel forwarding for owned child trees is not implemented. Shipped fixture
specializations are flat and do not establish nested-disclosure acceptance.

## Retention, limits and interaction

Expansion, search, default-view selection and lock are view-local. Lock pins the
target identity, not values. Refreshing the same target preserves controls,
expansion, native selection and scroll. Scope retirement or target deletion shows
Target unavailable and never retargets another same-named object.

Search retains the selected object's identity and component/ancestor context.
Filtering and collapse do not change document selection. Tool focus does not
activate another document. Hide/reopen, float/redock and maximize reuse the retained
tool view and shared docking/action infrastructure.

Implementation ceilings: maximum reflected depth 32, 128 visible collection entries,
2048 discovered
presentation rows per rebuild and 4096 Unicode scalar values per value preview.
Every exhausted budget has a visible limit indication. Expansion/search/custom
recursion share one rebuild budget; filtering cannot trigger an unbounded walk.
Preview truncation never modifies the source and is not presented as a complete
copyable value. Exact numeric values use round-trip formatting.

Collections currently show a bounded first segment, not interactive pagination.
Maps and sets expose membership without inventing positional field identity;
Bevy-opaque types (including `BTreeSet`) require a custom presentation. Source entity
references, including optional ports, show an explicit representation limitation.
Admission compilations are retained by `Workspace::open_compiled`; working revisions
other than their admitted revision invalidate inspection rather than returning stale values.
`kedra_ui/system-fonts` opts native hosts into Bevy/Parley's OS fallback discovery.
It fixes missing installed CJK glyphs without bundling proprietary fonts; available
fallbacks vary by host. Upstream ICU complex-script segmentation warnings remain.

Read-only text uses native selection/copy where supported; it is not an editable
field whose changes are ignored. Extend the existing native mode authority rather
than competing with it. Actual OS clipboard integration must be verified before
claiming OS copy/paste support. Search is not a source-editing field session.

## Visual authority and acceptance

Use the [canonical UI-kit](figma.md): Flat compact A `131:20213` (320px) and
`131:20388` (480px). Selection identity is 40px, section headers 24px and controls
22px including borders. Related property rows use 2px spacing; sections/groups
use 4px. The Window glyph reference stack occupies 394px with unchanged values
and units; this fixture is not a claim that every reflected schema has that height.
The scroll host owns 8px vertical / 12px horizontal padding so nested groups do
not repeatedly inset or compress their fields. Body typography remains 12/16 and
icons remain 16px. Below a 296px property
body, stack labels above values rather than squeeze numeric text. Use native
layout, existing semantic tokens, exact exported icons and shared authored assets.
Transform/Mesh/Material are examples, not a hard-coded component schema. Rotation
stays in its native representation until a separate Euler presentation is specified.

Do not expose Add component, Reset, Apply/Revert or asset-location actions without
implemented capabilities. Review no selection, unavailable/locked target, empty
components, missing capabilities, custom-editor failure, limits, no search results
and narrow/floating appearances before shipping; the baseline sample is not their
acceptance evidence. Missing required compositions need owner review in Figma.

Acceptance requires the non-document native field/value consumer and the actual
editor panel, using the same controls. Verify real frames/input, keyboard access,
copy limitations, scrolling, retention and host movement. Model tests cannot
certify native interaction. All source/history snapshots must remain unchanged.
Generated evidence belongs under `target/`; offline references originate in Figma,
not application captures. See [testing](../testing.md).

## Local acceptance commands

The Rust runner builds a verified delivery and launches isolated editor processes:

```sh
cargo xtask test -- features --only inspector,inspector-fixture --output target/test/inspector
```

The actual editor scenario opens a fresh working copy of the checked
`tests/fixtures/test_project`, with isolated state and new remote credentials.
It reads the existing `Inspector/` sources and checks source hashes, nested
effective values, locking, search and retained dock-host movement. No project or
prefab data is generated at test startup.
The `inspector-fixture` scenario selects `editor://fixtures/inspector.scene.ron`
through the ordinary executable's debug `--scene` argument. Its composition and
initial native reflected values are authored data. The registered
`inspector.presentation` Luau program drives refresh/default-view input, with Rust
retaining independent precision, specialization, fallback and retention assertions.
Native adapters retain editor registration, read-only refresh and disclosure behavior. This is
independence from source-document authority, not a runtime-to-editor dependency.
These scenarios inject Bevy input and capture native frames; they do not certify
physical OS input or provide screenshot coverage. Temporary GPU readbacks are
discarded after synchronization; behavior reports retain no image goldens or PNGs.
