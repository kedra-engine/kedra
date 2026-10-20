# ADR 0020: macOS client titlebar input in Winit

Status: accepted; owner authorized the public dependency fork and exact revision pin.

## Context

The integrated floating tab rail occupies the transparent native titlebar of a
full-size content view. Winit 0.30.13 inherits `NSView.mouseDownCanMoveWindow`.
For its non-opaque content view AppKit allows server-side window movement in that
band, even when `movable_by_window_background` is false. This can intercept tab
drags before Kedra receives input. Bevy-injected input bypasses this native path,
so shared client-region hit tests alone cannot detect the regression.

## Decision

Use Winit 0.30.13 from <https://github.com/kedra-engine/winit>, pinned to
`948d27f0007602d2536a8974fd483c6fd33d19d3`. The base is upstream tag `v0.30.13`,
commit `e9809ef54b18499bb4f2cac945719ecc2a61061b`, identical to the registry
package's recorded source revision. Cargo's normal `[patch.crates-io]` mechanism
replaces the existing transitive Winit package; no forwarding crate or bootstrap
source rewriting is introduced. The Git source is public and explicitly admitted
by `deny.toml`, with revision pins required and other unknown sources still denied.

The 13-line upstream delta passes the existing `movable_by_window_background`
attribute into `WinitView` and answers `mouseDownCanMoveWindow` with that value.
Default client views reject automatic native dragging; explicit background-movable
windows retain their opt-in. `Window::drag_window`, native controls, style masks,
maximize/fullscreen and non-macOS implementations are unchanged. Kedra's existing
shared hit classifier remains responsible for explicit drag requests on empty
header background. Docking owns tab drags as before.

One manifest-only adjustment removes `dpi`'s sibling path, matching the published
crate's registry resolution. This keeps the existing registry `dpi` 0.1.2 instead
of silently switching the Git build to the repository's `dpi` 0.1.1 workspace
member. No dependency versions or features are otherwise changed.

No workspace unsafe code or unsafe-policy exception is introduced. The patch
adds no raw-pointer access, Objective-C runtime mutation or new unsafe operation
to Winit. Its existing Apache-2.0 licensing and attribution remain intact.

## Alternatives and maintenance

- Disabling decorations or moving tabs below the titlebar changes the accepted
  composition and native controls rather than fixing input ownership.
- Disabling window movement globally breaks empty-header dragging and can affect
  Spaces/display reconfiguration. Per-hover toggles add a race before native input.
- Runtime view subclassing/swizzling needs unsafe platform integration and is not
  justified when the existing backend can implement the contract directly.
- Vendoring the entire upstream source would require separate reviewed repository
  policies; a public revision-pinned fork preserves existing workspace gates.
- A broad Winit/Bevy upgrade is not required for this fix. No compatible released
  upstream correction was identified when selecting the patch.

Keep the fork limited to this correction. On Winit/Bevy updates, inspect whether
upstream supplies the same native policy and run the native probe before removing
the Cargo patch. Keep the public pinned source available for fresh builds and
binary source/notice obligations. Do not replace the fork with unrelated patches.

## Verification and limits

`cargo run -p kedra_desktop --example macos_header_policy --locked` creates hidden
real AppKit windows on the main thread. It checks the default content view rejects
automatic movement, the explicit background-movable opt-in remains accepted, and
the default window remains movable for explicit native drag. Reverting the patch
makes the default-view assertion fail. macOS-only dev dependencies use the safe
`objc2-app-kit`/`objc2-foundation` APIs; they are already present transitively and
do not enter production dependencies or expose a public API.

Run `cargo xtask check` as well. The native policy probe is separate from the
headless gate. It does not generate physical pointer input or certify a complete
drag/drop gesture. Native acceptance still requires dragging a floating tab back
to the main window without moving its source window, dragging empty header space
to move the window, and checking native controls and split/single-group hosts.
