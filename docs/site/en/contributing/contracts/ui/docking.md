# Docking contracts

Docking separates topology, retained projection, gestures and preferences. For panel controls, see
[Editor workspace: arranging panels and tabs](../../../manual/editor/index.md).
[ADR 0008](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0008-docking-ownership.md)
defines the architecture. Native platform verification remains incomplete. There is no reserved
central region.
Docking owns neither feature documents, undo history nor source writers.

## Ownership

| Owner | Contract |
| --- | --- |
| `kedra_docking` | Pure Rust topology, stable IDs, bounded snapshots, validated commands, unpublished sessions; no Bevy/editor dependency |
| `kedra_ui::docking` | Authored chrome, retained content, native focus bookmarks, title fitting, recursive minima |
| `kedra_editor::workspace` | Feature bindings, input/targeting, menus, preferences, logical/native host mapping |
| `kedra_desktop::auxiliary` | Staged window/camera/root allocation, acknowledgment, publication, retirement |

Bevy UI/Taffy owns layout and pixel rounding; Flair owns visual properties and dynamic inline
geometry. `Projection::sync` validates skin bindings and prepares missing chrome before changing
attachments or focus. Failure retires only staged mounts; success retains content/chrome identities.
Consumers keep host, stash and content alive, and must not reparent or retire projection-owned
entities from external ECS observers during synchronization. Live/candidate splits use unrounded
flex layout to avoid cumulative odd-width rounding overflow; admission bounds remain enforced.

## Input and skin

Label drag moves one view; Shift-drag moves its group with order and active view intact.
Close slots and group actions are not drag origins. The five-target compass supports four edges
and tab merge; direct-edge targeting is separate. Rail insertion uses the complete tab, including
its close slot, with a separate marker. Identity ghosts keep a stable pointer offset and dim the
source; unavailable targets give a reason without a false destination preview.

The single Flat `…` menu supplies maximize/restore, float group/active tab, hide, floating redock
and complete tab titles. Rails scroll horizontally; focused labels use Left/Right/Home/End.
Splitters support pointer/arrow resize and Space/Enter/double-click preferred-ratio reset.
Escape cancels drag/resize previews. Maximize changes projection, not topology or retained content.

Authored rails are 32px, tabs 28px, dividers 4px and Flat menu hit areas 22px. Fixture tabs are
100px wide, not a universal panel-width rule. Tabs use native rounded borders, not rectangular
top-light overlays. Flat controls have no idle border, bevel or shadow. OS captions are separate.
See [Window compositions](window-chrome.md#window-compositions) for integrated floating rails.

## Transactions and measurement

Gestures leave published layout unchanged. Candidate projection uses the committed authored
chrome/Taffy path without duplicating feature content. Admission validates topology, measured
allocation and resolved minima; splitting cannot steal unrelated neighbours' space to meet a
destination minimum. Release commits only a current, valid, measured candidate.

Structural readiness requires three consecutive matching valid allocations, not elapsed frames.
Escape, stale identities, source retirement and capture loss cancel without publication.
Focus restoration with the pointer still held cannot restart a cancelled gesture.
Drops outside a maximized group's visible projection fail; Restore exposes the original splits.
Content roots, editable values and view-local state survive reorder, split, maximize, hide and
host transfer. Restart restores stable IDs, never process-local entities.

## Native surfaces and recovery

Float stages a native window, camera and authored frame before transferring content. Backend
readiness and candidate validity precede publication; failure retires staged resources and keeps
the source. Move content before retiring an emptied host.

Restored floats leave content in the live stash until acknowledgment. Creation failure or a
ten-second timeout recovers views into a visible main-window group with a diagnostic. View IDs,
restore keys, hidden records and valid sibling hosts survive. Floating Close hides tools and
preserves reopening locations; main Close exits. Future documents must supply their own close
approval/shared ownership: docking neither saves sources nor approves discarding edits.

Cross-host conversion checks backend support. Without global positions or destination-local
events, reject transfer visibly without publication; explicit Float/Redock remain available.

## Preset and preference persistence

The `ui.docking` Snapshot on `editor://scenes/editor.scene.ron` supplies the four-tool preset:
Hierarchy left, Document Preview above Assets centrally, Inspector right, approximately
20% / 60% / 20% width and 68% / 32% middle height, subject to native minima. No panel is reserved.
The adapter maps template aliases to the live host/retained view IDs; authored aliases do not
replace persisted identities. Read-only source Open hides the initial empty Preview and creates
ordinary document tabs; PNG/static-GLB previews are separate tabs. Panels remain ordinary retained
tools. Opening a project does not compile/execute its code. Editing, source Save, dirty-document
approval and Play are unimplemented; see [Editor workspace](../../../manual/editor/index.md).

Per-project preferences retain host topology, tab order, active/maximized groups and placements.
Missing monitors are clamped/centered; unknown kinds retain unavailable-panel records. Invalid
input is reported and preserved. Explicit Reset retains rejected bytes under a unique
`rejected-*.json` suffix and restores the preset without rebuilding live content. Validate the
complete replacement before publishing topology or detaching content; capacity rejection leaves
the live layout unchanged.

Layouts with retired Project/Scratchpad/Activity demonstration tools use the preset before
content/floats mount. Assets keeps its view ID; unknown extensions and restore keys become extra
Assets-group tabs. Already-authoring layouts restore unchanged, including hidden tools/floats.
Writes are bounded, serialized by a standard advisory file lock, baseline-checked and atomically
replaced with `tempfile`. I/O runs off the main thread; orderly exit flushes the latest published
state. Advisory locks coordinate cooperating processes, not hostile writers. This is not Save.

## Sources and verification limits

Structure: `assets/editor/prefabs/dock-*.prefab.ron`; presentation:
`assets/editor/styles/docking.css` and shared controls CSS. UI-KIT export-page identity is
`92:3713`; [Design provenance](design-provenance.md) separates navigation from pinned sources.

```sh
cargo test -p kedra_docking --locked
cargo test -p kedra_ui --test contracts --locked docking::
cargo xtask check --non-interactive
```

Use [Native docking tests](native-docking-tests.md) for real processes/input/frame fences and
preference restart checks. `editor.workspace` is read-only diagnostics, not another mutation path.
There are no docking screenshot goldens. Intermittent native interaction/capture failures remain
unresolved; native verification of this revision remains pending. Injected input/readbacks do
not certify physical OS capture/release, mixed-DPI monitors or Windows/Linux/Wayland interaction.
Bundled fonts preserve source strings but do not guarantee CJK coverage/IME. Document-close
approval and preference schema migrations require real consumers and separate acceptance.
