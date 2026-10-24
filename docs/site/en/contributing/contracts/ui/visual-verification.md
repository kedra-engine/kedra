# Visual comparison contracts

The offline comparator validates captures and applies a reviewed comparison policy. For commands
and reports, see [Visual verification](../../../manual/testing/visual-verification.md).
[ADR 0004](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0004-visual-regression.md)
defines this offline contract; it does not certify scenarios or platforms. Only the 22 button/input
cases in [UI corpus](ui-corpus.md) are active; generic comparator features do not enroll other UI.

## Engine and result contract

`cargo xtask visual` uses `kedra_test`: `image`/`png` decode/validate metadata; dify compares
perceptually; versioned JSON supplies policy. No Python/Pillow, AI or Figma connection is required.
It compares already-published actual JSON/PNG, without launching/capturing. Output must be new.
Exit 0 means automated match, not design acceptance; exit 1 means mismatch/blocked comparison.
`report.json` distinguishes `matched`, `mismatch`, `invalid_config`, `invalid_capture`,
`missing_reference`, `invalid_reference`, `unsupported` and `infrastructure_failure`.
Output-directory creation failure cannot publish a report.

Passive `index.html` retains original-scale side-by-side images, 50% overlay, perceptual/exact
differences and magenta exclusions. `reference-source.png`/`actual-source.png` preserve original
bytes. Numeric region crops, effective policy, sanitized capture declarations/results accompany
them; available raw inputs survive validation failure.

## Configuration and integer budgets

- `scenario`/`fixture` identify state and deterministic data. `profile` pins platform, renderer,
  environment, logical viewport, DPR, motion and color assumption. Actual declarations match
  exactly, without fallback. Source revision belongs in producer provenance.
- `reference` pins source, SHA-256, export size, physical **xywh** crop/reason, kind
  (`design_reference`/`regression_golden`) and review status. A generic reference canvas can place
  its crop at an independently fixed physical origin on an opaque background, without scaling.
  Actual frames are never resized, aligned, blurred or cropped to fit.
- Image paths are relative `.png` under the manifest directory. Traversal/symlink escape, unknown
  fields and duplicate fields fail. Profiles require finite DPR `0.5..=4`, integral physical
  dimensions `1..=8192` and at most `8_388_608` pixels. Decoder bytes/allocations are bounded.
- `normalized_yiq_tolerance` is finite `0..1`; dify 0.8.0 receives `35215 × tolerance²`.
  AA suppression is disabled.
- Both `max_changed_pixels` and `max_changed_ppm` must pass, using integers:
  `changed <= max_changed_pixels` and `changed * 1_000_000 <= eligible * max_changed_ppm`.
  `10_000` PPM is 1%; rounded displayed percentages never determine acceptance.
- Critical regions have independent budgets so small missing controls cannot hide in a globally
  passing frame. Exact diagnostic diffs expose masked changes too.
- Transparent client images fail. Uniform images need a specific `allow_uniform_reason`.

## Exclusions

Exclusions are named logical **xywh** rectangles with reason/kind: `volatile_data`,
`animation_interior` or `approved_visual_variance`. Bounds round outward at pinned DPR.
Volatility/animation cannot overlap critical regions. Approved variance requires a
`human_reviewed` reference, may cover reviewed critical interiors, and leaves remaining pixels
subject to their unchanged region budgets. Fully excluded domains fail.

Exception geometry must be fixed/reviewed, never inferred from a failed diff or moved with actual
bounds. `max_excluded_ppm` caps the union; overlaps count once and that union is removed from the
denominator. An empty domain fails. Exclusions do not bless images. The active kit uses **no
exclusion masks**, fixed origins, 0.05 tolerance, 2% local shape budgets, strict typography/layout
and only four exact reviewed SVG pairs; see
[UI corpus policy](ui-corpus.md#strict-typography-and-fixed-regions).
Startup/docking source PNG hashes remain provenance, not active policies. Comparator regression
tests reject missing local content and translated images.

## Actual evidence and readiness

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

Wait for expected state, assets, measured layout, capture receipt and file publication, not repeated
comparisons until pixels match. Stable blank output is not readiness. Receipt frame is neither
source-frame time nor OS presentation timestamp. The comparator checks declarations/pixels, not
capture authenticity/environment state. Keep credentials and sensitive personal paths out of
free-text provenance.

## Color, motion and acceptance boundaries

Only static 8-bit SDR PNG is supported. `srgb_assumed` interprets untagged bytes as sRGB, without
display certification. ICC, cICP/HDR, unqualified chromaticities, non-sRGB gamma and APNG fail
instead of conversion. `static` compares one frame; reduced motion is a separate profile.
`excluded_interiors` requires reasoned animation-interior exclusions, not evidence of duration,
movement or spinner bounds. Live bounds/input/motion require separate checks.

`keyframes`/`live`, temporal/source-frame correlation, native capture orchestration and baseline
promotion are unsupported by this offline comparator. Future temporal fixtures should use Bevy
clocks/test mechanisms, not a parallel scheduler. App-painted captions/controls/borders/corners
belong in strict comparisons; disclosed accepted variance is not identity. OS shadows need another
capture lane. Screenshots cannot certify move/resize/minimize/maximize/close, picker, IME, modal
focus or cross-monitor behavior: real input/frames are required on the claimed platform.
Verification never updates references. New sources, masks, thresholds, comparator versions and
promotions need explicit review. Automated result, accepted variance and platform certification
are distinct.
