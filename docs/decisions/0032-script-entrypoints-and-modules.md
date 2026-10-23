# ADR 0032: Explicit script entrypoints and standard Luau modules

Status: accepted for the current runtime API. Project delivery/Play and native
acceptance are separate from the headless runtime contracts.

## Decision

`kedra_script` exposes three complementary contracts over one BMS/Luau backend:

- Persistent **Behavior**: optional named `start`, `update`, `fixed_update`, `stop`
  functions with per-instance `self` and an explicit capability `ctx`.
- Finite **Sequence**: one `run(ctx, args)` coroutine with owned arguments/results.
- Reusable **Module**: standard Luau `require` with relative paths or `.luaurc`
  aliases, returning ordinary values/functions rather than implicitly registering
  another instance or acquiring execution authority.

The Rust host explicitly selects the entrypoint kind, owns loading permission and
supplies narrow tables on ctx. Lifecycle functions are top-level declarations;
the selected kind determines which functions the runtime invokes. `wait`, `runtime`
and `json` provide shared runtime vocabulary, not editor/game authority.

## Scheduling and ownership

Ordinary Bevy Update/FixedUpdate systems dispatch behavior callbacks through
public ordering sets. Fixed callbacks run at each actual fixed step. Start occurs
once before the first callback, not in the application's global Startup schedule.
Callback order between instances is unspecified. Lifecycle callbacks cannot yield;
each executes in a temporary coroutine so VM interrupts cross script pcall without
becoming catchable errors. There is no alternate ECS, reflection system or scheduler.

Sequences use a Last-phase coroutine pump and explicit game/real/update/fixed
wait distinctions. Waiting for a fixed step does not execute inside FixedUpdate.
Finite sequence limits and persistent behavior lifetimes are separate. Capacity
is host-owned; replacement validates a candidate while crediting the replaced
active slot, then retires the previous generation only after successful loading.

Attachment establishes linked entity lifetime before loading. ctx.entity is an
opaque, world-checked runtime handle, not persistent source identity. Scope access
validates current attachment, live owner and active dispatch. Stop is
best-effort notification after effects have been revoked; failure cannot prevent
resource cleanup. Removal hooks queue cleanup rather than re-entering BMS locks.
Callbacks/VM handles are released independently of retained terminal evidence.

## Module resolution and editor tooling

The mlua 0.11.6 public `Require` adapter delegates syntax, aliases,
configuration parsing and caching to upstream Luau. Kedra supplies an explicitly
admitted, bounded in-memory source tree, including the same project-relative entry
paths and `.luaurc` files used by luau-lsp. The upstream filesystem TextRequirer
would grant ambient disk/config access and is therefore unsuitable. The small
virtual-tree adapter is necessary for this authority boundary; it is not another
import grammar, package manager, scanner or persistent asset registry.

Source snapshots are input to an authorized program,
not an automatic loader for project assets. Future published script loading must
resolve through existing source authority and verify revisions/membership. Embedded
native consumers provide known source/config bytes explicitly.

Aliases and relative imports share one VM-local module cache. Replacement starts a
fresh cache; different capability scopes never share mutable modules. A module gains
no extra permissions through import. Entry evaluation cannot perform Scope effects
or yield. Module initialization may suspend a live sequence using standard require
semantics; it remains owned by that sequence's VM, waits, budget and revocation.
It cannot bypass the no-yield contract of entry loading or behavior callbacks.
Native source/memory and compilation costs remain outside complete VM
preemption. The BMS initializer evaluates the named entry exactly once because BMS
0.23's default loader evaluates unnamed chunks; the BMS chunk itself is empty.

Standard imports preserve LSP inference/navigation without custom URI support.
Context and state parameters still require explicit annotations; definition files
describe each host's actual context, not permissions inferred from a file suffix.
VS Code is a supported optional editing client, never a required build dependency.

## Alternatives and limits

- Copying Godot/Unity/Roblox object hierarchies would introduce a second gameplay
  model rather than extend Bevy. Borrow lifecycle concepts, not compatibility promises.
- Returning a large callback table obscures the simple script surface; named
  functions are sufficient when kind is explicitly host-selected.
- Custom `game://` import syntax requires special editor resolution. Standard
  `./`, `../` and `@alias` imports already express the requirement.
- Filesystem require violates scoped admission; a custom parser duplicates upstream.
- Making every behavior a finite coroutine would impose arbitrary total deadlines
  on persistent gameplay and would not provide actual fixed-phase callbacks.

Owner-bound child spawning, subsystem subscriptions, parameters, diagnostics and
narrow native object adapters follow [ADR 0033](0033-owned-script-capabilities.md).
Gameplay input/animation, scene-script serialization, generated SDKs, debugging,
state migration, process isolation and platform certification remain outside this API.

## Verification ownership

Runtime lifecycle, scheduling, cancellation, source admission, module-cache isolation
and replacement contracts belong to `kedra_script/tests/contracts/`. Application
native-system adapters belong to `kedra_app`; real input/frame workflows remain in
the existing native runner. Headless tests do not establish native acceptance.
Budget/cycle probes use an external killable process deadline. luau-lsp is checked
through real invocations, not a mandatory private CI service or parallel QA framework.

See the [runtime contract](../../crates/kedra_script/README.md),
[editing guide](../../tools/luau/README.md) and [testing guide](../testing.md).
