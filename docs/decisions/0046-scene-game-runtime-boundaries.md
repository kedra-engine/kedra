# ADR 0046: Scene/Game workflow and project-runtime boundaries

Status: proposed. Play, isolated project Apps, runtime Inspector and project preview
execution are not implemented. The current shared-World projection is specified by
ADRs 0017/0018; its continuous-frame policy is specified by ADR 0045. Backend
selection requires the experiments in the [implementation plan](../plans/scene-game-runtime.md).

## Decision

This proposal defines integrated Scene/Game authoring, runtime inspection and
scoped source editing. Bevy remains the ECS, reflection, schedules, assets and
renderer. Runtime libraries remain independent of the editor and authoring writer.

### Contexts, not applications per panel

An authoring session owns accepted document state, drafts, history and Save.
A project execution context owns a live world, its clocks, plugins and lifecycle.
A viewport owns a camera, presentation target and routed input, not a separate
simulation. Multiple cameras/windows observing a context do not duplicate its
entities, physics, animation or scripts. Additively loaded scenes may share one
context; opening a panel is not a reason to clone a large world.

| Mode | Scene View | Game View | Hierarchy / Inspector |
| --- | --- | --- | --- |
| Edit | Authoring-derived world, tools and permitted preview execution | Optional view through authored game cameras, without starting gameplay | Accepted authoring state |
| Play | Free editor camera into the running play context | Game cameras into the same running context | Runtime state, including runtime-only entities |
| Paused | Navigation, selection, tools and rendering still work | Frozen simulation state, not a frozen application | Inspectable runtime state |
| Prefab editing | Explicit isolated or in-context source editing | Does not implicitly switch or restart Play | The selected prefab source, not arbitrary surrounding objects |

The shell must identify the current context and edit destination through concise
control/selection states. It must not introduce permanent explanatory product UI.
Input is routed to the focused viewport/context: Scene navigation must not drive
the player's camera, and game input must not consume Inspector typing or survive
focus/capture revocation as stuck keys.

### Play and authoring authority

Play starts from a revision-pinned snapshot of accepted working sources,
including unsaved accepted edits, but excluding unresolved field drafts. Starting
Play does not implicitly Save or grant source-write authority. Derived runtime
deliveries retain the existing catalog/path/provenance checks; native references
use source-qualified logical paths, not a new ID lookup registry.

The authoring session remains authoritative while Play runs. Stop discards runtime
state and returns to that authoring session without losing its unsaved changes.
Runtime Inspector edits are transient by default. A future explicit Apply operation
must use supported source mappings, baseline/conflict checks, authoring transactions
and Undo; it must not serialize an arbitrary runtime World into source files.
Runtime-spawned entities may have no authoring counterpart. Prefab/source editing
while Play runs is a separate source-authorized context, not a transient game edit.

### Execution, clocks and extension code

Editor real time, edit-preview time and Play simulation/fixed time have distinct
owners. Rendering frequency is not the simulation tickrate. Pause must gate game
schedules, not stop the entire App or only pause a clock that arbitrary systems
can ignore. Scene navigation, inspection and render servicing continue on Pause.
Step must explicitly advance a defined simulation frame/step without consuming
wall time accumulated while paused. Two cameras into the same context cannot
independently rewind that context's simulation.

Production presentation/runtime implementations are shared with standalone games
through ordinary crates and Bevy plugins. Edit-preview systems, authoring tools
and gameplay are explicit execution profiles, not inferred from component names or
the absence of input. Authoring effects use scoped transactions; temporary visual
effects do not silently make sources dirty. Rust/Luau extension instances do not
share mutable globals between unrelated contexts by contract.

Opening sources remains data admission, not project-code execution authorization.
Full-fidelity custom project rendering can require project code; enabling that
requires explicit host authorization. Safe restricted preview remains distinct
from authorized project execution. Process separation alone is not a hostile-code
filesystem/security sandbox. Fast restart with retained execution state is deferred
until clean stop/restart semantics are demonstrated, not required of every plugin.

### Scheduling and current containment

A running runtime or realtime viewport must not depend on individual effects
remembering to notify the editor, or be classified idle after its first good frame.
The owning execution context declares continuous demand for its lifetime/run mode.
View visibility and simulation lifetime are different: hiding Game View must not
implicitly stop Play. Rendering of hidden views may be suspended separately.

The current restricted shared-World Scene View has no independent execution clock
or edit-preview mode yet. Its visible, serving document/raw preview conservatively
requests `WakeDemand::continuous()`, including after frame readiness. This maps to
ordinary Winit Continuous in both focus states and takes precedence over reactive
waits. Hidden, failed or retired views stop renewing the request. The empty shell
can retain bounded idle. This intentionally keeps the entire current shared App
active; it is containment, not project-runtime isolation or a fixed-tick guarantee.
ADR 0045 is not the pacing policy of a future gameplay runtime.

### Backend gate

Do not extend the shell's World into a host for arbitrary project worlds through
render layers, parent ownership or filtered queries. The editor must not synchronously
wait for a large world's update, loading, GPU readback or native plugin execution.
Separate Bevy Apps/Worlds are an ownership boundary, not by themselves CPU scheduling,
crash or GPU isolation. Apps updated sequentially on the UI thread still block it.

Evaluate independently executing ordinary Bevy Apps with shared GPU infrastructure
against a separate runtime process with a viable GPU image transport. The latter
has stronger CPU/address-space failure isolation, but neither avoids GPU/RAM/driver
contention. An external game window is a valid standalone/debug route, not the final
replacement for integrated Scene/Game View. Production frame transport must not be
based on per-frame CPU Screenshot readbacks; native screenshots remain verification.

Rendering experiments must establish image/device/asset ownership, per-context time
and shader globals, multi-camera composition, resize/DPR and synchronization on the
claimed platforms. Cross-context `Entity`/`Handle<Image>` values are not portable
identities. Bounded transport retains session/generation/revision/target identity,
rejects stale results and favors the latest completed image over an unbounded frame
queue. No new generic task, ECS, reflection or scheduling framework is authorized.

## Verification gate

The plan requires large-world loading, multi-view clocks, Play/Pause/Stop, scoped
input, unsaved-source retention, delayed/stale frames, cancellation and slow/hung
systems. Measure real UI input/frame behavior plus CPU/GPU/memory; compilation,
synthetic Worlds and RPC completion do not prove responsiveness or platform safety.
Do not remove native failures, add input replay or increase readiness timeouts to
claim this boundary works. A backend decision must state verified guarantees and
remaining driver/resource limits rather than promise absolute UI independence.
