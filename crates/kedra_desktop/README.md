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
