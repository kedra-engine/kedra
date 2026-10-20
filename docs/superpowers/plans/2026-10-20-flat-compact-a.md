# Flat Compact A Implementation Plan

> **For agentic workers:** Use `executing-plans` in the current worktree.
> Steps use checkboxes. No commits or publishing without owner approval.

**Goal:** Apply the owner's approved Flat compact A design throughout existing UI.

**Architecture:** Retain authored RON, Flair styles, shared property controls and
existing bounded virtual lists. Update styles and their matching layout/input
calculations together; preserve source ownership and all implemented interactions.

**Tech Stack:** Rust, Bevy 0.20, bevy_flair and the existing asset pipeline.

**Spec:** Owner's Flat compact A requirements in this conversation; Figma file
`2EZlK40cfx2Mub7GmDyI70`, Inspector `131:20213` / `131:20388`, editor `52:438`.

## Global Constraints

- Controls, tools, tree/property rows: 22px; sections/menu rows: 24px.
- Control radius: 2px. Ordinary gaps: 4px; related property rows: 2px.
- Inspector selection identity: 40px; equivalent reference property stack: 394px.
- Preserve 12/16 body text, 16px icons, 32px window/dock headers, 28px tabs,
  asset-card sizes, palette, branding and windowless startup behavior.
- Remove control bevel/highlight/inner shadows; retain popup external shadows.
- Retain hover, pressed, selected, focus, invalid, disabled and read-only behavior.
- Do not import removed drafts, A/B comparisons, experimental Scene View content
  or fake scene artwork. Existing functionality is not permission to add editing.
- No dependencies, new infrastructure, commits or overwriting unrelated work.
- `ui-kit/tokens.json` is absent from this worktree; use current bound Figma values
  for the specified roles, reporting any unavailable token source explicitly.

## Review Focus

- Nested fields remain 22px after border/padding and flex layout, never 18px.
- Virtualized scrolling, keyboard reveal and hit testing use the same row pitch.
- Narrow/Unicode/long values retain units, diagnostics and source identity.
- Focus/invalid/disabled remain distinct after removing decorative depth.
- Startup and native/dock header input preserve their existing lifecycle contracts.

## Task 1: Shared flat geometry and panel chrome

**Files:** `assets/editor/styles/`: `tokens.css`, `controls.css`, `startup.css`,
`docking.css`, `asset-browser.css`, `hierarchy.css`, `scene-view.css`;
`crates/kedra_import/tests/styles.rs`; new `styles/compact.rs`.

**Interfaces:** Existing CSS classes and custom properties; no public API changes.

- [x] Add imported-style regressions for 22px controls, 24px menu rows, 2px radius,
  unchanged header/tab/icon/card sizes, flat border and absent control shadows.
- [x] Run `cargo test -p kedra_import --test styles --locked`; expect old geometry
  assertions to fail before implementing.
- [x] Update shared tokens and explicit panel/header overrides; retain popup shadow
  and state selectors. Preserve toolbar container heights where Figma keeps 38px.
- [x] Update old geometry tests only where superseded by the approved design.
- [x] Re-run the style suite; expect all tests passing.

## Task 2: Inspector and nested controls

**Files:** `assets/editor/styles/{properties,inspector}.css`;
`crates/kedra_ui/src/fields/`; `crates/kedra_inspector/src/view.rs` only if grouping
requires structural changes; `crates/kedra_import/tests/styles/compact.rs`.

**Interfaces:** Existing PropertyRow/SectionView/ValueView and `.property-*` classes.
Related children use 2px gaps; section/group boundaries use 4px gaps.

- [x] Add real Bevy layout regressions at 320/480px for nested 22px fields, identity
  40px, sections 24px and the reference's 394px source-property stack.
- [x] Run the regressions and inspect the expected old height/shrinking failures.
- [x] Update property padding, minimum heights, nonshrinking value surfaces and
  semantic grouping without changing reflected values, paths or units.
- [x] Verify default/custom fields, collapse/search, diagnostics and narrow mode:
  `cargo test -p kedra_ui --test fields --locked` and
  `cargo test -p kedra_inspector --tests --locked`.

## Task 3: Virtual rows and integration verification

**Files:** `crates/kedra_editor/src/{hierarchy,asset_browser}/` layout/input code
and regression tests; `workspace/popup.rs` when explicit hit geometry is present;
`docs/ui/{figma,inspector,hierarchy,asset-browser,scene-view}.md` as applicable;
`tools/qa/{startup,inspector,hierarchy,asset_browser}.py` only for changed geometry.

**Interfaces:** Existing list viewport/render/reveal and actual runtime rectangles.
List row pitch changes to 22px; asset grid remains 112×96 with 120×104 pitch.

- [x] Add failing virtual scroll/reveal tests proving 22px pitch; keep grid tests.
- [x] Update list positioning, bounds, reveal and input pitch consistently.
- [x] Run `cargo test -p kedra_editor --lib --locked`; expect all tests passing.
- [x] Build a fresh delivery with existing CLI and verify all assets.
- [x] Run native Inspector fixture/editor and hierarchy/browser/startup QA with
  isolated preferences; inspect 320/480px Inspector and editor frames. Record
  unavailable physical OS interactions, not a fabricated acceptance result.
- [x] Update current design documentation without altering provenance snapshots
  or weakening visual budgets. Run Python QA unit tests and `git diff --check`.
- [x] Run `cargo xtask check`, one independent read-only review, and fix important
  findings with failing-then-passing regressions. Leave changes uncommitted.
