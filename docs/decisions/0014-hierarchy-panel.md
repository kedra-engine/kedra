# ADR 0014: Document Hierarchy panel

Status: read-only navigation/selection implemented; broader editing lifecycle and
platform acceptance remain proposed. See [current contracts](../ui/hierarchy.md).
This ADR covers document navigation and selection before structural editing.

## Purpose and authority

Hierarchy presents nodes of the active scene/prefab, including nested-instance
provenance, and requests selection through the
[document coordinator](0012-document-sessions-and-selection.md). It is not a tree
of component types or a dump of live Bevy entities. Inspector presents components
of the selected node. Source node identity uses existing persistent addresses.

Build a separate editor hierarchy feature module: ordinary tree-view state,
document projection and retained native presentation. The document owns node order,
parentage and selected address; the panel owns expansion, filter, keyboard row and
scroll. Rendered row entities are disposable presentation handles, never selection
authority. No second graph/composition engine or inspector-owned document copy.

## Docking contract

Register Hierarchy as its own retained tool view through existing docking. It can
move to any group, split, maximize, float and redock. It has no reserved left edge.
Moving it preserves expansion/search/scroll/focus; hiding it does not close the
active document. Reopen reveals the same view and current shared selection.
Settle active field/popup sessions before hide or target replacement.

Dock headers use the common group menu. Hierarchy-specific search/actions remain
inside feature content or the shared panel-action menu; do not duplicate window
chrome or create a parallel tab strip.

## Tree and selection behavior

Render authored child ordering from the validated composed source graph. Show the
document root, native/source node names and explicit prefab-instance boundaries.
Nested-instance child addresses include the full instance scope, so two instances
of the same prefab cannot share selection accidentally. Presentation icons do not
change component identity or require a special Kedra node type.

First support one selected node or explicit no selection:

- Click requests selection; keyboard arrows traverse visible rows with native
  tree semantics. Left collapses/goes to parent; Right expands/goes to child.
- Selection changes publish only after affected field sessions settle. Rejected
  settlement leaves the previous selected address and inspector target intact.
  Keyboard focus can remain on an attempted row without lying about selection.
- Collapse and filtering retain document selection, even if its row is hidden.
  Offer Reveal selected to expand ancestors and scroll to it explicitly.
- Focusing another tool only changes focus styling, not selected node identity.
- Repeatedly selecting the same node does not recreate its inspector or preview.
- A node removed by an accepted edit uses explicit source address mappings when
  available; otherwise selection clears. A reused row index never becomes a target.

Initial search filters node names while retaining ancestors as context. Matching
and context rows are visually distinguishable. Clearing search restores expansion
and scroll where valid. Filtering is non-destructive and does not issue scene edits.
Search-field acceptance is a search policy, not a source-property transaction.
Select a maintained Unicode matching strategy with the browser before implementation.

## Document and failure states

Distinguish no active document, opening, valid empty content, invalid/unavailable
source and a valid read-only document. An authored document root is not omitted
merely because it has no children. A compilation failure is not an empty hierarchy.
The panel may retain a last-valid tree only with an explicit stale/diagnostic state;
editing is disabled until the coordinator supplies an authoritative current graph.

Read-only source still permits selection, expansion, search and reflected inspection.
Unknown schema data must not produce a runnable/default replacement. Whole-scope
admission failure may prevent a tree entirely under current authoring APIs; show
that limitation and the preserved source diagnostic rather than claiming recovery.

Do not recursively render unbounded rows. Respect existing document/composition
limits and specify visible-row virtualization/expansion bounds before implementation.
Long names retain their actual text; clipping/tooltips do not mutate identity.
Async projections carry document revision and view generation; stale results cannot
replace a newer tree or restore old selection after navigation.

## Structural commands are separate increments

Figma includes rename, menus, reparent insertion/blocked states, prefab actions and
drag autoscroll. They are not required merely to navigate/inspect a document.
Enable them only with source transactions, validation, undo and explicit provenance:

- Rename preserves ID; validation and text cancellation are a dedicated workflow.
- Add/delete/reorder/reparent operate on sources, not runtime `Children` directly.
- Prefab-instance edits target overrides or an explicitly opened source; never
  silently move nodes across prefab boundaries or Unpack as a side effect.
- Spatial reparenting needs an explicit transform/world-pose policy. UI hierarchy
  moves cannot inherit a blanket 3D pose-preservation promise from a caption.
- Apply/Revert/Unpack use existing authoring contracts, not panel-local operations.

Until those increments are designed and verified, commands are absent or visibly
unavailable with reasons. No inert menu action or drag affordance implies support.

## Figma and acceptance

Use [UI-kit](../ui/figma.md) Hierarchy state library `125:17354`, masters
`120:15066`, rows `120:12800` and shared content `128:31375`. Search ancestors,
focused/inactive selection, collapsed rows and read-only navigation are intentional
contracts. Loading, admission failures, pending selection settlement and stale trees
need reviewed compositions in addition to the existing examples.

Test two nested instances with identical names/inner IDs, selected hidden rows,
filter clearing, failed/cancelled settlement, source revision replacement, deletion,
mapping on undo and tree limits. Assert persistent addresses and no source mutation
for navigation. Native acceptance covers keyboard semantics, independent focus,
scroll/selection retention through docking, float/redock and hide/reopen.

Delivery order: bounded model/projection, read-only native tree, document selection
integration, then separately designed structural operations. Run `cargo xtask check`
and relevant integration/native checks for each implementation slice.
