# Production native UI corpus

The reviewed visual policy pins the explicitly approved dependency graph. Changed
graphs require reviewed lock approval before screenshot comparison. Source exports,
SVG image pairs, budgets and font decisions remain independently pinned; capture
never promotes a new lock or policy and lock approval does not grant native acceptance.

Shared Rust tooling follows [ADR 0028](../decisions/0028-shared-native-test.md)
and the [testing contract](../testing.md).
The shared Rust engine and CLI routes are implemented. The profile, tolerances and
four exact accepted SVG pairs remain pinned. Capture fails closed on any dependency
lock that differs from the approved policy.
Feature scenarios are separate from the 22-case component comparison and cannot
grant acceptance to unlisted UI.

The active screenshot catalog contains buttons and text fields only: five production
prefab sources, two families and 22 scenarios. Other components, panels and windows
will be added incrementally after their implementation and independent design inputs
are ready. Unlisted product UI is neither deleted nor accepted by this scope.

## Shared buttons and text-field comparison

On an idle, unlocked native desktop:

```sh
cargo xtask test -- components
```

This is the Rust screenshot path. It builds the production asset delivery and
ordinary editor executable, captures real Bevy frames through the existing
application input/capture transport, and invokes the existing comparator in
`kedra_test` directly.
There is no browser substitute, custom image decoder or second comparison engine.

The command reserves a new directory under `target/test/ui-runs`, streams build and
asset diagnostics, prints per-case progress and finishes with a result plus the
`public/index.html` report path. Open that file directly, without a server or Figma
connection. `--output NEW_DIRECTORY` selects an explicit location; existing evidence
is never overwritten. Failed captures also retain a failure report and completed
frames. Old evidence is never silently substituted for a failed current capture.

```sh
cargo xtask test -- components --only button-primary,field-default
cargo xtask test -- components --source PATH_TO_NEW_SOURCES --policy PATH_TO_REVIEWED_POLICY
```

`--only` is explicitly partial, not full-catalog acceptance. Missing/failed cases,
wrong profiles, incomplete observations, instability and failed restoration remain
failures; no case is silently dropped.

### Scope and native setup

[`corpus/catalog.json`](corpus/catalog.json) lists Quiet, Secondary, Primary and
flat icon buttons in Default/Hover/Pressed/Focus/Disabled states, plus text fields
in Default/Focus states. Each listed source must exist and belong to one family;
each family must have scenarios. This is not an inventory of all product prefabs.

The ordinary `kedra` executable with `--scene` and debug-only
`crates/kedra_editor/src/catalog.rs` bindings mounts the authored
`editor://fixtures/catalog.scene.ron` canvas through the shared startup lifecycle.
Its slots use the same compiled prefabs and workbench stylesheet as production.
It supports only these button/field sources. Specimens use a 640 × 360 client canvas
at DPR 1, with a fixed origin of
(32,32): text buttons are 110 × 22, icon buttons 22 × 22 and fields 172 × 22.
Labels are “Save changes” and “Main Camera”. The icon specimen has no toolbar-padding
ancestor. Source-owned layout and input remain mounted during every paint layer.
The canvas's authored `editor.catalog` component supplies these text values, caret
period, layout rounding, native font hinting and density. Rust validates bounded
calibration values and applies them to the unchanged production prefab instances;
it retains native text measurement, capture and source-address validation.

`catalog.show` selects a specimen by asset path, for example
`{"source":"editor://prefabs/primary-button.prefab.ron","disabled":false}`.
The host resolves the already loaded Bevy asset to its source-owned document identity;
it has no hardcoded UUID dispatch table. The runner checks both the selected path
and the resolved document against independent catalog metadata. Persisted IDs and
the reviewed source fingerprint are unchanged.

The host uses the common debug-only application CLI. A registered `catalog.prepare`
Luau program drives real native resize, hover, held press and keyboard focus;
fixture mounting is a separate explicit precondition. Rust independently calibrates
source/state/slot geometry, validates frames and supervises the process. The canvas
is not recreated per state, while the selected prefab is freshly mounted each time.

Exactly two independent application sessions handle the entire selected sequence;
there is no per-case or per-screenshot restart. Before each case the runner releases
pointer input and remounts the source, resetting focus and paint. Hover/held press
and keyboard focus use real injected input; disabled state is an explicit fixture
precondition. Tab's select-all in a field is collapsed using End, not by repainting
the selection. Native caret/selection affordances are retained.

Each session captures full → glyph-free shape → restored full. Three consecutive
frames must agree per layer, both sessions must agree, and restoration must reproduce
the original full pixels and typography exactly. Full frames must be nonuniform.
Only a glyph-free diagnostic may be uniform, with an explicit comparator reason,
retained text/layout and independently verified nonuniform full/restored frames.

### Strict typography and fixed comparison regions

One [family binding](corpus/text-families.json) maps Figma text property IDs to
authored native `NodeAddress` values. Quiet, Secondary and Primary share the Button
Label binding across states. TextField Value uses its own authored node. Binding
does not match caption strings or nearest geometry. Icon-only cases require an
empty native text inventory too.

The optional `kedra_ui/text-capture` adapter measures Bevy `Text` and `EditableText`:
content, loaded font
path/hash, weight, size, effective RGBA, spacing, line height/count, allocation
bounds and clipping. Positioned glyph counts must agree with independent native
shaping, contain no missing glyphs and cover the complete shaped text. Unicode
characters are not assumed to map one-to-one to glyphs.

The [shared policy](corpus/text-policy.json) accepts font rasterization for its
pinned renderer/environment/DPR, dependency lock and allowed font assets. Size
and RGBA remain exact, as do content/font/weight/spacing/line checks. Caption
placement has one 0.5px allowance for DPR 1 rounding; bounds are allocated layout
boxes, not inferred glyph-ink rectangles. Full-image diffs remain informational,
not hidden or relabeled as pixel matches.

Shape uses the existing 0.05 YIQ tolerance and unchanged 2% local budget. Independently
fixed regions cover the complete component, including the 3px exterior margin in
focus cases. The full 640 × 360 images remain unchanged. No actual-image alignment,
crop, resize, blur or exclusion mask is used. Observed widget bounds never choose
the reference origin or dilute the local denominator.

The policy also pins a deterministic fingerprint of the complete source metadata,
images and family bindings. Changed source contracts or dependencies invalidate a
stale decision before launching an application, even for `--only`. Refreshing a
decision requires review; capture never copies its computed hash into the policy.

### Accepted SVG differences

The owner accepts the current native SVG quality without changing compositing.
Linear alpha blending can produce lighter semitransparent SVG edges than Figma's
sRGB composition, despite identical paths and opaque ink. This is not covered by
the font-rasterization decision.

`accepted_shape_variance` pins only the four reviewed Figma/native glyph-free PNG
hash pairs for flat Default/Hover/Pressed/Focus. These cases are labeled
`accepted_known_difference`, never pixel matches. The raw comparator verdict remains
`mismatch`, with its original diff and unchanged budget. A changed image, profile,
dependency lock or source/binding contract does not inherit the decision.

Exit 0 requires every selected case to pass normally or match its reviewed variance
pair with all other gates passing. Such runs are `passed_with_accepted_differences`.
Capture, typography, clipping, calibration, stability and restoration failures cannot
be waived. Unaccepted discrepancies exit 1 and retain their evidence. No source image
is replaced with application pixels and no reference/golden is automatically promoted.

## Evidence and source provenance

The [independent text source package](corpus/text/text-manifest.json) contains full
PNGs, automatically derived glyph-free PNGs and structured text metadata for all
22 scenarios. Metadata records exact frame/property/node identities, original
canonical component/variant provenance, numeric font metrics and text layout boxes.
Hidden text is excluded; missing, extra, ambiguous or unsupported roles fail.

| Material | Purpose |
| --- | --- |
| `source/` | Byte-identical independent Figma sources and metadata |
| `CASE/run-1/`, `CASE/run-2/` | Original native full/shape/restored frames and measurements |
| `CASE/full/`, `CASE/shape/` | Existing Rust comparator inputs, diffs and detail reports |
| `public/index.html`, `public/report.json` | Human-readable and machine-readable current results |
| `policy.json`, `families.json` | Exact policy/binding inputs copied into the report |
| `private/` | Local session credentials, native logs and generated delivery; never publish |

The report retains all cases, original full/shape pairs, expected versus actual
size/color and local diff percentages. Images link to complete native-resolution
PNGs; display scaling never changes comparison inputs. Partial failures identify
the layer/session and local diagnostic location without publishing private state.
Credentials are removed during process cleanup, including after failed scenarios.

The 44 active independent full/shape images are in `corpus/text`. No other UI
has screenshot coverage. The 28 source wrappers on
[QA / Native exports](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=352-1515)
use canonical-linked instances on full canvases. Wrapper content does not change
canonical masters.

The shared [test project](../../tests/fixtures/test_project/README.md) is available
for manual editor and independent feature scenarios. It is not needed by this
project-independent component host. [Docking behavior](docking-tests.md) and the
other real-input feature drivers have no screenshot coverage or image goldens.

## Refresh source material without an AI service

```sh
cargo xtask test -- export --text-plugin
```

Import the printed `plugin/manifest.json` as a local Figma development plugin in
the exact Kedra UI-KIT file. Run it and download the JSON bundle. The generated
plugin uses `tools/test/figma_text_export.js` and ordinary Figma Plugin APIs, without
Node/npm, network requests or publishing. Its private API flag exposes current-file
identity only. Existing fonts are loaded before operating on their text nodes.

For each source frame the exporter preserves the original full image, then creates
one disposable clone and changes only text opacity for the glyph-free export.
Layout, child icons and original masters are retained. Clones are removed on failure
too. If a later case fails, the plugin offers an explicitly incomplete diagnostic
bundle containing completed cases and available failed originals; it cannot be
imported as a complete source package or grant acceptance.

```sh
cargo xtask test -- export --text-bundle path/to/kedra-text-sources.json
cargo xtask test -- components --source PATH_TO_NEW_RUN/exports
```

Import reserves a new directory, validates hashes/provenance/metadata and invokes
the existing Rust decoder for PNG dimensions/integrity. It does not overwrite old
sources or approve them. Ordinary capture/CI uses stored sources offline and never
contacts Figma. A new supported variant/state needs its source frame/catalog record,
not manually maintained no-text variants, paint code or hand-entered font values.

The Rust-generated desktop development-plugin wrapper requires independent
interactive host verification; stored source inputs do not establish that acceptance.

## Production control styling

### Retained single-sample provenance

The shared `text-policy.json` is the only active font acceptance policy. The
Primary Default-only sample identities below are retained source provenance, not
a parallel executable policy or permission for any profile or dependency lock:

- Figma full/shape/font nodes: `352:1548`, `358:1625`, `358:1628`.
- Original sample contract: `b46c32ae72ecd3cd07fbbbfe4f0bb1012a599d6b0145cddcf0cb3f96ab3aa3fb`.
- Original full reference: `61685f08a9fa38ff1273a1ce9bbc490596e82a85f9306c2edd796c7049db5e26`.
- Original dependency lock: `79edb4f2650332afd5470f9ba616609da3a69e3b8511e02e64422ab53791bd34`.

### Shared styles

Quiet/Secondary/Primary labels use Medium 500. Disabled button text uses the
dedicated `--disabled-text` (`#858585`), not general `--muted`. Primary is borderless
in ordinary states but retains the focused variant's dark 1px border; blur explicitly
restores the borderless state.

Authored `control-focus` children preserve control/caption layout while reproducing
independent focus rectangles. Outer rings extend 3px with a 2px stroke and 6px radius;
flat icon rings are inset 1px with a 3px radius. Bevy's derived outline radius cannot
express both geometries. Authored roots opt in with `authored-focus`; other controls
and project rows retain existing outline rules. Native caret behavior is unchanged.

The flat production prefab owns its `flat` class, canonical right-chevron geometry
and token-driven image color. Disabled icon ink represents the design's composited
appearance, separately from disabled text and unrelated window/toolbar controls.

## Verification and CI boundary

QA drivers, fixture hosts and generated reports do not have a separate self-test
suite. Verify this workflow with actual product scenarios, native input, frames and
the real comparator; do not add mock/unit tests of those tests. Product Rust unit,
integration and regression checks remain required and unchanged in responsibility.

The dispatched **Native UI corpus** workflow (`.github/workflows/ui.yml`) runs the
same `components` command with an explicit reviewed policy path. It prepares Ubuntu
24.04, Xvfb/Openbox and Mesa software Vulkan, records installed-package/driver
identity and retains only public reports/environment artifacts. It does not run
obsolete capture/audit/replay lanes or QA-script self-tests.

The checked policy is pinned to the reviewed macOS/Metal profile, not Linux.
An unreviewed platform fails closed before capture. A Linux policy must be explicitly
reviewed and the workflow actually run before hosted acceptance or a required PR
gate is claimed. There is no capture-only fallback or automatic reference approval.

Compilation is not native acceptance. Static reduced-motion frames do not certify
animation, physical OS input, IME, native pickers, clipboard or mixed-monitor DPI.
There is no dedicated catalog example or diagnostic Cargo profile. The ordinary
debug editor installs `EditorPlugin` and its native capabilities for both startup
lifecycles; explicit-scene startup excludes project startup and project operations.
`kedra_editor` enables `kedra_ui/text-capture` through its normal dependency, while
catalog bindings and diagnostic services are debug-only. Authored calibration and
the named Luau input program do not replace ordinary application clocks or grant
visual acceptance. Compilation, Luau execution and native input/frame verification
of the current composition remain separate acceptance requirements.
