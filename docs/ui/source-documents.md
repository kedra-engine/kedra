# Read-only source document sessions

`kedra_editor::documents` owns one admitted source workspace per open project.
The first scene/prefab Open starts bounded, asynchronous source admission. Further
opens share that workspace rather than resetting accepted sources for each tab.
This is the read-only prerequisite of
[ADR 0012](../decisions/0012-document-sessions-and-selection.md),
not the complete editing/Save/close implementation proposed there.

## Admission profile

The current profile explicitly uses the validated project asset root and namespace
`game`. It does not infer editable sources from the editor delivery or register a
filesystem source in the runtime AssetServer. Other namespaces, including own-editor
authoring, require explicit source-scope configuration before support is claimed.

`walkdir` supplies maintained, non-link-following host discovery. The scanner is
editor-only; runtime still consumes explicit catalogs. Reusing this existing locked
dependency avoids another recursive walker. The scope is limited to 8192 entries,
depth 64, 128 documents, 64 KiB per document/sidecar, 8 MiB per other asset and
64 MiB aggregate source data. Linked/special files, invalid native asset paths,
duplicate identities, malformed documents and mismatched sidecars reject admission.
Orphan `.meta` files are not independent sources. Missing metadata is retained as
untracked data, never permission to create a UUID or sidecar.

The adapter builds an index-free `SourceSnapshot` and calls the existing
`kedra_authoring::persistence::Workspace::open` once. Existing document parsing,
composition, reflection policies and compilation validate the entire candidate.
One malformed or unsupported document can reject the whole scope; the browser
remains available. Runtime `.prefabs.ron` files are not renamed or pretended to be
`catalog.prefabs.ron`. No index is written, and index-free baseline bookkeeping is
not displayed as a user edit or an unsaved document.

A source-only `LoadFromPath` adapter validates authored `game://` references against
real inventory identities and supplies UUID handles for reflected source validation.
Unknown, foreign-scope or untracked referenced assets fail explicitly. This adapter
does not load assets, invoke metadata-selected processors/loaders or claim subasset,
dependency, font/image-decoding or renderer readiness. Original authored paths/labels
remain in the source documents; these source-only handles are not a rendering or
Save pipeline.

## Opening, activation and retention

Browser selection and focus are independent of document activation. Explicit Open
supplies the exact relative locator to the project-owned coordinator. Duplicate
pending opens join; later locators supersede activation intent. Source discovery
can be shared by those requests, but only the latest locator can publish a tab.
Project retirement drops that coordinator and its pending results.

Before first presentation of another admitted document, its exact source hash,
regular-file ancestors and current sidecar identity are checked again off-thread.
Missing or changed sources fail without replacing the active document. Already open
documents activate their existing source/view without rereading disk over accepted
session state. Newly added documents outside the inventory require reopening the
project; no live-scope extension or automatic reload resets the workspace.

Each opened document has one distinct docking view and a root source-node selection.
Reopening returns to that view and preserves its selection. Activating an admitted
document tab updates active-document identity. Native entities are presentation
handles, not document/node identities. `editor.workspace` exposes read-only
document/session diagnostics alongside docking state.

The initial empty Document Preview slot hides when the first source document opens.
Tabs host the initial read-only [Scene View](scene-view.md) for supported sources.
Rendering additionally requires an explicit matching verified project delivery;
source validity does not imply resource or frame readiness. PNG/static-GLB Open
uses a separate raw-asset view, not a manufactured source document session.
[Hierarchy](hierarchy.md) displays the active session's immutable composed graph
and selects validated scoped node addresses. Inspector and document editing
controls are not connected. Closing/hiding a
read-only tab keeps its accepted source and docking descriptor reachable through
Panels or another Open. Mixed floating-host Close remains safe only because these
documents cannot become dirty. Editing must add the proposed settlement and close
approval contracts before enabling source mutation.

Restart restores tab descriptors as explicit inactive source placeholders. It does
not trust preferences as document content or automatically admit stored paths.
Reopening through Assets validates the current scope and reuses the descriptor.
No crash recovery, source reconciliation, Save, Discard or project execution is
provided. Filesystem checks are not a sandbox against concurrent hostile replacement.

See [Asset Browser verification](asset-browser.md#verification) for the source/Open
native scenario. Model tests cover repeated/latest opens, identity collision,
unsupported references and failed admission without source writes.
