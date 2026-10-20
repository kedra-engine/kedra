# ADR 0013: Asset Browser panel

Status: full design proposed for owner review. A first read-only navigation slice
uses the existing Assets docking identity; [current behavior and limits](../ui/asset-browser.md)
now include lazy folders, Unicode search, virtualized grid/list and read-only
source-document Open. Complete visual/platform acceptance remains incomplete.
This ADR is not a complete asset
authoring/import toolset.

## Purpose and module ownership

The Asset Browser finds source files in the admitted project asset root and sends
explicit OpenSource requests to the [document coordinator](0012-document-sessions-and-selection.md).
It does not own scene drafts, node selection, import jobs, Save or runtime loaders.
Its first implementation is a separate editor feature module with an ordinary Rust
browser model and retained presentation, not more responsibilities in docking's
existing `workspace::panels` mounting code.

Use the shared native widgets and authored UI. Do not create a browser-specific
text editor, filesystem database or thumbnail scheduler. Reuse `std::fs::read_dir`
for immediate directories; evaluate existing `walkdir` integration for any future
recursive inventory. The importer has no public read-only browser inventory API;
do not call its source-writing initialization/build functions merely to list files.

## Docking contract

Register Asset Browser as an independent tool view in the existing `kedra_docking`
and `kedra_ui::docking` host. It can share any tab group, split, maximize, float and
redock. Keep the retained content root and `ViewId` through every move.
Tool Close hides it after field/popup settlement; Panels reveals the same view.
Location, navigation history, selection, scroll and search survive host transfer.
No embedded document tabs, extra OS window implementation or reserved bottom area.

The existing Assets tool identity needs an explicit feature/preference migration
when replaced; do not silently drop its restored view or duplicate it under a new
kind. Runtime listing tasks are cancelled or retired with their feature generation.

## Navigation and selection

Show a source-root breadcrumb, folder navigation and current-folder contents.
Separate browser selection, keyboard focus and document activation:

- Single click/keyboard navigation selects one source entry; no file is opened.
- Double click, Enter or explicit Open activates the selected folder/document.
- Directory activation navigates. A recognized scene/prefab submits OpenSource.
- Unsupported file activation explains its type and unavailable action; it does
  not run a process, script, importer or external executable.
- Back/Forward restore browser location and valid view-local state, not document
  history. Up is disabled at the configured root; breadcrumbs cannot escape it.
- Open again activates the existing document view; source reload is separate.

Use source filenames/extensions for initial classification, followed by actual
reader validation on Open. Do not claim a file is valid because its suffix matches.
Folders and unsupported files remain visible; `.meta` sidecars are asset metadata,
not separate asset tiles. Missing metadata is a diagnostic/untracked state, not
permission to generate it. Scene/prefab document identity comes from the document;
an existing mismatched sidecar is an error, not a replacement UUID.

Initial search filters the already enumerated current folder by filename, without
recursive project search. Preserve Unicode; use deterministic Unicode-aware
matching without rewriting source paths. The exact matching/case-fold library
must be selected before implementation; do not use ASCII-only filename identity.
Clearing a filter restores the prior visible selection where still valid. A hidden
selection remains identified in status; do not silently open the first match.

## Filesystem, loading and failure boundaries

Keep native `PathBuf`/`OsString` values until a portable source address is required.
Do not round-trip lossy display labels into file operations. Present non-Unicode
or invalid asset-address names explicitly as unsupported; retain the actual OS
locator for safe diagnostics. Enforce existing source path/namespace policy at
admission; filename `#` is not fixed by percent-encoding it.

For the first browser profile, enumerate only immediate regular files/directories
under the validated root. Do not follow directory symlinks, open special files or
read source bytes just to render a row. Show inaccessible/linked entries with an
explanation rather than silently traversing outside the root. Validate again at
Open. This is not a hostile-filesystem sandbox; races and stronger containment
require the source-host implementation's explicit guarantees.

I/O runs off the main thread. Requests/results carry project, location and filter
generations; late results cannot repopulate an old project/folder. Loading is visible.
Empty folder, empty filter results, unreadable directory and failed Open are
different states. Refresh is read-only; it preserves the last listing as explicitly
stale on failure and does not present it as a current complete inventory.

Specify listing count/byte limits, deterministic sorting, cancellation and rendering
virtualization before implementation. A limited listing must show its incompleteness
and must not be used as the source workspace's complete inventory. Do not claim
recursive search or exhaustive results from a bounded directory sample.

## Figma and scope

Use the [UI-kit](../ui/figma.md) Asset Explorer composition `163:37852` within flat
toolbar guidance `163:37771`: folder tree, independent scroll regions, breadcrumb,
folder-scoped search and source-location status. Retain common dock headers and
flat contiguous toolbar controls. Grid/list presentation shares one browser model.

New/Import controls shown in the composition do not authorize their implementation.
Until those commands have source-writing/processing contracts, omit or explicitly
disable them with reasons; never leave apparently functional no-op buttons.
Rich previews, recursive search, reference picking, rename/move/delete, drag into a
scene and import settings each need focused design. Baseline tiles may use type
icons with real metadata, not fabricated rendered thumbnails.

Before acceptance, review loading, unreadable folder, missing source, invalid name,
unsupported Open, source-scope admission failure, empty/no-results, capacity and
narrow/floating layouts. Existing Figma examples do not cover all these states.

## Verification and delivery

First prove read-only enumeration/navigation with ordinary model tests, then native
selection/navigation, then document Open integration. Test long Unicode names,
case-distinct entries on supporting filesystems, non-Unicode names, symlinks,
special files, permission failure, stale results and removed files between listing
and Open. No browse/open failure may write sources or start project code.

Native tests exercise keyboard/double-click, scroll/filter retention, dock move,
float/redock/hide/reopen and focus ownership. Verify real source identity reaches
the document coordinator exactly once per activation. Run repository checks and
retain native evidence under `target/`; full asset management is not this slice.
