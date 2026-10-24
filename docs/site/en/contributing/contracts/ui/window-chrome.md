# Window chrome contracts

Applications compose their own window headers through shared binding and native-input adapters.
[ADR 0019](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0019-composable-window-chrome.md)
defines this architecture. Native verification limits are listed below.

## Binding and borrowed slots

`kedra_ui::window_chrome` binds one authored root to an explicit live window. It supplies named
slots, borrowed attachments, renderer-derived input regions and Unicode title fitting; it creates
neither windows nor an editor skin. Applications may supply authored or ordinary native Bevy trees.

```rust
use kedra_ui::window_chrome::{Header, HeaderRegion, RegionRole};
use std::collections::BTreeMap;

let mut header = Header::bind(world, window, authored_root,
    BTreeMap::from([("tools".into(), tools_slot)]))?;
world.entity_mut(drag_background).insert(HeaderRegion {
    header: authored_root,
    role: RegionRole::Drag,
});
header.attach(world, "tools", existing_content, live_parking_parent)?;
// Before retiring the authored header:
header.unbind(world)?;
```

Roots/slots/content/parking destinations must target the same live window through camera ancestry.
Duplicate bindings/slots, cycles, foreign content and stale destinations fail without implicit
ownership transfer. Attach borrows content; detach/unbind parks it before header destruction.
`unbind(&mut self, ...)` validates all parking first, preserving attachments and the recovery handle
on error. Direct despawn bypasses this lifecycle. Fresh ECS binding generations prevent stale
handles invalidating later bindings to the same root. Revalidate slot cameras before attachment.

`Header::set_title` and the registered-root `set_title` adapter retain full text and reuse dock
labels' Bevy-shaped cluster ellipsis. Missing measurements neither erase Unicode source nor guess
byte/glyph widths.

## Input regions and native adapter

Client input is the default. Only explicit `Drag` backgrounds permit native gestures; overlapping
`Client`/`NativeControls` take priority. Buttons, fields and menus remain client regions even when
disabled. A modal blocks native gestures in its own window. Current layout, viewport, clipping,
DPR and `UiScale` determine geometry; invalid/unmeasured regions never start a native drag.

`kedra_desktop::chrome` installs via `DesktopPlugin`; `bind(world, window)` enables the native
adapter after UI binding. `ChromeControl` maps ordinary `Activate` to explicit `ChromeAction` and
window. Close emits `WindowCloseRequested`; lifecycle owners decide exit/hide/future approval.
`NativeChromeState` reports actual backend acknowledgment and focus/maximize/fullscreen state.
Window-local events, including injected input, drive gestures, not assumed hardware cursor changes.

[ADR 0020](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0020-macos-client-titlebar-input.md)
accepts the exact Winit macOS fix preventing AppKit automatic movement from transparent-titlebar
client tab drags. Empty-header movement still uses the shared adapter;
background movement is opt-in.

## Window compositions

Native rows are 32 logical pixels. Startup/general captions are centered; Loading/failure are
fixed-size and close-only, Projects resizable. Boot preparation is windowless, without splash.
Editor project/transport/command areas sit directly in the row. Unsupported Play/Save/Build/global
search stay disabled; panel/reset identities remain `workspace/panels` and `workspace/reset`.

One-root-group floats integrate their retained rail/common menu; split hosts keep a standalone
caption plus ordinary rails. Maximize does not change this topology rule. Split/merge switches
composition without rebuilding tabs/content/focus. `Projection::set_header_destinations` prepares
slot policy; successful sync publishes/reparents. Measurement uses the same skin. No preference
migration/reset is needed.

macOS reserves 82px for real system buttons, never illustrations. Other platforms use 44×32px
custom controls. Resize targets are 6px edges/12px corners. Integrated floats preserve 96px native
drag background outside dock targets. Fullscreen removes caption controls/reserves/gestures, while
toolbar/tabs remain client UI. The owner-approved frame has no light outer outline/bevel, retaining
a transparent 1px layout inset without changing resize geometry. OS shadows/edges are not replaced
with a borderless-window workaround.

## Verification and limits

```sh
cargo test -p kedra_ui --test contracts --locked
cargo test -p kedra_desktop --locked
cargo run -p kedra_desktop --example macos_header_policy --locked # macOS only
cargo xtask test -- features --only window-chrome --output target/test/window-chrome
cargo xtask check --non-interactive
```

Native launch requires trusted build inputs, an idle desktop and new isolated output; see
[Native scenarios](../../../manual/testing/native-scenarios.md#before-authorizing-a-run).
The runner uses dedicated state/credentials/private readbacks, resets only its isolated layout
to the four-tool preset, and checks DPR 1/2, commands, disabled actions and retained
integrated/split/reintegrated floating tabs. GPU fences/injected input are not physical native
button/OS-drag or mixed-monitor certification. Bevy captures exclude OS button drawing.
Platform double-click/fullscreen need separate acceptance; no unsafe shim or invented platform
double-click policy is supplied. There are no window-chrome screenshot goldens/pixel tests;
only kit controls have screenshot coverage. Current native acceptance remains pending, including
OS-drawn shadows/buttons and complete visual identity.
