# Application automation contract

Application automation controls access to diagnostics and manages credential publication,
operations and renderer captures.
Use the [Automation Manual](../../manual/automation/index.md) for connection/request recipes and
the [testing contract](testing.md) for evidence and test ownership.
[ADR 0003][adr-automation] records the architecture.

## Host composition and shipping boundary

Runtime-layer `kedra_app` is independent of the editor and test tooling. `kedra_ui` owns
stable retained identities and renderer-derived geometry. `KedraAppPlugin` installs
`kedra_script::LuauPlugin` but neither executes project code nor creates a window/renderer.
Hosts install ordinary Bevy asset/input/window/render plugins, native plugins and `PrefabPlugin`.
Place `ElementId` and `ScreenId` on the real retained tree, not a parallel test layout.
`RemoteAppExt` registers implemented feature BRP systems; duplicate names are programmer errors.
The BRP default method set is replaced by an empty allowlist before registration: there is
no generic reflection mutation, arbitrary shell or remote plugin-loading authority.

Diagnostic transport, injected input/capture and named test programs compile only with
`cfg(debug_assertions)`. Shipping parsers reject their flags; features/environment cannot
restore them. Normal startup and game-facing Luau remain available. Public request structs
are process-local contracts, not persisted project schemas.

Hosts flatten `kedra_app::launch::LaunchOptions` into their own clap parser, rather than
copying remote flags. After composing the plugins, call
`cli.application.configure(&mut app, startup)` with the normal `StartupScenePlugin`:

```rust
#[derive(clap::Parser)]
struct Cli {
    #[command(flatten)]
    application: kedra_app::launch::LaunchOptions,
    // Host-owned arguments belong alongside these options.
}
```

Release keeps that configured entry. Debug `--scene game://Scenes/Arena.scene.ron` changes
only the entry path: trusted code still selects sources, native plugins and script authority.
The editor's explicit-scene path uses one-shot authored startup, not concurrent project jobs
or project-manager startup. See
[ADR 0030][adr-startup].
Configuration errors abort launch, without falling back to a weaker policy.

## Authentication and credential publication

Only explicit `transport::enable` opens a listener. It synchronously binds IPv4 loopback;
port zero selects an available port. The fresh `Endpoint` contains `url`, `session` and
random bearer `token`. It grants application-user authority, not an OS sandbox.
Requests require `Authorization: Bearer TOKEN`; constant-time comparison rejects wrong tokens.
Every browser `Origin` is rejected, including local origins. There is no public-interface
binding, unauthenticated discovery or wildcard CORS.

The credential destination must not exist, including as a symlink. An owner-only Unix
temporary file is prepared in its parent. Windows requires a suitably ACL-protected parent;
the transport does not establish that ACL and native ACL acceptance is not certified.
Enablement returns a **pending** endpoint. Non-clobber publication follows the first
main-world BRP request/cleanup cycle and acknowledged HTTP runtime/listener initialization.
Initialization failure or a destination created during startup requests application error
exit. Neither the pending endpoint nor publication proves feature/native-frame readiness.

The token is neither a CLI argument nor printed by the launcher. Keep credentials, private
logs and raw failure observations out of reports. Teardown cancels/joins the server rather
than waiting indefinitely for slow connections. Credential cleanup requires successful
publication by this instance, a regular file and matching session/token; replacements and
symlinks are preserved. A killed process may leave stale credentials: use a new path next
time, never silently overwrite unfamiliar content.

## Mailbox and service bounds

Axum/Hyper/Tokio own HTTP; BRP owns main-world dispatch, not an alternate ECS scheduler.
The installed transport has these independent bounds:

| Boundary | Limit / result |
| --- | --- |
| HTTP JSON body, including envelope | 64 KiB; oversized body is HTTP 413 |
| Concurrent authorized requests | 16; excess is HTTP 429 |
| JSON-RPC batch | 1..16 requests; invalid envelope/batch is `-32600` |
| Mailbox response | Five seconds; `-32000` reports uncertain execution |
| Whole HTTP request | Six seconds; HTTP 408 |
| Authentication / browser origin | HTTP 401 / 403 respectively |
| Streaming `+watch` | Rejected with `-32601` |

Notifications have no normal reply; an all-notification result is HTTP 204. Batches are
not atomic: earlier effects can execute before later failure or outer timeout. No timeout
implies rollback. The external supervisor must still bound startup, process lifetime and
a stalled frame loop; an in-application coroutine cannot supervise its own process.

## Operations and observation identities

`app.describe` lists registered methods. Effect requests supply the current `session`, a
non-nil UUID `id` and, where required, an observed live `window`. `app.operation` takes
`{"id":"UUID"}` and returns `pending`, `succeeded` or `failed`, with request hash/detail.
Matching ID/argument retries reuse the receipt; conflicting reuse fails without effects.
Completion is terminal, including late/duplicate completions. At most 256 receipts are
retained: completed entries expire oldest-first, active entries are not evicted, and a
collection full of active entries refuses admission. This is not a durable replay journal.
Query the existing receipt after uncertain delivery before considering another effect.
Input success proves event delivery, not feature acceptance: observe the postcondition.

`app.observe` reports windows, native state, camera sizes, screens, real UI geometry,
visibility/enabled state, focus and editable values. Logical window pixels include UI scale
and camera viewport origin; PNG dimensions are physical pixels. Duplicate visible IDs fail
the snapshot; the 4096-element cap reports truncation. Click-by-ID rejects even a uniquely
matching prefix of a truncated snapshot. Disabled ancestors and clipped centers are
rejected, but this is neither a full accessibility tree nor proof of no covering widget.

Shared RPC window/entity IDs, including `app.observe`, remain full-width **integers**.
In Luau native observation IDs are decimal strings. Four editor raw-RPC positions also
emit decimal strings directly; do not normalize them back to numbers:

- `editor.workspace.navigation_input.viewports[*].eligible_windows[*]`
- `editor.workspace.navigation_input.viewports[*].pointer_windows[*]`
- `dock.fixture.host`
- `dock.fixture.fields[*]`

These identities are process-local, not persisted document/node identities. Clients must
preserve exact values and re-observe after replacement or layout/lifecycle changes.

## Input and capture ownership

Pointer events, held-left `button`, chords, text and close requests traverse Bevy's normal
input path. Pair holds with release or cancellation; receipts do not release them.
`close_window` emits standard `WindowCloseRequested` on both native event surfaces. Hosts
can refuse it; retirement may fail its final receipt. Observe retirement and successful
process exit instead of blindly retrying. `app.resize` supports logical dimensions
320..3840 and finite test DPR 1..4, not physical monitor changes. `editor.project` is a
semantic startup adapter, not evidence of keyboard entry.

Capture admits only after input delivery. It settles for at least six application frames
and fails after more than 180 frames or target retirement; these are not wall-clock
deadlines. Completion requires renderer readback, PNG encoding and non-clobber publication
as `<operation-UUID>.png` under `--remote-artifacts`, never a client-selected filename.
The receipt records physical dimensions, frame and uniform-image diagnostics. Uniform
frames can be legitimate application output but are not startup visual evidence.

Eight pending operations and eight owned native readbacks are separate bounds.
Same-window requests may share an unstarted readback but retain independent receipts and
artifacts. Host-owned screenshots can supply it and are never retired by this service;
already extracted older frames cannot satisfy a new request. Capture activates no window
and preserves focus in target and other windows.

Timeout/retirement fails a receipt immediately. An extracted native recipient remains
alive until Bevy completes/reclaims it; late completion cannot publish cancelled artifacts.
Retained recipients count against the eight-readback limit even after operation removal.
Capacity is not recycled on a timer: a never-completing native capture blocks admission
at the limit until renderer completion or host restart. Diagnostics disclose retention.

Injected input/readback does not certify OS pointer capture, desktop gestures, native file
pickers, real IME or hardware/mixed-monitor DPR transitions. Native quality requires actual
relevant input and frames, not compilation, RPC or semantic draft changes.

## Finite programs and external supervision

`app.describe.scripting` exposes Luau and finite host-owned names, never source eval.
`app.scenario` admits `session`, idempotency `id`, `scenario` and bounded JSON `arguments`:
64 KiB, depth 32 and exact-double-range integers, also subject to the whole HTTP body cap.
Unknown names, source/eval fields, stale sessions and overlapping scenarios fail closed.
Poll the existing operation; admission is not completion. `app.scenario_cancel` uses a
fresh cancellation `id` and active `operation` UUID. Revocation does not undo delivered
input, published artifacts or domain effects.

Built-ins `app.input`, `app.resize`, `app.capture`, `app.click` and `app.field` execute
native registered systems in-process, without HTTP self-calls. Sequences declare
`run(ctx, args)` and use scoped `ctx.app`, not host API globals. Field sequences await
focus, selection, text delivery and observed value. No editor/World authority is implied.
Only explicitly admitted program source/config snapshots provide standard modules;
there is no filesystem resolver, generic eval or extra test DSL. Current type names are
`AppContext`, `RuntimeContext` and `BehaviorContext`, without compatibility prefixes.
See the [runtime contract][runtime].

The editor's scoped `ctx.editor` supplies real read-only `state()`/`workspace()` and
`editor.startup.forms`: DPR 1/2, modal focus, Unicode, validation retention, cancellation
and renderer fences. External Rust validates/retires temporary files; these are not goldens.
`kedra_test::Client::{input,resize,capture}` explicitly select programs.
`Client::operation` calls exactly the named native method, with no implicit backend or
identity conversion. Rust scenarios retain setup, domain assertions and process deadlines.
The [shared tooling contract][test-tooling] defines client/process/evidence responsibilities.

## Concrete native host and evidence limits

The canonical `tests/fixtures/test_project` owns one ordinary runtime-only executable,
scenes and programs. Complete copies isolate scenarios; there is no second consumer
workspace or per-scenario target. Its fixed embedded catalogue is outside imported assets;
both independently verified editor/game deliveries are required resources, not capability
selectors. Loading data never authorizes project execution on Open.

Its finite `scene.switch`/`projection.exercise` and named reorder/split/activation/maximize/
visibility/restore programs are application-owned, not universal game conventions.
Activation/Unicode draft entry use real Bevy input. External `native-switch` and
`native-projection` independently assert state, retained identity, renderer/readback and
two-process lifecycle. Normal native close must end in successful process exit, not merely
a possibly retired receipt. Other applications compose `kedra_test` with their own verifier.
See the [native game contract][native-game].

Record compilation and native checks separately for the revision under review.
Only 22 kit button/input cases have
screenshot coverage; feature captures are transient GPU fences. Independent startup design
identities/geometry remain in the
[startup layout contract][startup-layout], never derived from app
captures. The [UI corpus][corpus] retains source/profile provenance.
Public reports admit only reviewed portable evidence, never credentials/private logs/raw failures.

## Implementation and verification owners

`kedra_app/src/{lib,debug,launch,transport,operations,observe,input,capture,scripting}.rs`
own these contracts. Private tests include transport publication/auth/mailbox failure paths,
input stale-target/hold ownership, truncated observation rejection, terminal receipts and
`capture/lifecycle_tests.rs` retention/late-completion limits. These use non-editor hosts
and the real BRP mailbox, not proof of native rendering. External evidence remains owned
by `kedra_test` and `tools/test/src/scenarios`. See
[Native scenarios](../../manual/testing/native-scenarios.md) for authorized run preparation.

[adr-automation]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0003-application-automation.md
[adr-startup]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0030-explicit-startup-scenes.md
[runtime]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md
[native-game]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/native.md
[startup-layout]: ui/startup-visual.md
[corpus]: ui/ui-corpus.md
[test-tooling]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_test/README.md
