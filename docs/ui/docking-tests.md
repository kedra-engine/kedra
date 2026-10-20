# Docking tests

## Reference and acceptance policy

`docking/figma/` contains 20 independent native-resolution Figma exports from
UI-KIT `2EZlK40cfx2Mub7GmDyI70`, page `92:3713`, frames `i00`–`i18`. The floating
state has separate main/window images. The manifest pins names, node IDs, sizes,
SHA-256 and revision `flat-icon-buttons-common-dock-menu`.

References are offline test inputs, never implementation assets. Required Rust
checks verify their integrity and complete state/critical-region coverage. They
do not depend on Figma access or a private service. Reference changes require
explicit review; application pixels cannot become the expected Figma image.

`docking/policy.json` defines full-frame and critical-region budgets. The strict
audit uses normalized YIQ tolerance 0.05, a 2% changed-pixel budget and no masks,
alignment search, resampling or reference cropping. It has no bless command.
An automated match and human design acceptance are different results.

Accepted visual variance includes text rasterization/weight and first inactive-tab
fill. `docking/accepted-variance.json` records owner-reviewed, fixed text interiors
for the current pinned reference revision. The accepted mode excludes only these
interiors (at most 1.5% of a frame); the existing YIQ tolerance and full-frame and
critical-region budgets are unchanged and apply to the remaining pixels. Current
inactive-tab fill differences already fit the existing shade tolerance and need
no extra mask. Tab rims, close icons, compass glyphs, preview outlines and Flat
action boundaries are not excluded. Exception locations never follow actual UI
bounds or changed pixels. A changed reference revision/new scenario requires review.

The strict audit still exposes all those differences and can return nonzero.
Accepted pixel identity does not cover typography inside the declared interiors;
native semantic text/value and interaction checks remain required. Original source
images and unmasked exact-difference diagnostics remain visible in both modes.
Menu behavior, Flat action geometry, tab rims, retained content and transaction
invariants remain test requirements. Platform certification is separate.

## Isolated native corpus

From the repository root, on a native desktop with the public prerequisites:

```sh
python3 tools/qa/dock_fixture.py --output target/qa/dock-check --visual-policy accepted
```

The output directory must not exist. The runner builds the opt-in host and a new
processed delivery, launches two independent processes, injects Bevy input,
captures renderer frames and invokes the existing `xtask visual` comparator.
An explicit `--delivery PATH` can reuse a verified delivery; changed source
assets require a new build.
`accepted` is the CLI default. `--visual-policy strict` runs the unchanged unmasked audit.
The report explicitly identifies which policy determines its visual status.

Each design capture fixes logical size, DPR 1, content, title, font assets and
reduced motion. Evidence records native renderer/backend and OS environment.
Three consecutive identical PNGs are required after input receipts and resource
readiness, with bounded attempts. Setup reset waits for settled measured layout
before the next gesture. Corresponding hashes must match between processes.
DPR 2 is tested for geometry/input, not as an upscale of a DPR-1 reference.

Artifacts include actual/reference copies, evidence/config JSON, native logs,
diff/overlay images and `report.json`. Missing scenarios, unsettled frames,
mechanics failures, unstable hashes and mismatches under the selected policy return nonzero.
Inspect mechanics/stability results separately from accepted visual variance.
Unrelated desktop input or focus changes invalidate an input run.

An acceptance-only policy change can also recompare existing complete stable
native evidence without launching an app or claiming a new input test:

```sh
python3 tools/qa/dock_visual.py --captures target/qa/dock-check \
  --output target/qa/dock-accepted --visual-policy accepted
python3 tools/qa/dock_visual.py --captures target/qa/dock-check \
  --output target/qa/dock-strict --visual-policy strict
```

Both outputs must be new directories. The replay requires complete matching frame
hashes across at least two native processes, successful source mechanics/stability
and normal cleanup, pins the source report hash and preserves its capture profile.
It reads pinned independent Figma references, not application-generated baselines.
An accepted green result and a strict red result can legitimately coexist; neither
overrides a mechanics failure, nor proves physical input or platform certification.

## Host boundaries

`kedra_editor/examples/docking_fixture.rs` is enabled by `docking-fixture`. It does
not install `EditorPlugin`, startup screens, project jobs or real editor panels.
It installs the production workspace input, targeting, projection, popover and
native-host systems with fixed A/B/C/D content and its own authored catalog.
There is no second drag implementation or substitute renderer.

Fixture CSS controls content identity, optional fields and 100px fixture tabs.
The shared dock skin affects both fixture and editor. Editor toolbar/panel growth
does not change the fixture's content or screenshot dimensions.

Setup controls:

| Key | Purpose |
| --- | --- |
| F6 | Restore fixture data and retire extra native hosts |
| F7 | Expose editable fields for retention assertions |
| F8 | Add long-title E content for overflow checks |
| F12 | Exit |

Setup reset is not a product preference-restore test. `dock.state` and
`dock.fixture` are read-only diagnostics. Tested docking actions use actual
production input, not semantic layout assignment.

Set `KEDRA_DOCK_TRACE=1` before launching the isolated fixture to expose the
authenticated read-only `dock.trace` diagnostic. It retains at most 128 event or
gesture-transition frames, with at most 32 input events per frame and an explicit
dropped-event count. It records cursor/window focus, button/modifier state and
gesture/revision changes, not typed character content. Fetch it after a failed
scenario before closing the process; extra observation calls during a gesture can
change race timing. The recorder never changes input or prevents focus cancellation.

The corpus covers all five compass targets, direct edges, reorder, insertion,
blocked drops, Escape, preview/committed rectangle equality, resize,
maximize/restore through the common menu, float/redock/close, whole-group order,
and native draft entity/value retention. Flat idle/hover/pressed feedback and
button-anchored popup bounds are checked at DPR 1 and 2. Clipped full titles remain
selectable through the menu; separate maximize/overflow header controls are absent.
Extra native scenarios also reject unmeasured maximized-group drops and check
window-local Tab/Shift+Tab traversal after floating an editable panel.

## Focused pixel and headless checks

```sh
KEDRA_DOCK_CAPTURE=target/qa/dock-check \
  cargo test -p kedra_editor --test docking_border_frames --locked -- --ignored
python3 -m unittest discover -s tools/qa -p 'test_dock_fixture.py'
cargo xtask check --non-interactive
```

The native frame test reads independent Figma pixels for tab corner arcs,
straight rims and transparent idle Flat-action edges. Straight samples permit
3/255 channel differences; rounded grayscale AA samples use the existing
comparator's approximately 13/255 shade allowance at normalized YIQ 0.05.
It requires two stable native runs and is explicitly ignored without captures.

Rust tests also cover reference integrity, authored skin/catalog bindings,
Flat style/focus behavior, title fitting, snapshot admission, ownership, stale
candidates, corruption, counter limits and persistence refusal. Python unit tests
cover bounded capture readiness and prevent using actual pixels as references.
The QA orchestrator tests earlier-run visual failures, incomplete runs, exact
nonempty draft input and unconditional bounded process cleanup after RPC errors.
`kedra_ui/tests/docking.rs` is an independent retained-projection consumer.

## Project integration and independent runtime consumer

Launch the editor with a dedicated `--state-directory`, authenticated automation
and new credential/artifact paths. The project integration driver refuses personal
preferences:

```sh
python3 tools/qa/docking.py target/qa/editor/session.json \
  --state-directory target/qa/editor/state --report target/qa/editor/report.json
```

Restart against the same isolated state and pass `--restore-report PATH` to compare
saved topology and native hosts. `docking_states.py` supplies additional targeted
editor interaction checks. [Automation](../automation.md) documents transport and
startup-driver options.

`kedra_app/examples/docking_lab.rs` is a separate model/projection smoke consumer
without an editor dependency. F1–F6 exercise retained state through explicit model
commands; it is not a replacement for production gesture acceptance. It uses a
processed delivery selected by `KEDRA_LAB_ASSETS` and opt-in QA credentials.

## Verification boundary

Stable captures, production-mechanics assertions and the focused rim/Flat-action
checks have macOS/Metal evidence. Complete pixel identity is not claimed. Physical
OS pointer capture, mixed-monitor DPI, IME, pickers and Windows/Linux/Wayland
interaction require separate native acceptance. Compilation, RPC receipts and
reduced-motion screenshots do not establish those properties.
