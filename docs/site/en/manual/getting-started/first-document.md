# Open and inspect your first document

First [open Kedra Test Project](launch-editor.md).
The project is included in the repository at `tests/fixtures/test_project`.
It contains authored demonstration/test data, not a starter game template.

**Keep the canonical fixture unchanged.** Inspect it read-only. Before experimenting with
external source changes, copy the complete project into a new private directory, including
`kedra.toml`, `assets/` and every existing `.meta` sidecar. Open and build that copy instead.
A copy preserves identities; it is not a new-project identity generator.

## 1. Find the prefab in Assets

1. Select the **Assets** tab.
2. Navigate to the project asset root using the browser breadcrumbs or folder tree.
3. Find **Cuboid.prefab.ron**. If needed, focus Assets and press Ctrl+F on Windows/Linux
   or Cmd+F on macOS, then search for `Cuboid`. Search covers the current folder only.
4. Single-click the source to select it.

The file is `tests/fixtures/test_project/assets/Cuboid.prefab.ron` on disk and
`game://Cuboid.prefab.ron` in the project's source catalog. In-project display paths are
relative to the project, for example `assets/Cuboid.prefab.ron`.
The adjacent `.meta` sidecar is not a separate browser entry.

## 2. Open the selected source

For folder, search and filter controls, see [Asset Browser](../editor/asset-browser.md).

Double-click **Cuboid.prefab.ron**, or press Enter with that source focused.
Alternatively, use **Open selected source** in the Assets group's menu.
A single click selects the browser item; it does not activate a document.

Expected result: a document tab titled **Textured cuboid** and a corresponding Hierarchy
tree. The first Open admits the project's complete document inventory into a shared
source workspace, so it can take longer than selecting an already open tab.
Opening the same document again returns to its existing view and selection.

Without `--project-assets`, Scene View can report missing delivery information. You can
still inspect accepted source nodes and components without rendering them.

## 3. Select a node and inspect its components

1. Select the **Hierarchy** tab and click **Textured cuboid**, the prefab's root node.
   This prefab has one node; a one-row tree is the expected result.
2. Select **Inspector**. It follows the active document's selected source node.
3. Expand component sections to inspect the authored values. Look for the source component
   keys `scene.primitive`, `scene.surface` and `scene.transform`.

The fixture describes a cuboid with size `(2.0, 2.0, 2.0)`, translation
`(0.0, 1.0, 0.0)` and a surface texture reference to `game://checker.png`.
Inspector presents source values, not running gameplay state. Supported fields are
editable, so keep this canonical fixture unchanged and use a private copy for edits.

If a tool is hidden, use **Windows** in the main menu. It lists existing
views with **Show …** for hidden views or **Select …** for visible ones.
If Inspector remains on another object, choose **Unlock Inspector** in its group menu.
If property search hides the values, clear the search field.

## 4. Open a scene with a composed hierarchy

For navigation and property controls, see [Hierarchy](../editor/hierarchy.md) and
[Inspector](../editor/inspector.md). For panel placement, see
[Editor workspace](../editor/index.md).

Return to the asset root, clear the browser search and open **Composed.scene.ron** using
the same Open action. The active document becomes **Composed spatial scene**.
Hierarchy now shows local nodes and nested prefab instances, not just files on disk.

Select **Warm sphere** in Hierarchy and inspect its components. You can also select the
nested **Textured cuboid** node to inspect the prefab as used by this scene.
Changing document tabs changes the active hierarchy; browser selection alone does not.
For the underlying concepts, read [Scenes and prefabs](../scenes/index.md) and
[Composition and overrides](../scenes/composition.md).

## Optional: check the rendered preview

If you launched with the matching project delivery from the
[launch instructions](launch-editor.md),
Scene View can project the supported spatial/UI content. The composed scene includes
the cuboid, **Warm sphere**, a generated model, lighting and a HUD prefab instance.
Wait for dependency and frame readiness rather than treating an open tab as a rendered frame.

Source paths, identities and bytes must match the delivery. A successful standalone
`assets verify` proves delivery integrity, not that it still matches externally edited
sources. Changing the binding requires relaunch with a newly built matching delivery.
This is an editor preview: it does not run the fixture's game executable,
execute project scripts or implement Play.

## Recover without changing the fixture

For preview camera and selection controls, see [Scene View](../editor/scene-view.md).

- **No file found:** return to the asset root and clear search. Searching a subfolder does
  not search the whole project. A new empty project has no `Cuboid.prefab.ron`.
- **Listing stale or unreadable:** use **Refresh sources** in the Assets group menu, or F5.
  A failed refresh retains the prior listing as stale and blocks activation until recovery.
- **Open rejected:** read the diagnostic. One malformed document elsewhere in the inventory
  can reject first admission. Preserve canonical files; use a clean complete copy for repairs.
  Failed Open does not replace an already active document with a substitute scene.
- **External changes not visible:** reopening an existing tab is not disk reload.
  Reopen the project to admit a fresh source inventory; rebuild and relaunch if rendering.
- **Source works, preview fails:** confirm the explicit project and both delivery paths,
  then follow [asset troubleshooting][preview-errors].

Native input and rendering still need testing on your machine. Supported source edits
and new-document publication are available; project Play remains unavailable. Save
for accepted source edits is described in the
[Inspector manual](../editor/inspector.md#save-active-document).

[preview-errors]: ../assets/troubleshooting.md#verify-succeeds-but-the-editor-cannot-preview
