# GUI responsiveness plan

Status: active plan. Bevy remains the application, ECS, input, layout and rendering foundation.
This document describes intended work, not completed native performance acceptance.

## Boundaries

Shared retained UI behavior belongs to runtime UI crates and ordinary Bevy plugins.
Documents, Inspector source authority, panel orchestration and editor-specific pacing policy
remain in the editor. Independent applications must not inherit editor dependencies or an
unrequested desktop update policy. Do not add a parallel scheduler, ECS, style engine or
generic invalidation framework to compensate for a missing profile.

Shared optimizations must improve the owning runtime library, with the editor
and an independent game consuming the same implementation and applicable feature
set. Verify both consumers before claiming shared acceptance; do not keep a
faster editor-only copy. Headless consumers stay renderer-free. Editor polling,
tool lifecycle and authorization are not game defaults.

## Measurement first

Use [bounded diagnostics and native profiling](../site/en/manual/testing/performance.md).
Keep compiler work and unrelated test execution out of measurement windows. Record idle,
typing, menus, scrolling, selection/Inspector refresh, resize and docking, with equivalent
small/large content. Keep CPU, GPU, allocations, update intervals and physical latency distinct.
Instrumented and uninstrumented controls must use explicit features/profiles.

Performance evidence belongs under `target/`, not in source-controlled run narratives.
Do not use screenshot stabilization, long readiness timeouts or successful RPC delivery as
evidence of responsiveness. Existing source-safety and lifecycle guarantees remain required.

## Implementation priorities

1. Keep the development execution profile usable, without removing debug assertions or the
   shipping capability boundary. Preserve a truly uninstrumented control build.
2. Remove measured broad work: unrelated-world hierarchy walks, repeated allocations and
   unchanged component writes. Reuse Bevy change detection and existing retained ownership.
   Hidden or paused animation must not invalidate transforms or keep requesting frames.
3. Make cost follow changed content. Verify source revision, selection, width, font and
   visibility invalidation before caching. Preserve foreign resources, Unicode shaping,
   pending drafts and focus across refresh and host transfer.
4. Evaluate native update demand separately from rendering cost. Prefer Bevy Winit reactive
   modes and existing redraw/wakeup mechanisms, not a replacement event loop. A lower idle
   frame rate alone does not establish lower interaction latency.
5. Verify representative assembled interactions and an independent shared-UI consumer.
   Keep domain matrices in their owning crates; do not multiply native suites to measure
   the same guarantee. Performance thresholds require an explicit reference environment.

## CSS optimization boundary

Trace selector matching back to its invalidation producers before changing the
cascade. First remove measured unchanged writes and unnecessarily broad work in
the owning shared UI library, using the existing Flair and Bevy APIs. Preserve
ancestor/sibling selectors, `:has`, hover/focus, inherited variables, nested
stylesheet boundaries, reparenting and stylesheet/asset readiness.

A narrowly scoped Bevy Flair fork is an allowed last resort when representative
profiles and a reproducer establish an engine limitation that the supported API
cannot resolve correctly. Before adopting it, record the upstream source and exact
revision, patch rationale, licensing/security and maintenance costs, and upstream
contribution/replacement strategy. Keep the existing dependency and source-review
gates; do not publish a fork without separate authorization. Both editor and
independent game must consume the same patched engine and pass relevant behavior
checks and separate native measurements. Do not add a parallel cascade or parser.

## Reactive-update prerequisites

The task/producer notification bridge is owned by `kedra_wakeup`; see
[ADR 0044](../decisions/0044-native-readiness-wakeups.md). The editor's bounded
reactive policy and shared timed/active demand integrations are defined by
[ADR 0045](../decisions/0045-bounded-reactive-editor.md). Indefinite sleep remains
outside the current implementation because some upstream wake seams are missing.

Readiness must be integrated at reusable subsystem boundaries, not remembered at
each operation or control. Task handles register automatically at creation and
bounded std producers notify after successful sends. Timed UI work should expose
its deadline through its owning subsystem and a common native event-loop adapter,
with automatic retirement/cancellation, not per-control `notify()` calls, timer
threads or a parallel scheduler. Third-party systems without a usable readiness
or deadline seam need a narrowly reviewed integration or explicit active-work
fallback; do not infer safe indefinite sleep from the absence of notifications.

A fully idle-sleeping editor must still wake for background delivery/project/source work,
Save completion, preview/shader readiness, native window lifecycle, authenticated RPC,
frame-ordered input, GPU readback, finite Luau execution, tooltips, caret blink, CSS transitions
and visible animation. Failure, cancellation and retirement must not leave a redraw loop alive.
Do not select a long idle timeout until these routes are demonstrated without physical input.
Runtime games may require continuous updates and must retain that option.
Visible serving previews conservatively retain continuous updates while the
editor and preview share one App. Readiness/unchanged entities do not authorize
sleep. Project-runtime clock/isolation work follows
[ADR 0046](../decisions/0046-scene-game-runtime-boundaries.md) and the
[Scene/Game plan](scene-game-runtime.md), not an extension of editor idle policy.

## Acceptance

Compare action latency distributions, long stalls and idle resource use before/after under
the same conditions. Verify actual native input/frames, not only synthetic layout or RPC.
Use the normal functional suite after changes, independently of timing measurements.
Wayland/Xwayland and macOS/Windows require separate evidence; one host's CPU profile does not
certify other backends or physical-input quality.
