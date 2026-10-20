# Startup Figma Refresh Implementation Plan

> **For agentic workers:** Use `executing-plans` to implement this plan task-by-task. Checkboxes track deliverables. No commits without explicit owner approval.

**Goal:** Match the approved startup compositions and editor-wide purple palette without changing project-opening behavior.

**Architecture:** Keep authored RON scenes, shared prefabs, Flair styles and the existing importer/runtime delivery path. Preserve semantic element IDs, native window ownership and job lifecycle. Introduce no dependencies or public APIs.

**Tech Stack:** Rust, Bevy 0.20, bevy_flair, authored RON, imported SVG assets.

**Spec:** The owner-approved scope below, using Figma file `2EZlK40cfx2Mub7GmDyI70` as visual authority.

## Approved scope and constraints

### Owner amendment: fast startup without splash

This amendment supersedes the Boot/release-art implementation below. Remove the
initial native splash, its renderer-readback barrier and heavy decorative startup
dependencies. Verify delivery and load editor sources concurrently before the first
real Projects/editor surface. Keep logo/palette and truthful error handling.

Fast project preparation must enter the editor without creating or waiting for a
loading window. Show the lightweight Loading view only after 150 ms of unfinished
work, or immediately on failure; the threshold never delays the work or completion.
Preserve generation checks, validation, history and source ownership.

Use existing `serde_with` Base64 support for compact image payloads instead of
numeric RON byte arrays. Version new packages explicitly and retain old numeric
package reading. Add compact roundtrip, legacy and malformed payload tests, then
measure normal native startup/opening without QA PNG-writing overhead. Keep
lossless UI pixels; do not introduce GPU codecs or custom texture/container formats
without a separate evaluated need. Decorative illustrations are no longer startup
dependencies, so they consume no startup GPU texture memory.

- Boot `109:8369`, Opening `109:8406`, Failed `109:8445`, Projects `109:8489`, Open `109:8873`.
- Also keep Empty/Create/CreateInvalid/OpenInvalid consistent through shared sources.
- Boot and Opening remain 640×360; Projects remains 1120×720; forms remain 640×400.
- Boot/Opening use full-window release artwork, readability gradient and the approved T3 logo. Failed uses an opaque surface without release artwork.
- Projects gains the compact brand row above its existing heading/actions.
- Shared colors: accent `#7950bc`, hover `#865cc3`, pressed `#603993`, selected `#57416f`, focus `#d08bff`. Retain unchanged neutral roles.
- Apply palette throughout editor controls, selections, focus, spinner and docking preview, not just startup.
- Preserve truthful source-validation copy, actual phase text, startup log and recovery actions. Do not introduce Rust code execution, trust workflow or build claims.
- Owner confirmed ownership of the logo and source reference; record that provenance without claiming independent clearance.
- Export assets exactly from Figma. Do not use whole-screen screenshots as implementation assets.
- Keep offline checks independent of Figma. Do not change tolerances, weaken checks or add masks to hide differences.
- Work only in the isolated `startup-figma-refresh` worktree. Do not commit or publish.

## Review focus

- Failure after an Opening frame must remove artwork, show the error icon and retain recovery/log controls.
- Bootstrap must load the new images before the first-frame readiness claim, without loading the full editor catalog early.
- Long Unicode project names/paths must not obscure actions or change stored filesystem identities.
- Decorative artwork must not intercept caption drag, close or form input.
- DPR 1/2 and reduced-motion rendering must preserve exact asset proportions and readable text.

## Task 1: Shared palette and exact artwork

**Files:** `assets/editor/styles/tokens.css`, `assets/editor/styles/docking.css`, `assets/editor/icons/spinner.svg`, new artwork and native `.meta` files under `assets/editor/`, `assets/README.md`, `crates/kedra_import/tests/styles.rs` (or a focused test module following its existing pattern).

**Interfaces:** Existing CSS custom properties and `ui.image` density variants; no new public API.

- [ ] Add a failing style regression using the real token stylesheet: primary default/hover/pressed, selection and focus resolve to the five exact approved colors.
- [ ] Run `cargo test -p kedra_import --test styles --locked` and confirm the new regression fails for the old palette.
- [ ] Fetch current Figma asset exports for the approved logo, release artwork and spinner. Inspect dimensions and reuse existing exact assets where suitable; initialize sidecars with the existing importer.
- [ ] Update shared tokens and production hard-coded blue artwork/preview color. Leave unrelated generic parser-test sample values alone.
- [ ] Record owner-supplied provenance in `assets/README.md`.
- [ ] Re-run the style tests and verify imported SVG variants are nonempty and correctly sized.

## Task 2: Startup composition and failure presentation

**Files:** `assets/editor/scenes/boot.scene.ron`, `loading.scene.ron`, `projects.scene.ron`, relevant prefab/catalog files, `assets/editor/styles/startup.css`, `crates/kedra_editor/src/boot.rs`, `loading.rs`, `screens.rs`, `views.rs` only where bindings require it; tests in `crates/kedra_editor/src/startup_tests/` and boot lifecycle tests.

**Interfaces:** Retain existing `boot.*`, `loading.*`, `projects.*` and `entry.*` IDs and semantic actions. Shared decoration must cover the titlebar while preserving its input regions.

- [ ] Add failing regressions for brand/decorative source hydration, preserved text/action bindings, and transition from working artwork to opaque failure presentation with an error icon.
- [ ] Cover missing artwork dependency readiness and long Unicode project text without adding invented build/trust behavior.
- [ ] Run `cargo test -p kedra_editor --lib --locked` and record the expected new-test failures.
- [ ] Reuse the current frame/header and authored scene mechanism. Add exact logo/artwork placements and gradient from the Figma contexts; verify Flair/Bevy support before choosing representation. Do not introduce a generic rendering framework.
- [ ] Implement Boot, Opening and opaque Failed layouts; add the Projects brand row. Preserve current log, retry/back, input focus and lifecycle semantics.
- [ ] Ensure all new shared prefab dependencies belong in the minimal boot catalog where required.
- [ ] Run editor library and importer style regressions, including existing startup failure/input tests.

## Task 3: Independent references and native acceptance

**Files:** `docs/ui/startup-reference.json`, `docs/ui/startup-layout-lock.md`, `docs/ui/figma.md`, `docs/ui/references/*-visual.json`, a coherent new startup export directory, `crates/kedra_editor/tests/visual_frames.rs`, `tools/qa/startup.py` and QA tests only where changed expectations require them.

**Interfaces:** Keep existing evidence schema and comparator budgets. Figma exports supply reference pixels and coordinates; implementation observations never generate expected values.

- [ ] Export the nine current startup states independently from Figma; update source IDs, dimensions, hashes and critical geometry from the design. Preserve old provenance until all references are accounted for.
- [ ] Update the manager button coordinates by the designed brand-row displacement and move color samples to independently verified control interiors.
- [ ] Run `python3 -m unittest discover -s tools/qa -p 'test*.py'`.
- [ ] Build and verify a fresh editor asset delivery using `cargo run -p kedra -- assets build assets/editor target/startup-refresh-assets --namespace editor` and `cargo run -p kedra -- assets verify target/startup-refresh-assets`.
- [ ] Launch with isolated QA preferences and private remote credentials. Run `tools/qa/startup.py` with DPR 1/2 captures and lifecycle coverage as documented in `docs/automation.md`; run the ignored `visual_frames` test against its report.
- [ ] Inspect actual Boot/Opening/Failed/Projects/form frames against Figma, including cropping, gradient and logo proportions. Exercise caption input, modal keyboard focus, errors, log toggle and recovery. Record unavailable OS-level input separately from Bevy-injected input.
- [ ] Run `cargo xtask visual` for captured scenarios with matching evidence declarations; report truthful-copy differences rather than masking them.
- [ ] Run `cargo xtask check` on the final changes. Report exact outcomes, unverified native/platform behavior and remaining risks. Leave the changes uncommitted for owner inspection.
