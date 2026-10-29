# ADR 0042: Runtime profiling and editor-owned analysis

Status: adopted. Platform and native interaction acceptance remain separate.
Recordings use the ready-made Perfetto UI; Kedra has no native trace viewer.

## Context

Kedra needs one profiling workflow for its editor and independent applications, including
future explicitly launched project execution. Bevy remains the ECS, scheduler, application
and render foundation. Existing debug automation statistics are not a full profiler and
must not become a separate collection backend or a prerequisite for profiling.

## Boundaries

- Runtime-layer profiling owns opt-in recording sessions and Bevy instrumentation integration.
  It cannot depend on editor, authoring, importing, QA, a filesystem authoring writer or RPC.
  Application/editor code uses Kedra's own `Marker`, `Counter` and `Operation` API.
  Bevy's standard tracing and diagnostics remain instrumentation sources; tracing
  and Perfetto types do not escape the application-facing facade. Do not introduce
  a generic backend registry merely to permit a future adapter replacement.
- Offline trace analysis is an independent responsibility: maintained trace-processing tools
  decode recordings and answer queries. Kedra owns bounded product queries, provenance and
  result admission, not a trace parser, SQL engine, symbolizer or profiler implementation.
- The editor owns recording authorization and session orchestration. It consumes the same
  recording mechanism as a standalone application. Standard captures open
  in ready-made Perfetto UI in a separate browser; no embedded WebView, new UI stack or
  native trace-viewer implementation is required. A native panel is future work driven by
  demonstrated workflow gaps, not a prerequisite for useful recording and analysis.
- Application instance, recording session and process identities are distinct. Worlds and
  worker/render threads must not be inferred to be separate processes. Future project
  execution isolation remains an independent decision; this ADR does not implement Play.
- No speculative registry, forwarding crate or generic backend framework is required.
  Each extracted crate needs a working consumer and its own real responsibility.

## Backend selection

The recording backend uses Perfetto's in-process SDK. The SDK owns bounded
trace buffers, encoding, timestamps and start/stop semantics; the Trace Processor owns
decoding and analysis. Record standard Perfetto traces, not a Kedra event format.

The resolved Rust packages are `perfetto-sdk` 1.2.1 and `perfetto-sdk-sys` 1.4.0,
published from the Perfetto ecosystem under Apache-2.0. The SDK's vendored C++ backend adds
native compilation cost. Upstream FFI is not permission for unsafe workspace code. Validate
the exact resolved graph, licenses and target builds before claiming platform support.
Rust bindings are community-maintained; their presence in Perfetto documentation is not
proof of the same support/stability guarantees as the C++ SDK.

The maintained generic `tracing-perfetto-sdk` 1.0.0 adapter emits source locations for
every scope entry and retains/emits arbitrary fields. It does not offer the required
identity-only field selection or capture-generation fencing. Kedra therefore implements
one narrow `tracing-subscriber::Layer`: map Bevy scope identity to SDK scopes, select only
the diagnostic `name` field, bound thread nesting and count suppressed/mismatched events.
This is an instrumentation adapter, not a recorder/parser: the SDK still owns timestamps,
interning, buffering, encoding and session controls. No upstream source is copied or forked.
The SDK configuration periodically resets incremental state so buffer wrap does not make
all remaining packets undecodable. Buffer loss still requires explicit inspection.

Recording requires bounded buffers, explicit Start/Stop and offline review without
a second product UI stack. A custom tracing recorder/parser would duplicate the
SDK and Trace Processor and requires a concrete unsupported requirement and a
separate dependency review.

## Recording contract

Recording is explicitly authorized and bounded in duration and buffer size. Compiling a
profiling capability does not start a capture, open a listener or discover the local network.
No private service, privileged application process or automatic download is required.
SDK session setup, stopping and file publication must not block native UI updates.
Only one application-owned recording may be active; the backend also admits only
one recording owner per process. Conflicting starts fail without
retiring or replacing the serving session. Owner retirement stops the owned session.

Keep an immutable recording identity, host/build metadata, configured bounds and completion
status alongside the standard trace. Publication must not overwrite another recording.
Backend failure, disconnect, cancellation, buffer loss and incomplete data remain visible;
neither retries nor clipping may turn a partial capture into an unqualified successful one.
Traces contain function/scope names and selected diagnostic identity, not arbitrary log fields.
Do not record arbitrary log messages or upload traces by default. Treat imported traces as
untrusted data; bound files, queries, result rows and external process lifetime.

An optimized profiling build is independent of debug automation. Shipping builds without
the profiling feature install neither recording backend nor diagnostic transport. Deep
allocation/sampling capabilities require separate explicit enablement and measured overhead.

## Custom instrumentation

`Marker` defines a synchronous thread-bound CPU scope. RAII ends it on return/error;
guards must not cross async suspension. `Counter` records finite absolute integer/float
samples with an immutable unit, not implicit rates, allocation attribution or increments.
Process-qualified names distinguish applications sharing a process. The backend supplies
standard native counter units and tracks without a competing history/lookup registry.

`Operation` is a distinct logical elapsed duration on a dedicated track. It may span
yielding/worker execution; explicit Finish records completion and owner Drop records
cancellation. Session-generation fencing prevents a stale operation's End from corrupting
a subsequent recording. These durations are never labeled CPU execution time.

The standard Luau binding belongs beside runtime diagnostics in `kedra_script`, not in
the editor: it is a thin VM-owned adapter with no recording authority or backend logic.
`ctx.profiler.scope` is synchronous; `ctx.profiler.operation` preserves yields/returns/errors;
`ctx.profiler.counter` emits absolute values with an explicit unit. Bound definitions and
in-flight operations per VM, qualify labels by program and revoke live operations on
completion/failure/cancellation/despawn. Do not keep a native thread scope open across yield.

## Analysis and presentation

The recorder captures editor CPU scopes and saves recordings for review in the
ready-made Perfetto UI, with its thread timelines and expensive-system queries. Opening
the UI does not authorize uploading a trace: the user opens a local recording. Exercise
the same mechanism in an independent application before claiming it is editor-independent.
Freeze a recording before expensive analysis; keep analysis off the frame thread
and reject stale worker results.

CPU span duration is elapsed time, not scheduler CPU residency. Parallel thread totals are
not frame critical-path time. Frame/update, render and presentation boundaries are distinct.
Show exclusive/inclusive scope time with its definition, invocation counts and incomplete
boundary scopes. Surface trace-processor loss/error statistics; unavailable is not zero.

Self-profiling includes observer costs. Avoid continuously rebuilding a live
timeline while measuring; label capture mode, source and instrumentation overhead. Preserve
standard external-tool access for investigation and independent verification.

The runtime facade owns semantic categories. Bevy identities are classified without
renaming their full scopes; scheduler details remain available in the trace but are
excluded from the default offline ranking. Optional host workload labels are immutable
during capture, bounded, and recorded with host/build/session identity in trace metadata
and the manifest. Never infer a scenario from a filename or user timing.

Update/render correlation uses an explicit update identity copied by Bevy resource
extraction into RenderApp, not nearest timestamps. The optional render integration
requires the host's render plugins to precede profiling installation. CPU render entry
does not prove submission, GPU completion or presentation. The offline kit in
`tools/profiling` consists of bounded SQL and a stable-command Perfetto UI macro; the
official processor and UI own execution/visualization. No bespoke analysis process,
storage format or embedded UI is introduced. Full native thread labels and overhead
isolation remain verification/workflow limitations.

GPU pass timing, process memory, known engine resource sizes and allocation call stacks are
different capabilities. Do not label frame intervals as GPU time or known resource sums as
total resident memory. In-process editor/project execution cannot automatically attribute
all process allocations to a particular world. GPU/backend and OS support must be reported
as capabilities, not silently substituted with other measurements.

## Verification ownership

Runtime profiling owns session admission, finite lifecycle, publication, cancellation,
disabled-build behavior and foreign-state preservation. Analysis owns real processor
invocation, deadlines, bounded admission and semantic query results. The editor owns panel
interaction, recording selection, retained docking and actual frames/input. Do not replicate
the same error matrix at all three levels or add a new QA framework.

Required acceptance includes repeated recordings with cached Bevy spans, worker-thread
scopes, mid-scope start/stop, buffer exhaustion, failure without overwrites, reopening,
editor self-profiling and an independent consumer. Compare enabled/disabled overhead on
equivalent optimized builds. Linux, Windows and macOS acceptance remain separate.
