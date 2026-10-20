# Read-only extensible Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or
> superpowers:subagent-driven-development to implement this plan task-by-task.
> Steps use checkbox syntax for tracking. Do not commit without explicit owner approval.

**Goal:** Deliver the source-bound read-only Inspector on reusable native UI controls,
with real default/type/field presentation composition and independent consumers.

**Architecture:** `kedra_ui::fields` owns shared controls; `kedra_inspector` owns
reflected presentation and dispatch; `kedra_editor::inspector` adapts source targets
and docking. Immutable source compilation, not Scene View, supplies typed values.

**Tech Stack:** Rust 2024, pinned Bevy 0.20.0, Flair 0.9.0, authored RON UI and CSS.

**Spec:** [Inspector foundation design](../../ui/inspector.md), refining ADRs 0009,
0011, 0012 and 0015 for the approved read-only delivery.

## Global Constraints

- Runtime cannot depend on editor, inspector or authoring packages.
- Inspector cannot depend on authoring, desktop, editor or a filesystem writer.
- Use Bevy reflection and paths; no alternate schema, registry or path parser.
- No source edits, source history commands, Save, pickers or project execution.
- Shared controls have independent consumers, not inspector-only implementations.
- Retain native UI roots on unchanged refresh and host movement.
- Preserve Unicode and exact numeric types; display only project-relative paths.
- Every authored text file stays within 1000 lines and 64 KiB.
- New dependencies/public APIs require a reuse evaluation and justification.
- No commits, publishing or reference/budget weakening without owner approval.

## Review Focus

- A field overridden inside a nested prefab must show the effective value and provenance.
- A detached/locked source target must never follow a reused display name or renderer ID.
- Custom recursive editors must share limits and retain a reachable default presentation.
- Refresh/docking must preserve native copy selection and view-local navigation.
- Large Unicode strings/collections and exact large integers must not hang or lose data.

## File responsibilities and planned interfaces

- `crates/kedra_ui/src/fields/{mod,access,row,value,section}.rs`: generic retained UI.
  `FieldAccess::ReadOnly { reason: String }`, `FieldTemplates` (explicit compiled
  authored templates), `PropertyRow` and `SectionView` own their mounted roots.
  `PropertyRow::mount(world, parent, templates, scope)` returns a fallible retained
  row; `update(world, label, display, access, diagnostic)` updates it in place.
  Values passed as display text are presentation output, never a shared value schema.
- `assets/editor/prefabs/property-{row,section,value}.prefab.ron` and
  `assets/editor/styles/properties.css`: canonical shared-kit presentation assets.
  Feature CSS only arranges these primitives; it does not duplicate their states.
- `crates/kedra_inspector/src/{binding,discovery,editors,view}.rs`: immutable
  `ValueBinding<'a>` referencing `&'a dyn PartialReflect`, opaque `BindingToken`,
  native represented type/path and access metadata; bounded discovery and retained UI.
  `EditorRegistry::register_type(TypeId, Arc<dyn ValueEditor>)` and
  `register_field(TypeId, ParsedPath, Arc<dyn ValueEditor>)` return conflicts.
  `resolve(containing_type, path, value_type, mode)` returns deterministic dispatch.
  `ValueEditor` mounts an owned `ValueEditorView`; its fallible update receives the
  current immutable binding and shared composition context. Retirement releases
  its UI subtree. Inspect the actual pinned `ParsedPath` traits before choosing
  registry storage; do not implement a replacement parser to make it orderable.
- `crates/kedra_inspector/examples/value_fixture.rs` and integration tests: real
  independent consumers of generic and custom presentation, no editor/authoring.
- `crates/kedra_editor/src/documents/inspection.rs`: immutable source-target adapter.
  `Documents::inspect(document, node, registry, policies)` returns a source-owned
  inspection snapshot or a typed diagnostic. Keep source tokens editor-owned and
  pass only opaque tokens and neutral metadata to the inspector.
- `crates/kedra_editor/src/inspector/{mod,model,view,input}.rs`: panel target/lifecycle,
  source context and feature actions, following the existing Hierarchy pattern.
- `docs/ui/inspector.md`: contract and truthful implemented/unsupported status.

Exact Rust argument reference lifetimes and error types are finalized by the owning
task's compiling consumer tests; preserve the above ownership and API responsibilities.
Do not create modules or APIs unused by the delivery.

## Task 1: Reusable read-only property UI

**Files:** Create the `kedra_ui::fields` files and property assets above; modify
`crates/kedra_ui/src/lib.rs`, `src/authored/style.rs`, relevant existing native-mode
handling, `assets/editor/scenes/startup.prefabs.ron` and shared stylesheet imports.
Add `crates/kedra_ui/tests/fields.rs` and an independent native field example.

**Interfaces:** Produces `FieldTemplates`, `FieldAccess`, `PropertyRow`, `SectionView`.
Templates come from the host; the crate knows no editor UUIDs or global editor state.

- [ ] Test read-only native selection/copy mode, disabled versus read-only distinction,
  in-place update, label/diagnostic ownership and missing-template cleanup.
- [ ] Run `cargo test -p kedra_ui --test fields --locked`; verify new tests fail first.
- [ ] Inspect existing authored hydration and native-mode writer; add one mode authority
  shared by existing controls and the new read-only surface. Do not alter startup
  forms' acceptance or claim the complete editable TextField from ADR 0010.
- [ ] Implement generic row, section and value surfaces using authored templates,
  native widgets and existing CSS tokens. Keep semantic IDs scoped per mounted view.
- [ ] Add tests for two independent hosts, unchanged-update entity identity, narrow
  stacked layout, long Unicode and retirement without deleting sibling controls.
- [ ] Rerun tests and existing authored/native-input regressions. Exercise the native
  field consumer; record actual keyboard, selection, clipboard and frame limitations.

## Task 2: Generic immutable reflected inspection

**Files:** Create `crates/kedra_inspector/Cargo.toml`, `src/lib.rs`, binding/discovery/view
modules, `tests/reflected_values.rs`, the value fixture and required root manifest entry.

**Interfaces:** Consumes Task 1 controls; produces `ValueBinding`, `BindingToken`,
bounded discovery and a default retained value view. The fixture is the first real
consumer and justifies the editor-layer crate and its existing-dependency edges.

- [ ] Write fixture tests for nested strings/numbers/bools, structs, tuples, current
  enum payloads, arrays/lists/maps/sets, opaque and unregistered capabilities.
  Assert that adding a new reflected component requires no component-specific drawer.
- [ ] Run `cargo test -p kedra_inspector --test reflected_values --locked`; confirm failure.
- [ ] Implement discovery from native represented type/value APIs. Reuse Bevy paths
  and maintained reflection traversal facilities where suitable; keep custom work
  limited to bounded presentation/expansion rather than a generic reflection walker.
- [ ] Enforce depth 32, 128 collection entries/page, 2048 discovery rows/rebuild and
  4096 scalars/preview; test exact boundaries, visible limits and shared search budget.
- [ ] Test `u64::MAX`, `i128::MIN`, high-precision floats, empty strings, Unicode and
  unsupported leaves without universal numeric coercion or misleading editable text.
- [ ] Run package tests and `cargo xtask architecture --non-interactive`; document why
  existing UI/reflection mechanisms are sufficient without another inspector library.

## Task 3: Real custom-editor composition

**Files:** Add `src/editors.rs`, split coherent dispatch/view responsibilities if needed;
add `tests/editor_dispatch.rs` and extend `examples/value_fixture.rs`.

**Interfaces:** Consumes Task 2 immutable bindings and Task 1 controls; produces
`EditorRegistry`, `ValueEditor`, `ValueEditorView` and explicit default-subtree mode.
The callback context carries templates, recursive composition and the shared budget.

- [ ] Test `field > exact type > structural default > unsupported`; duplicate keys
  must fail for both registration orders and never replace an existing registration.
- [ ] Run `cargo test -p kedra_inspector --test editor_dispatch --locked`; confirm failure.
- [ ] Implement dispatch using native exact type/path identities and explicit context,
  not labels, namespace prefixes or registration order. Bind errors to visible rows.
- [ ] Implement a fixture exact-type presentation and a nested field presentation
  using the same reusable controls. Do not replace native Transform/Node schemas.
- [ ] Test default-subtree mode reveals every reflected field; nested custom editors,
  callback failure, type changes and recursive calls cannot hide errors or bypass limits.
- [ ] Test unchanged refresh retains custom roots/native selection; changed dispatch
  retires only its owned subtree. Run all inspector tests and the independent fixture.

## Task 4: Source-bound inspection without viewport coupling

**Files:** Add `crates/kedra_editor/src/documents/inspection.rs` and adapter tests;
modify `documents/mod.rs` and `documents/source.rs` only for source inspection access;
add the inspector dependency to `crates/kedra_editor/Cargo.toml`.

**Interfaces:** Consumes generic bindings and existing workspace/compiler APIs;
produces immutable snapshots with authored component keys, effective reflected values,
source origin, exact source identity and read-only reasons. Lifetime ownership stays
with the source adapter; generic inspector has no `Workspace` or `NodeAddress` dependency.

- [ ] Test source-node inspection while renderer resources are absent or preview fails;
  include plain registered application-style components and inert `UiLayout(Node)`.
- [ ] Run `cargo test -p kedra_editor documents::inspection --locked`; confirm failure.
- [ ] Reuse admitted `Workspace` compilation and native `ReflectComponent` access.
  Cache by exact source identity/revision; no per-frame compilation or RON decoding.
  If compilation is not retained, retain the existing admission result, not a second workspace.
- [ ] Test nested inherited components and layered field overrides; report origin per
  effective field, not just per component. Exclude compiler-generated components.
- [ ] Test entity/asset references and unbound optional ports retain symbolic context
  or an explicit limitation. Reuse compiler/reference APIs; review any required narrow
  extension with tests instead of adding an inspector-owned serialization path.
- [ ] Test stale scope/node/type tokens reject access without retargeting; assert
  source bytes, revisions, history and Save baseline remain unchanged in all cases.

## Task 5: Retained dockable panel and shared actions

**Files:** Create the editor inspector feature modules and tests; modify `src/lib.rs`,
`workspace/panels.rs`, `workspace/input.rs`, `workspace/dock_menu.rs`, existing feature
action routing and panel tests as required. Add `assets/editor/prefabs/inspector.prefab.ron`
and `assets/editor/styles/inspector.css`, using the shared property primitives.

**Interfaces:** Consumes source inspection snapshots and generic retained views;
produces a `Panel` with the same mount/root/tick lifecycle shape as Hierarchy, plus
view-local follow/lock, expansion, search and default-view actions.

- [ ] Test no document, no selection, empty components, unavailable target and active
  document following. Browser selection/focus must not replace the target.
- [ ] Run `cargo test -p kedra_editor inspector --locked`; confirm failure.
- [ ] Replace only the inspector empty slot. Reuse the existing docking view, hide,
  reopen, focus, scrollbar and canonical menu/action families; no second panel framework.
- [ ] Add lock with source-generation/document/node identity, search with ancestor
  context and explicit default-view access. Keep selection identity visible during search.
- [ ] Test lock refresh, scope retirement, missing target, same-name replacement,
  no-result search and default/custom view switching. No unavailable Add/Reset actions.
- [ ] Test identical-target tick and float/redock/hide/reopen preserve content roots,
  expansion, search, scroll and native selection. Existing Hierarchy/Scene View tests pass.

## Task 6: Visual and native acceptance

**Files:** Add reviewed UI references/manifests under `docs/ui/references/`, native
scenarios under `tools/qa/` following existing conventions, and fixture evidence hooks.
Update `docs/ui/figma.md` with exact new asset provenance, not guessed icons.

**Interfaces:** Exercises production controls and the actual editor panel, not substitutes.

- [ ] Inspect canonical Figma property primitives and obtain high-fidelity design context
  before UI implementation. Export exact missing icons and independent reference images.
- [ ] Review missing no-selection/unavailable/error/limit/narrow states with the owner
  in Figma before shipping them; reuse existing canonical components, not duplicate variants.
- [ ] Capture the standalone consumer and real editor at specimen/narrow widths and
  a floating host; compare native geometry, typography, icons and clipping to Figma.
- [ ] Exercise physical keyboard/pointer selection, copying where supported, disclosure,
  search, lock, default view, scrolling and float/redock/hide/reopen. Label injected-only,
  missing OS clipboard and inaccessible hardware cases explicitly as not verified.
- [ ] Run deterministic QA regressions:
  `python3 -m unittest discover -s tools/qa -p 'test*.py'`.
  Store generated frames/logs under `target/`, never regenerate expected pixels from the app.

## Task 7: Final verification and truthful contracts

**Files:** Update `docs/ui/inspector.md`, `docs/architecture.md`, `docs/roadmap.md`,
`docs/startup.md` and `README.md` only where implementation changes their stated facts.

- [ ] Run all new package tests, existing scene/authoring tests, UI/input regressions
  and editor integration tests on the final dirty revision.
- [ ] Run `cargo xtask check --non-interactive` and `cargo xtask features --non-interactive`.
  Add negative production-edge assertions for inspector-to-authoring/editor/desktop
  and UI-to-inspector dependencies in the existing architecture test mechanism.
- [ ] Run Markdown/link checks for changed docs and dependency/license checks if the
  resolved graph changes; fix failures without relaxing gates or unrelated formatting.
- [ ] Check the complete diff for accidental source writes, duplicated kit controls,
  speculative APIs and unbounded discovery. Reconcile docs with actual supported shapes.
- [ ] Report changed files, exact check outcomes, native evidence and remaining risks.
  Request owner inspection; do not commit or publish until separately authorized.
