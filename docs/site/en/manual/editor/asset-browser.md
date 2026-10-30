# Asset Browser

The **Assets** panel browses the open project's source asset directory. Use it to locate
files and folders, then explicitly open a scene, prefab, PNG or static GLB. Browsing and
Open do not create sidecars, build resources, write sources or launch an external program.

## Navigate source folders

Click a folder in the left tree to enter it. Its disclosure control expands or collapses
that branch without entering it. In the content area, single-click selects a folder;
double-click or Enter opens it. Breadcrumb segments jump to their corresponding ancestors.

| Control | Behavior |
| --- | --- |
| Back / Forward | Return to successful visited locations with their search, selection and scroll. |
| Up | Go to the parent folder, stopping at the source root. |
| Backspace on a content entry | Go up one folder. |
| Up/Down/Home/End on a tree row | Move focus among visible folders and reveal the focused row. |
| Left/Right on a tree row | Collapse/expand, or move to a parent/child when already collapsed/expanded. |
| Enter on a focused folder-navigation button | Enter that folder. |
| Native scrollbars | Scroll the tree and content independently; drag a thumb or page the track. |

Folders sort before files, then by exact native name. `.meta` sidecars are hidden as
independent entries. The status line uses project-relative paths such as
`assets/Composed.scene.ron`, not absolute filesystem paths.

The status **untracked (no sidecar)** means no regular, non-symlink adjacent `.meta`
sidecar was detected. It does not distinguish every filesystem error or validate metadata.
A tracked badge alone does not prove valid metadata or a usable delivery. The browser
does not repair metadata; raw PNG/GLB preview requires admitted identity and resources.

## Search and change the view

Type in the search field, or press Ctrl/Cmd+F while focus is inside Assets. Search matches
names in the **current folder only**, with Unicode case folding. It does not scan nested
folders or file contents. The clear action empties the search.

The scene/prefab filter keeps folders visible and limits files to `.scene.ron` and
`.prefab.ron`. Disable it to see PNG/GLB and other source files. Grid and list show the
same entries and selection. The grid generates PNG/static-GLTF thumbnails for visible
tiles with a matching admitted resource delivery; unsupported/unavailable previews keep type badges.
Creating, editing or saving scenes/prefabs does not invalidate unchanged model/texture
thumbnails. Delivered resource identities and bytes, including model dependencies,
must still match; resource changes require a matching delivery.
Grid/list and the type filter are browser-wide choices, not separate folder histories.

With a content entry focused, arrows select neighboring entries; Up/Down follow grid rows
or list rows. Home/End select the first/last visible result, scrolling it into view.
Filtering can hide a selected entry without clearing its identity. The status still
identifies that selection: Open does not silently choose the first search result instead.

Search accepts up to 256 characters and rejects control characters. Folder navigation,
search and scroll survive Float/Redock and Hide/Reopen during the session. Restarting the
editor does not restore the browser's navigation history.

Thumbnails share the project's `.kedra/cache/` with other derived-cache consumers.
The cache is excluded from Git and can be deleted with the editor closed. Its images
are regenerated as needed. Cache hits survive project/editor restarts; source or
dependency changes invalidate them. This does not rebuild the runtime delivery.
Only visible grid tiles request work, with one native model job at a time. Skinned
or morphing models are not supported by the passive thumbnail profile.

## Open an entry

Single-click selects; double-click or Enter activates the focused entry. Alternatively,
select an entry and use **… → Open selected source**. Available activation depends on type:

| Entry | Result |
| --- | --- |
| Folder | Enter the folder. |
| `.scene.ron` / `.prefab.ron` | Admit/activate a source document tab; initially select its root. Open itself does not write sources. |
| `.png` | Open a read-only image preview with matching admitted metadata and delivery. |
| `.glb` | Open a read-only static-model preview with matching admitted metadata and delivery. |
| Other source file | Show an unsupported-type message; do not launch another application. |
| Linked/special or unaddressable entry | Refuse Open with a diagnostic. |

Document Open validates the shared source inventory, not only that one visible filename.
A malformed or unsupported document elsewhere can prevent admission. Valid source
inspection does not require a successful rendered preview. See
[document sessions](index.md#source-sessions-and-external-changes) and
[Scene View](scene-view.md) for the additional rendering requirements.

Right-click Assets for **Create ▸** (Scene, Prefab, Folder, Environment) and **Import files**.
**Create → Environment** writes a new profile with its identity and selects it in
Inspector. Configure and save it, then select a scene's root and use **Environment**
to assign the profile. Scene assignment has its own Undo/Redo and Save.
Creation/import are not separate toolbar buttons. Import opens a native multi-file
picker; processing deliveries remains a separate CLI action.
The **…** menu belongs to the dock group and adds source actions when Assets is active.

## Manage files and folders

Use **Create → Folder** (or Ctrl/Cmd+Shift+N) to create a folder in the current location.
Right-click a file or folder for **Rename** and **Delete**. F2 and Delete work
on focused tiles and folder tree rows; the Assets panel menu offers the same actions.
Drag a file or folder onto a destination folder to move it. Enter submits the name;
Escape cancels. Delete requires native confirmation. The asset root cannot be changed.

Save pending document edits first. Entry changes apply immediately, outside document
Undo/Redo and Save. Rename preserves file contents and metadata identities. Browser paths
and navigation history follow renamed folders; deleting the current folder returns to
its parent. Existing names are not overwritten. Operations that would break source
dependencies are refused; logical path references and explicit catalogs are not rewritten.
Runtime deliveries are not rebuilt. Deleted entries are retained in `.kedra-deleted-*`
directories beside `assets` for filesystem recovery; there is no in-editor restore action.

## Import source assets

Save source changes, navigate to the destination folder in Assets, then choose **Import files**.
The native picker supports multiple files. OS file/folder drops use the visible Assets
panel under the pointer and its current directory, including a floating browser window.
The drop target is highlighted. Drops outside Assets do not import anything.

Files are copied; originals are retained. Existing files or metadata cause a conflict
instead of an overwrite. Incoming `.meta` files are excluded and fresh metadata is assigned.
Folders preserve their relative structure. Links and special files are rejected.
Progress appears in the editor status, failures use a native message dialog, and Assets
refreshes after completion. Cancelling the picker changes nothing. Import writes source
files immediately and is separate from document Undo/Redo and Save. A publication I/O
failure can leave a reported partial import; inspect the destination before retrying.

Import does not build or replace the immutable runtime delivery. PNG/GLB preview and placement still
require a matching delivery; see [the asset pipeline](../assets/pipeline.md). The pinned Winit
backend lacks Wayland DnD; the compatibility fork changes described in
[ADR 0048](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0048-native-source-import.md)
are required for native Wayland drops. OS dialogs and physical drops require manual acceptance.

## Create a scene

Select **Create → Scene** or **Create → Prefab** (also available in the panel menu).
The editor shows the current Assets folder and asks for a name. Enter a filename and
choose **Create**; the matching `.scene.ron` or `.prefab.ron` suffix is appended if omitted.
Cancel or Escape before submission creates nothing. Creation publishes the file
through the existing Save writer, refreshes Assets, then opens a clean document tab
with its root selected. A failed publication stays in the name form and does not
open a successful document tab; retry retains the original pending source identity.
Subsequent edits use Ctrl/Cmd+S or **Save active document**. Existing files are never overwritten.

Creation uses the common source Undo/Redo history. Undo removes the working document;
Redo restores the same UUID, root and path. Neither writes files. If creation was already
saved, Undo leaves a dirty removal, published only by a later explicit Save. Removal of
a source with an existing `.meta` sidecar requires a managed asset lifecycle and refuses.
Discard restores acknowledged sources and releases unsaved path reservations. New scenes
start empty; object creation is available through the source editing controls.

Drag a static GLB from Assets onto the new scene's viewport to place it under the document root at
the cursor's placement preview, using a mesh hit or ground plane. Wait for a valid preview before
releasing. Dropping onto a Hierarchy node uses that node as the parent with the default local
transform instead. Placement uses source Undo/Redo; Save publishes the model's source-qualified
logical reference. A matching project delivery is required.

## Example: open the checked scene

Use the [first-document walkthrough](../getting-started/first-document.md) for project setup.
For source identity and hidden sidecars, see [Assets](../assets/index.md).

In a working copy of `tests/fixtures/test_project`:

1. Return to the Assets root and disable the scene/prefab filter if needed.
2. Search for `Composed.scene.ron` and single-click its entry.
3. Observe the selected path/type in the status line; the active document is unchanged.
4. Use **Open selected source** or double-click the entry.
5. The tab **Composed spatial scene** opens, with that root selected in Hierarchy.
   Inspector can inspect source values; Scene View renders only with matching resources.

As a search-only example, `STRASSE` matches the fixture's `Straße.txt` filename. Opening
that text file reports that no document editor exists; it does not execute or edit it.

## Loading, stale listings and limits

Loading, an empty folder and no filter matches are different states. The item count stays
visible independently of selection. Hover hints explain long names and unavailable actions.

Use **… → Refresh sources** or F5 with focus in Assets after filesystem changes. A failed
Refresh preserves the old listing as stale and blocks activation until a successful read.
Failed Open keeps the browser selection and previously active document.

Listings retain at most 512 entries per folder, with an incomplete-list indication.
Enumeration rejects folders beyond 4096 scanned entries or 1 MiB of encoded filename data.
The folder tree reads expanded branches lazily and also has bounded caches. An incomplete
list is not evidence that an absent file does not exist; narrow the folder organization
and refresh. Links are not followed. Browser Refresh does not reload an already accepted
document or extend its admitted source inventory.
