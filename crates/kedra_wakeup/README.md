# kedra_wakeup

Runtime-layer notification bridge between existing Bevy Tasks/producers and the
native event loop. It has no queue, executor, timer thread or operation registry.
Native hosts can explicitly opt into a Winit wait adapter; no pacing is installed
implicitly. The editor's asynchronous owners consume it; independent native hosts
can use the same adapter without editor dependencies.

The owning contract and crate-boundary rationale are in
[ADR 0044](../../docs/decisions/0044-native-readiness-wakeups.md).

Capture `Wakeup::for_world(app.world())` after adding the native window plugin,
then insert that value as a resource. The shared app plugin does this for its
consumers. At task admission, use the existing Bevy pool through the adapter:

```rust,ignore
let mut task = wakeup.spawn(IoTaskPool::get(), async move { load_project(path) });
// In the owner's ordinary update, without fetching a waker or notifying:
if let Some(result) = task.poll() {
    // Check ownership/generation and apply, then remove the task handle.
}
```

No initial owner poll or admission/completion `notify()` is needed. The adapter
registers on the actual Bevy Task immediately. An already-completed result is
retained and wakes the host too, so late admission and fast completion cannot
miss readiness. `track(task)` attaches the same mechanism to an already-spawned
or local Bevy task. `Task<T>` is an owned handle, not a second executor: drop
cancels the existing Task; `cancel().await` retains its result/cancellation race;
ordinary awaiting also wakes the awaiting executor. Nonblocking polls never
repeat output; awaiting a consumed handle violates the Future contract and
panics. Existing generation checks, cancellation decisions and authoring
authorization remain at the caller. No task result applies itself.
The supplied pool must still be driven normally. Background thread-pool completion
can wake a sleeping application; adapting a local/main-thread task does not make
its executor run while the main thread sleeps. Likewise, unadapted third-party
Tasks are not discovered globally. They need an integration boundary or an
explicit active-work fallback before indefinite sleep is safe.

For bounded std progress channels, wrap the producer once at construction:

```rust,ignore
let (sender, receiver) = std::sync::mpsc::sync_channel(32);
let sender = wakeup.sender(sender);
// Worker callbacks use ordinary sends; successful publication wakes automatically.
let _ = sender.try_send(progress);
```

The receiver and capacity are unchanged. Full/disconnected sends preserve their
errors and values, without notification or retry. Producer clones keep the same
behavior. Dropping a producer is not a result notification. Keep external channel
producers behind this boundary; sending through an unwrapped original bypasses
notification. `notify()` is only the escape hatch for integrations whose existing
publication API cannot be wrapped (the authenticated application mailbox).
Do not notify before returning a worker's Task result: the application could wake
before that result is ready and miss completion.

The `native` feature uses the host's existing Winit proxy and selected backend.
Default/headless builds use an inert adapter and can manually poll Tasks. A closed
event loop discards notifications; it does not retry or cancel operations. The
host must capture again if it adds native window support after capturing an inert
adapter. `for_world` can upgrade an installed inert default once a proxy exists;
already-held inert clones remain inert. Explicit host wakers are never replaced.

## Subsystem deadlines and active work

An explicit native host can install `ReactivePlugin { idle, focused_active,
unfocused_active }`. It owns WinitSettings, preserving immediate window/user
events and RequestRedraw. The editor selects a bounded 100 ms safety sweep;
games keep their own policy unless they explicitly opt in. No native backend,
renderer, thread or executor is installed by this adapter.

Subsystem integrations initialize `WakeDemand` and call `at(absolute_deadline)`,
`at_real(&time, elapsed_deadline)`, `active()` or `continuous()` during ordinary
updates. Continuous demand is distinct from a polling cadence: a live runtime or
serving viewport keeps normal Bevy updates in both focus states, even after its
first frame is ready. It dominates reactive waits until the owner stops renewing
it; in a shared App it keeps the entire App continuous, not just one viewport.
It does not define a fixed simulation rate, pause semantics or isolated clocks.
Use the
shared real-clock conversion instead of adding elapsed to `Time::startup()`:
Bevy elapsed time starts at the first update, which may follow slow initialization.
The adapter consumes these hints in Last;
Last producers must run before `WakeSystems::Apply`. Renew hints while work is
relevant; completion/cancellation/retirement simply stops renewing them. This is
one integration per subsystem, not per control or operation, and owns no timer
callbacks. Without a host adapter hints are inert. Input and tasks use their
external wake routes rather than waiting for a demand collector.

The earliest deadline can shorten waits. Absolute deadline identity is cached to
avoid Winit's configuration-change redraw loop; the final active-cadence window
services relative-wait early wakes. OS/load and active cadence can make work late;
this is not realtime scheduling. Owners must service or retire expired deadlines.
See [ADR 0045](../../docs/decisions/0045-bounded-reactive-editor.md) for the shared
UI/script/render integrations and explicit bounded-idle limitations. Indefinite
editor sleep is not supported: third-party assets and some animation states have
no usable public readiness seam, so the safety sweep remains intentional.

## Verification

`cargo test -p kedra_wakeup --all-features --locked` exercises adapter contracts.
The `native_readiness` example is a separate real native consumer, exercised by
the existing Rust scenario supervisor (Linux OS-wait observation):

```sh
cargo xtask test -- native-wakeup --output target/test/native-wakeup
```

It exclusively creates its output directory, waits for a submitted native view
and completed pipeline compilation, captures a blue frame, arms a
pending stdin task and enters diagnostic indefinite reactive sleep without ever
polling that handle from its owner before worker release. The Rust
supervisor confirms the native loop is sleeping, then releases stdin with
`complete` or `fail`, without window input, RPC or Luau. Success captures green;
the failure result preserves blue. Both publish result evidence and exercise a
closed-loop notification after exit. The supervisor enforces process deadlines.
The same supervisor also runs `--deadline` with indefinite diagnostic idle: a
350 ms deadline advances a GPU frame without any worker release or external input.
`--without-wakeup` is the example's explicit negative-control path: after
the same release it must remain asleep until the supervisor's deadline; do not
inject input to make it complete. This is not the editor's idle policy or a claim
of physical input quality. Headless contracts alone cannot verify native waking.
