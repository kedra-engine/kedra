# ADR 0016: Document Preview dockable views

Status: proposed for owner review. This specifies independent UI scene/prefab
preview for own-source authoring, not Play, a 3D viewport or live shell editing.
The implemented [read-only Scene View](0017-read-only-scene-view.md) provides
3D camera navigation and viewport selection alongside the read-only Inspector. This
ADR retains the passive UI-preview contract; its no-selection rule is not the
Scene View input policy.

## Responsibility

Each Document Preview view binds to one admitted document session and owns a
disposable native projection of an identified source/candidate revision. The
document coordinator owns sources and activation; the preview owns its mounted
instance, render target, camera/surface association and readiness diagnostics.
It never supplies serialized data to Save or stores a second mutable document.

Keep a separate editor preview feature module with an ordinary lifecycle model
and native presentation adapter. Use existing scene compilation/instantiation,
authored UI hydration, Bevy rendering and desktop host services. Do not create
another layout engine, preview runtime or rendering/window framework.

## Docking contract

A document preview is a document-bound feature view in the existing dockable
workspace, not a permanently central canvas. Each view has a docking `ViewId`
separate from its document ID and native preview entities. It can tab, split,
maximize, float and redock through the same docking commands/chrome as tools.

Activating its tab requests activation of its document after field settlement.
Other preview tabs stay bound to their own documents. Reopening a document reveals
its existing default view instead of duplicating source authority. Additional
views of the same document remain deferred, not forbidden by the model.

Closing the view uses the [document close contract](0012-document-sessions-and-selection.md),
not tool Hide. Closing one preview never implicitly saves/discards sources.
Floating-host Close must obtain document approval before topology removal; the
current generic tool-only HideHost path is not sufficient.

Dock transfer preserves the view's identity and local preview state. If native
camera/render-target ownership requires acknowledged reconfiguration, preserve
the old valid projection until the staged transfer succeeds. Do not equate a
retained panel root with proof that its rendered preview survived a host change.

## Source projection and readiness

Show accepted source revisions through the existing immutable compilation path.
For continuous field interactions, ADR 0009 permits an independently validated
candidate preview with an explicit candidate/session token. Candidate display
does not change document dirty, history or Save input. Cancel returns to the
current accepted revision, not an obsolete captured baseline.

Track source-valid, dependencies-ready, hydrated and frame-ready independently.
The panel distinguishes pending, current, stale and failed projection. Older
pixels may remain only with a stale indication; never label them as current.
Source edits can succeed even when rendering fails. Keep accepted document/history
and report the preview failure rather than rolling sources back silently.

Every asynchronous preparation carries source-scope/document/view generations and
candidate sequence. Retire old cameras/instances only after ownership transfer or
replacement succeeds; stale completions cannot replace a newer projection.
Failure cleanup despawns only preview-owned graphs and resources, not serving
editor UI, external bindings or another view's instances.

## Own-source safety and initial interactions

Editor UI sources render as a separate instance. Do not attach live editor action
handlers, filesystem operations, automation observation scopes or project scripts
to preview controls. Its text fields/buttons cannot take editing focus or dispatch
commands merely because their serialized widget data is present.

Initially preview is observational: no text entry, button activation, gameplay or
direct node selection. Hierarchy supplies selection; an optional noninteractive
selection overlay may use source-to-instance mappings once separately verified.
Interactive UI testing and click-to-select are distinct modes needing their own
input-ownership contract. Do not accidentally implement them through normal picking.

Choose a document preview size/density independently of panel dimensions. Preserve
that chosen viewport through docking so moving a panel does not edit authored
layout or pretend responsive layout changes are source changes. Zoom/pan/fit and
density controls need a focused interaction design before exposing them. Bevy
layout remains authoritative; CSS-derived geometry is not written back into Node.

Save changes source files only. Testing the updated serving shell requires a new
asset build/verify and an explicit subsequent launch. No restart-only rebuild or
hot application to the running shell is promised.

## Design and verification

The [UI-kit](../site/en/contributing/contracts/ui/design-provenance.md)
has document docking and a 3D viewport illustration;
that illustration is not a specification for this UI-document preview. Review an
actual own-source preview composition, revision/readiness indicators, passive input
boundary and narrow/floating behavior before implementing its presentation.

Test generation retirement, validated candidate cancellation, source acceptance
with render failure, dependency loss, repeated Open, isolated ownership cleanup
and dock/native-host transfer. Assert the serving shell remains usable and preview
controls cannot execute commands or capture text focus. Native input/frame evidence
is required; model/RPC readiness is not renderer acceptance.

Deliver accepted-revision passive preview first, then candidate projection when
continuous controls need it. Controls, interactive preview and 3D scene tooling
are separate increments. Run `cargo xtask check` plus applicable native checks and
keep generated evidence under `target/`.
