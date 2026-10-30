# ADR 0044: Native readiness notifications for existing tasks

Status: adopted. Explicit host pacing is covered by [ADR 0045](0045-bounded-reactive-editor.md).

## Decision

Runtime-layer `kedra_wakeup` adapts standard Future wakers to the existing Bevy
Winit event-loop proxy. It owns readiness notification only, not an executor,
scheduler, work registry or authoring authority. `Wakeup::spawn(pool, future)`
uses an existing Bevy pool and returns an owned `Task<T>` handle; `track(task)`
attaches the same mechanism to an existing/local Bevy task. The handle registers
the host waker immediately, retaining any already-ready output and notifying in
that case too. Owners only call `task.poll()` or await; they do not need an initial
poll or explicit admission/completion notification. The handle preserves drop
cancellation and provides `cancel().await`; it never detaches or applies results.
Awaiting preserves readiness for both the host and the awaiting executor.
Existing owners retain admission, generation checks, cancellation decisions,
result consumption and Save acknowledgment.

`Wakeup::sender` wraps an existing bounded std producer once. `send` and `try_send`
notify only after successful publication, preserving errors, capacity and
backpressure. Clones retain the notification boundary. No new queue or consumer
is introduced. Unsupported external publication APIs use explicit `notify()` at
their integration boundary, not throughout feature code. Sender drop is ordinary
disconnection, not a result event; task drop cancels work, not a new host action.

The task handle supplies the native waker to the actual Bevy Task's Future poll. Do
not notify just before returning from a worker: the main thread can wake before
the Task has published its result and miss the only completion signal. Standard
Future readiness registration handles the completion/registration race without
introducing another executor or operation lifecycle. A closed event loop discards notification;
it never authorizes replay of work.

Hosts capture and cache `Wakeup::for_world` after native plugin initialization.
An installed inert default can be upgraded once a native proxy exists; explicit
host wakers remain authoritative. The `native` feature is opt-in, while headless
consumers can poll manually with an inert adapter. This crate creates no window,
renderer, thread or polling loop and selects no native backend or update rate.
The existing executor must still make progress: a local/main-thread task does
not start executing while its thread sleeps merely because its completion is
adapted. Unadapted third-party Tasks are not globally discovered. Such paths need
a subsystem integration or active-work fallback before indefinite sleep is safe.

## Boundary and alternatives

The adapter has an independent responsibility, a small API and real editor and
native consumer routes, so it is not embedded as editor-only infrastructure.
`kedra_desktop` is currently editor-layer and cannot be a dependency of an
independent runtime application. `kedra_app` consumes the adapter for its existing
authenticated mailbox wake but does not own generic task readiness. Existing
Bevy Tasks, `std::task::Wake` and Winit proxy delivery supply the mechanism; no
new third-party dependency or async framework is needed.
The small owned handle is necessary because a first readiness poll can consume
an already-completed output. Retaining that output closes the registration race
without workers notifying before publication or callers remembering a manual
initial wake. A generic worker-future wrapper that notifies just before returning
does not provide that guarantee. There is no parallel cancellation registry.

Editor job orchestration and idle policy remain host-specific. Games may keep
continuous updates; no editor pacing is implicitly installed in runtime consumers. Result
notifications do not replace tooltip/caret deadlines, script clocks, preferences
save intervals, asset/pipeline readiness or in-flight native readback demand.
The bounded editor policy and remaining indefinite-sleep limitations are defined
by [ADR 0045](0045-bounded-reactive-editor.md).

## Verification boundary

Public adapter contracts cover completion without an initial owner poll,
already-completed Tasks, exact failure-result transfer, cancellation, awaiting,
headless use and producer publication/errors without consuming messages.
Existing editor owner suites retain source, stale-generation, cancellation and
Save failure/acknowledgment assertions through the same polling adapter.

The `kedra_wakeup` native example exercises a real window/event loop, worker
readiness and GPU readbacks, deferring all owner polling until worker release.
A pipe releases the worker without window input,
RPC polling or a running Luau scenario. Its explicitly selected diagnostic
sleep is not editor policy. The no-wakeup control uses the original poll-once
path under an external process deadline; the supervisor must retain failures
and must not inject incidental input to complete it. Native source readiness
does not certify physical input quality or the editor's full idle-sleep contract.
`cargo xtask test -- native-wakeup` supervises the concrete example through the
existing Rust Cargo/native-process mechanisms. Its OS wait-state observation is
currently Linux-only. No Python/Node service or QA-driver self-test is required.
