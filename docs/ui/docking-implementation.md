# Native docking

The editor has a retained, native split/tab workspace. Tools and document views
can occupy any group; there is no reserved central region. Scene editing and
document Save are not implemented. See [ADR 0008](../decisions/0008-docking-ownership.md).

## Responsibilities

| Package | Responsibility |
| --- | --- |
| `kedra_docking` | Pure Rust topology, stable IDs, bounded snapshots, validated commands and unpublished interaction sessions |
| `kedra_ui::docking` | Authored chrome, retained content projection, native focus bookmarks, title fitting and recursive minima |
| `kedra_editor::workspace` | Feature bindings, input/targeting, menus, preferences and logical/native host mapping |
| `kedra_desktop::auxiliary` | Staged window/camera/root allocation, readiness, publication and retirement |

Bevy UI/Taffy owns layout and pixel rounding. Flair owns visual properties and
dynamic inline geometry. The runtime model has no Bevy or editor dependency.
Feature documents, undo history and source writers remain outside docking.

## Controls

- Select and reorder tabs, merge into a tab group, or split at any edge.
- Drag a label to move one view; Shift-drag moves its whole group, preserving
  order and the active view. Close slots and group actions are not drag origins.
- A five-target compass exposes left/right/top/bottom/tab destinations. Direct
  edge targeting is an additional path. Rail drops use full-tab insertion bounds,
  including the close slot, with a separate insertion marker.
- A stable-offset identity ghost and dimmed source indicate a drag. Unavailable
  targets show a reason without a misleading destination preview.
- A single Flat `…` action opens the group menu: maximize/restore, float group,
  float active tab, hide group, redock a floating group and select any tab.
  Clipped titles remain available in full through this menu.
- Tab rails scroll horizontally. Focused labels support Left/Right/Home/End.
- Splitters support pointer resize, keyboard arrows and preferred-ratio reset
  through Space/Enter or double-click. Escape cancels drag and resize previews.
- Maximize changes the projection, not topology. Restore retains original splits,
  tab order and feature content.

The authored skin has 32px rails, 28px tabs, 4px dividers and 26px Flat menu hit
areas. Fixture tabs are 100px wide; that is not a universal panel-width rule.
Tabs use native rounded borders, not an overlaid rectangular top-light strip.
Flat controls have no idle border, bevel or shadow. OS caption controls are separate.

## Transactions and retention

The published layout is unchanged during a gesture. Candidate projection uses
the same authored chrome and Taffy layout as committed projection, without
duplicating feature content. Admission checks topology, measured allocation and
resolved minimum constraints. A split cannot take unrelated neighbours' space
to bypass the destination's minimum size.

Release commits only a current, valid, measured candidate. Escape, stale
identities, source retirement and capture loss cancel without publication.
Elapsed frames alone do not establish readiness: a structural candidate needs
three consecutive matching valid allocations. A drop outside the maximized
group's visible projection is rejected; Restore makes those splits available.
Native content roots, editable values and view-local state survive reordering,
splitting, maximizing, hiding and host transfer. Restart restores stable IDs,
not process-local entities.

Floating creation stages a native window, camera and authored frame before
transferring content. Backend readiness and candidate validity are checked before
publication. Failure retires staged resources while preserving the source.
Content moves before an emptied host is retired.
Restored floating hosts follow the same bounded readiness rule: their content
stays in the live stash until acknowledgment. Creation failure or a ten-second
timeout moves their views into a visible main-window group and reports recovery;
view identities, restore keys, hidden records and valid sibling hosts are retained.

Floating Close hides its tools and preserves reopening locations. Main Close
exits. A future document feature must supply close approval and shared document
ownership; generic docking does not save documents or approve discarding edits.

## Editor panels and preferences

Opening a validated project mounts four independently dockable panels:
Hierarchy on the left, Document Preview above Assets in the middle, and Inspector
on the right. The preset allocates roughly 20% / 60% / 20% of the width and 68% /
32% of the middle column's height, subject to native layout minima. No position
is reserved: every panel can move, tab, float, hide and reopen through docking.

The [source Asset Browser](asset-browser.md) remains read-only. Inspector and the
initial Document Preview are empty authored panel roots.
[Read-only source Open](source-documents.md) creates ordinary document tabs and
hides the empty Preview slot. Tabs host [Scene View](scene-view.md), rendering
supported spatial/UI documents with an explicit matching verified delivery;
PNG/static-GLB assets use separate preview tabs. Spatial camera navigation and
scoped opaque-mesh selection do not edit sources. Inspector editing, source Save,
dirty-document close approval and Play remain unimplemented. Opening a project
does not compile or execute its code.

[Hierarchy](hierarchy.md) is a retained read-only projection of the active source
document, with per-document navigation and shared node selection. It remains an
ordinary dockable tool and has no reserved position or independent tab system.

Per-project preferences store host topology, tab order, active/maximized groups
and window placement. Missing monitors are clamped/centered; unknown feature
kinds retain unavailable-panel records. Invalid preferences are reported and
preserved. Explicit Reset layout retains rejected input under a unique
`rejected-*.json` name and restores the preset without rebuilding live content.
Reset validates the complete replacement before changing published topology or
detaching retained content. Capacity rejection preserves the existing layout.

Saved layouts containing retired Project, Scratchpad or Activity demonstration
tools are replaced with this preset before content/native floating hosts mount.
Assets retains its view identity; unknown extension descriptors and restore keys
are retained as additional Assets-group tabs. Layouts already using the authoring
tools restore unchanged, including hidden tools and custom floating placements.

Writes are bounded, serialized with a standard file lock, baseline-checked and
atomically replaced with `tempfile`. I/O runs off the main thread; orderly exit
flushes the last published state. Advisory locks coordinate cooperating processes,
not hostile filesystem writers. Layout preferences are not authoring Save.

## Sources and verification

`assets/editor/prefabs/dock-*.prefab.ron` owns structure;
`assets/editor/styles/docking.css` and shared controls CSS own presentation.
The Figma reference is UI-KIT `2EZlK40cfx2Mub7GmDyI70`, page `92:3713`.
The [source map](figma.md) lists current component-library navigation separately
from pinned export provenance.
Its independent exported state family is pinned in
[`docking/figma/manifest.json`](docking/figma/manifest.json).

```sh
cargo test -p kedra_docking --locked
cargo test -p kedra_ui --test docking --locked
cargo xtask check --non-interactive
```

[Docking tests](docking-tests.md) covers the isolated production-mechanics host,
native screenshots, project preference restart checks and acceptance boundaries.
`editor.workspace` is read-only diagnostics, not a second mutation path.

## Limitations

- Native input injection and renderer readbacks are verified on macOS/Metal.
  Physical OS capture/release, mixed-DPI monitor movement and Windows/Linux/Wayland
  interaction are not certified.
- Cross-host coordinate conversion checks backend support. Without global
  positions or destination-local events, transfer is visibly rejected without
  publication. Explicit Float/Redock menu actions remain available.
- Bundled fonts preserve source strings but do not guarantee CJK glyph coverage
  or native IME behavior.
- Document-close approval, scene editing, source Save and preference schema
  migrations require real feature consumers. See the [roadmap](../roadmap.md).
