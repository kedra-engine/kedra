# ADR 0041: Shared asset, selection and drag context

Status: accepted architecture; native interaction acceptance incomplete.

## Decision

Editor-layer `kedra_editor_context` owns borrowed logical Asset Library views,
bounded multi-selection and typed asset/node drag payloads. Native editor windows
and extensions consume `kedra_editor::{context,drag}`. This extends the single-node,
Browser-local asset-selection slice of [ADR 0012](0012-document-sessions-and-selection.md).

The source inventory and WorkingSet remain authoritative. No duplicate asset
registry, source journal, ECS or generic event framework is introduced. These
small ordinary Rust values represent Kedra-specific source lifetime/membership;
native Bevy picking and existing docking continue to own pointer delivery and views.

- Asset references contain a host-owned source lifetime, exact source-qualified
  logical path, presentation kind and optional persistent identity.
- Node references contain the same lifetime, receiving document and full composed
  NodeAddress, never a transient row or runtime entity identity.
- Selection is independent of Open, active document tabs, mutation grants and Save.
  Asset selection may admit the existing bounded source inventory without opening
  a document. A failed selection must preserve the previous shared context.
- Document activation restores remembered node context only when the document
  changes. Tool focus, foreign views and repeated activation do not collapse a
  multi-selection or replace an asset selection.
- Inspector follows the active shared item or retains its locked context. Source
  drafts still obey the existing host settlement rules. History presentation
  attachments carry logical selection, keyed by the actual source transaction.
  Explicit Discard retires the old source lifetime and resolves surviving selected
  assets/nodes into the replacement scope; missing members are pruned. Captured
  external references and drag payloads from the old lifetime remain invalid.
- Typed drag captures a homogeneous selection without silently discarding members.
  Drop rechecks source membership/lifetime and producer bindings; source mutation
  additionally requires the receiving operation's existing authoring admission.
  Resource wells accept one compatible asset and stop context-drop propagation.

## Current limits and verification

The editor initially has one admitted game source scope. Live source-inventory
extension is unavailable. Empty filesystem folders are not inferred library members.
Browser uses single-entry pointer selection; the shared API supports multiple assets
and mixed selections. Hierarchy supports modifier toggle and visible Shift ranges.
Inspector presents one active item, not common-field multi-editing; Scene View shows
one node marker. Non-document assets expose existing name/path presentation only.
Generic drops on a locked Inspector do not override its pin; they update shared
selection for following consumers. These APIs do not execute project code.

Pure contracts belong to `kedra_editor_context/tests/contracts`; source admission,
tab coordination, visible producers, Inspector following/locking and draft ownership
belong to `kedra_editor`. Native input, cancellation, cross-window drops and rendered
frames require separate acceptance. Compilation/headless tests do not establish it.
