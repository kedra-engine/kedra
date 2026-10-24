# Composition and overrides

Composition lets a document reuse content without copying its source nodes. The result
is an effective hierarchy with the origin of nodes, components and field patches retained.
It is still source data, not a live simulation.

Start with the [document fields and reference forms](index.md#document-fields) if you
have not yet read the source format.

## Nest a prefab or derive a variant?

**Nesting** places a prefab below a parent in another document. Each composition site
has its own stable `InstanceId`, referenced prefab `asset`, parent, port bindings and
ordered overrides. Repeating a prefab creates independent scopes, not duplicate node IDs
in one flat namespace.

**A variant** derives from a `base` document of the same kind. It has its own document
identity but keeps the inherited root-node identity. A variant can add local nodes and
instances and layer overrides over inherited content. `PrefabDocument::variant` creates
this source relationship in memory; it does not write a file.

Choose nesting to assemble several objects. Choose a variant to specialize one existing
document while retaining its base relationship. A Scene may derive from another Scene,
but neither can be used as a nested prefab.

## How layers combine

For an entry document, the resolver first composes its base, if present. It adds local
nodes, composes direct nested instances and then applies document overrides in order.
At each nesting site, the referenced prefab is resolved and its site overrides applied
before its addresses are prefixed with that site's identity.

An override at one site does not edit the referenced prefab or a sibling site. Updating
the referenced source changes later compositions; an already compiled snapshot or live
instance continues to own its previous result.

### Example: a caption on one nested instance

The `Inspector/Main.prefab.ron` fixture nests `Inspector/Child.prefab.ron` and
sets a caption on that site. This is its site override, reformatted as an excerpt:

```ron
SetField(
    node: (instances: [], node: "e5d2452a-3c4c-49a3-8623-1ae42a5cfe69"),
    component: "ui.caption",
    version: 1,
    path: "text",
    value: "Override 日本語",
)
```

Here `instances: []` is relative to the referenced prefab inside the site override.
In the containing document's effective graph, that same node address is prefixed by
the site's `679f200f-4a9f-440d-8097-50f1647f3b62` identity. Another site can use the
same child-node UUID with a different prefix and a different caption.

## Supported override operations

- `SetField` replaces one reflected field using its exact component version and a typed
  RON value. Overlapping field patches in a layer are rejected.
- `SetComponent` replaces a whole component and clears its inherited field patches.
  `RemoveComponent` removes an existing component.
- `AddNode`, `RemoveNode` and `RenameNode` change authored structure or labels.
  Removing a node removes its descendants; the root cannot be removed.
- `ReparentNode` moves within a prefab boundary. Crossing a boundary requires Unpack.
- `SetChildOrder` lists every direct child exactly once; it is not a partial reorder.
- `AddNestedInstance`, `RemoveNestedInstance` and `RemoveNestedInstanceAt` manage
  composition sites. The latter uses a complete nesting scope.
- `SetBinding` changes a declared direct-site port binding. `RemapReferences` preserves
  symbolic addresses during explicit source restructuring.

These operations change source composition, not arbitrary ECS state. Missing targets
and invalid hierarchy changes reject the candidate.

## Ports connect to content outside a prefab

A local reference stays within its authored composition scope. A port lets the containing
document or runtime caller supply a target without embedding that target's identity in
the reusable prefab. Nested bindings can target a local node or forward another port.
All declared bindings are checked, including bindings unused by effective components.

Required ports must be bound. Optional ports may be absent, but their presence must match
the compiled contract at runtime. An unknown binding or a stale target is an error.
External targets do not become children owned by the instance merely because a component
references them.

For passing these bindings to live entities, see
[explicit instantiation](lifecycle.md#instantiate-a-prefab-explicitly).

## Apply, Revert and Unpack

These names describe existing Rust authoring operations, not current editor buttons.

**Revert** removes an originating override so the inherited value becomes effective.
**Apply** moves an effective field or component value to its actual source and removes
the originating override atomically. This can affect other consumers of that source.
Apply rejects a shadowed destination or a reference that cannot be represented there;
it must preserve the originating effective value.

**Complete Unpack** materializes a selected nested subtree into local source nodes,
including descendants, and removes that composition relationship. It allocates new
identities where needed, remaps references, retains effective ordering and leaves other
instances linked. It returns an address mapping for restoring selection through history.
It preserves symbolic optional-port intent rather than capturing runtime `None` values.

Apply and Unpack require a compilation matching the exact current transitive sources,
even if a changed value is hidden by an override. See
[Source editing and Save](source-editing.md) for revision guards and persistence.
