# Hierarchy

Hierarchy shows the active scene or prefab's **composed source graph**: its root, local
nodes, inherited content, overrides and nested prefab instances. It does not list every
runtime entity, preview helper, imported model part or editor control.

Open a scene/prefab from [Assets](asset-browser.md) first. A successful Open selects its
root; switching document tabs changes the tree and restores that document's selection.
Hierarchy remains usable when Scene View cannot render the document.

## Navigate and select

Branches start expanded. Click a node's name row to select it; click its disclosure button
only to expand/collapse. Selection is shared with Scene View and an unlocked Inspector.
It is not an edit, and selecting a node does not move the preview camera.

| Control | Result |
| --- | --- |
| Up / Down | Select the preceding/next visible row. |
| Home / End | Select the first/last visible row. |
| Left | Collapse an expanded branch; otherwise select its parent. |
| Right | Expand a collapsed branch; otherwise select its first visible child. |
| Vertical scrollbar | Reach rows outside the current viewport. |
| Horizontal scrolling | Reach deeply indented content. |

Keyboard focus and selection are distinct. Moving focus away leaves the selected node
unchanged, with an inactive selection appearance. Collapsing a branch does not discard
a selected descendant. The panel displays visible rows on demand rather than omitting
the tail of a large admitted graph.

Instance roots have blue names and a link icon. Active selection uses the selection text color;
inactive selection retains the prefab color. Equal names inside repeated prefabs are not the same
target: each selection includes its full nested instance scope. Ordinary branch icons and generic
node icons do not identify a node as a mesh or imply component types.

## Create and manage nodes

Right-click inside Hierarchy for **Create**: **Empty Node**, **3D Object → Cube /
Sphere / Plane**, **Light → Directional Light**, or **Camera**. Right-clicking a row
creates a child; empty space creates under the document root. Plane is a thin cuboid.
Camera adds authored perspective settings; it does not replace Scene View's camera
or enable the unimplemented Play workflow.

Right-click a node for **Rename** (F2) or **Delete** (Delete key). Delete removes the
selected subtrees; the document root cannot be deleted. Drag nodes onto another row
to change their parent, or onto empty space to move them to the root. World position,
rotation and scale are preserved; moves requiring shear or a zero-scale parent are
rejected. Cross-document moves, sibling reordering and drag autoscroll are unavailable.

Edits support **Undo/Redo** and remain in memory until **Save** (Ctrl/Cmd+S).

## Search and reveal

Use **… → Search nodes**, or Ctrl/Cmd+F with focus in Hierarchy, to open the search bar.
It is hidden by default. Search matches node names with Unicode case folding, retaining
ancestors as muted context, including matches inside collapsed branches.

- Filtering does not change document selection, even if the selected row disappears.
- Close search with its close action, or Escape while in the search field. This clears
  the query and restores ordinary expansion and pre-filter scroll.
- **… → Reveal selected node** clears the query, expands the selected node's ancestors,
  then scrolls to and focuses its row.
- **… → Toggle source details** shows additional name, source and scoped-identity context.

Search is limited to 256 characters and rejects control characters. The search bar's
additional filter action is disabled; extra node-type/component filters are not available.
The **…** menu is the shared dock-group menu, not a structural node context menu.

Expansion, filter and scroll are retained per document during the session, including
Hide/Reopen, Float/Redock and Reset. Do not rely on their restoration after an editor restart.

## Example: distinguish repeated prefab nodes

For fixture setup, start with [your first document](../getting-started/first-document.md).
For scoped source identities, see
[Scenes and prefabs](../scenes/index.md#identities-and-references).

The repository's test project contains `assets/Hierarchy/Main.scene.ron`. Open it from Assets:

1. Confirm the selected root is **Main scene**.
2. Open **Search nodes** and search for `STRASSE`.
3. The composed tree contains two **Straße leaf** matches, each under a different instance.
4. Select the first, then the second. The displayed names match, but the scoped selection
   changes; **Toggle source details** helps distinguish their addresses.
5. Search for `nothing matches`. The tree shows no matches, but retains the second selection.
6. Choose **Reveal selected node** to clear the filter and return to that exact instance.

For a spatial example, open `Composed.scene.ron` and select **Warm sphere**. Inspector
shows that node's authored components; a ready Scene View shows its selection bounds.
Use Scene View's **Frame selection**, not another Hierarchy selection, to move the camera.

## Empty and failure states

With no active source document, Hierarchy provides Open guidance. Activating a PNG/GLB
asset preview also leaves it without a document graph. A root-only document is different:
it still shows one real root node, even if that node has no components or children.

Source-opening status, no search matches and a failed Open have distinct messages.
Failed Open preserves the previous document, tree and selection and identifies the kept
tree in its diagnostic. A malformed source can reject admission before any tree exists;
the editor does not substitute a default hierarchy.

Duplicate and prefab unpacking are not implemented. Effective graphs
are bounded to 4096 nodes by source composition; this is a rejection limit, not a promise
that arbitrarily large files will load. See [composition](../scenes/composition.md).
