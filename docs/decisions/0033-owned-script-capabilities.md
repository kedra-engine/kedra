# ADR 0033: Owner-bound script capabilities and native adapters

Status: accepted for the current opt-in API; native input/frame acceptance is separate.

## Decision

Extend `kedra_script`'s existing Behavior/Sequence ownership rather than introduce a
global event bus, task scheduler, property registry or reflected World API.

- Behavior children are finite host-approved `Program`s with independent VMs, bindings,
  parameters and budgets. Standard Luau Require resolves entry identities without
  evaluating source, with a distinct cache namespace from module exports. Admission
  occurs in ordinary Bevy Last outside VM dispatch; only target/lifetime is inherited.
- Cancellation/failure/replacement/despawn revokes active and pending children before
  further effects. Handles retain owned terminal outcomes, not running VM state.
  Bounds apply both to per-owner handles and global execution capacity.
- Native subsystems publish bounded owned notifications through `events::notify`.
  Behaviors subscribe only to host-approved names on their own target; Lua cannot
  publish. Non-yielding callbacks are delivered in bounded Last batches before child
  admission. Reentrant notifications wait until a later update. Overflow fails the
  owner; disconnection removes pending delivery and revocation clears subscriptions.
- Immutable `ctx.params` uses the ordinary JSON boundary and a trusted host validator
  before allocation. Hosts reuse normal domain/serde types rather than a new schema.
- `ScriptError` and bounded `Diagnostics` preserve stable failure classes, source,
  phase and available traceback. `ctx.log` supports owned structured fields; hosts
  can consume diagnostics without an editor dependency.

`kedra_script_bevy` is a separate runtime crate: native component/widget integration
has a cohesive responsibility and a real canonical-game consumer, but is not VM
execution policy. It owns attached-target local Transform translation, existing UI
background mutation and real Bevy Activate forwarding. Hosts explicitly install
bindings; scripts never acquire generic World/component access or source Save.

The canonical switch embeds a game Behavior, receives validated color parameters,
subscribes to Activate and starts a UI-only child Sequence. The native verifier drives
the existing widget through real input; there is no parallel Rust switch behavior.
Published project loading/Play remains unimplemented; explicit embedded source
authorization does not create another asset registry or execute arbitrary scene code.

## Rationale and limits

BMS supplies contexts/dispatch and mlua supplies Luau resolution/coroutines. Existing
Bevy hierarchy, components, observers and schedules own object lifetime and native
events. The small bounded mailbox/admission adapter is necessary to avoid entering
BMS registries recursively or letting queued work outlive revoked script authority;
it is not an independent executor or general-purpose event framework.

Only behaviors can spawn children; children cannot recursively spawn. There is no
inter-owner dispatch order, unbounded queue, automatic retry, rollback or authoring
transaction. Local Transform writes do not promise physics/world-space behavior.
VM budgets remain cooperative and native work requires host-owned limits/watchdogs.

## Verification ownership

`kedra_script/tests/contracts/` owns parameter validation/immutability, child outcomes,
permission boundaries, revocation, notification order/disconnection/overflow and
diagnostic attribution/retention. `kedra_script_bevy/tests/contracts/` owns native
component mutation and Activate forwarding. These are headless VM/Bevy contracts,
not proof of native input/frame quality. The existing canonical `native-switch` and
`native-projection` verifiers own assembled native acceptance on complete fresh copies.
Luau declaration checks use real optional luau-lsp invocations, not private CI services.
