# Hierarchy contract

Hierarchy implements source navigation, selection and structural commands under
[ADR 0014](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0014-hierarchy-panel.md),
with shared source transactions/history. See [Hierarchy in the Manual](../../../manual/editor/hierarchy.md)
for navigation, search and Reveal.

## Graph authority and scoped selection

`kedra_editor::hierarchy::model` projects the existing `ResolvedGraph`, not live Bevy entities
or the entry document's raw `nodes` array. Its iterative preorder follows the composer's
effective `children` order, including inherited/overridden nodes and nested prefab instances.
The panel does not interpret overrides or resolve references itself. A root-only document
still produces one real node. Each open document retains one immutable composed snapshot.
Accepted edits replace the composed snapshot, rebuild affected retained rows and reconcile
source selection. There is no background hierarchy projection.

The [document coordinator](source-documents.md) owns selected `NodeAddress`, including the
complete nested instance scope. Equal inner IDs/names in two prefab instances remain different
targets. Source names never become locators or UI identity; renderer entities are disposable
row handles. Instance roots use a link icon without modifying authored names; ordinary
branches use folders and generic node icons do not imply spatial/mesh component types.
Optional details expose the full name, source document and scoped identity.

Selection must belong to the active admitted graph. Deferred input from inactive documents
or hidden tools is rejected. Focusing/reopening another tool cannot replace selection.
Native keyboard focus is distinct from persistent document selection. Filtering/collapse
retain selection; clearing search restores ordinary expansion and pre-filter scroll. Reveal
clears search, expands selected ancestors and scrolls/focuses the exact row.

## Retention, budgets and failure

The tool retains filter, expansion and scroll per document. Float/Redock, Hide/Reopen and
Reset preserve the panel/search identity; these are session guarantees, not restart recovery.
Search reuses `unicase` Unicode folding, retaining matching ancestors as muted context even
inside collapsed branches. The retained native field permits 256 characters and rejects
control characters on insert/paste. Additional node filters are disabled with a hover reason;
display names do not imply runtime type inference.

Existing composition ceilings apply: 4096 effective nodes per graph, 128 admitted documents
and the composer's work/depth limits. Projection handles the whole admitted graph; native
viewport virtualization mounts visible rows plus overscan, without silently truncating its
tail. Horizontal scroll keeps deep rows reachable; native Bevy `Scrollbar` owns vertical
dragging. No custom scroll-drag implementation is introduced.

No document, opening, no matches and failed Open are distinct states. Failed Open preserves
document/tree/selection and explicitly identifies the retained tree. Whole-scope admission
failure can prevent any tree; unsupported source is never replaced by default nodes.
Navigation changes neither source bytes nor authoring revisions/history or gameplay state.
[Inspector](inspector.md) follows the shared selection. Content right-click menus create
empty spatial nodes, cubes, spheres, thin-cuboid planes, directional lights and authored
perspective cameras. Blank-space creation targets the document root; row creation targets
the clicked node. These commands do not appear in dock-tab menus. Rename (F2), subtree
Delete and source Undo/Redo share the ordinary authoring transaction and explicit Save.
Commands pin source scope, target and revisions; stale commands and unresolved Inspector
drafts refuse without publishing a partial edit. Root deletion is refused.

Node drops onto rows reparent within the same receiving document; dropping onto empty
Hierarchy space reparents to its root. Ancestor/descendant multi-selection moves only the
selected outer subtrees. Spatial world pose is retained; cycles, singular parents and
nonrepresentable shear refuse atomically. Composition boundary rules remain source-owned.
Sibling insertion/reordering and drag autoscroll are not implemented.

`scene.camera` stores validated perspective parameters without activating a rendering
camera in the editor. Scene View retains its navigation camera; game camera activation
and Play remain separate runtime work.

## Authored presentation and reference

`hierarchy.prefab.ron` composes existing fields, buttons and scrollbars;
`hierarchy-row.prefab.ron` specializes the shared disclosure/name row. Flair and existing
tokens own presentation; docking chrome stays shared. The default view has neither a
permanent search toolbar nor a UUID footer. The optional content-search toolbar is 38px.
Rows are 22px with 12px depth indentation, continuous selection/hover, 16px icons, 6px
icon/name gaps and 7px horizontal padding. Keyboard focus has a separate 2px perimeter
without layout change. Unfocused selection uses inactive gray without changing the target;
active selected leaves use the exact selected export. Hierarchy-only groups have a 232px
minimum; mixed groups retain 240px. Other tool minima do not change.

Canonical sources are UI-KIT `128:32776` under Hierarchy `125:17354`, search `128:32885`,
keyboard focus `128:32955`, linked marker `128:33170` and collapsed `128:33627`.
Node/scene SVGs originate there; disclosure/folder icons reuse exact project assets.
See [Figma provenance](../ui/design-provenance.md#hierarchy-exports).
Unpack specimens remain references; prefab unpacking is not implemented.

## Verification and evidence boundaries

Production owners and their local tests are under
[`hierarchy`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/hierarchy/mod.rs).
`tests.rs` covers composed order, variants, duplicate inner IDs, root-only graphs, 4096 nodes
and non-destructive search/reveal. Coordinator and workspace panel tests own active-document
checks, hidden/stale input and unchanged sources. Required repository checks use
`cargo xtask check --non-interactive`.

```sh
cargo xtask test -- features --only hierarchy --output target/test/hierarchy
cargo xtask test -- features --only hierarchy-editing --output target/test/hierarchy-editing
```

On an idle desktop the native runner uses isolated credentials/state/artifacts and its own
copy of `tests/fixtures/test_project`, opening `Hierarchy/Main.scene.ron` with two 181-node
nested instances. It generates no source scene/prefab data. Native controls and renderer
completion fences exercise distinct scoped selection, virtualized tail navigation, search/
reveal, tab changes and retained docking.

Behavior and usability run sequentially once in one native session, with separate phase
statuses and renderer-frame receipts. A behavior failure leaves usability explicitly not
run. Usability checks row containment and hierarchy ordering, keyboard focus, optional
search/details and interaction in compact floating and resized docked panels, not exact
spacing or palette values. Reports retain
physical dimensions and frame receipts, not viewable images; validated temporary PNGs are
discarded after synchronization. These checks inject Bevy input; physical OS input, IME and
mixed-monitor behavior need separate tests. They do not compare pixels against Figma or verify
scene rendering. Native verification of this revision remains pending.
