# ADR 0004: Configured Rust visual comparison

Status: implemented offline contract. Scenario acceptance and platform
certification are independent.

## Decision

`kedra_test::visual` owns strict scenario/evidence policy and offline image
comparison; `xtask visual` is a thin repository CLI/diagnostic adapter.
Runtime supplies real pixels through `kedra_app`. There is no reverse runtime
dependency, screen-specific comparator or second native capture implementation.

Use the workspace `image`/`png` stack, serde/JSON, SHA-256, clap and xtask diagnostics.
`dify` 0.8.0 supplies in-memory perceptual comparison without its default image
features. The adapter converts normalized tolerance to squared YIQ units and tests
that conversion against known cases. `png` exposes color/HDR metadata needed for
explicit rejection rather than silent conversion.

## Rationale and cost

Maintained libraries own image decoding, PNG metadata and the distance metric.
Custom code is limited to Kedra's profile, provenance, budget and evidence rules.
Dify adds library dependencies including CLI/parallelism utilities; normal
license, advisory and duplicate-version checks still apply.

SSIM/aggregate metrics are not the sole gate because they can obscure a missing
small control. Raw binary snapshots compare bytes rather than rendered pixels.
Browser automation is not a native Bevy capture backend. No new HTTP service,
scheduler, image parser or separate comparison-only crate is needed.

## Contract

- Validate profile, viewport/DPR, color contract, hashes and input bounds before
  applying tolerances. Invalid input must not be replaced by a previous capture.
- Version masks, animation policy, reference provenance and global/critical budgets
  in configuration. Excluded areas form a union. Ordinary masks, including
  animation interiors, cannot overlap critical regions. Only an explicit
  `ApprovedVisualVariance` exclusion with a `HumanReviewed` reference can overlap
  them; this does not authorize adding exclusions or widening accepted budgets.
  Both absolute and integer ratio limits must pass.
- Preserve raw inputs and readable reports. Do not auto-align, resize, blur,
  suppress AA differences or update baselines during verification.
- Publish automated comparison and human design-review status separately. New
  output directories prevent accidental evidence replacement.
- Reject unsupported animation modes. Excluding an animation interior verifies
  neither motion nor its bounds.
- Independent per-profile references and explicit review are prerequisites for
  promotion. No automatic promotion tool is installed.

## Verification and limits

Unit/CLI tests cover malformed input, unsupported modes, mutation cases, critical
regions, integer budget boundaries, metadata rejection, provenance, report escaping
and immutable source inputs. Native fixtures establish actual capture/input
properties separately.

The comparator does not authenticate capture declarations, certify color or OS
presentation timing, launch applications or implement temporal sequence comparison.
See [usage and policy](../ui/visual-acceptance.md) and
[docking fixtures](../ui/docking-tests.md).
