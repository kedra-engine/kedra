# UI corpus contracts

The UI corpus compares native buttons and text fields against independent design exports. See
[Visual verification](../../../manual/testing/visual-verification.md).
[ADR 0028](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0028-shared-native-test.md)
defines the shared tooling. Compilation and native verification of this revision remain unconfirmed.

## Active scope and launch contract

Five production prefab sources, two families and 22 cases are active: Quiet/Secondary/Primary/flat
icon buttons in Default/Hover/Pressed/Focus/Disabled, and fields in Default/Focus. Other UI remains
implemented where documented but neither enrolled nor accepted by this corpus. Full-window
startup/docking/panel/scene goldens are absent. The source catalog is
[`docs/ui/corpus/catalog.json`](https://github.com/kedra-engine/kedra/blob/main/docs/ui/corpus/catalog.json).
Each listed source must exist, belong to exactly one family, and each family needs scenarios.

```sh
cargo xtask test -- components
cargo xtask test -- components --only button-primary,field-default
cargo xtask test -- components --source PATH_TO_NEW_SOURCES --policy PATH_TO_REVIEWED_POLICY
```

Use a trusted, idle/unlocked native desktop. The Rust runner builds the production delivery and
ordinary debug editor, captures real Bevy frames through existing input/capture transport and calls
`kedra_test` directly. No browser substitute, custom decoder or second comparator is involved.
Default output reserves `target/test/ui-runs/`; `--output NEW_DIRECTORY` must not exist. Diagnostics
stream with per-case progress and final `public/index.html` path. The report is offline/passive.
Failed runs retain failure reports/completed frames, never substitute old evidence. `--only` is
partial, not full acceptance; missing/failed/unrun cases, profile errors, incomplete
observations, instability and failed restoration cannot disappear from the verdict.

## Authored native fixture

Debug bindings in `crates/kedra_editor/src/catalog.rs` mount
`editor://fixtures/catalog.scene.ron` via shared `--scene` startup. Slots use production compiled
prefabs/workbench CSS, supporting only the listed sources. A 640×360 DPR-1 client has fixed origin
(32,32): text buttons 110×22, icon buttons 22×22, fields 172×22; labels are “Save changes” and
“Main Camera”. Icons have no toolbar-padding ancestor.
Source layout/input stays live for all layers.

The authored `editor.catalog` supplies text, caret period, rounding, native hinting and density.
Rust validates bounded calibration and applies it to unchanged prefab instances, preserving native
measurement, capture and source-address checks. `catalog.show` takes a logical source path, e.g.
`{"source":"editor://prefabs/primary-button.prefab.ron","disabled":false}`. The host resolves the
loaded asset's source document identity, not a hardcoded UUID dispatch table. The runner checks
path/resolved document against independent metadata; persisted IDs/fingerprint remain unchanged.

Host-registered `catalog.prepare` drives native resize/hover/held press/focus via common debug CLI.
Mounting is an explicit precondition. Rust independently checks source/state/slot geometry and
supervises frames/processes. The canvas persists across states; each specimen mounts freshly.
Exactly two independent sessions run the complete selected sequence, not per-case restarts.
Each case releases pointer input/remounts/reset focus and paint. Hover/press/keyboard use injected
input; disabled is a fixture precondition. End collapses field Tab select-all without repainting
selection; native caret/selection affordances remain.

Each session captures full → glyph-free shape → restored full. Three consecutive identical frames
per layer and agreement between sessions are required; full restoration must reproduce original
pixels/typography exactly. Full frames are nonuniform. Only reasoned glyph-free diagnostics may be
uniform, retaining text/layout and independently checked nonuniform full/restored frames.

## Strict typography and fixed regions

[`text-families.json`](https://github.com/kedra-engine/kedra/blob/main/docs/ui/corpus/text-families.json)
binds Figma text property IDs to native authored `NodeAddress`, never caption matching/nearest
geometry. Quiet/Secondary/Primary share Button Label; TextField Value has its own node. Icon-only
cases require an empty text inventory. Optional debug-only `kedra_ui/text-capture` measures
`Text`/`EditableText` content, loaded font path/hash, weight, size, effective RGBA, spacing, line
height/count, allocation and clipping. Positioned glyphs must match independent shaping, with zero
missing glyphs/complete coverage; Unicode characters are not assumed one-to-one with glyphs.

The sole active font policy is
[`docs/ui/corpus/text-policy.json`](https://github.com/kedra-engine/kedra/blob/main/docs/ui/corpus/text-policy.json).
It accepts rasterization only for the exact renderer/environment/DPR, lock, source fingerprint and
font assets. Content/font/weight/size/RGBA/spacing/lines stay exact. Allocated caption boxes,
not ink bounds, have only a 0.5px DPR-1 rounding allowance. Full-frame diffs remain visible
and informational, never relabeled pixel matches.

Shape uses 0.05 YIQ tolerance and 2% local budget in independently fixed component regions,
including 3px exterior focus margin. Full 640×360 bytes remain unchanged: no alignment, crop,
resize, blur or exclusion masks. Actual widget bounds cannot move the reference origin or dilute
the local denominator. A deterministic complete metadata/image/family fingerprint and dependency
lock are checked before launch, even with `--only`. Review is required to refresh any decision;
capture never writes computed hashes into policy.

### Reviewed identity pins

Policy scope is `catalog-component-buttons-and-fields`, review `human_reviewed`, rasterization
`accepted_as_rendered`. Exact profile: `local-native-text-v1`, platform `darwin`, renderer
`{"backend": "Metal", "driver": "", "driver_info": "", "name": "Apple M3 Pro"}`,
environment `macOS-27.0-arm64-arm-64bit-Mach-O/local/uncontrolled-packages`, logical `[640,360]`,
DPR 1, `srgb_assumed`, `reduced` motion. These are reviewed declarations, not current machine proof.

The repository's `Cargo.lock` does not match the reviewed lock pin below. The corpus must reject
that graph before launch; changing the pin requires reviewed lock approval.

```text
dependency_lock_sha256:
  d410a9a12a59fc84a5407938485c36c87e429ae16755da763200f1ef57023b33
source_contract_sha256:
  862fd2a4624d9c88c075a6233e77e2b006e408d01b0dcf1658dc9ab7d24bcad5
Inter Regular — editor://fonts/Inter-Regular.ttf:
  6a974fecb20c21cd2c2ec3af4ee2bd6a96ae449f250c9990b84199e7d1288d50
Inter Medium — editor://fonts/Inter-Medium.ttf:
  7304ac6809616f3d0c7a9969593b50ca4d61e1ce3e268c9b1e819cd09ea7ecad
```

### Accepted SVG differences

The owner accepts native SVG quality without changing compositing. Linear-alpha edges can be
lighter than Figma sRGB composition despite identical paths/opaque ink; this is not font variance.
Only these four glyph-free shape pairs are accepted under the exact policy/lock/source/profile:

```text
button-flat (Figma / native):
  f96f24e349e74b786a901f99646f277b2f3d5dc993ffdb0b9f01574df4cae4b9
  eef77d02b031bbe57f2ccfec3d156e593b3d10fcab228f124ddea5680b692d16
button-flat-hover (Figma / native):
  d6f43bc01aec843fa57d0375a89860f243236dd504a09b7c9c8722d8df3f16c9
  d777f272563058c1bde88222d791a0e8e69cb93e820004c294222059c2365bc0
button-flat-pressed (Figma / native):
  f48cf9a0a560fa56e219e684e5c0e36f8dfa99298eb7e3d09e1b570a4f611b82
  00082515277fe492e6aa529df4729ed22f988e0eccba5e96f862b82232384ec3
button-flat-focus (Figma / native):
  702e936529c6e97d4f8bd21d9fabbc60045bec5d4b430cc9d35f307ec4605618
  1ebf7f15668b11bb537c55c8919341cef29ed83636738ab6d12259b6d39c6703
```

They report `accepted_known_difference`, never pixel matches. Raw verdict stays `mismatch` with
unchanged diff/budget. Changed images/profile/lock/source/binding cannot inherit approval. Exit 0
requires each selected case to pass or match its exact pair with every other gate passing; such
runs are `passed_with_accepted_differences`. Capture/typography/clipping/calibration/stability/
restoration failures cannot be waived. Unaccepted differences exit 1 with evidence. No application
pixels replace source images and no reference is automatically promoted.

## Evidence and source package

[`text/text-manifest.json`](https://github.com/kedra-engine/kedra/blob/main/docs/ui/corpus/text/text-manifest.json)
contains independent full PNGs, derived glyph-free PNGs and structured metadata for all 22 states:
exact frame/property/node IDs, canonical component/variant provenance, font metrics/layout boxes.
Hidden text is excluded; missing/extra/ambiguous/unsupported roles fail. The 44 active images remain
under `docs/ui/corpus/text`. The 28 canonical-linked wrappers on
[QA / Native exports](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=352-1515)
use full canvases without changing canonical masters.

| Run material | Purpose |
| --- | --- |
| `source/` | Byte-identical independent design PNGs/metadata |
| `CASE/run-1/`, `CASE/run-2/` | Original native full/shape/restored frames and measurements |
| `CASE/full/`, `CASE/shape/` | Comparator inputs, raw diffs and detail reports |
| `public/index.html`, `public/report.json` | Human/machine current results |
| `policy.json`, `families.json` | Exact copied policy/bindings |
| `private/` | Local credentials/logs/generated delivery; never publish |

Reports retain all cases, original layer pairs, expected/actual size/color, local percentages and
native-resolution links. Viewer scaling never changes inputs. Partial failures identify layer,
session and diagnostic path without publishing private state; cleanup removes credentials even
after failure. The canonical test project is for manual/feature scenarios, not needed by this
project-independent host. Feature checks have no screenshot goldens.

## Manual source export without an AI service

```sh
cargo xtask test -- export --text-plugin
cargo xtask test -- export --text-bundle path/to/kedra-text-sources.json
cargo xtask test -- components --source PATH_TO_NEW_RUN/exports
```

Import the printed `plugin/manifest.json` as a local Figma development plugin in the exact UI-KIT,
run interactively and download its JSON bundle. `tools/test/figma_text_export.js` uses ordinary
Plugin APIs, no Node/npm/network/publishing; the private API flag exposes current-file identity
only. Fonts load before text operations. Each original full export is preserved; one disposable
clone changes text opacity only for shape. Layout/icons/masters remain; failure removes clones too.
Later failures may offer an explicitly incomplete diagnostic bundle, never complete import or
acceptance. Import reserves a new directory, validates hashes/provenance/metadata and uses Rust
PNG integrity/dimension decoding; it overwrites/approves nothing. Offline capture/CI never contacts
Figma. New supported states require frame/catalog entries, not manual no-text variants, paint
code or hand-entered metrics. The generated desktop wrapper still needs independent interactive
host verification; stored inputs do not establish that acceptance.

## Production styling and retained sample provenance

Quiet/Secondary/Primary use Medium 500. Disabled text uses `--disabled-text` `#858585`, not
`--muted`. Primary is normally borderless; Focus has a dark 1px border and blur restores borderless.
Authored `control-focus` children preserve control/caption layout: outer rings extend 3px with
2px stroke/6px radius; flat rings inset 1px/3px radius. Derived Bevy outline radii cannot express
both. `authored-focus` opts in; other controls/project rows retain existing rules.
Caret is unchanged.
Flat prefab owns `flat`, canonical right-chevron geometry and token-driven image color. Disabled
icon ink models design composition separately from disabled text/window/toolbar controls.

Primary Default-only provenance below is **not** another executable font policy or approval:

```text
Figma full/shape/font nodes: 352:1548 / 358:1625 / 358:1628
Original sample contract:
  b46c32ae72ecd3cd07fbbbfe4f0bb1012a599d6b0145cddcf0cb3f96ab3aa3fb
Original full reference:
  61685f08a9fa38ff1273a1ce9bbc490596e82a85f9306c2edd796c7049db5e26
Original dependency lock:
  79edb4f2650332afd5470f9ba616609da3a69e3b8511e02e64422ab53791bd34
```

## Verification and CI boundary

QA drivers/hosts/reports have no separate self-test suite; verify actual scenarios/input/frames and
the real comparator. Product Rust contracts remain required. The dispatched **Native UI corpus**
workflow `.github/workflows/ui.yml` uses the same command/explicit policy on Ubuntu 24.04,
Xvfb/Openbox/Mesa software Vulkan, records package/driver identity and keeps public/environment
artifacts only. No obsolete capture/audit/replay lanes or QA-script self-tests are implied.

The checked macOS/Metal policy is not Linux approval. Unreviewed platforms fail closed before
capture; review a Linux policy and actually run the workflow before claiming hosted acceptance or
a required PR gate. There is no capture-only fallback/automatic approval. Compilation is not native
acceptance; reduced-motion stills do not certify motion/OS input/IME/pickers/clipboard/mixed DPI.
There is no catalog example or special Cargo profile. Ordinary debug `EditorPlugin` supplies both
lifecycles; explicit scenes exclude project startup/operations. The editor's ordinary dependency
enables text capture, but bindings/services remain debug-only. Authored calibration/named Luau
programs replace neither application clocks nor independent compiled/input/frame acceptance.
