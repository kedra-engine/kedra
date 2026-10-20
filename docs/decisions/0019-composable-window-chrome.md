# ADR 0019: Composable native window chrome

Status: architecture in use; native platform acceptance remains bounded separately.

## Context

The accepted direction requires one reusable window-header mechanism with an
integration API. Screens must not each implement caption controls, drag regions
and window-state synchronization. Figma defines three compositions of that
mechanism, not three separate implementations.

The current editor mounts a generic caption above its workspace toolbar and
above floating dock rails. Its native-drag handler recognizes a fixed top strip
and a right control exclusion. Embedded commands and tabs require explicit input
regions instead. `kedra_ui` is runtime-layer; `kedra_desktop` is editor-layer.

## Decision

### Ownership

- `kedra_ui::window_chrome` owns binding authored header slots, semantic input
  regions and geometry resolution. It knows neither editor commands nor docking
  topology and does not create or close native windows.
- `kedra_desktop::chrome` consumes these contracts and owns native window state,
  platform control reserves and native move/resize/minimize/maximize operations.
  Close emits Bevy's existing `WindowCloseRequested`; lifecycle owners decide
  whether that exits, hides tools or requires future document approval.
- `kedra_editor::chrome` composes authored skins and connects feature-owned
  commands. Docking supplies its existing retained rail/menu rather than building
  another set of tabs for the native header.
- Bevy components/events, authored `UiView`, Flair and Taffy remain the integration
  mechanisms. No new crate, ECS, layout engine or third-party dependency is needed.

The new `kedra_desktop -> kedra_ui` workspace dependency is justified by actual
native consumers of shared renderer geometry and region contracts. It obeys the
layer DAG. The opposite dependency is forbidden.

### Header contract

A binding targets an explicit live window, authored header root and local slots.
Only one active header can bind a window. Slots accept existing UI roots; they do
not create feature content. Header skins are consumer-supplied assets, not runtime
dependencies on the editor asset catalog. Caption, Editor and Floating are
composition policies in the editor, not variants hard-coded into shared UI.

The API validates live owners, unique slots, window ancestry and reparent cycles
before mutation. Rejected bindings preserve existing attachments. An attached
feature root remains feature-owned: detach returns it to an explicit live parking
parent before the header retires. No implicit descendant destruction transfers
ownership. Rebinding rejects stale handles and another window's live content.
Runtime entity handles are not persisted.

Every binding has a fresh ECS generation identity. Successful unbind retires its
handle, so an old handle cannot remove a replacement binding on the same root.
Slot window ownership is validated both at bind and immediately before attachment.

Input regions distinguish `Client`, `Drag` and `NativeControls`. The complete
header defaults to client input; only explicitly marked background can move the
window. Nested client controls override drag ancestors, even when disabled.
Docking tabs are client regions to native chrome; docking retains its own gesture
policy. Popups and modal input must not leak through to background dragging.

Resolved geometry uses the actual laid-out, visible, clipped UI tree, its target
camera, viewport offset, DPR and `UiScale`. There is no parallel geometry cache.
Missing layout, stale ownership, non-finite coordinates and ambiguous roots fail
closed: they never initiate native dragging. Resizing is a separate native-frame
operation with full control-region priority and window capability checks.

### Compositions

- Startup/general windows: one centered Caption row, with platform controls and
  symmetric title balance. Implementation choice: Boot and Loading, including
  failure/recovery presentation, are fixed-size and close-only; Projects and editor
  hosts remain resizable. Native capabilities and authored controls agree.
- Main Editor: one Editor toolbar row with project, transport and global command
  slots; no additional generic title row or demonstration workspace toolbar.
- Floating host whose topology root is one tab group: one Floating row containing
  the existing retained tabs and one group menu, with no duplicate dock rail.
- Floating host whose topology root is a split: a standalone Caption row and
  ordinary headers on all dock groups below, analogous to the main-window
  separation. Maximizing one group does not change that topology-based rule.
- Splitting a single-group floating host switches to standalone Caption; merging
  back to one root group restores integration. Content, tab identities, focus,
  scroll and preferences survive without feature reconstruction.

Docking projection must expose an explicit header destination rather than callers
reparenting private rail descendants after reconciliation. Candidate measurement
and committed projection use the same composition and minimum constraints. The
native drag spacer is never a dock drop target. Close/menu hit areas never start
tab dragging.

### Visual and platform contract

Figma file `2EZlK40cfx2Mub7GmDyI70`, window family `82:3049`, is authoritative:

- Caption/Editor/Floating rows are 32 logical pixels high.
- Dock tabs are 28px, bottom-aligned; Flat menu targets are 26px.
- Right-side native caption actions are 44 by 32px with 16px glyphs.
- macOS reserves 82px at the left for real system buttons. Do not draw replacement
  traffic lights. Editor toolbar starts after that reserve; floating menu is right.
- Right-layout Floating places the group menu before tabs. Preserve at least 96px
  of explicit native-drag background without squeezing native controls.
- Resize targets are 6px inward edges and 12px corners, distinct from the 1px
  layout inset. Maximized/fixed-size windows cannot initiate native resize.
- Owner-approved visual override: remove the light client outline and top bevel.
  Keep a transparent 1px layout inset so this changes neither content allocation
  nor native resize targets. OS-owned shadows/edges remain platform-managed.
- Native fullscreen has no persistent caption-only row, native control reserve,
  native-drag or resize strips. Implementation choice: Editor toolbar and Floating
  tabs/menu remain visible as ordinary client UI so commands and navigation are
  not lost; they are not a fullscreen caption. Caption-only hosts hide their row.
  Native OS exit mechanisms remain available; do not invent unsupported native
  auto-reveal. Double-click behavior must follow supported platform policy rather
  than silently imposing Windows behavior on macOS.
- Active/inactive state, maximize/restore glyph and controls follow actual native
  acknowledgment, not a guessed state immediately after sending an operation.

Main high-fidelity sources are `222:51056` and `222:51199`; single-group floating
sources are `222:51320` and `222:51364`. Caption references are `83:5309` and
`83:5327`. Existing ordinary docking uses `97:3495`.

### Scope limits

This work does not implement gameplay Play, source Save, project Build, global
search or editable Inspector. Their toolbar slots may appear disabled with honest
unavailable descriptions. Existing panel/reopen and reset-layout operations remain
reachable through the toolbar menus. No inert control appears enabled.

Source formats and docking preferences do not need migration for presentation-only
changes. Existing splits and floating layouts must restore without reset.
Physical mixed-DPI behavior and untested platforms remain explicitly uncertified.

## Alternatives

- Per-screen caption implementations duplicate platform and gesture policy and
  violate the requested integration API.
- A universal header aware of projects and dock topology reverses ownership and
  cannot be reused by another Kedra application.
- Manually moving rail entities after projection breaks ancestry hit tests,
  candidate measurement and retirement. Projection must own its destinations.
- Drawing macOS controls reproduces their appearance but not native behavior.

## Verification

Shared UI tests cover independent non-editor binding, rejected attachments,
detachment, window isolation, clipping and input priority. Native adapter tests
cover capabilities, state and request routing. Docking tests cover the transition
from integrated to standalone header and back, including candidate cancellation,
retained identities and layout minima. Editor tests cover every surface role and
restored split layouts.

Run focused Rust tests, the production native docking fixture and
`cargo xtask check`. Actual input and renderer frames are required for native
acceptance. Check DPR 1/2, narrow widths, long Unicode titles, menus, controls,
native drag versus dock drag, resize, maximize and fullscreen.

Old pinned reference pixels must not be silently overwritten or budgets widened.
New role-specific Figma exports require inspection and review; actual application
captures are evidence, never replacement expected images.
