# Hierarchy contract

Hierarchy implements read-only navigation and selection under
[ADR 0014](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0014-hierarchy-panel.md),
without structural editing. See [Hierarchy in the Manual](../../../manual/editor/hierarchy.md)
for navigation, search and Reveal.

## Graph authority and scoped selection

`kedra_editor::hierarchy::model` projects the existing `ResolvedGraph`, not live Bevy entities
or the entry document's raw `nodes` array. Its iterative preorder follows the composer's
effective `children` order, including inherited/overridden nodes and nested prefab instances.
The panel does not interpret overrides or resolve references itself. A root-only document
still produces one real node. Each open document retains one immutable composed snapshot.
Future editing must add explicit graph-revision invalidation and selection remapping before
changing that snapshot; there is no background hierarchy projection or revision-update path
in this read-only profile.

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
display names do not imply structural menus, new filters or runtime type inference.

Existing composition ceilings apply: 4096 effective nodes per graph, 128 admitted documents
and the composer's work/depth limits. Projection handles the whole admitted graph; native
viewport virtualization mounts visible rows plus overscan, without silently truncating its
tail. Horizontal scroll keeps deep rows reachable; native Bevy `Scrollbar` owns vertical
dragging. No custom scroll-drag implementation is introduced.

No document, opening, no matches and failed Open are distinct states. Failed Open preserves
document/tree/selection and explicitly identifies the retained tree. Whole-scope admission
failure can prevent any tree; unsupported source is never replaced by default nodes.
Navigation changes neither source bytes nor authoring revisions/history or gameplay state.
[Inspector](inspector.md) follows the shared selection. Rename, add/delete, drag/reparent,
source editing and Save remain unavailable. Editable Inspector must add field settlement
before selection can replace an edited target.

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
Rename/reparent/Unpack specimens are
references, not implemented read-only actions.

## Verification and evidence boundaries

Production owners and their local tests are under
[`hierarchy`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/hierarchy/mod.rs).
`tests.rs` covers composed order, variants, duplicate inner IDs, root-only graphs, 4096 nodes
and non-destructive search/reveal. Coordinator and workspace panel tests own active-document
checks, hidden/stale input and unchanged sources. Required repository checks use
`cargo xtask check --non-interactive`.

```sh
cargo xtask test -- features --only hierarchy --output target/test/hierarchy
```

On an idle desktop the native runner uses isolated credentials/state/artifacts and its own
copy of `tests/fixtures/test_project`, opening `Hierarchy/Main.scene.ron` with two 181-node
nested instances. It generates no source scene/prefab data. Native controls and renderer
completion fences exercise distinct scoped selection, virtualized tail navigation, search/
reveal, tab changes and retained docking.

Behavior and appearance run sequentially once in one native session, with separate phase
statuses and renderer-frame receipts. A behavior failure leaves appearance explicitly not
run. Appearance checks default chrome, 22px row/flat-action geometry, 4px insets, 12px
indentation, selection color, optional search/details and 232/320px hosts. Reports retain
physical dimensions and frame receipts, not viewable images; validated temporary PNGs are
discarded after synchronization. These checks inject Bevy input; physical OS input, IME and
mixed-monitor behavior need separate tests. They do not compare pixels against Figma or verify
scene rendering. Native verification of this revision remains pending.
