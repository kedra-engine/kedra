# Asset Browser contract

Asset Browser provides read-only navigation and source Open under
[ADR 0013](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0013-asset-browser-panel.md).
See the [Manual](../../../manual/editor/asset-browser.md) for keyboard controls, search,
Open and Refresh.

## Model, effects and identity

`kedra_editor::asset_browser::model` and `tree` are ordinary Rust feature state, not an ECS
source database. `Panel`, `view`, `tiles`, `input` and `hints` adapt asynchronous effects,
authored views and native input. `workspace` hosts the tool and routes explicit Open through
the [document coordinator](source-documents.md); the browser has no private document loader.
Browsing, filtering, Refresh and Open never initialize metadata, build deliveries, execute
project code or write sources. Listing tasks/results are scoped to their retained feature
and request generation; obsolete location/tree completions cannot replace current results.

Operations use the exact native `PathBuf`/`OsString` values. Never reconstruct a locator
from its lossy display label. Directory ancestors are rechecked before reads. `.meta` sidecars
are not independent tiles; links are not followed, and linked/special or non-addressable
entries cannot activate. A tracked badge checks for a regular non-symlink adjacent sidecar,
not metadata correctness. Displayed status paths are project-relative `assets/...`, never
absolute paths. Concurrent filesystem replacement is not sandboxed.

## Listing and cache budgets

Immediate `std::fs::read_dir` enumeration permits at most 4096 entries and 1 MiB of encoded
filename bytes. Exceeding either fails the listing. Successful results sort folders first,
then exact native names, retain at most 512 entries and identify incompleteness. This bounded
directory sample is not the complete source workspace inventory. Search neither recurses
nor reads content; locked `unicase` 2.9.0 supplies Unicode folding without changing identity.

The tree reads expanded branches only. Expansion/visible rows are limited to 512, cached
child paths to 4096 and encoded cached path bytes to 1 MiB. Collapsing preserves cache until
Refresh. When a cache limit is reached, diagnostics offer Refresh; the tree may be incomplete.
Navigation
history is bounded to 64 locations. Native search accepts at most 256 characters and rejects
control characters on insertion/paste; this is not source-field settlement.

## State retention and failures

Browser selection, keyboard focus and document activation are distinct. Grid/list share
selection/data; mode and document-type filtering are browser-wide. Successful Back/Forward
restores location-specific search, valid selection and content scroll. Missing entries clear
only invalid selections. A filtered-out selection remains identified; Open never substitutes
the first result. Float/Redock and Hide/Reopen retain the root, native search and model.
Restart restores docking descriptors, not browser navigation history.

Loading, empty folder, no matches, unreadable/stale listing, incomplete listing, untracked
data, unsupported activation and failed admission remain distinct. Failed Refresh preserves
the previous listing as stale and blocks activation until recovery. Failed Open preserves
selection and the active document; no default source is manufactured.

## Retained presentation and visual authority

Only visible rows plus overscan mount, using native viewport/scroll geometry and retained
authored tiles. Temporarily unallocated/hidden native hosts do not rebuild virtualization
during transfer. `assets/editor/prefabs/asset-browser.prefab.ron` composes shared fields,
flat icon actions, breadcrumbs, folder rows, source tiles and scrollbar prefabs. Flair CSS
in `styles/asset-browser.css` owns styling through existing tokens. Bevy owns text editing,
buttons, layout, scrolling and scrollbar drag. The adapter removes the authored thumb's
layout `Node`: native `ScrollbarThumb` owns post-layout geometry; CSS owns its color.

The visual source is UI-KIT `163:37852`, under flat-toolbar guidance `163:37771`; preserve
the [canonical provenance](../ui/design-provenance.md#asset-browser-exports).
Shared docking/header chrome is not
redrawn. Grid tiles are 80 × 96 logical pixels; list rows are 22px.
Single-line filenames use intrinsic-width text nodes centered by the grid tile's
flex alignment. Bevy's `NoWrap` shaping is unbounded, so text justification inside
a fixed-width node does not center these glyphs. List mode allocates the remaining
row width and retains left-aligned text. Long names clip on the right without
changing their source value; filename ellipsis is not implemented.
The native `asset-folders` scenario checks captured filename ink through grid/list
switching and dock resizing, including Unicode labels and long-name bounds.
The sidebar's 200px
outer width includes a 4px divider, leaving 196px of content where space permits. The toolbar
wraps, then scrolls if wrapping exceeds its bounded height.
Shared tooltip hints dwell beside the cursor rather than replacing content with a banner;
selected source/type/tracking status remains separate from the always-visible count.

PNG/static-GLTF previews use the shared thumbnail service; unsupported types retain
truthful badges. Thumbnail admission uses `Delivery::matches_resources` plus each
request's native type/identity check. Authoring document changes do not invalidate
unchanged resources; all delivered non-document dependencies still require matching
source identities and bytes. Resource admission failures emit a diagnostic once per
observed revision. Scene View's full document admission remains separate.
Context and panel menus share slash-separated command paths under
**Create**, with **Import files** alongside. The toolbar has no creation/import buttons.
**Create → Environment** uses the existing staged importer and metadata initializer;
successful publication/readmission selects the new source root in Inspector. New
profiles are offered by the scene-root Environment picker without a binary delivery.
Creation publishes via the existing Save coordinator before opening a tab.
Import invokes the native picker and the shared
`kedra_import::ingest` source-copy mechanism; OS drops use that same operation.
Entry commands use `kedra_authoring_fs::entries` under the shared source writer lease.
Create/Rename use the existing authored name form; Delete uses a native confirmation.
The coordinator pins the project, browser and target, settles sources and prevalidates
the proposed inventory through the common source compiler/reconciliation path. Rename
preserves identities and retains source history; missing document tabs are hidden after
successful deletion. Browser locations/history are rebased and stale reads invalidated.
Broken dependencies refuse before publication. Folder changes are immediate filesystem
operations outside document Undo/Redo; deletion keeps a sibling recovery copy. See the
[filesystem contract](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring_fs/README.md#asset-folders).
File and folder tiles support Rename/Delete; tree rows also produce folder drags.
Moves validate retained bindings and target directories before the same source operation.
Pointer context menus use the click position with native viewport placement.
Rendered thumbnails and recursive search remain unimplemented.
Source Save belongs to the active-document host and refreshes browser listings on completion.
Resource-free Scene Views need no project delivery; raw assets and resource-bearing sources
require matching resources. Shared text
Undo/Redo, OS clipboard/IME, physical input, mixed monitors and independent Figma pixel
equivalence are not certified by this profile.

## Verification

Import pins the original project and directory while the picker/worker runs. Pending
import blocks source Save/history commands and normal main-window close. Source inventory
refresh uses the common authoring reconciliation path, preserving history; external
document changes are not silently admitted as part of import. Immutable runtime deliveries
are not rebuilt or hot-swapped. See
[ADR 0048](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0048-native-source-import.md).

Filesystem conflict, sidecar, Unicode and link guarantees belong to
`kedra_import/tests/contracts/ingest.rs`. Editor tests cover native-message target routing,
cancelled drops and imported-inventory/history preservation. These do not certify the
OS picker or physical file transfer.

Production sources and local model/regression tests live under
[`asset_browser`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/asset_browser/mod.rs).
They cover listing/state/virtualization/tree behavior and failure retention; document
admission tests belong to `documents`, not a duplicate browser compiler suite.

```sh
cargo test -p kedra_editor --lib --locked
cargo test -p kedra_editor --test contracts --locked icons::
cargo xtask check --non-interactive
cargo xtask test -- features --only browser --output target/test/browser
cargo xtask test -- features --only asset-folders --output target/test/asset-folders
```

The opt-in native route requires an idle desktop, builds a delivery and opens an isolated
copy of the checked test project. It exercises native widgets, source admission/failures,
identity retention and renderer frames. Evidence belongs under `target/`, never design
reference directories. Native verification of this revision remains pending.
The `asset-folders` route checks folder Create/Cancel, invalid names, prefilled Rename,
collision refusal and refreshed visible results through native input and frames. The OS
Delete confirmation and keyboard/context-menu entry points require separate acceptance;
filesystem deletion/recovery and dependency refusal are covered by their owning tests.
