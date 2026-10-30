# Source document sessions

Source sessions implement part of
[ADR 0012](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0012-document-sessions-and-selection.md),
with in-memory Transform/UiCaption editing/history, New scene/prefab and document-group Save,
but without general visual editing or automatic field settlement. See
[Editor workspace](../../../manual/editor/index.md) for Open, tabs and external changes.

## Authority and identity

`kedra_editor::documents` owns one admitted `kedra_authoring::persistence::Workspace` per open
project. Tabs share accepted sources; they do not create independent workspaces. The coordinator
owns active-document identity and optional selected `NodeAddress` per opened document. Addresses
retain complete nested instance scope. Native entities and docking `ViewId` are presentation and
placement identities, not source document/node identities. Labels never become locators.

The supported scope is the validated project asset root, namespace `game`. Neither the serving
editor delivery nor AssetServer handles infer a writable source mount. Own-editor admission and
other namespaces need explicit source-scope configuration. Native code references editor assets
by full logical paths such as `editor://prefabs/source-document.prefab.ron`, resolved through the
loaded catalog with membership and published-revision checks. Persisted UUIDs, sidecars, scoped
addresses and provenance retain their identities. In-project display uses project-relative paths,
not absolute filesystem paths; display formatting never rewrites native locators.

## Authorized script transactions

Trusted Rust Program bindings explicitly call `kedra_editor::edit_scripts::install`
with exact admitted paths such as `game://Prefabs/First.prefab.ron`. Grants come from
native authorization, not script arguments. Editor-layer `kedra_authoring_luau` installs
`ctx.edit.begin(label)`, not discovery, automatic project-code execution or Save.
The existing debug observation API remains read-only and does not install this capability.
The first successful begin binds that installed capability to one source session;
retirement requires fresh native authorization even when a new session has matching paths.

Leases expose `nodes(path)`, `set_property(node, component, path, value)`, `rename(node,
name)`, `remove_node(node)`, `reparent(node, parent)`, `remove_component(node, component)`,
`commit()` and `cancel()`. Nodes are opaque transaction-bound userdata with
display-only `name`. Values use native reflected deserialization and PropertySession;
JSON is transport, not a source schema. Each request must produce a valid typed candidate;
temporarily invalid cross-field staging is not provided by this binding. Nested edits
remain overrides of the receiving document. Access refuses pending gestures/drafts,
pending Save, retired scopes and stale revisions. Commit validates the complete catalog
and publishes one named source-history entry, returning an opaque ID string or nil for a no-op.

Limits: 128 staged mutations, 4096 enumerated nodes per call, 64 KiB JSON per property,
1024 bytes per field path/name and 256 bytes per component key. Only existing admitted
documents can be edited. Uncommitted work cannot publish after exception, cancellation
or run retirement. Explicit commits remain accepted even if later script code fails;
runtime failure is not rollback of a commit. There is no separate script/Inspector journal.
Files remain unchanged until separate Save, which preserves the complete coupled group.

Explicit startup launch uses `kedra <project> --editor-script Editor/path.luau
--edit-source game://Main.scene.ron` (repeat `--edit-source` for each writable source).
The script defines `function run(ctx)`; standard relative modules use its immutable
Editor/ snapshot. Loading executes nothing. Snapshots admit at most 256 files / 8 MiB,
64 KiB per file, within a 1024-entry / depth-16 inventory; links and special files refuse.
This is not a sandbox against concurrent hostile filesystem replacement. Opening a
project alone does not launch scripts. `edit_scripts::status` and `cancel` serve native
hosts; debug workspace observation exposes status without authoring/eval authority.
Editor tools use a one-second per-resume budget for synchronous source compilation,
with the ordinary 45-second whole-run deadline. Long batches should yield between
requests with `wait.updates(1)`; runtime behavior budgets are unchanged.

Source history restores optional logical selection/focus and value-free collection
mappings keyed by `HistoryId`, not by matching values. Missing/closed panels do not
block source history. Dirty is based on saved content, not revision equality.

## Bounded admission

The editor-only scanner uses locked `walkdir` 2.5.0 with link following disabled. Runtime consumes
explicit catalogs and acquires no filesystem scanner. The limits are:

| Inventory boundary | Ceiling |
| --- | --- |
| Discovered entries / depth | 8192 / 64 |
| Scene/prefab documents | 128 |
| Each document / adjacent sidecar | 64 KiB / 64 KiB |
| Each other asset | 8 MiB |
| Aggregate source payload bytes | 64 MiB |

Aggregate accounting sums document/other-asset payloads; sidecars have their separate per-file
limit. Linked/special files, unaddressable native asset paths, duplicate/nil sidecar identities,
malformed documents and document/sidecar identity mismatches reject admission. Orphan `.meta`
files are not independent sources. Missing sidecars remain untracked; Open never generates UUIDs
or metadata. Document identity still comes from the validated source document.

The adapter creates an index-free `SourceSnapshot`, then calls `Workspace::open_compiled` once
for admission. Existing parsing, composition, reflection policy and compilation validate the
whole candidate and retain immutable compiled values for inspection. One unsupported/malformed
document can reject the whole scope while the browser remains usable. Runtime `.prefabs.ron`
files are neither renamed nor treated as `catalog.prefabs.ron`. No index is written; index-free
baseline bookkeeping must not appear as an unsaved user edit.

The reflection-only `LoadFromPath` adapter checks authored `game://` references against actual
inventory identities and supplies UUID handles. Unknown, foreign-scope and untracked referenced
assets fail explicitly. This does not load assets, execute metadata-selected processors/loaders,
or prove subasset/dependency resolution, image/font decoding or renderer readiness. Authored
paths and labels remain intact. These handles authorize neither rendering nor Save.

## Asynchronous Open and replacement

New scene/prefab collects a filename in an editor-owned modal scoped to the current Assets folder, then
uses `source::create_document` on an isolated candidate of the current workspace. It
publishes one named history entry and opens through the ordinary document-view path.
Existing document history is retained. Cancellation and invalid paths publish nothing.
Save is separate. Creation completion belongs to its owning Documents/project lifetime;
pending creation blocks Open, history/discard and project close. Undo/Redo retains UUIDs
and paths; removed sources stop participating in Hierarchy and preview. Discard restores
the acknowledged workspace and its path reservations. See [ADR 0047][creation].

Hierarchy's content context menu offers empty nodes, primitives, directional lights
and authored perspective-camera settings for an active source.
Each preset stages a child and its registered components through `SourceEdit` and commits
one history entry. The selected node is the parent; UI parents refuse spatial children.
Undo/Redo preserves child identity, and only explicit Save publishes bytes. Structural
preview changes restart the ordinary preparation/readiness lifecycle; supported property
changes retain the existing in-place refresh path.

Inspector's header Add control and per-component Remove menus discover commands from the source schema
registrations and the retained Inspector target, including locked targets. Commands pin
the scope and revision inventory; stale commands refuse without source changes. Addition
uses Bevy `ReflectDefault` and the schema validator, not an editor-maintained type list.
A type without a registered default requires an explicit initial value and currently
reports that requirement instead of inventing resource handles or component data.

Dropping one admitted GLB from Assets on a Hierarchy node stages a model child with a
Transform in one source transaction. Native GLTF metadata supplies its default scene
(or sole scene); ambiguous multi-scene files refuse. The original parent and revisions
remain pinned while metadata loads. Delivery membership is rechecked before publication.

Browser selection/focus does not activate a document. Explicit Open passes the exact relative
locator to the project coordinator. Repeated Open requests for the same pending locator share
its request. A later request for another locator replaces the document to activate. Discovery
may complete for shared admission, but only the latest locator may publish a tab.
Request counters are checked rather than wrapping. Project retirement drops the
coordinator and pending work; stale results cannot repopulate another project.

Before first presentation of another inventory member, off-thread checks revalidate its source
hash, regular-directory ancestors and current sidecar identity. Changed/missing input fails
without replacing the active document. Already opened documents activate retained accepted state
without rereading disk over it. Newly added inventory members need project reopen; Refresh does
not extend the scope, reconcile accepted sources or reload documents. Filesystem checks do not
provide a sandbox against concurrent hostile replacement.

## Views, selection and retirement

Each opened document has one distinct retained docking view and initially selects its root.
Reopen returns to that view and its selection. Document-tab activation updates the coordinator;
tool focus does not. Selection must belong to the active admitted graph. Debug `editor.workspace`
reports source revisions/dirty/history availability alongside docking state,
not a remote editing API.

The first source document hides the empty Document Preview slot. Supported documents use
[Scene View](scene-view.md), whose matching-delivery and frame readiness are separate from
source validity. PNG/static-GLB Open uses raw-asset views, not invented document sessions;
activating those views clears the active source target. [Hierarchy](hierarchy.md) and
[Inspector](inspector.md) consume the same scoped source selection without needing a frame.

Hide/Close retains accepted sources and descriptors, reachable through Windows or Open. Mixed
floating-host Close retains hidden source views and all shared working documents. Transform
drafts remain retained in Inspector; source history refuses unresolved drafts. Main-window
Close is gated before desktop close admission while input or unsaved sources remain.
The close dialog offers **Save and exit**, **Exit without saving** and **Cancel**.
Save waits for source publication; drafts must first be accepted or cancelled.
Exit without saving restores the acknowledged source baseline without rereading/writing
disk, clears history and retires field/scope bindings. Pending publication blocks discard.
Automatic blur settlement or crash recovery is not implied by this close policy.
Restart restores descriptors; visible source tabs request ordinary current-inventory
admission automatically, and inactive tabs request it when selected. Preferences supply
locators, never trusted content. A failed attempt displays its error without a retry loop;
explicit Open through Assets can retry using the same descriptor.
Unsaved-session recovery, external merge and gameplay execution are
not implemented. Interrupted authorized Save groups require explicit recovery before Open.

## Save ownership

`documents::saving` pins `prepare_document_save` for the active source and runs
`kedra_authoring_fs::commit_documents` on the IO pool. Complete coupled groups may include
new sources and source-history removals. Runtime catalogs and sidecars remain untouched;
removal with an existing sidecar refuses. Parents must already exist. New names publish
exclusively. Inventory changes need an index-free source scope. The authoring library's
separate `prepare_existing_save` profile retains its inventory-preserving contract.
All Inspector drafts must be settled explicitly. Save does not grant Luau filesystem access.

The writer checks complete inventory/hash preconditions, preserves permissions and uses
atomic replacements under a stable sibling advisory lease and a recoverable group journal.
The host also
reuses source admission's asset/sidecar identity checks. External conflicts preserve working
values and history. Only the exact returned receipt advances the source baseline; newer
working revisions stay dirty. The original renderer-delivery admission stays pinned separately.

Known refusal releases the operation. An uncertain outcome retains exact pending bytes and
blocks discard/close; Retry Save reconciles the same request, including already-published
bytes/absence. Save refreshes Assets but does not build assets or clear source history.
Dirty-state/close checks include removed saved documents. Admission holds a shared lease
and refuses unfinished groups. After restart use `kedra recover-save <source-root>`;
Open never writes recovery data. See
[ADR 0039](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0039-existing-source-save-groups.md).
See [Save instructions](../../../manual/editor/inspector.md#save-active-document) and
[ADR 0037](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0037-existing-document-save.md).

## Owners and verification

[creation]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0047-document-creation-and-history-save.md

Production owners are
[`documents/mod.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/documents/mod.rs),
[`kedra_authoring_fs::source`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring_fs/src/source.rs)
and
[`documents/inspection.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/documents/inspection.rs).
Editor `tests.rs`/`inspection_tests.rs` and filesystem `source/tests.rs` cover
repeated/latest opens,
collision/malformed/missing/mismatched input, reference rejection, scoped selection and unchanged
source state on failure. The [browser verification route](asset-browser.md#verification) exercises
source Open natively. Native verification of this revision remains pending.
