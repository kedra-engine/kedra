# ADR 0011: Universal reflected inspector and specialized editors

Status: proposed for owner review. This refines the inspector boundary in
[ADR 0009](0009-property-editing-and-inspector.md). The existing `kedra_inspector`
crate and editor panel provide the [read-only profile](../ui/inspector.md).
Full editable bindings, source settlement and additional component-authoring
capabilities remain proposed; read-only implementation does not accept that plan.

## Context

Kedra extends Bevy rather than replacing its components. Bevy's `Node`,
`Transform` and other reflected types must not require equivalent Kedra types
or handwritten editors merely to appear in the inspector. Kedra UI components
are real early consumers, not the inspector's universe of supported schemas.

The inspector needs a generic default editor and optional specialized editors
for common or domain-specific types. The same mechanisms must eventually support
project-defined editors without creating a second reflection or serialization
system. This is an architectural requirement, not a promise that every reflected
type can already be saved or has a meaningful editable representation.

## Decision

### 1. Reflection drives the default editor

Use Bevy `TypeRegistry`, represented `TypeInfo`, `PartialReflect`/`Reflect` and
native reflected field access. Editor descriptors reference those identities and
paths. Do not maintain another list of property types, replicate native component
schemas, or define a Kedra-only component trait as a condition of inspection.

Given a registered value, the default editor discovers its reflected structure
and composes existing shared field controls. A new reflected component with
supported fields must work without a new inspector implementation. Do not key
discovery on source namespaces such as `ui.*`, renderer entity IDs, translated
labels or a hard-coded list of Bevy/Kedra components.

Some reflection capabilities are optional. A `Reflect` implementation alone does
not ensure a component is registered in the host's type registry, supports ECS
access through `ReflectComponent`, can be cloned or constructed, or has a native
serializer. Check capabilities explicitly and show missing ones with a reason.
Do not execute project code on open merely to obtain missing registrations.

The default editor follows this coverage model as controls are implemented:

| Reflected shape | Default presentation and edit semantics |
| --- | --- |
| Supported primitive/string | Exact-type shared field; no conversion through a universal float/string |
| Struct | Named fields, recursively using default or specialized child editors |
| Tuple/tuple struct | Positional fields with stable schema paths, not invented semantic names |
| Enum | Current variant and its reflected payload; variant changes require valid construction |
| Array/list | Bounded element presentation; edits carry a revision and structural preconditions |
| Map/set | Bounded entries; keys/membership are not ordinary positional field assignments |
| Opaque or unsupported value | Type and explicit limitation; a specialization may supply an editor |

This table is the intended universal model, not all-at-once implementation scope.
Until construction or collection commands exist, show the corresponding controls
as unavailable instead of manufacturing defaults or exposing a misleading editor.
Ignored/non-reflected Rust fields are not discovered by parsing source or memory.
Bound recursion and displayed elements; recursive data cannot hang the inspector.
Use explicit expansion and a visible truncation/limit indication, not silent omission.

Primitive leaves still need typed UI adapters: reflection does not itself provide
a text or color control. Implement each shared control once and compose it for
every reflected type. A user component made of supported fields does not need a
new leaf adapter merely because its Rust component type is new.

### 2. Separate value inspection from write authority

The inspector consumes a reflected value snapshot, host binding identity/token,
read/write capabilities and presentation metadata. It emits typed proposed edits.
The host owns the target, validates the candidate and acknowledges acceptance.
It is not handed an unrestricted mutable gameplay `World` or document workspace.

There are three separate admission questions:

1. **Can the value be discovered?** Does the host expose the component/value with
   sufficient Bevy reflection registration?
2. **Can this field be presented/changed?** Is its shape supported, and does the
   binding permit the requested operation?
3. **Can this change be accepted and persisted?** Does the consumer's source,
   serialization, validation and lifecycle contract admit it?

Source authoring uses existing canonical component keys/versions, symbolic
references, policies, candidate compilation and `WorkingSet` transactions. Those
checks belong to the source adapter. They must not become a second global type
registry or a reason to hide otherwise discoverable reflected values.

For a compatible source-authorable component, registering the type and its source
policy must be enough to use the default inspector; a bespoke drawer is optional.
For values lacking a source representation or write permission, show read-only
capabilities and explain the limitation. Do not silently serialize live state or
claim source persistence based on reflection alone.

An independent value fixture exercises generic inspection without authoring
documents. The first product adapter binds the same inspector to authored sources.
A future live/debug target needs an explicit mode and lifecycle decision; this
ADR does not authorize direct runtime editing or blur it with persistent authoring.

### 3. Default and specialized editors share one protocol

Support these distinct scopes:

- **Default reflected editor:** recursively exposes supported structure.
- **Type editor:** convenient presentation for every occurrence of one exact type,
  whether a component root or a nested field.
- **Field editor:** specialization for an explicitly identified field/context.
- **Component presentation:** grouping/actions around a component's value editor;
  it does not replace its write/undo/Save rules.

Selection order is explicit field editor, exact type editor, default structural
editor, then visible unsupported presentation. Duplicate registrations at the same
precedence fail visibly. An unavailable or erroneous specialized editor reports
that failure; it cannot silently hide fields. Provide an explicit raw/default
view using the same value binding, including when a specialization exists.

Custom editors compose the shared field kit and emit the same Begin/Propose/
Accept/Cancel interactions. They may batch related field proposals atomically,
but cannot bypass the host's revision checks, validation, history or persistence.
A specialized editor can deliberately show a compact subset; the remaining
reflected fields must stay reachable through an explicit advanced/default view.

Use ordinary Rust registration for built-in editors initially. Custom type/field
metadata uses existing Bevy type data/custom attributes where suitable, or an
editor-owned lookup referencing native type/field identities. Runtime component
crates must not depend on editor UI to define their ordinary reflected types.
Do not introduce mandatory editor macros, a new schema DSL or per-type crates.
Project loading, unloading, trust and compatibility remain stage 3 work.

### 4. Native components get convenient editors, not replacement components

`Transform` is a motivating specialized editor:

- Present translation, rotation and scale with shared numeric/vector controls.
- Keep the actual native representation and write only the intended fields.
- An Euler-angle rotation view requires an explicit axis order, units and continuity
  policy before implementation. Showing a quaternion as angles must not quantize
  an untouched rotation or generate source edits on refresh.
- Default reflected access remains available; specialization is convenience, not
  another transform model, authoring format or hidden runtime component.

`bevy::ui::Node` is another motivating case:

- Default inspection follows its real reflected fields and native enums/units.
- A specialized layout editor can group size, spacing, alignment and positioning,
  using the same property paths and edit protocol.
- Computed layout is not a writable source substitute. CSS ownership and explicit
  authored values remain distinguishable; disabling a write explains its owner.

Existing `UiLayout` stores native `Node` data inertly because materializing native
UI components can install presentation behavior. Inspect its contained native
value through the same generic editor; do not implement a duplicate Node schema.
The wrapper's current source key/version remain unchanged. A future flatter
presentation must preserve the actual source address and reveal that provenance.

Reflection alone does not remove source-world hook restrictions. When a native
component needs inert source representation/materialization, use the existing
data-to-live boundary or a reviewed adapter, not a mandatory replacement gameplay
component and not direct construction of hooked components in the source world.
Support for each such adapter needs explicit invariants and tests, independently
of whether its generic or specialized editor is already available.

### 5. Package boundaries support universality

| Owner | Responsibility |
| --- | --- |
| `kedra_ui::fields` | Shared field controls and native interaction; no reflection-based document authority |
| Existing `kedra_inspector` | Default reflected presentation and editor selection/composition; consumer-neutral edits remain proposed |
| Planned `kedra_authoring::properties` | Reflected source bindings, candidates, policy checks and source edit sessions |
| `kedra_editor` source adapter | Connect inspector bindings to authoring sessions and document/selection lifecycle |

`kedra_inspector` is an editor-layer crate with a real standalone value fixture
and editor panel consumer. It depends on Bevy and `kedra_ui`, not directly on
`kedra_authoring` or `kedra_editor`. The host adapter composes these responsibilities;
it is initially a coherent editor module, not another speculative crate.

Keep the consumer boundary small: value/capabilities, stable binding/token and edit
requests/results. It is not a general object model, command bus, reflection system,
service locator or plugin runtime. No additional inspector-core/value-schema crate
is justified by this split. The existing crate has concrete panel/value consumers.

## Delivery order and verification

The generic editor comes before convenient component specializations:

1. Complete the shared primitive field contracts and native-fit prerequisites.
2. Extend the existing read-only inspector with editable default fields for a
   reflected fixture containing nested strings, numbers and booleans. No Kedra
   component-specific drawer.
3. Bind it to an actual Kedra UI source, proving the same discovery/field path works
   without embedding its schema in the inspector. Add other reflected shapes in
   focused increments, with explicit unsupported states until each is complete.
4. Add type and field specialization with deterministic dispatch, default view and
   identical edit/validation semantics. Exercise a native type and a plain
   project-style type, not only Kedra-owned types.
5. Design and add the convenient Transform/layout editors individually, reusing the
   proven generic mechanism. Do not use these to postpone the default editor.

Tests must demonstrate that a new reflected fixture component needs no bespoke
inspector code; nested specialization and explicit default view agree on values;
unsupported/opaque/unregistered capabilities are explicit; stale paths and collection
changes cannot edit another element; registration conflicts are diagnostic; and
specialized edits receive the same source rejection and undo behavior as defaults.
Native `Node` data and `Transform` values must round-trip without conversion to
replacement component schemas. Deny inspector-to-authoring/editor production edges.

Exact UI/collection construction limits and enum defaults need focused specifications
before those editors ship. Reflection shape coverage, field-kit readiness and
source-authoring compatibility are separate acceptance claims. A passing generic
fixture is not proof of arbitrary source loading or arbitrary project code execution.

## Consequences

Kedra can inspect ordinary reflected Bevy and application data while progressively
adding better editors for common types. This requires a host adapter and explicit
capability diagnostics instead of convenient unrestricted mutation. It avoids
coupling the inspector to Kedra UI source schemas, rebuilding Bevy's type system,
or requiring custom inspectors for every component. It does not waive existing
source/runtime isolation or promise editable opaque values without an adapter.
