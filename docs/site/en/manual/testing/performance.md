# GUI performance diagnostics

Profile a concrete user action before changing rendering or update policy. Keep an
unprofiled control run: tracing, sampling, RPC observation and frame readbacks have costs.
Do not run compiler jobs or the full native suite alongside latency measurements.

## Build and observe

The development profile optimizes workspace code at level 1 and dependencies at level 2.
This keeps native ECS, text and clipping costs representative while retaining debug
assertions and the opt-in automation service. Release uses the separate shipping boundary.
Compare equivalent builds and retain the exact command, source revision and environment.

Explicitly enabled debug automation collects up to 4096 main-world update intervals and
UI component-change samples through Bevy's diagnostics. `app.performance` is a read-only
RPC, also available as `ctx.app.performance()` in scoped Luau. It does not serialize the
UI tree, sample text values, reset history, perform input or enable an HTTP listener.
With `{"include_changed_nodes":true}`, RPC additionally returns at most 32 changed Node
entities from the latest sampled update, their stable IDs/parents when present and Node
properties, never text values. This option is also accepted by `ctx.app.performance`.

The result contains:

- `enabled`, `frame` and aggregate `inventory` for UI/text/editable nodes and windows;
- `update_intervals`: sample count/capacity, mean, min, p50/p95/p99 and max in milliseconds;
- `ui_changes`: latest and mean changed Node/Text/TextFont/UiTransform components per update.
- `focus_events`: cumulative observed window-focus events while collection is enabled,
  useful for rejecting a measurement interrupted by focus changes.

Without enabled automation, collection is inactive; unavailable measurements are null.
Histories are rolling, not scoped to one action. A component change is Bevy change detection,
not proof that the value differs or that the entire subtree was rebuilt. Update intervals
include pacing, waits and presentation backpressure: they are not CPU work duration,
GPU execution time or physical input-to-photon latency.

The editor uses an explicit Winit policy with a 100 ms idle safety sweep. Native
window/input events and adapted task/progress publication wake it immediately.
Subsystem deadlines can shorten the wait; active work uses a 1/60 s focused or
100 ms unfocused polling cadence. Visible serving Scene Views require Continuous
updates in both focus states, including after frame readiness. With the current
shared World this keeps the whole editor App continuous. These are wait policies,
not exact FPS caps or an isolated simulation clock. Runtime games inherit none of
these settings. Indefinite editor sleep is not supported; see
[ADR 0045](../../../../decisions/0045-bounded-reactive-editor.md).

## Record and open a CPU profile

Runtime-layer `kedra_profiling` owns explicitly started, bounded CPU recordings.
Its optional `capture` feature integrates with Bevy's existing log layer and uses the
Perfetto SDK internally. Application code uses Kedra's instrumentation API, not backend
types. Ordinary builds have no recording backend and expose no capture CLI flags.
This capability is separate from debug automation: recording does not require RPC.

Build the editor with `--features profiling`. Pass an explicit output directory and
the ordinary verified editor/project delivery arguments:

```sh
cargo run --locked -p kedra --features profiling -- <editor-arguments> \
  --profile-output target/performance/editor --profile-manual
```

Use the [verified delivery/startup prerequisites](../getting-started/index.md).
With authorized profiling, **F9** starts/stops a finite editor recording. The default
maximum duration is five seconds and the SDK trace buffer is 32 MiB. Optional
`--profile-duration-ms` (100..=30000) and `--profile-buffer-kib` (1024..=65536) bound
collection. Without `--profile-manual`, recording starts at application Startup.
These are recording duration controls, not sleeps used to establish UI readiness.

The CLI reports setup, recording and terminal success/failure. Success names a new private
directory containing `trace.pftrace` and `recording.json`. Stop/publication are asynchronous;
closing the owner before publication cancels, rather than advertising an incomplete file.
Existing recordings are never overwritten. Saved files survive application exit.

Open [Perfetto UI](https://ui.perfetto.dev/) and use **Open trace file** for the local
`trace.pftrace`. Review uses the ready-made browser UI: Kedra does not install
a WebView or implement a native trace viewer. Opening a local trace does not require
uploading it; do not use sharing/upload features for private recordings by default.
Inspect the trace's loss/error statistics and unfinished boundary slices. A bounded
buffer and successful file publication do not guarantee lossless collection.

The [independent Bevy example][profiling-crate] exercises the same recorder without an
editor, window, assets or automation. Hosts configure the standard Bevy log hook and
consume `Recorder`; no competing global subscriber or scheduler is installed.

## Custom markers and counters

### Semantic local overview

Add `--profile-label menus` to identify the workload explicitly. Host/build/session
identity and the label are saved in `recording.json` and the trace's `kedra.recording`
event. No label means unspecified; rolling-buffer overwrite can also remove context.

The [local Perfetto review kit](../../../../../tools/profiling/README.md) provides
bounded SQL for health/system ranking and a ready-made UI macro for categories,
update intervals and CPU render correlation. Run health checks before interpreting
timings; inspect the manifest's adapter drops as well as processor loss/error stats.
The optional runtime `render` feature passes an explicit update identity through
Bevy extraction. These CPU entry points do not measure GPU work or screen presentation.
The editor enables it; headless applications do not need rendering dependencies.

`Category` is part of the engine facade; use `Marker::with_category`,
`Counter::with_category` or `Operation::begin_in`. Luau instrumentation uses Scripts.
Bevy categories are best-effort name classification, not guaranteed attribution.
The default ranking excludes scheduling internals and logical operation tracks;
both remain available in the original trace. Native thread labels may still be
truncated by the OS. Macro SQL execution is not visual acceptance of its UI layout.

Rust code depends only on the engine facade:

```rust
static REFRESH: kedra_profiling::Marker =
    kedra_profiling::Marker::new("editor.inspector.refresh");
let _scope = REFRESH.enter();
let objects = kedra_profiling::Counter::new("game.visible_objects", kedra_profiling::Unit::Count)?;
objects.sample(42_i64)?;
```

Own/reuse counter handles. Samples are absolute values with immutable units, not implicit
increments/rates. Names qualify application/subsystem within a process. Signed integer
samples retain 64-bit precision; floating samples must be finite. CPU scope guards are
thread-bound and must not survive an await/yield. Use `Operation::begin` / `finish` for
logical elapsed operations that cross suspension; Drop without Finish records cancellation.

Every scoped Luau context also has:

```lua
ctx.profiler.scope("refresh", function()
    ctx.profiler.counter("visible_objects", 42, "count")
end)
ctx.profiler.operation("load", function()
    wait.updates(2)
end)
```

`scope` forbids yielding; `operation` includes suspension and is not CPU execution time.
Both preserve callback values/errors. Labels are program-qualified, 1..=96 UTF-8 bytes
without control characters. Per run, at most 64 CPU marker definitions, 64 counter
definitions, 64 operation names and 32 concurrent operations are admitted. Counter units
are `count` (default), `bytes`, `ms` or `percent`; a name cannot change units within a run.
Luau numeric samples retain Luau's numeric precision, not arbitrary 64-bit integers.
Revocation/failure/despawn closes outstanding logical operations. Instrumentation grants
no Start/Stop, filesystem, World or remote-access capability and is inert without recording.

Traces contain scope names and selected diagnostic identity. Keep labels stable and free of private
payloads. Self-profiling includes observer/UI costs; use stopped recordings for detailed
analysis, not an expensive live view while measuring. GPU, allocation stacks, physical
latency and cross-platform acceptance are separate capabilities, not supplied by the recorder.

## Native tools and diagnostic workload

Linux `perf` samples CPU stacks; `strace` investigates blocking I/O; `pidstat`/`iostat`
separate CPU, memory and storage pressure. Heaptrack samples allocation call paths.
RenderDoc inspects actual Vulkan passes/resources; its captures are not UI acceptance.
Check recording and reading with a small process before profiling the editor.

The explicitly invoked `editor.profile.menus` Luau program runs 1..64 workspace-menu
open/dismiss cycles (default 20) inside the application and returns bounded per-cycle
durations plus before/after diagnostics. Invoke it through `app.scenario` with
`{"scenario":"editor.profile.menus","arguments":{"iterations":20}}` and the ordinary
session/operation envelope. It requires an open workspace and is not part of the default
functional suite. Timings include in-application helper waits, not external RPC polling;
they still do not establish physical-input or visual acceptance.

If sampling requires privilege, elevate only the profiler attached to the owned process,
not the editor. Do not globally relax kernel profiling restrictions or add file capabilities
to make a run work. Disable remote debug-symbol downloads during isolated measurements
when they would introduce network waits. Keep generated evidence under `target/`.

## Workloads and evidence

Shared runtime/UI optimizations must use the same library implementation in the
editor and an independent game. Enable the applicable native features in both
consumers and verify each integration before claiming shared acceptance; do not
extrapolate editor timings to game scenes. The canonical native game enables
`kedra_ui/render` through its ordinary runtime dependency and AuthoredUiPlugin.
Keep headless consumers minimal and editor-specific polling/authorization out of games.

Measure idle, typing, menus, scrolling, selection/Inspector refresh, resize and docking.
Distinguish a small fixture from a large project and visible from hidden panels. Track
tail latency and stalls, not only average FPS. Compare Wayland and Xwayland separately,
and record monitor refresh, scaling, driver and power state.

RPC round trips include transport, polling and script scheduling; they do not measure
the physical keyboard/mouse-to-display path. Native renderer fences prove frame delivery,
not perceptual quality. Physical-input and macOS/Windows acceptance remain separate.

[profiling-crate]: ../../../../../crates/kedra_profiling/README.md
