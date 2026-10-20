# Read-only document Hierarchy

Opening a scene or prefab in Assets populates Hierarchy with every node of its
validated **composed** source graph: the root, inherited/overridden nodes and
nested prefab instances. It does not enumerate runtime Bevy entities or just the
entry document's raw `nodes` array. Switching document tabs switches the tree.

## Navigation and identity

All branches initially expand. Click selects a source node through the project
document coordinator; disclosure buttons only expand/collapse. Up/Down/Home/End
select visible rows, Left collapses or selects the parent, Right expands or selects
the first child. Native keyboard focus is separate from persistent selection.

The default tree has no permanent search toolbar or UUID footer. **Search nodes**
in the shared group menu, or Ctrl/Cmd+F while focus is in Hierarchy, opens the
38px content search toolbar. Its flat close action (or Escape from the field)
clears/closes search without changing document selection. **Reveal selected node**
and **Toggle source details** remain available from the same group menu.

Search uses the browser's existing `unicase` Unicode matching. Matching rows keep
their ancestors as muted context, including inside collapsed branches. Filtering
and collapsing do not clear selection. Closing search restores ordinary expansion and
pre-filter scroll; Reveal clears search, expands selected-node ancestors and
scrolls/focuses its row. Search is a native retained field, bounded to 256 characters
with control characters rejected on insert/paste.

Document sessions own selected `NodeAddress` values, including the complete nested
instance scope. Two instances of a prefab can have equal inner Node IDs/names but
remain distinct targets. Instance roots carry the reference's link icon rather
than a suffix added to the authored name. Ordinary branches use a folder icon.
Optional source details expose full name, source document and scoped node identity.
The generic node icon does not claim that every node is a mesh or spatial object.
Source-provided names are never used as locators or UI identity.

Selection is checked against the active session's admitted graph. Deferred input
from an inactive document or hidden tool is rejected. Reopening/activating another
tool does not replace document selection. Each Hierarchy view retains per-document
filter, expansion and scroll; Float/Redock, Hide/Reopen and Reset keep the native
panel/search identity. Navigation is session-local, not process-restart recovery.

Rows use one continuous selection/hover surface, 16px icons, 6px icon/name gaps
and 7px horizontal padding. Keyboard focus has an independent 2px perimeter,
without changing layout. Selection in an unfocused tool uses the design's inactive
gray; focus changes never select a different node. Active selected leaf icons use
the reference's selected export. The search filter action is explicitly disabled
with a hover explanation: additional node filters are not implemented. No new
filter semantics, structural menus or runtime component types are inferred from
display names merely to reproduce a design specimen.

## Authority, limits and failure

`kedra_editor::hierarchy::model` projects `ResolvedGraph` into preorder rows using
the existing composer's effective `children` order. It never interprets prefab
overrides or resolves references itself. Document sessions retain one immutable
composed snapshot per open source. This read-only profile has no revision changes
or background hierarchy projection; future edits must add explicit graph-revision
invalidation and selection remapping before mutating that snapshot.

The source/composition limits still apply: at most 4096 effective nodes per graph,
128 admitted documents and existing composition work/depth limits. An iterative
projection handles the full graph; visible-row virtualization mounts only a native
viewport window plus overscan. Rows are 22px high with 12px depth indentation.
Groups containing only Hierarchy use the reference's 232px minimum; mixed groups
keep the existing shared 240px minimum. This does not alter other tool minima.
Horizontal scrolling preserves access to deeply nested rows; vertical scrolling
uses native Bevy `Scrollbar`, not a custom drag implementation. No node-count
truncation silently hides an admitted graph's tail.

No-document guidance, source-opening status, no search matches and failed Open are
distinct. An authored root with no children still renders as one node. Failed Open
keeps the existing document/tree/selection and identifies that preserved tree in a
diagnostic. Whole-scope source admission can fail before any tree is available;
Hierarchy does not replace unsupported source data with default nodes.

Navigation never changes source bytes, authoring revisions/history or gameplay
state. Inspector, source editing, drag/reparent, rename, add/delete and Save remain
unavailable. There is no field settlement contract yet because no editable source
fields are installed; future Inspector editing must integrate it before selection
can replace an edited target.

## Authored UI and verification

`hierarchy.prefab.ron` composes the existing field, button and scrollbar controls.
`hierarchy-row.prefab.ron` specializes the shared disclosure/name row; Flair and
existing tokens own presentation. Common docking chrome remains unchanged.
The reference is UI-KIT `128:32776` under Hierarchy `125:17354`; node/scene SVGs are
exported from that reference, and disclosure/folder icons reuse exact project assets.
Search `128:32885`, keyboard focus `128:32955`, linked-prefab marker `128:33170`
and collapsed `128:33627` supply the supported state compositions. Shared docking
chrome is reused, not locally redrawn. Rename/reparent/Unpack specimens remain
outside the read-only tool's supported actions.
See [source map](figma.md) and [ADR 0014](../decisions/0014-hierarchy-panel.md).

Run `cargo xtask check --non-interactive` for required repository checks. Model tests
cover composed order, variants, duplicate inner IDs, root-only documents, the
4096-node graph and non-destructive search/reveal. Coordinator/panel tests cover
active-document selection checks and hidden/stale input without source writes.

For native acceptance, launch a fresh editor at Projects with isolated credentials,
artifacts and state, then run:

```sh
python3 tools/qa/hierarchy.py target/hierarchy-credentials.json \
  --state-directory target/hierarchy-state --report target/hierarchy-report.json
python3 tools/qa/hierarchy_appearance.py target/hierarchy-credentials.json \
  --state-directory target/hierarchy-state --report target/hierarchy-appearance.json
```

The scenario opens a scene with two 181-node nested instances, exercises real
native controls and captures renderer frames, verifies distinct scoped selection,
virtualized tail navigation, search/reveal, tab switching and retained docking.
The appearance scenario additionally checks default chrome, 22px row/flat-action
geometry, 4px insets, 12px indentation, selection color, optional search/details,
and 232/320px hosts, retaining renderer frames for state review.
It uses injected Bevy input, not physical OS input, and does not certify IME,
mixed-monitor behavior, independent Figma pixel equivalence or scene rendering.
