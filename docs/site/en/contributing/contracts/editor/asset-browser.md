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
redrawn. Grid tiles are 112 × 96 logical pixels; list rows are 22px. The sidebar's 200px
outer width includes a 4px divider, leaving 196px of content where space permits. The toolbar
wraps, then scrolls if wrapping exceeds its bounded height.
Shared tooltip hints dwell beside the cursor rather than replacing content with a banner;
selected source/type/tracking status remains separate from the always-visible count.

Type badges are truthful placeholders, not rendered thumbnails. New/Import are disabled
with explanations. Rendered thumbnails, recursive search, reference picking, rename/move/
delete and creation/import remain unimplemented. Source Save belongs to the active-document
host, not the browser. Supported Scene View/raw-asset
previews require matching resources; Inspector and Hierarchy remain read-only. Shared text
Undo/Redo, OS clipboard/IME, physical input, mixed monitors and independent Figma pixel
equivalence are not certified by this profile.

## Verification

Production sources and local model/regression tests live under
[`asset_browser`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/asset_browser/mod.rs).
They cover listing/state/virtualization/tree behavior and failure retention; document
admission tests belong to `documents`, not a duplicate browser compiler suite.

```sh
cargo test -p kedra_editor --lib --locked
cargo test -p kedra_editor --test contracts --locked icons::
cargo xtask check --non-interactive
cargo xtask test -- features --only browser --output target/test/browser
```

The opt-in native route requires an idle desktop, builds a delivery and opens an isolated
copy of the checked test project. It exercises native widgets, source admission/failures,
identity retention and renderer frames. Evidence belongs under `target/`, never design
reference directories. Native verification of this revision remains pending.
