# ADR 0028: Shared native test tooling and project-owned Rust scenarios

Status: shared tooling and project adapters implemented in source; compilation
and native acceptance of the current composition remain unverified.

## Decision

Native QA tooling belongs to reusable tooling-layer `kedra_test`. Its consumers
are the editor's button/field Rust scenarios and the canonical runtime-only game
in `tests/fixtures/test_project`, without an editor dependency.
xtask and the project-local Kedra launcher are thin adapters.

[ADR 0029](0029-luau-runtime-evaluation.md) adds shared runtime-side Luau sequences
for native actions and editor-owned workflows. Rust scenarios remain the external
supervisor and evidence authority; they may delegate multi-step actions to finite
host-registered programs. Language execution lives in the runtime, not this tooling
crate. No arbitrary remote eval or custom scenario DSL replaces ordinary programs.

Project-based native editor scenarios use one checked ordinary source project at
`tests/fixtures/test_project`. Each scenario opens a fresh complete working copy;
source and asset identities are preserved, not generated at startup. Matching
derived delivery is built once per run from a clean copy. Deliberate source-failure
mutations affect only the scenario copy. Startup Create remains an explicit product
operation under test, not data generation. Component-only hosts remain independent
consumers, without introducing another project fixture. See the
[project contract](../../tests/fixtures/test_project/README.md).

The shared tool owns Cargo artifact discovery, bounded process/session ownership,
authenticated transport, stable frames, two-session evidence, comparison, accepted
differences and reports. Reuse the existing visual comparator. Maintained Cargo
metadata and ureq own Cargo parsing and HTTP; std owns processes. These libraries
already exist in the tooling graph; do not rebuild their parsers or transports.

Projects own ordinary Rust scenario functions, launch arguments, readiness,
feature setup and graceful shutdown. JSON contains data/reference identities,
not executable scenarios. There is no plugin registry, scheduler, parallel ECS or
mandatory Figma/text dependency. Full-frame scenarios use the same path without
editor prefabs. Optional authored-UI measurements live in `kedra_ui`, not an editor
example or runtime-to-tooling dependency. Applications opt into existing `kedra_app`
observation/input/capture. Shipping apps depend on neither QA nor editor tooling.

An isolated component host is an optional project-specific consumer, not required
infrastructure. An ordinary game may drive its own application from a Cargo example
with a QA dev-dependency; neither a separate test application nor a copied editor
stand is required. Shared input/capture and optional UI measurement remain engine
capabilities available to every opting-in project.

No reflection mutation or arbitrary remote execution is exposed. Bevy input is
not physical OS input certification. Credentials/private logs never enter public
artifacts. Failed shutdown fails the suite.

`cargo kedra test` builds and executes a project's Rust scenario target.
It accepts a bin or an ordinary example target.
It does not supply missing scenarios or turn data files into programs. The editor
xtask delegates to the same launcher. Source authoring is explicit and separate;
the Figma Plugin API exporter remains JavaScript inside Figma, without Python/Node
requirements for capture or CI.

## Acceptance and limitations

Preserve original sources/reports, fixed regions, typography, exact restoration
and reviewed SVG pairs. Do not widen budgets or infer new renderer acceptance.
Distinguish tooling-only lock changes from runtime dependency changes; reviewed
source/native identities and the exact lock/profile remain acceptance inputs.

Verify all 22 editor cases in two sessions and a real independent app from outside
the editor workspace, with actual input/frames and no copied repository scripts.
Do not add QA-driver/report self-tests or mocks. Existing product Rust and visual
comparator checks retain their roles.

Hosted Linux needs explicit policy review and a real native run. Local success is
not platform certification or authorization to commit, push, publish or promote
references automatically.

Screenshot coverage is restricted to kit buttons and text inputs: 22 cases.
Other editor scenarios and the independent consumer verify behavior through real
input and transient GPU fences, without PNG galleries, image references or pixel
comparisons. See [testing](../testing.md) for the current commands and boundaries.
