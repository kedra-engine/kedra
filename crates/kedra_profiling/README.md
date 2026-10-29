# Application profiling

Runtime-owned finite CPU-scope recording for ordinary Bevy applications. No editor,
authoring, importer, QA or remote-access dependency. Ordinary builds have no capture
backend. The optional `capture` feature enables Bevy instrumentation and Perfetto's
in-process SDK; it does not start recording or open a listener.

## Host integration

Configure `kedra_profiling::log_layer` through Bevy's `LogPlugin::custom_layer` before
building that plugin. Add `ProfilingPlugin` with an application-instance/build identity
and an explicit native output directory. Call `Recorder::start` with finite limits,
observe `CaptureState`, and optionally request Stop or Cancel. Setup, native SDK work
and publication run off the Bevy update thread. One owner admits one active recording.
Do not replace Bevy's global tracing subscriber or install a parallel instrumentation API.

`CaptureOptions` supplies shared explicit CLI admission. `--profile-output DIRECTORY`
starts a finite capture at Startup; `--profile-manual` leaves Start to the host.
`--profile-duration-ms` bounds recording to 100..=30000 ms; `--profile-buffer-kib` bounds
the SDK buffer to 1024..=65536 KiB. Disabled builds expose none of these CLI fields.
`--profile-label` supplies an optional bounded workload label. `Recorder::set_label`
sets it for subsequent captures; active recording context is immutable. Host/build,
instance, session and label also appear in the trace's `kedra.recording` instant.
Native SDK internal start/stop calls are not preemptible; owner Drop cancels on its worker
without blocking the frame thread. A fatal native backend failure can still affect the host.

## Recording and ownership

Each capture reserves a new private directory. `trace.pftrace` is a standard Perfetto
recording and `recording.json` records host/session/process identity, configured bounds,
backend versions and persisted byte count. Existing files are never overwritten.
Cancellation wins until publication commits; afterwards it returns `Inactive` rather
than pretending to undo a saved recording. Dropping an owner revokes uncommitted publication.
Failed/cancelled sessions do not advertise a partial recording. Saved output is not removed
when the application exits. These outputs are not source documents and never use Save.

Buffers can lose events. File publication proves persistence, not lossless collection.
Inspect Perfetto's loss/error statistics and incomplete boundary scopes before drawing
conclusions. CPU scope durations include waits and instrumentation overhead; they are
not scheduler CPU residency, GPU timing or physical input-to-photon latency. Thread totals
may overlap. The recorder does not collect allocation call stacks or GPU timestamps.

The application-facing `Marker`, `Counter`, `Unit`, `CounterValue` and `Operation` types
hide backend types. CPU guards are thread-bound; operations explicitly include suspension.
The script runtime supplies the same semantics as `ctx.profiler` and owns VM revocation.
`Category` classifies scopes/counters/operations without exposing backend types;
application instrumentation defaults to Application and Luau uses Scripts. Bevy
system categories are best-effort classifications by identity; full names remain.

Capturing hosts emit numbered `kedra.update` instants. The optional `render` feature
adds `kedra.render_cpu` instants with the exact identity copied through Bevy resource
extraction, if RenderApp exists when ProfilingPlugin is installed. Install profiling
after the host's render plugins. No RenderApp means no render correlation, not zero
render work. These are CPU schedule entry points, not GPU/presentation boundaries.
The editor enables this integration; headless consumers need only `capture`.

Traces include scope names and selected diagnostic identities; these can still contain
private names. Arbitrary span fields/source locations and ordinary log events are excluded.
No network discovery, listener, upload,
symbol download or private service is installed. Profiling is independent of debug-only
application automation and remains explicitly optional in optimized/shipping builds.

## Independent consumer

```sh
cargo run --locked -p kedra_profiling --features capture --example record_application -- \
  --profile-output target/performance/independent --profile-duration-ms 1000
```

This is an ordinary headless Bevy application and real recording, not a profiler benchmark
or native UI acceptance. Open the saved local `trace.pftrace` using Perfetto UI's Open trace
file action. Detailed workflow and limits live in the
[performance manual](../../docs/site/en/manual/testing/performance.md).
The [local review kit](../../tools/profiling/README.md) supplies bounded health and
elapsed-system queries and an official Perfetto UI macro for semantic tracks,
recording context and explicitly correlated update/render entries. It is not a
new analysis engine or viewer; thread names can still be truncated by the OS.

## Dependencies and verification

Perfetto SDK 1.2.1 and its sys package 1.4.0 are Apache-2.0 ecosystem packages; the vendored
native library is compiled as a dependency, not imported into workspace source. The Rust
SDK is community-maintained. `tracing-subscriber` supplies its standard Bevy-compatible
layer/filter integration; `tempfile` owns unique private output files/directories. Kedra
implements admission, cancellation/publication, host metadata and an identity-only Bevy
scope adapter, not a trace encoder. The generic upstream adapter cannot select identity
fields or fence scopes across sessions; its repeated source locations are unsuitable for
bounded per-frame capture. SDK incremental state is reset periodically for rolling-buffer
decodability. Adapter depth/mismatch drops are reported separately in the manifest.
See [ADR 0042](../../docs/decisions/0042-runtime-profiling-and-editor-analysis.md).

Windows MSVC consumers must define `NOMINMAX` and `WIN32_LEAN_AND_MEAN` when compiling
the SDK's C++ backend. These prevent Windows min/max macro substitution and the
legacy Winsock declarations pulled in by `windows.h` before `winsock2.h`.
The workspace supplies both through `CXXFLAGS_x86_64_pc_windows_msvc` in Cargo
config; an explicit consumer override must retain them. Warnings remain enabled.
The header-order requirement is documented by
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/winsock/creating-a-basic-winsock-application).

Contract tests exercise actual repeated native recordings, cached spans, invalid admission,
cancellation, file failure and unchanged foreign recordings. Run disabled and enabled
configurations under an external process deadline. Platform build/native support and
overhead require separate measurements; a passing Linux recording does not certify other OSes.
