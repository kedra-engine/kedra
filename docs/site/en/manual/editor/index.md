# Editor workspace

The editor is a desktop workspace for browsing project sources and editing supported
scenes, prefabs and asset properties. Source edits use Undo/Redo and explicit Save.
Opening a project does not run project code; moving a camera or panel does not edit sources.

Start with an [open project](../projects/create-open.md). The examples in this section use
an unchanged working copy of the repository's test project, `tests/fixtures/test_project`.
These assets are examples, not content automatically added to a new project.

## Find the right panel

For fixture setup and a guided first Open, follow
[your first document](../getting-started/first-document.md).

The default layout places Hierarchy on the left, Inspector on the right, and Document
Preview above Assets in the middle. This arrangement is a starting point, not a constraint.

| Panel | Use it to |
| --- | --- |
| [Assets / Asset Browser](asset-browser.md) | Browse source folders, search filenames and explicitly open a source. |
| [Hierarchy](hierarchy.md) | Select, create, rename, delete and reparent eligible source nodes. |
| [Scene View](scene-view.md) | Preview content, navigate, select meshes and transform eligible nodes. |
| [Inspector](inspector.md) | Inspect and edit supported source properties and manage components. |

Document Preview starts empty. Opening a scene or prefab creates a document tab and hides
that empty slot. A document tab contains Scene View, not a separate editable scene window.
PNG and static GLB files open in separate asset-preview tabs.

The header's Play, Pause, Stop, Build and global-search controls are disabled.
Save is available for eligible source changes; the main menu also offers Save All.
Build deliveries through the
[command-line workflow](../assets/pipeline.md).

## Arrange panels and tabs

Every panel and document view can move, share a tab group, split, float or hide. No central
area is reserved for documents. Use the group's **…** menu for commands; this is not a
node or asset context menu.

| Control | Result |
| --- | --- |
| Click a tab | Activate that view. |
| Drag a tab label | Reorder it, merge it into another tab rail, or split a group at an edge. |
| Shift-drag a tab label | Move its whole group, retaining tab order and active tab. |
| Drop on the compass center | Join the destination as tabs. |
| Drop on a compass edge | Split left, right, above or below the destination. |
| Drag a divider | Resize adjacent regions within their minimum sizes. |
| Double-click a divider | Restore its preferred ratio. |
| Escape during drag/resize | Cancel the pending layout change. |
| Left/Right/Home/End on a focused tab label | Activate the preceding, next, first or last tab. |
| Arrows on a focused divider | Adjust its ratio. |
| Space/Enter on a focused divider | Restore its preferred ratio. |

A destination preview indicates an admissible drop; an unavailable destination gives a
reason. Release over a valid target to commit. Drops that cannot fit the panels or whose
layout is not ready leave the published layout unchanged. Close buttons and group-menu
buttons are not drag handles. Tab rails scroll horizontally when titles do not fit.

The **…** menu contains panel-specific actions, **Close tab** and a **Layout** submenu.
Layout offers **Maximize group** / **Restore group** and **Float tab** in the main window.
Groups with multiple tabs also offer **Float group**, **Close group** and a **Tabs**
submenu with complete titles for selecting tabs whose rail labels are truncated.
Maximizing keeps the original split layout for Restore.

In a floating group, **Redock to main window** returns it to the main workspace.
Use explicit Float/Redock if your window backend cannot target a cross-window drag.
Floating a view preserves its panel state; it is not a second source session.

## Hide, reopen and reset

A tab's close button hides the view rather than deleting its source session. Closing a
floating window hides its views; closing the main window requests exit. Unsaved changes
offer **Save and exit**, **Exit without saving** and **Cancel**. The main menu's
**Windows** submenu lists views by name, reopening hidden views or selecting visible ones. Opening the same
source from Assets also returns to its retained tab.

**Reset layout** restores the default arrangement without recreating live panel content.
For example, open `Composed.scene.ron`, select **Warm sphere** in Hierarchy, then float
Inspector with **Layout → Float tab**. Redock it and reset the layout: the source document and
selection remain, while panel placement returns to the preset.

Per-project local preferences retain docking topology, tab order, active/maximized groups
and window placement. They are not document Save. Browser navigation and spatial camera
poses are session state, not a promise of restoration after restarting the process.
Restored document tabs begin as source placeholders. Visible tabs automatically request
the same validated source admission as Assets Open. Preferences alone never supply
trusted document contents; failed admission requires resolving the diagnostic and retrying Open.

Invalid preferences are reported and preserved. An explicit Reset keeps rejected input
under a unique `*.rejected-*.json` name. If a restored floating window cannot become ready,
its views recover into the main workspace with a diagnostic rather than disappearing.

## Understand shared selection

Three actions have different meanings:

1. **Select an asset in Assets:** identify a file or folder. This does not activate a document.
2. **Open a scene/prefab or activate its tab:** choose the document used by the other panels.
3. **Select a node:** update that document's shared source-node selection.

A newly opened document selects its root. Hierarchy and eligible Scene View mesh clicks
update the same selection. Inspector follows it unless locked. Selecting in Hierarchy
does not automatically frame the camera; use **Frame selection** when wanted.
Switching document tabs restores each document's own selection. Merely focusing a tool
does not activate a different document.

Opening a raw PNG/GLB preview clears the active document target. Hierarchy then has no
authored graph to show, and an unlocked Inspector has no source node to inspect. The
model-preview stage is not presented as an authored document. Return to a document tab
to recover that document's selection.

## Source sessions and external changes

The first supported Open admits a bounded source inventory shared by subsequent opens.
Already open documents reuse accepted contents and selection rather than rereading disk.
Before first opening another admitted source, the editor checks its bytes and identity
again. A failed Open preserves the previous active document and selection.

**Refresh sources** refreshes browser listings, not accepted documents. Newly added
externally added documents outside the admitted inventory require reopening the project. For external
source changes, reopen the project; for previews, also rebuild a matching delivery and
relaunch with that delivery. There is no automatic external-change reload or merge workflow.
See [Scene View prerequisites](scene-view.md#prepare-resources).

Physical input, OS clipboard, IME, screen readers and mixed-DPI behavior still need
platform-specific testing; certification across desktop platforms is incomplete.

For verification, follow [Native scenarios](../testing/native-scenarios.md); use
[editor CLI reference](../cli/editor-and-assets.md) for launch arguments.
