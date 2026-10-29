# Local Perfetto review

These are bounded product queries for the official Perfetto Trace Processor, not a
trace parser or viewer. The command examples target Trace Processor v59.1. Treat
imported traces as untrusted; do not enable SQL file access or symbol downloads.
From the repository root, with an explicitly selected local file:

```sh
timeout 60s trace_processor_shell query --no-agent --quiet \
  -f tools/profiling/health.sql /path/to/trace.pftrace
timeout 60s trace_processor_shell query --no-agent --quiet \
  -f tools/profiling/overview.sql /path/to/trace.pftrace
```

Check `recording.json` alongside the trace: the byte count must match and
`adapter_dropped_events` must be zero. Nonzero processor loss/error/overwrite
signals mean incomplete collection. Zero decoded slices is unusable; absent
context means unknown provenance, not a default build or scenario. End-boundary
unfinished scopes are excluded from timing, not silently completed. Missing
OS/GPU/allocation data is unavailable, not zero. Processor success alone is not
visual acceptance or evidence of low recording overhead.

`overview.sql` ranks completed thread scopes by own elapsed duration after
subtracting direct children. This includes waits and overhead, not CPU residency;
totals across threads overlap. Logical operation tracks are deliberately excluded.
Scheduling remains in the full trace but is hidden from this summary. Categories
are diagnostic classifications, not stable system ownership or cost attribution.

For a ready-made UI overview, import `overview-macro.json` into **Settings > Macros**
in the official Perfetto UI, then run **Kedra overview** from the command palette
after opening a local trace. The macro uses Perfetto's documented stable automation
commands and changes presentation only. It does not upload traces. Repeating it
adds duplicate tabs/tracks; use it once per loaded trace. Inspect the original
thread tracks for nesting and full function names. The update track shows update
intervals, not rendering or display latency; the final interval is omitted.

Sources: [stable commands](https://perfetto.dev/docs/visualization/commands-automation-reference)
and [UI macros](https://perfetto.dev/docs/visualization/ui-automation).
