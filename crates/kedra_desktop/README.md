# kedra_desktop

This editor-layer crate owns native windows, cameras and UI roots. Install it after Bevy's window
and input plugins, using `window_plugin()` to avoid implicit windows and last-window
exit races. `replace` runs on the main Bevy thread and retires only its former
surface. Client-drawn captions issue native window operations; on macOS the hidden
AppKit caption retains the titled/resizable window structure.

`auxiliary` stages independently owned window/camera/root sets, waits for backend
readiness, publishes them and retires only their owned resources. It does not own
docking topology or feature content.

Ownership and stale-close behavior have headless ECS tests. They do not verify
real native event delivery, multi-monitor placement or input quality. See
[startup acceptance](../../docs/site/en/contributing/contracts/startup.md).

## Registry consumers on macOS

Cargo does not propagate a dependency's workspace patches to an application using
the published crate. The client-titlebar input correction requires this patch in
the consuming application's root `Cargo.toml`:

```toml
[patch.crates-io]
winit = { git = "https://github.com/kedra-engine/winit.git", rev = "948d27f0007602d2536a8974fd483c6fd33d19d3" }
```

Without it, successful compilation does not establish correct native client-region
drag behavior. Keep the reviewed pin and the acceptance limits in
[ADR 0020](../../docs/decisions/0020-macos-client-titlebar-input.md).
