# Automation and diagnostics

Kedra applications can expose a local diagnostic service for observation, injected
input, renderer captures and finite application-owned programs. The editor uses the
same runtime service as independent applications; it is not a remote game server.

Connect a trusted local tool to inspect a debug application or investigate a failed
operation. Application APIs and source formats are still unstable.
Discover the running application's methods rather
than assuming that every host implements editor commands.

## Start with the right boundary

Diagnostic automation requires **both** a build with `cfg(debug_assertions)` and
explicit host enablement. Installing `KedraAppPlugin` alone opens no listener.
Release excludes the transport, input/capture services and named diagnostic programs;
features and environment variables cannot restore those excluded modules.

Game-facing Luau execution and normal configured scene startup are separate mechanisms
that remain available in release. Remote diagnostics do not implement editor Play,
automatically execute project code, or make a read-only document editable.

The trusted Rust application chooses its plugins, source catalog, startup hydrator
and registered capabilities. Neither a remote request nor a debug `--scene` override
can choose new Rust plugins. The editor's explicit-scene lifecycle excludes concurrent
project-manager operations.

## Choose a task

| Task | Read next |
| --- | --- |
| Enable a private endpoint and understand credential ownership | [Connecting](connecting.md) |
| Discover methods, inspect UI and poll an operation | [Observation and operations](observation-and-operations.md) |
| Deliver events, resize a test viewport or request a PNG | [Input and captures](input-and-captures.md) |
| Run a registered sequence or distinguish failure layers | [Programs and diagnostics](programs-and-diagnostics.md) |

For writing game behaviors or host bindings, start with
[Programming](../programming/index.md). For inspecting documents manually, use the
[Editor guide](../editor/index.md). Automation does not expand those features' authority.

## A typical diagnostic session

1. Prepare the application's normal resources and an isolated state location where
   the host supports one. Decide which supervisor will stop the process and keep its logs private.
2. Explicitly enable diagnostics with a new credential path. Wait for its publication
   under an external startup deadline; do not treat a startup log line as readiness.
3. Authenticate, call `app.describe`, and confirm the current session and methods.
4. Call `app.observe`. Check target windows, complete UI observation and feature state
   before sending an effect.
5. Submit one effect with a fresh non-nil operation UUID. Poll its existing receipt.
6. After delivery, observe the feature's postcondition. If pixels matter, wait for
   renderer readback and independently inspect the resulting image.
7. Shut down through the host's authorized lifecycle and verify process exit. Keep
   credentials and unrestricted observations out of public reports.

## What each result proves

| Evidence | What it establishes | What it does not establish |
| --- | --- | --- |
| Credential file published | Main-world service cycle and HTTP initialization reached the publication boundary | Feature readiness or a rendered frame |
| `app.describe` | The current process's registered methods and program names | Permission to invoke missing APIs |
| `app.observe` | Current retained UI, window and camera state | Complete accessibility, unobscured hit targets or visual fidelity |
| Input receipt succeeded | The adapter delivered its queued Bevy events | Widget acceptance or physical OS input |
| Feature observation changed | The application reports the domain postcondition | That a keyboard or pointer caused it |
| Capture receipt succeeded | Renderer image readback and PNG publication completed | Stable visuals, correct design or hardware interaction |
| Scenario receipt succeeded | A registered finite program completed | Process health beyond completion or editor Play |

Do not replace one evidence category with another. A semantic draft update is useful
for setup, but it is not a typing test. An image cannot explain which input path ran.
Headless contracts and synthetic readback events verify service rules, not live GPU
or desktop behavior.

## Security and lifecycle responsibilities

The bearer token grants the application's registered capabilities with the authority
of the application user. Loopback binding and authentication are not an OS sandbox.
The service exposes neither arbitrary shell execution nor generic reflection-based
World mutation, but a host-owned command can still have real effects.

Use private local credentials, bounded waits and fresh observations. A network timeout
does not cancel admitted work. Receipts are bounded and process-local, not durable
transactions; cancellation does not undo already delivered effects.

Input injection does not test IME composition, monitor transitions or desktop gestures.
Check those with real input and rendered frames in the intended environment. Native
interaction and capture can fail intermittently, and no native platform is certified.

## Engineering references

Continue to [Testing](../testing/index.md) to choose evidence for these capabilities,
or the [CLI task map](../cli/index.md) to distinguish launch and verification commands.

The [application automation contract][automation] defines host integration and its limits.
The [testing guide][testing] explains how to run tests and record their results.

[automation]: ../../contributing/contracts/automation.md
[testing]: ../../contributing/contracts/testing.md
