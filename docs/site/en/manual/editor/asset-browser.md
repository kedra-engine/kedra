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
same entries and selection; their badges identify source types, not rendered thumbnails.
Grid/list and the type filter are browser-wide choices, not separate folder histories.

With a content entry focused, arrows select neighboring entries; Up/Down follow grid rows
or list rows. Home/End select the first/last visible result, scrolling it into view.
Filtering can hide a selected entry without clearing its identity. The status still
identifies that selection: Open does not silently choose the first search result instead.

Search accepts up to 256 characters and rejects control characters. Folder navigation,
search and scroll survive Float/Redock and Hide/Reopen during the session. Restarting the
editor does not restore the browser's navigation history.

## Open an entry

Single-click selects; double-click or Enter activates the focused entry. Alternatively,
select an entry and use **… → Open selected source**. Available activation depends on type:

| Entry | Result |
| --- | --- |
| Folder | Enter the folder. |
| `.scene.ron` / `.prefab.ron` | Admit/activate a read-only document tab; initially select its root. |
| `.png` | Open a read-only image preview with matching admitted metadata and delivery. |
| `.glb` | Open a read-only static-model preview with matching admitted metadata and delivery. |
| Other source file | Show an unsupported-type message; do not launch another application. |
| Linked/special or unaddressable entry | Refuse Open with a diagnostic. |

Document Open validates the shared source inventory, not only that one visible filename.
A malformed or unsupported document elsewhere can prevent admission. Valid source
inspection does not require a successful rendered preview. See
[document sessions](index.md#source-sessions-and-external-changes) and
[Scene View](scene-view.md) for the additional rendering requirements.

**New** and **Import** are disabled and explain why on hover. Creation, processing,
rename, move, delete, reference picking and source Save are not available in this panel.
There is no implemented per-asset editing context menu; the **…** menu belongs to the
dock group and adds Open/Refresh when Assets is the active tab.

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
