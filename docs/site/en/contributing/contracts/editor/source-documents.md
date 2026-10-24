# Read-only source document sessions

Read-only source sessions implement part of
[ADR 0012](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0012-document-sessions-and-selection.md),
without its proposed editing, settlement or Save lifecycle. See
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
reports read-only session state alongside docking state, not an editing API.

The first source document hides the empty Document Preview slot. Supported documents use
[Scene View](scene-view.md), whose matching-delivery and frame readiness are separate from
source validity. PNG/static-GLB Open uses raw-asset views, not invented document sessions;
activating those views clears the active source target. [Hierarchy](hierarchy.md) and
[Inspector](inspector.md) consume the same scoped source selection without needing a frame.

Hide/Close retains accepted sources and descriptors, reachable through Panels or Open. Mixed
floating-host Close is safe only under the current read-only restriction. Before mutation is
enabled, implement field settlement, dirty-document reachability and close approval from ADR 0012.
Restart restores inactive placeholders only: preferences do not supply trusted document content
or automatically admit stored paths. Reopen through Assets validates the current scope and
reuses the descriptor. Crash recovery, reconciliation, Save, Discard and project execution are
not implemented.

## Owners and verification

Production owners are
[`documents/mod.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/documents/mod.rs),
[`documents/source.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/documents/source.rs)
and
[`documents/inspection.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/documents/inspection.rs).
Their local `tests.rs`, `source_tests.rs` and `inspection_tests.rs` cover repeated/latest opens,
collision/malformed/missing/mismatched input, reference rejection, scoped selection and unchanged
source state on failure. The [browser verification route](asset-browser.md#verification) exercises
source Open natively. Native verification of this revision remains pending.
