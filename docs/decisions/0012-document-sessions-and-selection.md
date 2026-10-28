# ADR 0012: Document sessions, opening and selection

Status: full editing/Save lifecycle proposed for owner review. A narrower
[read-only source-session profile](../site/en/contributing/contracts/editor/source-documents.md)
supports browser Open, retained document views, Hierarchy selection and the
connected read-only Inspector. The initial
[Scene View profile](../site/en/contributing/contracts/editor/scene-view.md)
adds supported read-only projection.
Shared field settlement, editable Hierarchy/Inspector and dirty-document close
remain unimplemented. Spatial navigation and opaque-mesh picking are implemented
with separate native acceptance requirements.

## Decision and ownership

One project session owns one authoring source workspace for each explicitly
admitted source scope. A scope identifies a source root, namespace and exact
source/dependency inventory. For the first product slice, admit one scope per
project session; opening several documents does not create several mutable copies
of it. Multiple-source-scope interaction needs a separate decision before enabling.

The source workspace owns accepted sources, revisions, validation, history and
Save baseline through existing `kedra_authoring` mechanisms. A document session
identifies an admitted document and its open views; it is not another document
store. Closing a tab does not remove a document from the source inventory.

| Identity/state | Authority |
| --- | --- |
| Project and source-scope generation | Project session coordinator |
| Document ID, source path, baseline and working revision | Authoring workspace |
| Active document and selected node per document | Editor document-session model |
| Source address of a selected instance child | Existing `NodeAddress`, including instance scope |
| Browser path/search/selected source entry | Each Asset Browser view |
| Hierarchy expansion/search/keyboard row | Each Hierarchy view |
| Inspector lock and field drafts | Each Inspector view, bound to document sessions |
| Preview instance and renderer readiness | Each Document Preview view |
| View ID, placement and native host | Existing docking integration |

Document identity, docking `ViewId`, native `Entity` and asset path are distinct.
Use document IDs after validated admission and exact scope-relative paths before
admission. No identity comes from a label, row index or lossy path conversion.

## Independent dockable instruments

Each tool is a separately registered feature view in the existing docking system:

- [Asset Browser](0013-asset-browser-panel.md).
- [Hierarchy](0014-hierarchy-panel.md).
- [Inspector](0015-inspector-panel.md).
- [Document Preview](0016-document-preview-panel.md), with document-bound views.
- [Scene View](0017-read-only-scene-view.md), the implemented read-only 3D presentation
  of a document view, with editor camera navigation and viewport selection.

All can occupy any tab group, split, float, maximize and redock. There is no fixed
center or alternate panel/window framework. A default layout is a preference, not
an architectural placement constraint. Panel content uses retained authored UI and
native widgets; docking chrome is shared, not recreated inside each feature.

The first workspace exposes one browser, hierarchy and inspector tool instance;
reopening a tool reveals that retained view instead of duplicating it. Each opened
document has one default document view. The model keeps view identity separate so
additional independent views need not duplicate sources later. UI for duplicating
tools/documents is deferred until its lifecycle and resource limits are specified.

Focus is not document activation. Focusing a browser, tool menu or locked inspector
must not select an unrelated document. Activating a document view explicitly
requests that document as active. Hierarchy and unlocked Inspector follow it.
Preview views remain bound to their own documents even when another is active.

## Source admission is not runtime loading

The host reads editable sources from an explicitly configured project asset root.
It never infers writable sources from installed deliveries, cache directories or
the running shell's asset handles. Opening files does not build assets, initialize
sidecars, write a catalog, migrate schemas or execute project code.

The current persistence API admits a complete `SourceSnapshot`, compiles it
atomically and retains diagnostics on failure. It is not a per-tab file loader.
Use that source authority; do not call `Workspace::open` for every tab or replace
the existing workspace to open a second document and thereby lose drafts/history.

The source-scope admission adapter must define an exact bounded inventory before
opening documents. Reuse existing import/source identity rules and maintained
filesystem tooling for discovery; discovery remains host-only. Runtime consumes
explicit catalogs and does not acquire a scanner. Catalog membership, directory
listing and the set of currently visible document tabs are different things.

Existing source catalogs can have noncanonical names, while persistence currently
only admits an indexed baseline at `catalog.prefabs.ron`. Do not rename those files,
pretend a runtime catalog is that source index, or silently write a new index on
Open. The first writable source-scope adapter must explicitly decide its inventory
and source-index mapping; if persistence needs an extension, review it with an
actual source consumer before enabling Save. An index-free source snapshot is
supported, but its index-dirty behavior must not be presented as a user edit.

For own-source development, explicitly configure a validated project/source root
and namespace containing the editor assets. The repository is not automatically
a runnable Kedra project, and the running shell delivery is not a writable mount.

Files outside the admitted inventory remain browsable, but Open reports that source
admission is required. Extending a live scope must preserve accepted sources,
dirty drafts, history and identity, with collision checks before publication.
That admission API is not currently available; do not emulate it by resetting the
workspace. Initial fixtures can declare the complete scope before opening tabs.

## Open and activation lifecycle

Browser activation submits OpenSource with project/scope generation, exact locator
and request identity. It does not directly mount a document or mutate selection.

1. Resolve and read within the admitted source scope using current path/identity
   policy and bounded native readers. Preserve exact source bytes for diagnostics.
2. Identify the document kind/ID and its admitted dependencies. Reject duplicate IDs,
   mismatched metadata, unsupported schemas and dependencies with a useful reason.
3. Resolve an already open document to its existing working copy/view; do not reread
   disk over unsaved changes. Repeated activation joins the pending request or
   activates the existing view, not a second tab with another draft.
4. Stage document presentation without retiring the active document. On successful
   admission and view readiness, settle affected field sessions under ADR 0009,
   then publish activation. Cancelling settlement leaves the previous context.
5. A newly opened document selects its root. Returning to an open document restores
   its last valid node selection. Hierarchy and Inspector observe the same change.

Source-ready, inspector-ready and renderer-frame-ready are distinct. A document
may become inspectable while its preview is pending or failed. Show that state;
do not require a rendered frame to inspect valid source data.

Failures retain the previous active document and browser selection. Diagnostic
source information is not a synthetic empty scene or a partially admitted editable
graph. Current whole-scope compilation can reject the entire candidate; per-file
recovery is not implied. Keep the browser available to explain that limitation.

Every asynchronous stage carries project/scope generation and request identity.
An older request cannot steal activation after a newer user intent. It may finish
as an explicitly background-open document only if that behavior is requested;
initially obsolete requests retire staged resources without publishing a view.
Project change retires all old-scope requests, never another session's documents.

## Selection and history

The initial single-node/Browser-local selection below is superseded by
[ADR 0041](0041-shared-editor-context.md): logical asset/node selection is shared,
separate from Open/active tabs, with bounded multi-selection and typed drag payloads.
The source-history and persistent-address requirements remain applicable.

Start with single-node selection plus an explicit empty selection. Store the
persistent address in the document model, not in a rendered row or preview entity.
Clicking a source asset only selects it in the browser; it does not silently replace
node selection or the active document. Asset inspection is a later explicit target
mode, not accidental routing through a generic selection global.

Selecting another node settles affected field sessions before publication.
Filtering/collapsing the hierarchy does not change selection. If an accepted
operation removes the selected address, use its explicit address mapping when
available; otherwise clear selection and explain the disappearance. Never select
the next row just because it reuses the same visual index. Undo/redo uses existing
authoring mappings; selection is not stored as runtime entities.

History remains one ordered history per admitted authoring workspace. The current
`WorkingSet` does not provide independent document undo stacks. A host Undo command
must identify the affected document/group and reveal its result even if it is not
the active tab. Do not skip an intervening cross-document operation to simulate
per-tab Undo. History descriptions/affected-document reporting need a narrow API
extension before exposing document commands; private history is not UI authority.

## Hide, close and project exit

Tool Close settles its drafts, then uses docking Hide and retains its descriptor
and bookmark. It does not close the active document. Reopening restores the same
tool identity. Closing a Document Preview tab is CloseView, not tool Hide.

Closing the last document view first settles every field session bound to it,
including a locked inspector. A clean document can lose its view while remaining
admitted in the source workspace. A dirty document offers Save and close, Keep
changes in session and close view, or Cancel. Hidden dirty documents stay visible
in an explicit open/unsaved-document list and can be reopened; they cannot become
unreachable because their preview tab closed. No crash recovery is promised.

Destructive Discard is not equivalent to closing a view. The current API has no
safe document-unload/discard-history operation. Do not enable Discard until its
baseline restoration, coupled-document history and dirty-state contract has been
specified separately. Do not fake Save receipts or reset the whole workspace.

Main exit/project switch settles pending input, lists all dirty documents including
viewless ones, and requires Save all, explicitly confirmed Discard all and leave,
or Cancel. Session-wide discard retires the complete source session only after
confirmation and pending Save reconciliation; it is not a per-document history edit.
Unavailable Save must explain why and retain Cancel. A lost Save response requires
reconciliation before destruction, not a new write request or a false rollback.

Floating-host Close must distinguish tool views from document views. Gather all
feature approvals before committing a topology change. Current `HideHost` semantics
for tool-only hosts cannot silently approve closing document tabs. Partial Save
side effects can exist before a later refusal; report them while keeping views.

## Module boundaries and delivery prerequisites

Initially put the ordinary document-session model, source adapter and request
coordinator in a coherent `kedra_editor::documents` feature module. Tool models
remain separate feature modules; Bevy entities remain in presentation adapters.
The source authoring APIs stay in their existing crate, the universal inspector
in `kedra_inspector`, and docking in its existing crates. Do not create a generic
event bus, DI container or one crate per tab solely because it has docking chrome.
Extraction needs an independent responsibility/consumer, not an arbitrary file cap.

Implement model tests before panels: duplicate/pending opens, stale generations,
identity collisions, failed admission, cancelled selection settlement, reopening
without draft/history loss, viewless dirty documents, cross-document history and
mixed floating-host close. Assert source, selection, view count and ownership.
Native acceptance separately verifies retained fields, focus and host transfer.

First specify and test source-scope admission and document/selection authority;
then implement the browser and hierarchy read paths; then bind the inspector and
independent preview. Source writing remains a separately reviewed implementation.
Run `cargo xtask check` and applicable feature/native checks for each code slice.
