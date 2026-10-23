# Shared application automation

`kedra_app` is a runtime-layer Bevy plugin consumed by the editor and independently
tested without editor dependencies. `kedra_ui` supplies stable element identities
and actual Bevy layout geometry. See [ADR 0003](decisions/0003-application-automation.md).

## Explicit opt-in

Diagnostic automation exists only in builds with `cfg(debug_assertions)`. Release
does not compile the transport, injected-input/capture services or named test-program
surface, and rejects their CLI flags. Features and environment variables cannot
enable it. Normal scene startup and game-facing Luau execution remain available.

The argument schema belongs to `kedra_app::launch`, shared by games and the editor.
Games compose `LaunchOptions` with their own CLI, then call `configure` with the
host's normal `StartupScenePlugin`. `--scene` is a debug-only path override; native
plugins and sources still come from the application's trusted code. The editor's
project-first lifecycle shares these options. An explicit debug `--scene` selects
the one-shot authored entry lifecycle instead, without concurrently starting project
jobs or the project manager. See [ADR 0030](decisions/0030-explicit-startup-scenes.md).

An independent game's CLI composes the shared schema instead of copying remote
flags or transport setup:

```rust
#[derive(clap::Parser)]
struct Cli {
    #[command(flatten)]
    application: kedra_app::launch::LaunchOptions,
    // Game-owned arguments belong alongside the standard application options.
}
```

After installing `KedraAppPlugin`, the host's native plugins and `PrefabPlugin`,
call `cli.application.configure(&mut app, startup)` with its normal
`StartupScenePlugin`. Release uses that configured entry unchanged; debug permits
`--scene game://Scenes/Arena.scene.ron`. The same application executable handles
both cases. Remote remains off unless explicitly requested with fresh credentials.

```sh
cargo run -p kedra -- --assets target/editor-assets \
  --remote-port 0 --remote-credentials target/test/local-session.json \
  --state-directory target/test/isolated-state
```

The credential path must not exist. Enablement returns a pending endpoint; the
credential file is published without overwriting only after the first main-world
BRP request/cleanup cycle and acknowledged HTTP runtime/listener initialization.
Initialization or publication failure requests an application error exit. This
boundary does not establish native frame or feature readiness. The file contains
a loopback URL, session and bearer token. Keep it private; do not upload it with
captures or reports. Normal shutdown removes only this instance's successfully
published credential, provided its session/token still match. A killed process can
leave a stale file; use a new path for the next session. The token is not printed. Requests require
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
does not retire a screenshot entity owned by another feature or request native
window activation. Capture preserves focus in both the target and other windows.
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

`{"kind":"close_window"}` injects Bevy's standard `WindowCloseRequested` on
both native event surfaces for the authenticated live target. This exercises the
host's normal close policy, not an OS-owned caption button. The host may refuse
the request. Retirement before the final input receipt can fail that receipt;
observe target retirement and the still-live session rather than retrying blindly.

## Native UI regression checks

### In-application Luau sequences

`KedraAppPlugin` installs the shared `kedra_script::LuauPlugin`, without executing
project code. `app.describe.scripting` identifies Luau and the finite host-owned
program names. An authenticated `app.scenario` request supplies `session`, an
idempotency `id`, `scenario` and bounded JSON `arguments`. Unknown names, source/eval
fields, stale sessions and overlapping scenario admissions fail closed. Poll the
existing `app.operation` receipt; admission is not completion. `app.scenario_cancel`
uses a fresh idempotency ID and the active scenario's `operation` UUID to revoke it.
Cancellation does not roll back already delivered input or domain effects.

The built-in sequences are `app.input`, `app.resize`, `app.capture`, `app.click` and
`app.field`. Their native methods run in-process through the same registered Bevy
systems, without HTTP self-calls. Input still traverses the normal queue/widgets;
field sequences await focus, selection, text delivery and observed value. Sequences
declare `run(ctx, args)` and access these operations through `ctx.app`; host API
tables are not globals. The app API grants no editor or generic reflection authority.
Native observation IDs inside Luau are decimal strings, not potentially lossy numbers.
The RPC protocol retains its
existing integer identities.

The editor registers a separate scoped `ctx.editor` table with real `state()` and
`workspace()` observations, plus `editor.startup.forms`. This ordinary Luau source
tests DPR 1/2 geometry, modal focus, Unicode entry, validation retention and form
cancellation, with actual renderer captures. Rust independently validates and retires
those temporary files; they are fences, not new screenshot goldens.

`kedra_test::Client::{input,resize,capture}` explicitly call these programs.
`Client::operation` calls exactly the named native method, without implicit backend
selection or identity conversion. Existing project-owned Rust scenarios retain setup
and domain assertions, and use `Client::scenario` for multi-step application programs.
The outside transport timeout/process supervisor remains necessary for crashes or a
stalled frame loop; an in-app coroutine cannot supervise its own process. No generic
`eval`, filesystem module resolver or test DSL is added. Standard Luau modules can
be imported only from each program's explicitly admitted source/config snapshot.
See [runtime](../crates/kedra_script/README.md).

The independent application's `scene.switch` and `projection.exercise` are finite
application-owned programs, not conventions imposed on every game. Its fixed
debug capability set registers named reorder/split/activation/maximize/
visibility/restore programs over scoped native operations. Activation and Unicode
draft editing use real Bevy input.
The concrete `native-switch` and `native-projection` external verifiers retain
independent state, retained-identity, renderer/readback and two-process assertions.
They are not arbitrary-manifest scene/script runners. Other applications compose
the reusable `kedra_test` client/session/evidence APIs with their own verifier.
Shutdown requests normal native window close; successful process exit, not a
possibly retired input receipt, establishes clean termination.
The ordinary independent runtime host and all scene/scenario data live in the
canonical `tests/fixtures/test_project`, with one runtime-only Cargo executable and
no separate consumer project. Complete copies isolate named scenarios. Its fixed host catalogue is
embedded outside the independently verified editor/game asset manifests. Both
resource deliveries are required; CLI resource locations cannot select capabilities.

Compilation and current-revision native input/frame acceptance remain unverified.
The available scenarios and assertions define verification contracts, not past-run
success claims or permission to execute project code on Open.

### Running native tests

Commands, process isolation and feature coverage are documented in
[testing](testing.md). Project contents and working-copy ownership are documented
in the [shared test project](../tests/fixtures/test_project/README.md).
Feature checks use transient GPU fences, not image goldens. Only kit buttons and
text inputs have screenshot coverage; their source/profile and acceptance contract
is documented in the [UI corpus](ui/ui-corpus.md).

Public reports contain portable evidence only. Credentials, private logs and raw
failure observations must not be published. Startup design identities and independent
geometry remain in [the startup contract](ui/startup-layout-lock.md), never derived
from application captures.

## New application consumers

Install ordinary Bevy asset/input/window/render plugins before `KedraAppPlugin`.
The shared plugin does not create a window. Add `ElementId` and `ScreenId` to the
real retained tree; no separate test layout is required. Register implemented
feature BRP systems with `RemoteAppExt`; duplicate method names are programmer
errors. Only a host calling `transport::enable` opens a listener. Public structs
and requests remain process-local contracts, not project source-format schemas.
