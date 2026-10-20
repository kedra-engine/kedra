# Native visual comparison

`cargo xtask visual` compares actual native captures with independently pinned
references. Rust `image`/`png` owns decoding and metadata; `dify` owns perceptual
comparison. Scenario policy is versioned JSON. Python/Pillow, an AI service and
Figma access are not required by the offline comparator.

See [ADR 0004](../decisions/0004-visual-regression.md),
[startup references](startup-layout-lock.md) and [docking tests](docking-tests.md).

## Run a comparison

```sh
cargo xtask visual --config docs/ui/references/loading-locked-visual.json \
  --actual target/qa/loading.evidence.json --output target/qa/loading-review
```

The actual-evidence JSON and its PNG must already exist and match the selected
policy. This command does not launch an application or create a capture. The
output directory must be new. Use [application automation](../automation.md) for
renderer readbacks and the isolated docking runner for a complete native corpus.

Exit 0 means an automated match, not design acceptance. Exit 1 indicates a mismatch
or blocked comparison. `report.json` distinguishes `matched`, `mismatch`,
`invalid_config`, `invalid_capture`, `missing_reference`, `invalid_reference`,
`unsupported` and `infrastructure_failure`. Failed output-directory creation
cannot publish a report.

`index.html` is a passive local report with original-scale side-by-side images,
50% overlay, perceptual/exact differences and magenta exclusions. Original bytes
remain in `reference-source.png` and `actual-source.png`. Numeric region crops,
effective policy, sanitized capture declarations and comparison results are kept
alongside them. Available raw inputs survive validation failures.

## Configuration contract

- `scenario` and `fixture` identify the state and deterministic input data.
- `profile` identifies platform, renderer, environment, logical viewport, DPR,
  motion and color-space assumption. Actual declarations must match exactly;
  there is no fallback baseline. Source revision belongs in producer provenance.
- `reference` pins source, image SHA-256, export size, optional crop/reason,
  `design_reference` or `regression_golden` kind, and review status.
- Image paths are relative `.png` files under their manifest directory. Traversal,
  symlink escape, unknown fields and duplicate fields are rejected.
- Crop coordinates are physical **xywh**. Actual captures are never resized,
  aligned, blurred or cropped to match a reference.
- `normalized_yiq_tolerance` is finite in `0..1`; dify 0.8.0 receives
  `35215 × tolerance²`. AA suppression is disabled.
- Every budget requires both `max_changed_pixels` and `max_changed_ppm`.
  `10_000` PPM is 1%. Both integer comparisons must pass; rounded display
  percentages do not determine acceptance.
- Exclusions are named logical **xywh** rectangles with a reason and kind
  (`volatile_data`, `animation_interior` or `approved_visual_variance`). Bounds
  round outward at the pinned DPR. Volatility/animation cannot cover critical
  regions. Explicit `approved_visual_variance` requires `human_reviewed` reference
  acceptance and may exclude reviewed critical interiors; remaining pixels still
  pass the region budget and a fully excluded domain fails. It is not an automatic
  bless operation. Exception geometry must be fixed and explicitly reviewed, never
  inferred from a failing diff or moved to follow actual bounds.
- `max_excluded_ppm` caps the union of excluded areas. Overlaps count once and
  the comparison denominator excludes that union. An empty domain fails.
- Critical regions have independent budgets, so a missing small control cannot
  be hidden by a globally passing frame. Exact diagnostic images expose masked
  differences as well.
- Transparent client images fail. Uniform images require a specific
  `allow_uniform_reason`. PNG dimensions, bytes and decoder allocations are bounded.

Current startup and docking audit policies use tolerance **0.05**, a **2%**
perceptual budget for full frame and each critical region, and no exclusions.
The separate accepted docking mode adds owner-reviewed text-interior exclusions;
see [docking policy](docking-tests.md). Its tolerance and budgets are unchanged.
These thresholds are reviewable scenario policy, not a universal quality metric.
Mutation tests verify rejection of missing local content and translated images.

Only static 8-bit SDR PNG is supported. `srgb_assumed` interprets untagged bytes
as sRGB; it does not certify a display. ICC, cICP/HDR, unqualified chromaticities,
non-sRGB gamma and animated PNG are rejected rather than silently converted.

## Actual evidence

```json
{
  "schema": 1,
  "scenario": "the-configured-scenario",
  "fixture": "the-configured-fixture",
  "profile": {"id": "runner", "platform": "macos", "renderer": "metal",
    "environment": "controlled-runner", "logical_size": [640, 360], "dpr": 1,
    "color_space": "srgb_assumed", "motion": "reduced"},
  "image": "captures/actual.png",
  "sha256": "the-64-character-sha256-of-that-file",
  "producer": "Bevy app.capture",
  "readiness": "Expected application state, resources, layout and completed readback",
  "receipt_frame": 30
}
```

Readiness depends on application state, assets and measured layout before capture,
not repeated comparisons until pixels happen to match. A stable blank screen is
not ready. Wait for the real capture receipt and file publication. The receipt
frame is not source-frame time or an OS presentation timestamp. The comparator
validates declarations and pixels, not capture authenticity or environment state.
Free-text provenance must not contain credentials or sensitive personal paths.

## Motion and platform boundaries

`animation.mode = static` compares one frame. Reduced motion is a separate profile,
not evidence of normal motion. `excluded_interiors` requires explicitly reasoned
animation-interior exclusions and provides no evidence of duration, movement or
spinner bounds. Bounds and live input/motion require separate checks.

`keyframes` and `live` are unsupported. Temporal comparison, source-frame
correlation, native capture orchestration and baseline-promotion tooling are not
implemented by the offline comparator. Use Bevy's existing clocks/test mechanisms
for future temporal fixtures, not a parallel scheduler.

App-painted captions, controls, borders and corners belong in strict comparisons;
explicit accepted-variance scopes are disclosed separately, not called identity.
OS-owned shadows require a separate capture lane. Native screenshots do not prove
move/resize/minimize/maximize/close, picker, IME, modal focus or cross-monitor
behavior. These require real input and frames on the claimed platform.

Verification never updates a reference. New references, masks, thresholds,
comparator versions and promotions require explicit review. Automated status,
accepted visual variance and native platform certification remain distinct.
