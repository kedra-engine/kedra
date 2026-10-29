# ADR 0045: Bounded reactive editor with subsystem-owned demand

Status: adopted; indefinite editor sleep is not supported.

## Decision

`kedra_wakeup::ReactivePlugin` is an explicit native host adapter to Bevy's
existing Winit reactive wait. The editor selects a 100 ms idle safety sweep,
1/60 s focused active cadence and 100 ms unfocused active cadence. Window/user
events and task/producer notifications wake immediately; RequestRedraw can
sustain rendering independently. No game inherits these settings from a runtime
library. A host installing this plugin gives it exclusive WinitSettings ownership.

Subsystems supply frame-scoped `WakeDemand`: `at(Instant)` selects their earliest
monotonic deadline, while `active()` retains the host's active cadence for work
without an exposed deadline/completion seam. `continuous()` is a distinct owner
requirement for normal ongoing runtime/render updates: it selects Winit Continuous
in both focus states, taking precedence over deadlines and idle/active polling.
It is not a fixed simulation tickrate or CPU/GPU isolation. Hints are drained in Last; Last
producers run before `WakeSystems::Apply`. Owners renew hints during normal
updates; cancellation, completion and retirement stop renewing them. There is no
timer thread, callback queue, task registry, new executor or parallel scheduler.
`at_real(&Time<Real>, elapsed_deadline)` centralizes conversion from Bevy elapsed
time: its epoch is the first update, not clock construction/startup. Uninitialized
clocks retain active demand rather than guessing an epoch.
The adapter caches deadline identity rather than continuously changing a relative
wait: Winit treats a changed UpdateMode as a request for another immediate redraw.
The final active-cadence window handles relative-wait early wakes without repeating
a long timer. Deadlines are not hard realtime or physical-latency guarantees.

## Integration and fallback boundaries

| Owner | Demand / wake contract |
| --- | --- |
| Tasks and bounded progress | Automatic registration/publication through ADR 0044 |
| Tooltip subsystem | Earliest 350 ms dwell deadline; target/text/click reset the existing owner timer; no per-control wake code |
| Workspace preferences | Existing two-second save-check deadline; existing async writer/flush/conflict authority unchanged |
| Native caret | Active fallback while a live visible blinking field has focus; Bevy's private cursor phase remains authoritative |
| Script runtime | Active fallback while any authorized instance is Running, including real/game/fixed/update waits; terminal/retired owners no longer renew it |
| Authored source readiness | Active fallback while the loaded source catalog is pending without terminal failure |
| Native renderer | Render-world pending pipeline flag and native Screenshot/Capturing recipients retain active demand until completion; cancellation does not fake GPU completion |
| Editor lifecycle | Boot/loading, pending authored views, host staging, gestures, scene dependency/frame preparation, navigation capture and camera alignment retain active demand |
| Visible serving Scene View | Continuous demand, including after frame readiness; hidden/failed/retired views stop renewing it. The current shared App remains continuous as a whole; see ADR 0046 |
| Existing animation/input/capture integrations | Their stock RequestRedraw routes are retained |

The safety sweep is deliberate: Bevy AssetServer does not expose a global pending
load enumeration/completion wake, and render/main publication can cross frame
boundaries. Flair's public active-animation iterator does not include every delayed
keyframe and can include paused/forwards-filled values. The editor has no authored
CSS animation/transition declarations, but arbitrary consumer CSS must not be
claimed safe for indefinite sleep. Unadapted background changes can wait up to the
safety interval plus ordinary scheduling/render cost. A focused caret or running
script intentionally prevents the focused idle reduction; no synthetic blink or
script clock is substituted to improve an idle number.

## Alternatives and verification

Indefinite sleep would silently stall uncovered work. Reimplementing cursor,
animation, asset or script schedulers would duplicate upstream ownership. A broad
engine fork solely to enumerate pending work has substantial maintenance cost and
is not required for bounded idle. Future indefinite sleep needs reviewed upstream
readiness/deadline seams and separate native evidence, not a larger idle timeout.

The adapter's policy tests cover earliest deadlines, stable timer identity,
expired hints and return to idle after unrenewed demand. UI, script and editor
owner suites cover hint lifecycle along with existing failure/cancellation/source
invariants. `native-wakeup` uses a real Winit wait and GPU frames: worker completion
without an initial owner poll, exact worker failure, a no-wakeup negative control,
and a deadline with no pipe release, RPC, Luau or window input. Editor interaction
and independent consumer checks remain separate from OS-input/platform acceptance.
CPU/update-frequency evidence belongs under `target/`, not in this architecture
contract; reduced update frequency alone does not establish input latency.
