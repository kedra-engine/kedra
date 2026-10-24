# Visual verification

Image integrity, native capture, automated comparison and human acceptance are separate tasks.
The only active screenshot corpus contains 22 kit button/text-field cases.
Startup, docking, panels, windows and scenes have behavior scenarios, not active image goldens.
The offline comparator's generic capabilities do not expand that scope.

## Inspect independent sources without launching

Stored source images are independent design exports, not screenshots promoted from the app.
To decode/hash-check the existing integrity manifest into a new diagnostic directory:

```sh
cargo xtask visual --inspect-reference docs/ui/corpus/text/integrity-manifest.json \
  --output target/test/source-review
```

This offline operation does not contact Figma, capture a native frame or approve the reference.
Its report records integrity, not native acceptance. Inputs remain unchanged.
Ordinary comparison needs neither Figma access nor Python/Pillow or an AI service.
Refreshing source material is an explicit authoring/review operation, not automatic test recovery.

## Capture the reviewed controls

Read the [renderer capture contract](../automation/input-and-captures.md) and use the
[CLI reference](../cli/repository-and-tests.md) to distinguish capture from offline comparison.

Only launch this on a prepared, idle native desktop:

```sh
cargo xtask test -- components --output target/test/components
cargo xtask test -- components --only button-primary,field-default \
  --output target/test/component-subset
```

Each output path must be new. The second command is diagnostic and partial.
Before launching, the runner checks the reviewed dependency lock, complete source fingerprint,
family bindings and policy/profile. A changed dependency graph requires reviewed lock approval.
Capture does not refresh policy hashes or fall back to an unreviewed platform.
The checked policy is specific to a macOS/Metal environment; Linux acceptance is not implied.

The runner builds/verifies the production editor delivery and uses the ordinary debug editor's
authored catalog scene. Exactly two independent sessions run the entire selected sequence.
Each case resets input and mounts the selected production prefab, then captures
full → glyph-free shape → restored full layers without replacing production layout/input.

Three consecutive byte-identical frames are required per layer, within sixteen capture attempts.
Both sessions must agree; restored full pixels and typography must reproduce the original exactly.
A uniform full frame fails. Only a reasoned glyph-free diagnostic may be uniform.
Stable bytes alone are insufficient: independent state, source and geometry readiness still apply.

## Understand the verdict

Typography checks content, font identity, weight, size, RGBA, spacing, lines, layout and clipping.
The reviewed policy accepts native rasterization, not arbitrary changes to those properties.
Caption placement has a 0.5px DPR-1 allowance. The component shape comparison uses tolerance
0.05 and a 2% local budget in independently fixed regions, retaining the full 640 × 360 images.
The runner does not align, resize, blur or crop actual images to obtain a match.

Four exact reviewed flat-button shape hash pairs may produce `accepted_known_difference`.
Their raw comparator verdict remains `mismatch`, with unchanged diffs and budgets.
They are not pixel identity and do not waive typography, capture, stability or restoration failures.
A run containing them reports `passed_with_accepted_differences`, not a universal visual match.
Changed bytes, source contracts, profile or dependency lock do not inherit approval.

## Recompare existing evidence offline

After a component run has published the required case files, compare one shape independently:

```sh
cargo xtask visual --config target/test/components/public/button-quiet/shape/config.json \
  --actual target/test/components/public/button-quiet/shape/actual.json \
  --output target/test/button-review
```

This does not launch an application or create missing captures.
Exit 0 means an automated match, not design acceptance; exit 1 means mismatch or blocked comparison.
Inspect `report.json` for `matched`, `mismatch`, `invalid_config`, `invalid_capture`,
`missing_reference`, `invalid_reference`, `unsupported` or `infrastructure_failure`.
Failure to create a new output directory cannot publish a report there.

Actual evidence declares scenario, fixture, profile, image hash, producer, readiness and receipt
frame. The comparator checks declarations and bytes, not capture authenticity or the real desktop.
A receipt frame is not an OS presentation timestamp or an animation source-frame sample.
Relative PNG paths are bounded to their manifests; unsafe paths and mismatched hashes fail.
Only static 8-bit SDR PNG with supported sRGB assumptions is admitted, not HDR or animated PNG.

## Inspect the report before deciding

Open `index.html` locally. Inspect original-scale side-by-side images, the 50% overlay,
perceptual/exact diffs and disclosed exclusions. Keep `reference-source.png` and
`actual-source.png`: these preserve original bytes, unlike a screenshot of the report viewer.
The component run's `public/index.html` also shows per-case/session/layer and typography results.
Keep `private/` credentials/logs/project state local; review even public provenance before sharing.

For generic policies, both changed-pixel count and changed-PPM budgets must pass, including
critical regions. Exclusions require fixed reviewed geometry/reasons and a capped union.
An empty comparison domain fails. Do not move a mask to follow a failing actual image.
These capabilities are not permission to add masks to the active component corpus.

## Review and recovery boundaries

First distinguish invalid evidence/profile/setup from a genuine product mismatch.
Retain the failed output and repair its cause; use a new directory for the next attempt.
Never copy current application pixels over independent references or adjust budgets to hide a diff.
References, masks, thresholds, comparator versions and promotions need explicit review.
Automated verification never updates a reference or grants design acceptance.

Static/reduced-motion images do not certify normal motion, timing or interruption.
`keyframes` and `live` comparison modes are unsupported rather than silently reduced to static.
Screenshots also do not prove physical pointer capture, OS-owned buttons/shadows, IME, pickers,
focus transitions or mixed-monitor DPI. Those need actual input and frames on the claimed setup.
Use [Native scenarios](native-scenarios.md) for behavioral integration and retain a separate
human acceptance statement for the exact controls/platform/profile actually reviewed.
