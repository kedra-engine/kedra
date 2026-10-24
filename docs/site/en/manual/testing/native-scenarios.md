# Native scenarios

Native scenarios exercise ordinary applications with real windows and renderer frames.
They are explicit local operations, not an automatic consequence of Rust tests.

## Before authorizing a run

See [test dispatch](../cli/repository-and-tests.md) for command selection and
[input and capture contracts](../automation/input-and-captures.md) for the service boundary.

1. Trust the repository/project and its Cargo configuration; compilation can execute build code.
2. Install the [native prerequisites](../getting-started/index.md), including graphics support.
3. Use an idle, unlocked desktop. Reserve it for windows, focus changes and injected input.
4. Choose a new output directory. Never overwrite a previous run to hide a failure.
5. Keep canonical project sources and personal editor preferences out of mutable test state.
6. Arrange an external process deadline for potentially blocking custom probes.

The desktop preflight checks the unlocked console on macOS. Linux requires `DISPLAY`, X11 and
a window manager. This is not Wayland acceptance or hosted platform certification.
The Windows path does not provide equivalent unlocked-console proof; arrange the desktop yourself.
Do not interpret passing preflight as evidence that the requested interaction succeeded.

## Select a feature workflow

From the repository root, explicitly launch a bounded subset or the complete feature suite:

```sh
cargo xtask test -- features --only hierarchy --output target/test/hierarchy
cargo xtask test -- features --only scene-navigation --output target/test/navigation
cargo xtask test -- features --only luau --output target/test/luau
cargo xtask test -- features --output target/test/features
```

Use fresh paths for each invocation. With no `--output`, the driver reserves a new run under
`target/test/ui-runs/`. Unknown or duplicate feature selections fail rather than disappearing.
`--only` uses comma-separated exact names; a selected run is not complete-suite acceptance.

| Task | Existing selection | What to expect |
| --- | --- | --- |
| Project manager and forms | `startup`, `luau` | Isolated startup; Luau checks an editor-owned form program |
| Source navigation | `browser`, `hierarchy`, `inspector` | Private project copy and editor state assertions |
| Native workspace/chrome | `workspace`, `window-chrome` | Hosts, layout, focus and lifecycle behavior |
| Docking | `docking`, `docking-states`, `dock-fixture` | Production gesture path, state checks and renderer fences |
| Preview/navigation | `scene-view`, `scene-navigation` | Resource/layout readiness and scoped interaction |
| Isolated Inspector | `inspector-fixture` | Authored explicit scene, without project startup |

Hierarchy includes behavior and appearance phases in one session, with separate results and frame
receipts. There is no `hierarchy-appearance` command and no Hierarchy screenshot golden.
`scene-navigation` uses the model and mixed scene needed for navigation; it does not replace the
separate PNG/model/UI/mixed preview and host-transfer checks of `scene-view`.
RMB flight and MMB pan still require dedicated acceptance beyond the navigation scenario.

## What the driver owns

The driver builds the ordinary debug editor and builds/verifies editor and project deliveries.
Project-based cases receive complete copies of `tests/fixtures/test_project`, preserving identities.
It verifies copied bytes and checks the canonical source inventory again at suite completion.
The startup Create case is an explicit product operation under test, not fixture generation.

Scenario state and preferences stay under `private/<scenario>/`; the support code refuses personal
preference paths. Docking restarts against that scenario's saved state to check restoration.
The isolated docking scene runs twice without project panels/startup/preferences.
Explicit scenes use the ordinary editor and shared `--scene` lifecycle, not alternate app targets.

Generic `kedra_test` APIs find Cargo executables, manage processes and connections, and validate
captured results. Project-owned Rust functions choose when the application is ready, what behavior
to assert and when shutdown is allowed.
Runtime applications do not depend on this tooling crate or receive arbitrary remote source eval.

## Readiness and deadlines

The supervisor waits for named state predicates, admitted resources and observed layout rather than
sleeping until a screenshot looks plausible. A credential file only establishes transport readiness.
It does not prove a document is loaded, a control is mounted or a preview has resources.

| Mechanism | Implemented bound |
| --- | --- |
| Native credential startup | 45 seconds, checking for early child exit |
| One HTTP request | 8-second total timeout; no redirects or proxy |
| Native operation receipt | 12-second polling deadline |
| Named in-application Luau sequence | 55-second outer polling deadline |
| Common editor state/node readiness | 40-second polling deadline |
| Authorized graceful shutdown | Successful child exit within 10 seconds |

Polling deadlines are checked between attempts; a bounded RPC/predicate can extend elapsed time.
These are not a single hard deadline for compilation or the whole suite.
Potentially blocking custom tests need an independent killable-process deadline as well.
Drop kills/reaps unfinished native children as cleanup, never as successful lifecycle evidence.

Effects are submitted once and their existing receipts polled. A timeout does not roll back
an effect. Do not blindly resend an uncertain operation or silently switch execution mechanisms.
Missing/failed host-registered programs fail the scenario;
the client does not inject new Luau source.

## Input is injected, not physical

`Client::input`, `resize` and `capture` execute registered finite in-application programs.
The input path reaches native Bevy processing, but it is not a physical mouse or OS caption click.
An injected `close_window` exercises close-request policy, not the operating system's button.
Projection model operations in the independent game do not replace editor gesture checks.

For separate physical acceptance, explicitly reserve the machine and use its actual pointer and
keyboard: drag tabs, release/cancel, move/resize windows, close through native controls and verify
focus/retained state in real frames. Test IME, pickers and mixed-DPI monitor transitions on the
claimed platform when relevant. Record hardware, input route and observed result separately.
Do not claim those actions were performed merely because an injected scenario passed.

## Independent runtime consumers

The [test-fixture projection reference](../../api/luau/testing.md#fixture-only-projection-api)
describes explicitly admitted diagnostic calls. The
[game scripting reference](../../api/luau/game-host.md) separately covers switch authorization;
neither is a universal game SDK.

`native-switch` and `native-projection` require the concrete `kedra-test-project` package/bin,
a complete private copy of the canonical project and two independently verified deliveries.
They are not arbitrary-manifest runners. Copies must preserve the fixture's relative dependency
layout; generating a lockfile is allowed only in that copy, not canonical sources.

Given a prepared copy at `target/native-switch/project`, verified deliveries at the paths below,
and a new result path, the concrete invocation is:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo xtask test -- native-switch \
  --manifest-path "$PWD/target/native-switch/project/Cargo.toml" \
  --assets "$PWD/target/test/editor-delivery" \
  --project-assets "$PWD/target/native-switch/game-delivery" \
  --output "$PWD/target/native-switch/result"
```

Both independent scenarios first run the excluded game's projection-readiness contracts, then
build/launch two processes and require successful native-close exits. Root workspace checks do
not execute that excluded project. Other applications supply their own ordinary Rust verifier;
`cargo kedra test` selects such a project-owned Cargo target,
not an automatically supplied scenario.

For complete-copy, lockfile and both-delivery preparation, follow the canonical
[native game recipe][native-game-recipe].
After that preparation, its excluded host has a focused **headless** route:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --locked \
  --manifest-path target/native-switch/project/Cargo.toml \
  --bin kedra-test-project docking::scripting::tests
```

Run from the repository root. This executes the three host projection-readiness contracts,
not native input/frame acceptance. Report its execution separately. `native-projection`
accepts the same command arguments as `native-switch`, selecting the independent docking
composition in two processes rather than switch startup.

## Diagnose without losing evidence

Open `public/index.html` and `public/report.json`. Setup failure, unrun selections and failed
cases are explicit. Feature scenarios check behavior: they delete temporary validated PNG
readbacks and retain renderer receipts and dimensions, not pixel comparisons or galleries.
Inspect `private/` logs locally, including `native.log` and `rpc-errors.jsonl`, before a fresh run.
Do not publish credentials, unrestricted RPC data, private project copies or personal paths.

Focus loss cancels docking gestures; do not refocus/restart them invisibly to turn a failure green.
Report interruption and platform limits honestly. Continue with
[Visual verification](visual-verification.md) only for the separately reviewed component scope.

[native-game-recipe]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/native.md#run-explicitly
