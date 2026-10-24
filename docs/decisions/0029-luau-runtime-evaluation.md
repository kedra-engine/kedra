# ADR 0029: Shared Luau runtime and host-owned API scopes

Status: accepted runtime integration. Native acceptance and platform certification
require the separate checks described in the testing guide.

## Decision and scope

Use interpreted Luau through published BMS 0.23.0 against pinned Bevy 0.20.0 in
runtime-layer `kedra_script`. Games and the editor consume the same language
implementation. Hosts supply narrow capability tables; the editor does not own a
second interpreter, and gameplay does not inherit editor authority merely because
it runs in an editor process. This is an initial API, not a stable game SDK or Play.

BMS provides public context loading, callback dispatch and scoped Bevy world access.
Kedra controls explicit admission, finite sequence coroutines, persistent behavior
callbacks, admitted-source modules, Bevy-schedule waits, budgets, candidate replacement
and retirement. [ADR 0032](0032-script-entrypoints-and-modules.md) defines the
entrypoint/context/module contracts. No alternate ECS/reflection system,
generic task executor or generated complete Bevy API is introduced. BMS's binding
registries are backend adapters, not project/source type authority.

The runtime is a coherent crate with real editor/application consumers. It depends
on neither editor, authoring, filesystem authoring writer nor QA tooling. Trusted
Rust bindings reacquire current ECS access synchronously and return owned values;
no component borrow survives a coroutine yield. Full-width native identities use
opaque strings/handles instead of potentially lossy Luau numbers.

Game/tool API isolation is a product contract in `kedra_script/tests/contracts/lifecycle.rs`.
Embedded `.luau` probes assert absent
editor/reflected-World capabilities in game scopes, explicit tool-only selection
access, read-only tables and separate globals. The tool capability is a test-owned
stand-in, not the actual editor's selection authority. Lifecycle, waits, cancellation
and budget contracts belong to that crate. One canonical
`tests/fixtures/test_project` owns shared authored data/scenes/scenarios and the
ordinary runtime-only game used to demonstrate the no-editor dependency graph.

Source loading requires an explicit trusted-host `start`/`attach`. Project open,
registration, source discovery and script execution are not equivalent permissions.
`replace` validates a fresh candidate before retiring the old generation; it resets
globals/tasks, not arbitrary VM stacks. Effects are not rolled back on failure or
cancellation. Editor source edits must use document validation/history/Save, never
live gameplay World state. Current editor bindings are read-only state/workspace.

## Native QA consumer

`kedra_app::scripting` supplies shared native input/observation/capture capabilities
using the same registered feature systems as RPC, without network round trips per
internal step. Its finite scenario catalog accepts names and bounded owned arguments,
not source strings, reflection mutation or arbitrary eval. One application scenario
runs at a time; operation identities enforce admission/retry guarantees.

Rust QA scenarios own fixture/filesystem setup, domain assertions,
process supervision and evidence validation. Their shared input, resize, capture,
click and field sequences execute inside the actual application in Luau. The
editor additionally supplies `editor.startup.forms`, a full ordinary Luau sequence
using actual editor state, native Bevy input and renderer receipts. Rust independently
validates and discards synchronization PNGs; only kit buttons/fields have screenshot
comparison coverage. Product assertions remain owned by the Rust scenarios.

The QA client exposes script-backed input, resize and capture explicitly. Its
ordinary `operation` call invokes exactly the named native method: there is no
implicit selection between RPC and Luau execution.

The externally supervised Rust path remains necessary for crashes/hangs and clean
shutdown. A successful script/input receipt is not feature or physical OS input
acceptance. Native scenarios require actual input and renderer evidence. No mock
QA-driver/report self-test framework is introduced.

## Alternatives and limitations

- Editor-only interpreter would make application reuse depend on the editor.
- Exposing all reflected World operations grants accidental authority and makes
  borrowing across suspension difficult; use curated bindings instead.
- A custom VM bridge duplicates maintained Bevy integration before a concrete
  mismatch is established. BMS's public hooks support this initial contract.
- Multiple production languages multiply API/tooling cost. Lua/Rhai remain future
  alternatives if required target constraints cannot be met with Luau.
- BMS's default in-place reload retains old globals; explicit fresh contexts give
  clearer ownership and candidate failure behavior.

The Lua heap/interrupt/total/resume budgets do not preempt compilation, native host
work or all native memory. This is not hostile-code or OS process isolation. No
automatic source watching, module filesystem access, generated Luau SDK definitions,
debugger or browser/mobile certification is provided. Play isolation remains unselected.

## Dependency review

The owner-approved [ADR 0007](0007-dependency-policy.md) exception admits
`RUSTSEC-2026-0173` for registry `proc-macro-error2 2.0.1` through the exact reviewed
mlua/BMS chain until 2026-12-26 UTC. It concerns unmaintained build-time diagnostics,
not a known runtime VM vulnerability. Future advisory IDs or broadened callers do
not inherit approval. Main-lock policy regressions are mandatory; independent
consumers supply their resolved lock explicitly to the same validator.

Admit only the exact lower `variadics_please 1.1.0` procedural-helper version for
duplicate detection: BMS 0.23 requires 1.x while Bevy 0.20 requires incompatible 2.x.
These are build-time tuple expansion helpers, not a shared runtime type/API boundary.
Do not patch a major constraint to pretend they unify. Sources, advisories and
licenses remain checked; remove this exception when upstream converges.

The mlua serialization feature uses `serde-value 0.7.0`, which requires
`ordered-float 2.10.1` while WGPU uses 5.5.0. Admit only that exact lower identity
for duplicate detection; there is no shared OrderedFloat API crossing the scripting
and renderer boundaries. This reuses maintained serde conversion rather than a
custom Lua/JSON serializer. Advisory/license/source checks remain unchanged.

Dependency adoption changes Cargo.lock. It does not authorize new design references,
pixel tolerances, renderer variance pairs or automatic approval of a refreshed
visual policy. Reconcile the component policy explicitly against the new graph and
retain its original independent images and acceptance boundaries.

See [runtime contracts](../../crates/kedra_script/README.md) and
[automation](../site/en/contributing/contracts/automation.md)
for executable API and current verification limits.
