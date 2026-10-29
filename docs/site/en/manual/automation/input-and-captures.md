# Input and renderer captures

Input injection and renderer capture act on a live, authenticated window observed in
the current session. They are useful diagnostic tools, but neither is desktop remote
control. Read [Observation and operations](observation-and-operations.md) before using
the effect envelopes below.

## Deliver input through the running UI

`app.input` queues frame-ordered Bevy events. A click moves the pointer, presses and
releases the left button; it does not directly emit widget activation or assign a
feature's model. Text passes through keyboard/text input rather than direct field
assignment. The application still decides whether to accept the action.

To click the editor's New-project control, send the request below. It requires
a current, complete observation containing that visible enabled target. Replace
`SESSION` and numeric `123` with this process's session and observed window:

```json
{
  "jsonrpc": "2.0",
  "id": "click-1",
  "method": "app.input",
  "params": {
    "session": "SESSION",
    "id": "123e4567-e89b-42d3-a456-426614174002",
    "window": 123,
    "input": {"kind": "click", "element": "projects.new"}
  }
}
```

Poll `app.operation`, then observe the form and its feature state. A succeeded input
receipt says `delivered`, not “the project form opened.” A busy input transaction
rejects overlapping admission; serialize dependent effects.

## Supported input shapes

Use these shapes for `params.input`; they are not complete requests. Each new
effect needs its own session/window envelope and fresh non-nil UUID:

```json
{"kind":"click","position":[40,80]}
{"kind":"move","position":[60,90]}
{"kind":"button","pressed":true}
{"kind":"button","pressed":false}
{"kind":"modifier","modifier":"shift","pressed":true}
{"kind":"modifier","modifier":"shift","pressed":false}
{"kind":"drag","from":[40,80],"to":[160,120]}
{"kind":"wheel","delta":[0,40]}
{"kind":"key","key":"select_all"}
{"kind":"text","text":"Кедра 日本語"}
{"kind":"close_window"}
{"kind":"focus_window"}
```

A click accepts exactly one of `element` or `position`. Pointer coordinates must be
finite and inside the logical window. Wheel components are finite with absolute
value at most 10000; UTF-8 text is limited to 16 KiB. Chords are named, not arbitrary
key expressions: examples include `enter`, `escape`, `tab`, `shift_tab`, arrows and
`select_all`. Clipboard chords use Super on macOS and Control elsewhere.

For held gestures, explicitly pair press/release and use the host's Escape/cancellation
path when applicable. A succeeded press receipt does not release its hold. Adapter-owned
holds retain window ownership independently of receipts; retirement releases only
owned, unshared inputs, preserving hardware state. Cross-window pointer movement and
owned pointer release can address another live window; overlapping holds or keyboard
input in another window are rejected. Do not borrow hardware-held inputs for a test.

`close_window` injects standard `WindowCloseRequested`, not a click on the OS caption.
The host can refuse it. If the target retires before the final receipt, observe
retirement and verify process exit instead of blindly retrying the close.

`focus_window` explicitly requests backend activation once and awaits backend/Bevy focus
acknowledgment for up to eight seconds. It rejects injected holds and never fabricates
focus by setting an observed flag. Use it for deliberate window transitions, not to
restart a gesture cancelled by focus loss. Unsupported/unacknowledged activation fails.

## Resize and test DPR

`app.resize` uses `width` and `height` in logical pixels, each within 320..3840.
Optional `dpr` is a finite test override within 1..4. It does not move a window between
physical monitors or change monitor hardware. Wait for delivery and new layout/camera
observations before comparing geometry or requesting a capture.
The script-backed resize helper confirms delivery, not exact backend/Bevy/camera size
agreement. Native rounding is retained; await usable feature state and renderer evidence separately.

## Request a renderer frame

`app.capture` takes the standard `session`, operation `id` and observed `window`
parameters, without an arbitrary output-file parameter. It publishes
`<operation-UUID>.png` beneath the configured `--remote-artifacts` directory without
overwriting. Wait for input delivery first: capture admission rejects a busy input queue.

The service waits at least six application frames before selecting a readback and
fails a request after eight real-time seconds or target retirement. The timeout is
independent of FPS and is checked again by completion before publication. The Bevy
clock still needs updates; a stalled application needs an external watchdog.

A succeeded receipt's `detail` contains `path`, `width`, `height`, `uniform`, `window`
and `frame`. Width/height are physical image pixels. Success requires renderer readback,
PNG encoding and file publication, not merely creating a screenshot request entity.
Inspect and decode the published image independently before accepting it as evidence.

`uniform: true` means all RGBA pixels match. That can be valid for an application, but
does not qualify as startup visual evidence. A nonuniform image is not automatically
correct either. One readback does not establish stable consecutive frames or agreement
with an independent design reference.

## Capture ownership and limits

There are separate limits of eight pending capture operations and eight owned native
readbacks. Same-window requests may share an unstarted readback while retaining
distinct receipts and PNG names. Host-owned screenshots can supply a readback and are
not retired by the remote service. Capture never activates its target or changes focus.

A timeout can fail the operation before an already-started renderer readback completes.
The service retains the entity receiving that readback and counts it against capture
admission until completion; a timer does not free the slot. A late result cannot publish
a cancelled artifact. If a readback remains stuck, wait for completion or have the host's
lifecycle owner restart it rather than repeatedly requesting captures.

## What remains outside the claim

Continue to [Native scenarios](../testing/native-scenarios.md) for behavioral evidence and
[Visual verification](../testing/visual-verification.md) for reviewed capture/comparison workflows.

Injected events are not OS pointer capture, real IME composition, native file-picker
interaction, desktop gestures or mixed-DPI monitor certification. Renderer readback is
not a screenshot of the whole desktop. Neither input delivery nor a PNG alone shows
whether the feature responds correctly. Test the relevant interactions with real input
and rendered frames on the intended setup; the [testing guide][testing] explains how to
validate and record the results.

[testing]: ../../contributing/contracts/testing.md
