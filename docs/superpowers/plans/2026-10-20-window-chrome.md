# Composable Window Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** Implement one reusable header mechanism and connect all editor windows
to the accepted Figma compositions without breaking docking or native input.

**Architecture:** Runtime UI owns authored slots and semantic region geometry;
desktop owns native actions and state; editor and docking compose existing content.
One-group floating hosts integrate their rail; split hosts use standalone captions.

**Tech Stack:** Rust, pinned Bevy 0.20.0, Winit through Bevy, Flair, authored RON.

**Spec:** `docs/decisions/0019-composable-window-chrome.md`.

## Global Constraints

- No new crate, unsafe code, third-party dependency or parallel layout authority.
- Rows are 32px; dock tabs 28px; group menu 26px; right controls 44 by 32px.
- macOS uses real native buttons with an 82px reserved left area.
- Floating native-drag spacer has a 96px minimum; resize uses 6px edges/12px corners.
- Boot/Loading are fixed-size and close-only; Projects/editor hosts remain resizable.
- Fullscreen hides caption controls/reserves/drag but retains toolbar and tabs as client UI.
- Preserve content/tab identities, focus, preferences and lifecycle acknowledgment.
- No Play, Save, Build, search or Inspector implementation is authorized.
- No reference blessing, relaxed gates, commits or publishing without owner approval.
- Authored text files remain within 1000 lines / 64 KiB.

## Review Focus

- An unavailable/disabled toolbar button must not become native-drag background.
- Hidden, clipped, overlapping or stale-window regions must not capture native drag.
- Splitting/merging or cancelling a candidate must not orphan a rail or retire content.
- Restored multi-group floating windows must use separate captions without migration.
- DPI/viewport conversion must apply once and preserve menu/control click priority.

---

### Task 1: Shared header slots and region contract

**Files:** Create `crates/kedra_ui/src/window_chrome/{mod,slots,hit}.rs` and
`crates/kedra_ui/tests/window_chrome.rs`; modify `crates/kedra_ui/src/lib.rs`.

**Interfaces:** Export `Header`, `HeaderSlot`, `RegionRole::{Client, Drag,
NativeControls}` and `HeaderRegion { header: Entity, role: RegionRole }`.
`Header::bind(world: &mut World, window: Entity, root: Entity,
slots: BTreeMap<String, Entity>) -> Result<Header, String>`;
`Header::attach(&mut self, world: &mut World, slot: &str, content: Entity,
parking: Entity) -> Result<(), String>`;
`Header::detach(&mut self, world: &mut World, slot: &str) -> Result<Option<Entity>, String>`;
`Header::unbind(&mut self, world: &mut World) -> Result<(), String>`;
`hit(world: &World, window: Entity, point: Vec2) -> Option<RegionRole>`.
The binding installs header ownership; detach parks borrowed content. Slots must
be unique descendants and content must belong to the same native window.

- [ ] Write tests `independent_consumer_attaches_and_detaches_without_editor`,
  `rejected_bindings_preserve_attachments`, `cycle_and_duplicate_window_rejected`,
  `client_descendant_overrides_drag_even_when_disabled`,
  `hidden_clipped_stale_and_ambiguous_regions_do_not_drag`, and
  `viewport_ui_scale_and_dpr_convert_once`. Assert exact entity retention,
  unchanged parents on rejection and `None` for invalid/missing geometry.
- [ ] Run `cargo test -p kedra_ui --test window_chrome --locked`; confirm red.
- [ ] Implement bindings with ordinary ECS components and actual computed geometry;
  reuse `logical_rect`, target-camera and clipping contracts, no shadow layout.
- [ ] Run that test and `cargo test -p kedra_ui --test docking --locked`; require pass.
- [ ] Review public invariants, errors, partial failure and owner-retirement semantics.

### Task 2: Native adapter and one input arbiter

**Files:** Create `crates/kedra_desktop/src/chrome/{mod,input,state}.rs` and focused
tests; modify `crates/kedra_desktop/{Cargo.toml,src/lib.rs,src/auxiliary.rs}` and
`crates/kedra_editor/src/chrome.rs`.

**Interfaces:** Add the justified workspace dependency `kedra_ui.workspace = true`.
Export `ChromeAction::{Minimize, ToggleMaximize, Close}` bound to an explicit
window, and `NativeChromeState` containing actual focus/maximized/fullscreen
state and platform reserve. Install via `DesktopPlugin`; editor installs no
competing native drag or resize system. Action activation consumes ordinary Bevy
`Activate`; Close emits `WindowCloseRequested` for the bound window.

- [ ] Add failing tests `actions_target_bound_window`,
  `disabled_controls_never_start_drag`, `native_reserve_beats_resize`,
  `resize_uses_six_pixel_edges_twelve_pixel_corners`,
  `fixed_maximized_fullscreen_and_modal_capabilities_block_invalid_actions`,
  `native_sync_waits_for_backend` and `close_remains_a_request`.
- [ ] Run `cargo test -p kedra_desktop --locked`; confirm new tests red.
- [ ] Move shared native behavior out of editor `chrome.rs`; use Task 1 hit results,
  actual backend state and explicit window capabilities. Preserve AppKit native
  edge tracking; use only supported safe Bevy/Winit APIs for system buttons.
- [ ] Implement platform double-click/fullscreen behavior only where the maintained
  backend exposes it; record concrete gaps rather than unsafe shims or fake controls.
  Boot/Loading allocation must set `resizable: false` and matching button capabilities.
- [ ] Run desktop tests and `cargo xtask architecture`; require pass.

### Task 3: Projection-owned dock header destinations

**Files:** Modify `crates/kedra_ui/src/docking/{mod,project,staging}.rs`;
extend `crates/kedra_ui/tests/docking.rs`; modify
`crates/kedra_editor/src/workspace/{mod,hit,input,hosts}.rs`.

**Interfaces:** Add `HeaderDestination { row: Entity, menu: Entity }` for a
consumer-supplied header's tab/menu slots. Expose
`Projection::set_header_destinations(&mut self, world: &mut World,
destinations: BTreeMap<NodeId, HeaderDestination>) -> Result<(), String>`.
Projection owns rail/menu mounting and restores default group slots when a
destination is removed. Host composition selects integration only for a floating
topology root `Node::Group`, never for a split, including maximized splits.

- [ ] Add failing tests `integrated_header_retains_tab_and_content_entities`,
  `split_and_merge_restore_default_and_integrated_headers`,
  `candidate_cancel_does_not_publish_header_moves`,
  `invalid_header_destination_preserves_live_projection`, and
  `external_rail_hit_bounds_exclude_native_drag_spacer`.
- [ ] Run `cargo test -p kedra_ui --test docking --locked`; confirm new tests red.
- [ ] Integrate destinations into staged validation/publication, retention and
  measurement, not post-sync private-entity reparenting. Extend rail-aware hit tests
  and native minimum calculations for the selected composition.
- [ ] Run UI docking tests and `cargo test -p kedra_editor --lib --locked`; require pass.

### Task 4: Authored skins and editor integrations

**Files:** Modify `assets/editor/prefabs/{titlebar,loading-titlebar,dock-group,
dock-workspace}.prefab.ron`, `assets/editor/styles/{startup,docking}.css`,
`assets/editor/scenes/{startup,boot}.prefabs.ron` and editor
`src/{chrome,views}.rs`, `src/workspace/{mod,hosts,fixture}.rs`.
Create `assets/editor/prefabs/editor-titlebar.prefab.ron` with its normal `.meta`,
plus focused editor binding tests. Extend shared skins only where required.

**Interfaces:** `chrome::bind_window` becomes the editor adapter using Tasks 1–2;
it selects Caption/Editor/Floating compositions and supplies typed slots to Task 3.
Existing Panels and Reset commands remain reachable. Unsupported feature actions
use Bevy disabled semantics and honest unavailable labels.

- [ ] Add failing tests `surface_roles_have_one_native_header`,
  `restored_split_float_has_standalone_caption`,
  `toolbar_commands_route_without_native_drag`,
  `unimplemented_commands_are_disabled`, `startup_controls_match_capabilities`,
  `fullscreen_retains_toolbar_and_tabs_without_native_drag`, and
  `caption_long_unicode_text_never_squeezes_controls`.
- [ ] Run `cargo test -p kedra_editor --lib --locked`; confirm new tests red.
- [ ] Re-fetch high-fidelity context for any changed design slots. Reuse exact local
  SVGs; download missing exports through the supplied Figma asset instructions and
  register density/provenance normally. Never use traffic-light illustration assets.
- [ ] Implement one 32px row per native host. Replace the demonstration workspace
  toolbar, retain a truthful status/notice destination, and preserve fixed startup
  source catalogs. Use actual project names rather than fixture strings.
- [ ] Run editor/UI/desktop focused tests and asset build/verify with a new delivery;
  require pass. Do not claim visual acceptance from these tests.

### Task 5: Native acceptance and documentation

**Files:** Extend production native QA under `tools/qa/`, relevant QA unit tests,
`docs/ui/{figma,docking-implementation,docking-tests}.md` and
`docs/architecture.md`. Update ADR status only to reflect actual implementation.
Store generated evidence exclusively under `target/qa/`.

**Interfaces:** Reuse the production docking fixture, authenticated input injection,
renderer captures and dedicated isolated preferences. Do not add a second gesture
driver or a runtime dependency on Figma/private tooling.

- [ ] Add QA assertions for command clicks, native background drag, tab drag,
  menu/control priority, integrated→split→integrated transition and restart restore.
  Include DPR 1/2, window movement, narrow/Unicode titles, maximize and fullscreen.
- [ ] Run QA tool unit tests; require pass before launching the corpus.
- [ ] Capture fresh reviewed Figma role references without modifying old budgets or
  blessing application frames. Preserve old reference provenance separately.
- [ ] Run
      `python3 tools/qa/dock_fixture.py --output target/qa/window-chrome-native --visual-policy accepted`
      with a fresh source delivery and extended scenarios. Inspect real input/frame evidence;
      distinguish old-reference incompatibility from mechanics failures.
- [ ] Run `cargo xtask check --non-interactive`; require the full summary and retain
  diagnostics. Report incomplete or unavailable native/platform checks explicitly.
- [ ] Update current-contract docs and review the whole diff for DAG, retention,
  stale bindings, inactive windows and unsupported controls. No commit until the
  owner inspects and explicitly approves the changes.
