# Composable window headers

The native header is a mechanism independent of editor screens and dock topology.
[ADR 0019](../decisions/0019-composable-window-chrome.md) defines its boundaries.

## Ownership and integration

`kedra_ui::window_chrome` binds one consumer-authored root to an explicit window.
It supplies named slots, borrowed-content attachment, renderer-derived input
regions and Unicode caption fitting. It creates no windows and supplies no editor
skin. Independent applications can use their own authored or native Bevy tree.

```rust,ignore
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

Roots must belong to the same live window through their target-camera ancestry.
Duplicate bindings/slots, cycles, foreign content and stale destinations are
errors, not implicit ownership transfers. Attach borrows the feature root;
detach/unbind parks it before the header is destroyed. `unbind` borrows its handle
mutably so a parking validation error preserves the caller's recovery handle.
Directly despawning the header bypasses this lifecycle contract.
Each binding has a fresh ECS generation; an unbound handle cannot invalidate a
later binding on the same root. Slot cameras are revalidated before attachment.

`Header::set_title` and the registered-root `set_title` adapter retain full source
text while reusing dock labels' Bevy-shaped cluster ellipsis. Missing measurements
do not erase Unicode strings or guess byte/glyph widths.

The header defaults to client input. Explicit `Drag` marks empty background;
`Client` and `NativeControls` exclude native gestures. Buttons, text inputs and
menus remain client regions when disabled. A modal blocks native gestures in its
own window. Geometry uses current layout, camera viewport, clipping, DPR and
`UiScale`. Invalid/unmeasured regions never initiate a native drag.

On macOS, [ADR 0020](../decisions/0020-macos-client-titlebar-input.md) pins a narrow
Winit fix so AppKit cannot move the window automatically when a client tab drag
starts in the transparent titlebar. Empty-header movement still uses the shared
native adapter; background-movable windows remain explicitly opt-in.

`kedra_desktop::chrome` installs through `DesktopPlugin`. Its `bind(world, window)`
enables the native adapter after a shared UI header is bound. `ChromeControl`
connects ordinary `Activate` to an explicit `ChromeAction` and window. Close emits
`WindowCloseRequested`; lifecycle owners still decide exit, hiding or future
document approval. `NativeChromeState` reports actual backend acknowledgment and
focus/maximize/fullscreen state. Input consumes window-local pointer events,
including injected input, rather than assuming a changed hardware cursor cache.

## Editor compositions

Native rows are 32 logical pixels. Startup/general windows use centered captions;
Boot and Loading are fixed-size and close-only. Projects remains resizable.
The editor embeds project, transport and command areas directly into its row.
Unsupported Play, Save, Build and global search are disabled. Panel/reopen and
default-layout commands retain `workspace/panels` and `workspace/reset` observation
identities after moving out of the demonstration toolbar.

A floating host with one root group integrates its existing rail and common menu.
A split host uses a standalone caption and ordinary group rails. Maximizing one
group does not change this topology rule. Split/merge switches composition without
rebuilding tabs, content or focus. `Projection::set_header_destinations` prepares
slot policy; successful sync owns publication/reparenting. Measurement shares the
same skin. No docking preference migration or reset is required.

macOS reserves 82px for real system buttons, not illustrations. Other platforms
use 44×32px custom caption controls. Resize targets are 6px edges/12px corners.
Integrated floats preserve 96px of native-drag background outside dock targets.
Fullscreen removes native caption controls/reserves/gestures; toolbar and tabs
remain ordinary client UI.

The owner requested no light outer outline or bevel. The authored frame keeps a
transparent 1px layout inset without changing native resize geometry. OS-owned
shadows/edges are not replaced by a borderless-window workaround.

## Verification and limits

```sh
cargo test -p kedra_ui --test window_chrome --test docking --locked
cargo test -p kedra_desktop --locked
cargo run -p kedra_desktop --example macos_header_policy --locked # macOS only
python3 -m unittest discover -s tools/qa -p 'test_window_chrome.py'
python3 tools/qa/window_chrome.py target/qa/session.json \
  --state-directory target/qa/state --report target/qa/window-chrome.json
cargo xtask check --non-interactive
```

Launch the editor with dedicated state, credentials and capture paths first.
The opt-in native command resets only its explicitly isolated layout to the
four-tool preset, then covers DPR 1/2, commands, disabled actions and retained
integrated/split/reintegrated floating tabs. Actual frames and injected input are
evidence, not physical native-button/OS-drag or mixed-monitor certification.
Bevy renderer captures exclude OS-owned button drawing. Platform double-click
policy and native fullscreen still require separate acceptance; no unsafe shim
or invented platform double-click policy is supplied.

Old pinned startup/docking references are not blessed or relaxed for the new
family. New role-specific Figma exports need independent review; old corpus
differences remain visible rather than becoming application-generated baselines.

`KEDRA_CHROME_FRAME=<full-frame.png> cargo test -p kedra_editor --test window_chrome_frames --locked -- --ignored`
checks the owner-requested absence of a light client rim on a real capture. It is
not an assertion about OS-drawn shadows or buttons.
