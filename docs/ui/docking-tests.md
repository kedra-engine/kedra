# Docking behavior checks

Screenshot coverage is limited to kit buttons and text inputs in the
[22-case corpus](ui-corpus.md). Docking, windows and panels have no active image
goldens, pixel budgets or screenshot acceptance suite. The retained docking
manifest records source provenance only; its PNG exports are not present or active
comparison inputs.

## Real native behavior

On an idle native desktop:

```sh
cargo xtask test -- features --only docking,docking-states,dock-fixture --output target/test/docking
```

The output directory must be new. Rust builds a verified delivery and launches
isolated processes. The editor scenario refuses personal preferences, exercises
the production input path and restarts against its own saved state to verify
topology and native-host restoration.

Native focus loss cancels an active docking gesture without publishing its
candidate. Focus restoration while the mouse remains held does not restart it;
a new press is required. The fixture checks gesture liveness while waiting for
guides, insertion markers and previews, and reports cancellation immediately
rather than waiting for feedback from an ended gesture. It does not refocus the
application, restart interrupted gestures or retry a failed scenario automatically.

The isolated docking scene runs twice in the ordinary editor executable without
the project lifecycle, project panels or preferences. It checks compass/direct-edge
drops, insertion/order, blocked drops,
Escape, measured-preview/committed geometry, resize, maximize/restore, floating,
redocking, close requests, retained drafts and window-local Tab/Shift+Tab.
Flat feedback and popup anchoring use native observations, not image references.
Measured split previews must stay within the production 0.5px host-containment
bound, including fractional allocations and native pixel rounding.

`app.capture` is used only as an existing GPU synchronization mechanism: temporary
private readbacks are validated and retired. Behavior reports retain frame receipts,
geometry, state and input results, not PNG galleries, image hashes or comparisons.
Those receipts do not certify visual quality. Only the kit corpus publishes and
compares screenshots.

## Host boundaries

The ordinary debug `kedra` executable selects
`editor://fixtures/docking.scene.ron` through `--scene`. Its native bindings use
production workspace input, targeting, projection, popovers and native hosts with
authored A/B/C/D topology decoded by the existing `Snapshot` validator. Both normal
and explicit-scene startup install `EditorPlugin` and the same native capabilities;
the explicit scene excludes project startup and rejects project operations. It
does not use a dedicated example target or a second drag implementation.

| Named Luau program | Setup purpose |
| --- | --- |
| `dock.reset` | Settle input, resize, restore fixture data and retire auxiliary hosts |
| `dock.drafts` | Toggle fields for native retention assertions |
| `dock.overflow` | Add a long title for overflow actions |

These host-registered programs own diagnostic sequence selection and ordering.
Their scoped native capabilities validate the live scene/workspace owner; they are
not general remote topology mutation methods. F12 is the explicit-scene exit shortcut.

Setup reset is not product preference restoration. `dock.state`, `dock.fixture`
and opt-in `dock.trace` are read-only diagnostics. Tested docking actions use
actual Bevy input, not semantic layout assignment. The bounded trace records
gesture transitions, cursor/window ownership and modifiers, never typed text.

Pure Rust product tests cover topology, ownership, stale candidates,
snapshot validation, persistence refusal, authored projection and retained content:

```sh
cargo test -p kedra_docking --locked
cargo test -p kedra_ui --test contracts --locked docking::
cargo xtask check
```

The runtime-only [canonical test game](../../tests/fixtures/test_project/README.md)
also consumes the shared model/projection capability, activated by an authored
`ui.docking` component. Its ordinary executable selects
`editor://fixtures/projection.scene.ron` with an explicit visual delivery. Explicit
model commands there do not replace the production gesture checks.

## Verification limits

Injected close requests test the host's normal policy, not an OS-owned caption
button. Physical pointer capture, native buttons/drag, IME, pickers, mixed-monitor
DPI and Windows/Linux/Wayland interaction require separate platform acceptance.
Bevy renderer readbacks exclude OS-owned button drawing.
Implementation and regression source do not establish compiled or native
acceptance of the changed implementation; real process/input/frame verification
remains required.
