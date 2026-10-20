# Shared application automation

`kedra_app` is a runtime-layer Bevy plugin consumed by the editor and independently
tested without editor dependencies. `kedra_ui` supplies stable element identities
and actual Bevy layout geometry. See [ADR 0003](decisions/0003-application-automation.md).

## Explicit opt-in

```sh
cargo run -p kedra -- --assets target/editor-assets \
  --remote-port 0 --remote-credentials target/qa/local-session.json \
  --state-directory target/qa/isolated-state
```

The credential path must not exist. The file contains a loopback URL, session and
bearer token. Keep it private; do not upload it with captures or reports. Normal
shutdown removes it. A killed process can leave a stale file; use a new path for
the next session. The token is not printed. Requests require
`Authorization: Bearer TOKEN`; browser Origin headers are rejected. There is no
arbitrary shell or reflection-based ECS mutation API.

`app.describe` lists registered methods. Effect requests carry `session` and a
non-nil UUID `id`; native targets use observed window IDs. For example, `app.input`
accepts `{"session":"SESSION","id":"UUID","window":123,"input":{"kind":"click",
"element":"projects.new"}}`. Observe IDs rather than hardcoding ECS handles.

`app.operation` takes `{"id":"UUID"}` and reports pending, succeeded or failed.
Input success means event delivery, not that a feature accepted the action. Query
feature state and rendered observations after delivery. A network timeout is not a
rollback. Reuse the same operation ID and arguments or query its receipt before
retrying. A different request with the same ID is rejected. Completed receipts
can expire from the bounded collection; these guarantees are not durable journals.

`app.observe` reports windows, camera sizes, screen IDs, stable UI geometry,
visibility, enabled state, focus and text-editing values. Coordinates are logical
window pixels, including the application's UI scale and camera viewport origin.
It reports truncation and duplicate visible IDs rather than silently
returning incomplete or ambiguous targets. It is not a full accessibility tree or
proof that an element is unobscured by another widget.
Click-by-ID fails closed on a truncated observation, even if its returned prefix
contains one apparent match: uniqueness beyond that prefix is not established.

`app.capture` writes UUID-named PNGs under `--remote-artifacts` and completes only
after renderer readback and file publication. The receipt includes physical
dimensions, frame and a uniform-image diagnostic. Uniform frames are valid for
some game applications but must not be accepted as startup visual evidence.
Concurrent requests for the same window can share one renderer readback while
retaining separate operation receipts and UUID-named artifacts. The capture service
does not retire a screenshot entity owned by another feature.
Timeout or window retirement fails the receipt immediately, but an extracted
native recipient remains alive until Bevy completes and reclaims it. Late results
cannot publish cancelled artifacts. The eight-owned-readback limit includes these
recipients, independently of the eight-operation queue. If native capture never
completes (for example, a window disappears before native preparation), capacity
is not recycled on a timer: diagnostics report retention and further admission
fails at the limit. Renderer completion or a host restart releases that capacity.

`app.resize` can set an explicit test DPR override, not change a physical monitor.
Pointer events, chords and text are injected through Bevy. OS-native file pickers,
desktop gestures, real IME composition and hardware monitor transitions are outside
this transport's claims. `editor.project` is a semantic adapter to startup rules;
it does not make semantic draft changes into evidence of keyboard interaction.

`app.input` also accepts `{"kind":"button","pressed":true|false}` for held-left
pointer gesture tests. Pair press with release or Escape/cancellation; movement
and captures between them exercise the running UI's actual gesture session. This
remains Bevy input injection, not physical OS pointer-capture certification.

## Native UI regression checks

Visual acceptance is screenshot-led and human-reviewed, not a font-name or layout
assertion. See [screenshot acceptance](ui/visual-acceptance.md) for Figma/actual,
overlay and difference bundles, including the platform-specific custom frame.
The structural checks below are supporting diagnostics, not design acceptance.

The optional standard-library Python client drives the running native application:

```sh
python3 tools/qa/startup.py target/qa/local-session.json --report target/qa/startup.json
KEDRA_QA_REPORT="$PWD/target/qa/startup.json" \
  cargo test -p kedra_editor --test visual_frames -- --ignored --nocapture
```

The first check uses actual pointer/keyboard events, validates error retention,
compares renderer-derived geometry at DPR overrides 1 and 2, and saves PNGs. The
second decodes those real frames with the existing image library and compares
dimensions and color samples to Figma. Neither is a mandatory headless CI check:
unavailable native rendering is not a passing test. Failures retain the report.
These checks cover targeted geometry and colors, not complete image equivalence.

Add `--lifecycle-state-directory target/qa/isolated-state` to the Python command
to test actual creation, fast open without a required loading window, native window
retirement and failure receipts. It refuses personal preferences and retains newly created fixture
sources under that isolated directory. `--geometry-only` can diagnose input/layout
when frames are unavailable, but reports frames unverified and cannot pass the
frame/color test.

`docs/ui/startup-reference.json` is manually transcribed from the identified Figma
nodes. Do not update it from implementation observations. Product-specific copy
must remain truthful: this slice creates sources, not a Rust workspace, starter
scene or executable project code. The approved layout does not authorize claiming
unimplemented functionality in UI text.

## New application consumers

Install ordinary Bevy asset/input/window/render plugins before `KedraAppPlugin`.
The shared plugin does not create a window. Add `ElementId` and `ScreenId` to the
real retained tree; no separate test layout is required. Register implemented
feature BRP systems with `RemoteAppExt`; duplicate method names are programmer
errors. Only a host calling `transport::enable` opens a listener. Public structs
and requests remain process-local contracts, not project source-format schemas.
