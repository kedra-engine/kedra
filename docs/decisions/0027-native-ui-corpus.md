# ADR 0027: Production native UI corpus

Status: reviewed buttons/fields reference policy includes explicit SVG differences;
current native and hosted acceptance remain unverified.

Active visual references obey the ordinary file-size gate without storage exceptions.
Adding an oversized reference requires explicit policy review; storage approval is
not design acceptance.

The active corpus strictly compares buttons/text fields against the existing design
and requires the implementation to match it. The active catalog contains only those two
families and their 22 scenarios. Other product prefabs remain in the product;
their screenshot scenarios are outside this corpus and must be added explicitly
when ready. Independent original exports retain their source provenance; generated
capture/discrepancy evidence belongs under `target/`.
Pending Figma exports may be compared diagnostically without approving them.
Comparison continues after mismatches and reports absent captures separately.
Equal viewport dimensions alone do not prove content/state parity. Source and
actual components use the same independent fixed full-canvas origin. Original
source PNGs and the entire actual frame remain intact. No inferred alignment,
actual cropping or resizing is allowed;
fixed local regions prevent canvas padding from diluting the error budget.
The specimen host exposes technical slot geometry, content and native pseudo-state
so stale/wrong fixture setups are rejected before styling comparisons. Reference
origins come from the independent contract, never observed widget positions.
Partial evidence cannot establish complete native acceptance. Product
changes and reference promotion require owner decisions.

## Decision

Keep one `components` screenshot path linking scoped production prefab families
and reproducible states to canonical Figma node identities. Use an ordinary Bevy
host and the existing `kedra_app` input/observation/capture transport. Source export
is separate explicit authoring, not another capture/acceptance pipeline. Do not
retain parallel pilot, capture/audit/replay or experimental font/compositing paths.

Keep one ordinary source project with stable asset sidecars for manual use and
feature scenarios, outside the component screenshot path. Working-copy ownership
and data scope are defined by the [project contract](../../tests/fixtures/test_project/README.md),
not a parallel generator.

Keep three distinct authorities: independently exported Figma images, captured
application evidence, and separately owner-reviewed per-profile regression
goldens. Capture cannot promote its output into either reference authority.
Design-audit side-by-sides disclose incompatible data/viewports instead of
manufacturing numeric design equivalence.

Shared Rust tooling in `kedra_test` owns native sessions, evidence and comparison
under [ADR 0028](0028-shared-native-test.md); `xtask visual` is a thin adapter.
Image/PNG and dify own decoding and perceptual comparison. The orchestration does
not implement an image decoder, native event driver or generic test framework.
QA scripts, fixture hosts and reports do not have separate mock/unit self-tests:
verify the workflow by executing real product scenarios with input and frames.
Product Rust regression and integration tests and comparison guards remain required.
The shared Rust tooling crate does not add runtime-to-tooling/editor edges.

## Alternatives

- Only full-window comparisons: small control regressions can disappear in a large
  error budget, and failures are difficult to localize. Add component/panel states
  and fixed independently reviewed critical regions instead.
- Only isolated component fixtures: cannot certify panel integration or full-window
  layout. Those are explicitly outside this corpus; retain separate real-input
  feature drivers and the richer independent docking gesture corpus.
- Live Figma access in CI: introduces private authentication and mutable external
  state. Pin exports/hashes offline and review their changes explicitly.
- Browser/Storybook/Playwright rendering: useful prior art for a state catalog, but
  cannot stand in for this native Bevy UI/renderer. Reuse the existing native path.
- Accept the first application screenshot automatically: creates a regression
  baseline without establishing that the original behavior/design was correct.
  Generate pending candidates and fail verification until review instead.

## Consequences

The owner-authorized first shared rollout covers the current buttons and text
field only. One Plugin API exporter produces original/derived images and structured
typography from temporary clones; one native adapter handles Text and EditableText.
Family property-to-source-node bindings supply paint and typography values.
A shared reviewed font policy accepts rasterization for its
pinned profile while exact size/color and independent layout/shape checks remain.
It also binds the scoped source/binding contract. Positioned glyph inventory must
agree with native shaping and cover the text; a positive glyph count alone is not
sufficient. Two persistent application sessions cover the selected component
sequence with per-case input/focus/paint reset, rather than cold-starting every case.
No owner decision, source refresh or diagnostic image match automatically approves
the whole corpus. Panels and windows are outside the active catalog and acceptance.
Uniform glyph-free diagnostic frames require an explicit comparator rationale plus
nonuniform full frames, retained layout and exact restoration; normal capture's
uniform-frame guard remains unchanged.

The owner accepts the current native SVG quality without changing compositing.
An explicit policy records only the reviewed source/native shape-image hash pairs,
bound to the existing source contract, dependency lock and renderer profile. The
raw comparator verdict remains a mismatch; the case is separately labeled
`accepted_known_difference`. Changed pairs need fresh review, not a wider budget.
No capture, typography, layout, stability or restoration failure can be overridden.
This limited decision does not accept SVG rasterization generally or other profiles.

The complete sequence runs in two independent processes with fixed logical size,
DPR, UI-motion policy, content and bounded expected-state/readback readiness.
Unexpected uniform full frames, partial, unstable or wrong-profile evidence cannot
pass. Verification rechecks both runs and never chooses another baseline after a mismatch.

The standalone component host owns static presentation without modifying normal
editor behavior or clocks. It is not proof of physical OS input. Native window
exposure may take foreground focus and needs an idle desktop/window manager.

The initial Linux/Xvfb/Openbox/Mesa workflow is dispatched, not a new required PR
gate. A successful actual hosted run, profile stability, independently reviewed
policy and useful failure reports are prerequisites to promotion. Unknown profiles
fail closed without a capture-only fallback. OS-owned chrome, normal animation and
additional platforms retain separate acceptance obligations.

See [the corpus contract](../site/en/contributing/contracts/ui/ui-corpus.md)
for commands, coverage, reference
schema, provenance, review procedure and current limitations.
