# Scene View Controls Implementation Plan

> **For agentic workers:** Use executing-plans to implement this plan task-by-task.
> Steps use checkbox syntax for tracking. Do not commit without owner inspection
> and explicit approval, regardless of any default workflow recommendation.

**Goal:** Replace the old Scene View controls with the selected C-select/C-narrow
Blender-direction design and fully functional read-only camera navigation.

**Architecture:** Keep authored RON/CSS chrome and existing shared controls.
Extend view-owned presentation and the window-scoped input lease; reuse native
PanOrbitCamera, existing framing/projection commands and document selection.

**Tech Stack:** Rust, pinned Bevy 0.20, Bevy Flair, authored UI, existing Python QA.

**Spec:** [Scene View controls design](../../ui/scene-view-controls-design.md).

## Global constraints

- No Move/Rotate/Scale, authoring writes, Save, Play, marquee or multi-selection.
- No new dependency, unsafe code or alternate controller/framework.
- Existing shared docking chrome remains authoritative and unchanged elsewhere.
- Use English identifiers, diagnostics and docs; preserve source identities.
- Authored text files stay within 1000 lines and 64 KiB.
- Native UI acceptance requires input and actual frames, not compilation/RPC alone.
- Generated fixtures, captures and run evidence live under `target/`.

## Review focus

- Overlay-originated LMB must not also clear/select a scene object or drag a tab.
- Hidden/unready/UI-only targets and another native window must reject camera input.
- Escape, focus loss and docking must end every new drag without sticky motion.
- Overlapping signed axes at aligned views must retain deterministic hit ordering.
- Narrow/short canvases and DPR changes must not leave invisible interactive areas.
- Animated pointer alignment must share one camera/gizmo pose, preserve pivot
  radius, retarget from its current pose, and honor reduced motion immediately.

## File responsibilities

- `assets/editor/prefabs/source-document.prefab.ron`: authored scene chrome/IDs.
- `assets/editor/styles/scene-view.css`: scoped layout using shared control tokens.
- `assets/editor/icons/scene-*.svg` and metadata: exact exports and density variants.
- `assets/README.md`, third-party notices: provenance and required license notices.
- `crates/kedra_editor/src/scene_view/controls.rs` (new): view-owned chrome handles,
  readiness/option state and responsive presentation, extracted from `mod.rs`.
- `crates/kedra_editor/src/scene_view/orientation.rs` (new): camera-relative signed
  axis geometry and deterministic endpoint hit policy, not a generic widget system.
- `crates/kedra_editor/src/scene_view/camera.rs`: existing camera policy/commands.
- `crates/kedra_editor/src/scene_view/camera_motion.rs`: interruptible 200ms
  pointer alignment using Bevy quaternion interpolation and its Bezier solver.
- `crates/kedra_editor/src/scene_view/selection.rs`, `ui_marker.rs`: reversible
  overlay visibility, independent of document selection and framing bounds.
- `crates/kedra_editor/src/workspace/scene_input.rs`: gesture acquisition/lifetime.
- `crates/kedra_editor/src/workspace/scene_menu.rs`, `input.rs`, `mod.rs`: actions,
  shared popups, binding and accessible labels.
- Focused sibling test modules: geometry, controls, ownership and failure paths.
- `tools/qa/scene_navigation.py` and its focused helper/tests: native input/frames.
- `docs/ui/scene-view.md`: actual implemented contract and remaining limitations.

### Task 1: Baseline and authored chrome

**Interfaces:** Preserve `View::mount`, `View::navigation`, `View::area` and
`View::focus`. Introduce private `Controls` in `scene_view/controls.rs` to replace
the old select-label/button fields; its constructor resolves authored element IDs.

- [ ] Run `cargo test -p kedra_editor --lib --locked` as the clean baseline; stop
  and report baseline failures before changing production code.
- [ ] Add failing authored-control tests for View/Select, eye/grid/sun, selection
  rail, orbit, zoom/pan/frame/projection and absence of old World/units/selects.
- [ ] Add geometry assertions for 38px toolbar, 26px controls, 16px icon slots,
  96px orbit region, the selected A separators and 480px narrow layout.
- [ ] Run the focused tests to establish the failures.
- [ ] Download exact static icon exports from high-fidelity Figma context; reuse
  existing assets only after confirming exact matches. Add four-density metadata
  and preserve source/provenance and Lucide ISC notices in the asset inventory.
- [ ] Replace authored chrome and scoped CSS; extract chrome ownership into
  `Controls`, preserving existing readiness behavior and shared Flat/Quiet styles.
- [ ] Adapt `crates/kedra_import/tests/scene_view_icons.rs` to check every new icon
  has non-empty source and densities `[1, 2, 3, 4]`.
- [ ] Run `cargo test -p kedra_import --test scene_view_icons --locked` and focused
  editor tests. Do not infer rendered geometry from authored dimensions alone.

### Task 2: Functional menus, display options and selection mode

**Interfaces:** Extend existing `Action` and `scene_menu::Kind`, not a second
dispatcher. Keep `View::grid` and `View::light`. Add
`View::toggle_overlays(&mut self, world: &mut World)`; visibility is per view.

- [ ] Add failing tests: Clear selection changes only the active coordinator
  selection; raw GLB does not create document authority; selection tool focuses
  the viewport without source writes.
- [ ] Add failing tests: hiding/restoring spatial and UI selection overlays retains
  source selection and camera framing bounds, including unchanged selection.
- [ ] Add failing tests: all new controls disable on pending/error/retirement and
  spatial controls remain unavailable on UI-only/image targets.
- [ ] Run focused tests to confirm failures.
- [ ] Bind View/Select, eye/grid/sun, Frame and Projection to real existing actions.
  Keep explicit scene/editor lighting names in the sun menu. Expose narrow-layout
  Clear selection via View. Add accessible labels and focus behavior.
- [ ] Implement overlay suppression/restoration in presentation, without clearing
  `Navigation::selection` or changing source materials/visibility.
- [ ] Synchronize selected/disabled states from actual view/camera options, not
  from the most recently clicked button.
- [ ] Run focused controls, selection and UI-selection tests.

### Task 3: Live orientation and gesture ownership

**Interfaces:** `orientation.rs` supplies signed-axis presentation from camera
rotation and deterministic hit resolution. Existing camera `start`, `drag`,
`zoom`, `cancel` and `command` remain the motion implementation. Extend the
existing input capture with LMB-origin Orbit/Pan/Zoom and its release button.

- [ ] Add failing orientation tests for free rotation, Front/Back/Right/Left,
  Top with screen-up −Z, six signed endpoints and depth-ordered coincident hits.
- [ ] Add failing camera-command tests for repeated direction flipping, retained
  pivot and projection-scale preservation; reject nonfinite geometry/input.
- [ ] Add failing ownership tests: an overlay drag never selects; click axis
  never orbits; movement past the click threshold orbits instead of snapping.
- [ ] Add failing lifecycle tests for release, foreign-window release, Escape,
  focus loss, menu opening, tab hide, layout revision, host transfer and retirement.
  Assert capture ends and later motion cannot move the old camera.
- [ ] Run focused tests to establish failures.
- [ ] Inspect pinned Bevy/native UI mechanisms before implementing dynamic geometry.
  Use retained native UI primitives and existing transforms for camera-dependent
  disc/lines/endpoints; do not freeze the Figma perspective illustration as an icon.
- [ ] Route control hits before viewport picking, using rendered logical geometry,
  window ownership and clipping. Capture LMB orbit/pan/zoom through the existing
  lease and keep the initiating control excluded from ordinary scene selection.
- [ ] Update live orientation and hover/capture state from actual camera pose.
  Reuse existing camera commands for signed alignment and projection changes.
- [ ] Run focused orientation, camera, controls and scene-input regression tests.

### Task 4: Responsive and native acceptance

**Interfaces:** Keep production observation IDs stable where semantics remain
unchanged. Native QA consumes actual control rectangles and existing pose/selection
snapshots; do not add production-only test shortcuts.

- [ ] Add failing responsive tests at 1000/480/320/280/240px widths, short canvases
  and changed DPR. Check hidden Select has its action in View and clipped controls
  cannot intercept input. Keep complete 26px targets; never wrap the toolbar.
- [ ] Implement measured narrow mode at 640px and height-aware overlay visibility,
  retaining menu access when navigation controls cannot fit.
- [ ] Update QA geometry assertions and old projection/lighting menu assumptions;
  retain negative tests and source-inventory checks instead of weakening them.
- [ ] Extend native QA for axis click/repeat, disc drag, Zoom/Pan drag, overlay
  hiding/restoration, Frame/Projection, narrow layouts and Float/Redock. Assert
  unchanged selection during camera gestures and unchanged project source bytes.
- [ ] Run `python3 -m unittest discover -s tools/qa -p 'test*.py'`.
- [ ] Build fresh editor/project fixture deliveries using the documented
  `scene_view_fixture` commands; run native Scene View and navigation scenarios.
  Capture wide/narrow, hovered/captured, standard-direction and error states.
- [ ] Inspect actual native frames against the selected Figma chrome geometry,
  including icon slots and nested overlays. Keep real model rendering, not the
  illustration. Exercise UI-only/mixed and raw PNG/GLB views for regressions.
- [ ] Perform available physical RMB/MMB/cursor-lock checks; explicitly report
  any hardware, monitor or input scenarios that cannot be exercised.
- [ ] Update `docs/ui/scene-view.md` with actual behavior, reference nodes and
  remaining limitations; do not put local run narratives in repository docs.
- [ ] Run `cargo xtask check` and review focused diffs, file limits and asset
  provenance. Report actual results, failures and unverified acceptance boundaries.
- [ ] Request owner inspection; leave changes uncommitted until explicitly approved.

## Execution

The four tasks are sequential because chrome IDs, presentation state and input
ownership share contracts. Native inline execution is the recommended approach;
independent task implementers would repeatedly need the same ownership context.
The owner must review this plan and choose execution before implementation starts.
