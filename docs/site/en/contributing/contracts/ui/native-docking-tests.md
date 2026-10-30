# Native docking test contracts

Docking tests check topology, gestures, retained content and native-host lifetimes. Launch setup
is in [Native scenarios](../../../manual/testing/native-scenarios.md). Docking/windows/panels have
no active image goldens, pixel budgets or screenshot acceptance suite. The retained manifest is
[source provenance only](design-provenance.md#independent-references-and-provenance).

## Native behavior route

On a trusted, idle, unlocked native desktop, with a new output directory:

```sh
cargo xtask test -- features --only docking,docking-states,dock-fixture --output target/test/docking
```

Rust builds/verifies deliveries and launches isolated processes. The editor scenario refuses
personal preferences, uses production input and restarts against its own saved state to check
topology/native-host restoration. Native interaction/capture reliability has unresolved intermittent
failures. Do not silently retry, refocus or restart an interrupted gesture to claim acceptance.

Focus loss cancels an active gesture without publishing; focus restoration while the pointer
remains held cannot restart it. A new press is required. The fixture checks liveness while waiting
for guides, insertion markers and previews, immediately reporting cancellation instead of waiting
for feedback from an ended gesture.

The isolated scene runs twice in the ordinary editor, without project lifecycle/panels/preferences.
It checks compass/direct edges, insertion/order, blocked drops, Escape, measured preview/commit
geometry, resize, maximize/restore, floating/redock, close requests, retained drafts and
window-local
Tab/Shift+Tab. Flat feedback/popup anchoring use native observations, not image references.
Measured splits must fit the production 0.5px host-containment bound, including fractional
allocation
and native pixel rounding.

`app.capture` is a GPU synchronization fence: temporary private readbacks are validated/retired.
Reports retain frame receipts, geometry, state and input outcomes, not PNG galleries, hashes or
comparisons. Receipts do not certify visual quality; only the [kit corpus](ui-corpus.md) compares
screenshots.

## Host and scenario authority

The shared `dock-fixture` owns the native direction/order/refusal/cancellation matrix.
`docking` checks real editor panel retention, source-host integration and preference restoration;
`docking-states` checks the real workspace/panel menus, including a compact floating host.
It does not replay the fixture's compass matrix. Preview waits use the same candidate-readiness
predicate as production drop admission, not an independent screenshot-stability algorithm.

The ordinary debug `kedra` selects `editor://fixtures/docking.scene.ron` via `--scene`.
Production workspace input, targeting, projection, popovers and native hosts consume authored
A/B/C/D topology through the existing Snapshot validator. Normal/explicit-scene startup install
`EditorPlugin` and the same native capabilities; the explicit lifecycle excludes project startup
and rejects project operations. There is no example target or second drag implementation.

| Registered Luau program | Setup authority |
| --- | --- |
| `dock.reset` | Settle input, resize, restore fixture data, retire auxiliary hosts |
| `dock.drafts` | Toggle fields for native retention assertions |
| `dock.overflow` | Add a long title for overflow actions |

These named programs own diagnostic ordering, not general remote topology mutation. Their native
capabilities validate the live scene/workspace owner. F12 exits explicit-scene startup.
Reset is setup, not product preference restoration. `dock.state`, `dock.fixture` and opt-in
`dock.trace` are read-only; trace bounds gesture transitions, cursor/window ownership and modifiers,
never typed text. Tested docking actions use actual Bevy input, not semantic layout assignment.

## Product owners and independent consumer

Pure Rust contracts cover topology/ownership/stale candidates/snapshots in `kedra_docking`, retained
projection in `kedra_ui`, and persistence refusal/host orchestration in editor workspace tests.

```sh
cargo test -p kedra_docking --locked
cargo test -p kedra_ui --test contracts --locked docking::
cargo xtask check
```

The runtime-only
[test game](https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md)
consumes the same model/projection through authored `ui.docking`, selecting
`editor://fixtures/projection.scene.ron` with explicit verified visual delivery. Its explicit model
commands do not replace gesture checks. See
[Independent runtime consumers](../../../manual/testing/native-scenarios.md#independent-runtime-consumers).

## Acceptance limits

Injected close tests normal close-request policy, not an OS caption click. Physical capture/release,
native buttons/drag, IME, pickers, mixed-monitor DPI and Windows/Linux/Wayland require separate
platform input/frame checks. Bevy readbacks exclude OS controls. Compilation and native
verification of this revision remain unconfirmed.
