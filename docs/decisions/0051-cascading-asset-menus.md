# ADR 0051: Shared cascading menus and immediate document creation

Status: implementation; owner review pending.

## Decision

Use in-application cascading menus for Asset Browser commands. Native application
menus are a separate platform integration; they do not supply consistent styling
or an existing cross-platform, Wayland-tested context-menu implementation here.

`kedra_ui::popup` owns slash-separated display-path construction and shared submenu
interaction. `from_paths` accepts typed commands such as `Create/Scene`; labels
are not filesystem paths or command identities. Empty segments, duplicate commands,
leaf/branch conflicts and oversized trees fail before returning a candidate tree.
The editor supplies its authored item controls and command authorization.

Bevy 0.20's menu implementation explicitly lacks submenu coordination. The adapter
therefore reuses native focus, menu-item activation, popover positioning and flip
candidates while owning branch activation, hover delay and cascading dismissal.
No new window toolkit or parallel layout engine is introduced. Consumers install
`CascadePlugin` (included by native AuthoredUiPlugin), configure branches with
`cascade::branch`, and populate `cascade::attach` on ordinary `Activate` events.
The `Closed` event releases host bookkeeping before root retirement. Disabled
branches allocate nothing; child menus belong to the invoking item hierarchy.

Assets has no New/Import toolbar buttons. Its context menu and panel menu use the
same Create subtree and Import command. Rename/Delete remain selection-dependent.
Scene and Prefab creation both publish a file through the existing Save coordinator
before opening a clean tab; failure remains in the form. Source Undo/Redo still
changes working state and needs explicit Save to change already published bytes.

## Verification

Shared menu contracts own path conflicts, Unicode command identity, sibling
retirement and focus/close ownership. Editor creation contracts own disk-before-tab
ordering and collision preservation. Native Assets scenarios own submenu activation,
edge placement and creation through the assembled UI. System dialogs and physical
OS drops retain their separate manual acceptance boundary.
