# Source Asset Browser

The retained `assets` tool browses the validated project source root. Browsing,
filtering, Refresh and Open never initialize metadata, build deliveries, run project
code or write sources. Scene/prefab Open uses the shared
[read-only document coordinator](source-documents.md), not a private browser loader.

## Controls

- Flat Back/Forward/Up icons and individually clickable source breadcrumbs.
- A lazily expanded folder tree with separate scrolling and native expand/navigation
  buttons. Enter navigates; Left/Right collapse/expand; Up/Down/Home/End move focus.
- Current-folder search, clear action and a scene/prefab-only filter. Search uses
  `unicase` Unicode case folding, not ASCII filename identity or recursive search.
- Grid/list modes share selection and data. Grid tiles are 112 × 96 logical pixels,
  list rows are 22px, and the folder sidebar is 196px where space permits.
- Single click selects; double-click/Enter activates the focused source. Arrows and
  Home/End select and reveal entries, including those outside the virtual window.
- The Assets group menu exposes **Open selected source** and **Refresh sources**.
  F5 refreshes; Ctrl/Cmd+F focuses the retained native search input.
- New/Import are disabled, with source-writing/processing explanations. They are
  not apparently functional buttons which silently do nothing.
- Native Bevy scrollbars provide drag/page interaction. Shared `kedra_ui::tooltip`
  hints show source names, unavailable commands and folder failures after a short
  dwell, beside the cursor rather than in a panel-width banner. Full paths remain
  in the status line as project-relative `assets/...` paths, never absolute paths.
- Status separates the selected source/type/tracking state from an always-visible
  item count. A filtered-out selection remains identified; Open never substitutes
  the first matching result.

Back/Forward restore successful locations with their search, selection and content
scroll. Missing entries clear only invalid selections. Grid/list and the document
type filter are browser-wide presentation choices. Float/Redock and Hide/Reopen
preserve the panel root, native search and model; process restart restores docking
descriptors, not browser session navigation.

## States and limits

Loading, empty folder, no filter matches, unreadable/stale listing, incomplete
listing, untracked sources, unsupported activation and failed document admission
are distinct. A failed Refresh retains the previous listing as explicitly stale
and blocks source activation until recovered. A failed Open keeps the active
document and browser selection.

Immediate directory enumeration examines at most 4096 entries and 1 MiB of encoded
filename data. It sorts folders first and then exact native names, retaining at most
512 entries with an incomplete-list indication. This is not the source workspace's
complete inventory. The grid/list renders only visible rows plus overscan using
native viewport/scroll geometry and authored retained tiles. Hidden or temporarily
unallocated native hosts do not rebuild the virtual window during transfer.

The folder tree enumerates only expanded branches, not the entire project. It
limits expansion/visible rows to 512, cached child paths to 4096 and cached path
bytes to 1 MiB; capacity diagnostics offer Refresh rather than claiming exhaustive
results. Collapsing branches preserves their cache until Refresh. Search never
initiates a recursive scan or reads file contents.

`.meta` sidecars are not independent tiles. Directory links are never followed;
linked/special files and non-addressable names are unavailable. Exact native
`PathBuf`/`OsString` values own operations; lossy labels are never turned back into
locators. Ancestors are checked before directory reads. Concurrent filesystem
replacement is not sandboxed.

## Composition and ownership

`kedra_editor::asset_browser::model` and `tree` are ordinary Rust feature state.
`Panel`, `view`, `tiles`, `input` and `hints` adapt asynchronous effects, authored
views and native input. `workspace` hosts the feature and routes explicit Open to
the document coordinator. Source entries are not an ECS database.

`assets/editor/prefabs/asset-browser.prefab.ron` composes the shared field, flat
icon actions, breadcrumbs, folder rows, source tiles and scrollbar prefabs. Flair
owns styling in `styles/asset-browser.css` using existing tokens. Bevy owns native
text editing, button interaction, scrolling, layout and scrollbar dragging. The
scrollbar adapter removes the authored thumb's layout `Node` because native
`ScrollbarThumb` owns that entity's geometry after layout; CSS owns its color.

`unicase` supplies maintained Unicode folding from the existing locked dependency
graph. Standard `read_dir` supplies immediate enumeration; no new walker, filename
parser, database or thumbnail scheduler is introduced. Native search limits input
to 256 characters and rejects control characters on insert/paste. Full shared
TextField Undo/Redo and OS clipboard/IME acceptance remain separate work.

## Visual reference and remaining scope

The target is UI-KIT `163:37852`, under flat-toolbar guidance `163:37771`; see
[Figma provenance](figma.md). Header/docking chrome stays shared. The toolbar wraps
on narrow hosts and becomes scrollable when wrapping exceeds its bounded height.
Source type badges are truthful placeholders, not fabricated rendered previews.

Rendered tile thumbnails, recursive search, reference picking, rename/move/delete,
creation, import and source Save are not implemented. Open displays supported
spatial/UI documents in [Scene View](scene-view.md) with an explicit matching
verified project delivery; PNG/static-GLB Open uses separate raw-asset previews.
Inspector is not connected; [Hierarchy](hierarchy.md) displays the active
document's composed nodes and shares scoped selection with Scene View. No independent Figma pixel
equivalence, physical OS input, IME or mixed-monitor certification is claimed.

## Verification

```sh
cargo test -p kedra_editor --lib --locked
cargo test -p kedra_import --test browser_icons --locked
cargo xtask check --non-interactive
```

Launch a fresh delivered editor at Projects with `--remote-port 0`, a new
`--remote-credentials` path and isolated `--state-directory`, then run:

```sh
python3 tools/qa/asset_browser.py target/browser-credentials.json \
  --state-directory target/browser-state --report target/browser-report.json
```

The opt-in scenario creates only isolated QA sources and verifies actual native
widgets, source admission, failures, identity retention and renderer frames.
Generated evidence belongs under `target/`, never design-reference directories.
