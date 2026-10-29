# Scene/Game runtime integration plan

Status: active architecture plan, not an implemented Play/backend API.
The proposed product/ownership direction is
[ADR 0046](../decisions/0046-scene-game-runtime-boundaries.md).

## Current boundary

The editor's restricted Scene View instantiates audited presentation into the
shared editor World. It is not a full project-runtime host. Existing source node,
asset-size, format, resource and renderer-admission limits remain enforced; this
plan does not claim large-world, animation, arbitrary plugin or Play support.
Visible serving previews request continuous updates; the shell can sleep only
when those owners no longer require them. Shared-library task/deadline wakeups
remain useful, but are not a simulation clock or evidence of viewport inactivity.

## 1. Containment and failure attribution

- Exercise continuous preview demand after readiness, not only while assets load.
- Cover hidden, failed and retired owners releasing demand without sticky loops.
- Retain actual native navigation failures. Compare before/after only at matching
  viewport geometry/DPR and native lifecycle points; do not infer causality from
  different window sizes or a successful restricted probe.
- Keep existing source/Save/cancellation checks. Do not import old prototype assets,
  silently increase admission limits or replace failures with sleeps/input retries.

## 2. Execution/render feasibility experiment

Use ordinary Bevy Apps/plugins and existing build/native supervision. Keep generated
stress data, logs, captures and measurements under `target/`, with provenance.
Do not create speculative production crates or a permanent backend abstraction
before identifying a real mechanism and consumer.

Compare two concrete candidates:

1. Independently serviced project App/World with shared in-process GPU resources.
2. Project runtime process with GPU-capable image transport; an independent window
   can first establish gameplay behavior, but cannot certify embedded transport.

Resolve Bevy/wgpu version-specific APIs from the locked sources. Evaluate maintained
tools/integrations before custom plumbing, including platform support, ownership,
licensing, unsafe/dependency cost and maintenance. Record why any custom bridge is
necessary. Do not assume multiple DefaultPlugins instances or a cross-App Image
handle provide shared rendering. Do not substitute a CPU-readback stream and call
it the production transport.

### Required experiment matrix

| Concern | Observable contract |
| --- | --- |
| Large load | UI input, cancellation and diagnostics remain serviced while a world is loading; no full-world mirror is required merely for Hierarchy |
| Multiple views | Two Scene cameras and a Game camera observe one simulation, not repeated animation/physics/scripts |
| Clocks | Paused Play, running edit-preview and shell animation retain separate time/shader state |
| Slow/hung work | UI behavior under a long update, blocked native system, failure and worker termination is measured; CPU isolation does not imply GPU/driver isolation |
| GPU path | Actual images, bounded in-flight resources, latest-frame behavior, resize/DPR, stale generation rejection, device loss and cleanup |
| Input | Scene camera, player input and text editing remain context/focus-scoped; focus/capture loss clears holds |
| Resources | CPU, GPU, RAM/VRAM, duplicated assets and multiple viewport targets are measured; backpressure does not block the UI thread |

The backend decision follows this evidence, with explicit guarantees and limits.
If an available safe maintained GPU path is insufficient, report the blocker
before widening unsafe policy, adopting a broad fork or replacing Bevy rendering.

## 3. Authoring/runtime contexts

Reuse source snapshots, composition, instance mappings, scoped selection and guarded
transactions. Add only the concrete session/generation ownership needed by the
chosen backend. Preserve full source-qualified asset paths and existing catalogs;
do not add a second asset identity/lookup registry.

Define bounded revisioned messages for source publication, viewport input, selection,
inspection and diagnostics. Loading/reflecting custom project components must not
implicitly execute project code in the shell. Explicitly authorized project
integration and safe built-in preview have distinct capability boundaries.

## 4. Integrated Play workflow

- Start from accepted working state, including unsaved edits; do not Save or accept
  unresolved drafts as a launch side effect.
- Switch Scene and Game to the same play context. Runtime-spawned entities are
  inspectable without inventing authoring identities.
- Gate gameplay schedules on Pause while navigation, rendering and inspection
  continue. Define Step independent of elapsed paused wall time.
- Stop retires the runtime and restores authoring context/selection without losing
  accepted unsaved state or reviving old callbacks.
- Prefab editing remains an explicitly scoped authoring context, including during
  Play. Any future Apply from runtime is a guarded source transaction, not Save of
  arbitrary runtime state.
- Reuse standalone production plugins. Keep editor tools, orchestration and source
  writing out of runtime/game dependencies. Optimize startup only after clean
  restart/isolation contracts work; no implicit global-state retention contract.

## Acceptance

Use a real independent game consumer and the actual editor, with real native
frames and input and bounded external supervision. Compare matched workloads
without compiler/QA jobs inside timing windows. Keep product acceptance distinct
from architecture/unit tests, injected input, RPC, GPU timings and physical latency.
Do not claim other platforms, large worlds or arbitrary plugin isolation based on
small-fixture or one-host success. Update current contracts only when their
replacement implementation and provenance actually exist.
