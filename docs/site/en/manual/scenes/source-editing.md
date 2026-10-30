# Source editing and Save

Use the Rust authoring APIs to build source-editing tools and integrate Save into a host
application. For the editor command, see
[Save active document](../editor/inspector.md#save-active-document). See the
[Rust API guide](../../api/index.md) for exact interfaces.

Contributors changing transaction safety or persistence should read the
[source authoring contract](../../contributing/contracts/source-authoring.md).

Source editing changes authored documents, not live instances. The Rust authoring APIs
provide validated transactions, history, Apply/Revert, complete Unpack and a Save protocol.
That library does not perform filesystem I/O or implement Play. The editor supports
the documented [Inspector source edits](../editor/inspector.md), including Transform,
primitive/surface/light parameters and embedded Luau UiCaption fields,
with validated source history and guarded recoverable document Save groups through
`kedra_authoring_fs`. [New scene](../editor/asset-browser.md#create-a-scene) uses the same
history and separate Save. Play remains unavailable.

With a scene or prefab active, Hierarchy's content context menu provides **Create →
Empty Node**, primitive objects, directional light and authored camera settings.
The clicked node becomes the parent; blank space targets the document root.
Each action is one source Undo/Redo step; use Save to publish it. Spatial objects cannot
be placed beneath UI nodes. Structural changes rebuild the document preview.

Inspector's header **+** opens **Add component** for its current target; each component section's
**…** offers **Remove component**. The list comes from registered source schemas. Adding requires a
valid registered reflected default; components needing explicit initial data report that
requirement. Each successful operation shares source Undo/Redo and explicit Save.

To place a model, drag a GLB from Assets onto its parent node in Hierarchy. A matching
project delivery is required. The model must declare a default scene or contain exactly
one scene. The new child retains a source-qualified model reference and a Transform.

Prerequisites: [document structure](index.md#document-fields) and
[composition ownership](composition.md#apply-revert-and-unpack).

For manual source changes, work on a project copy, preserve identities for existing
objects and prepare a new matching
[delivery for rendering](../assets/pipeline.md#editor-previews-need-matching-sources).
Reopening an already open editor tab reuses its admitted session;
it is not a disk reload command. Newly added documents require reopening the project.

## Open a source workspace

`kedra_authoring::persistence::Workspace` owns two distinct views of the source:

- `source`: the exact host-supplied `SourceSnapshot`, including namespace, generation,
  document bytes/hashes, asset inventory and optional explicit catalog baseline.
- `working`: a `WorkingSet` containing one working document per identity, revisions
  and source history.

The host reads the filesystem and supplies this snapshot; the runtime does not scan it.
`Workspace::open` validates admission with the registered types, component policies and
asset-reference loader. `open_compiled` also returns the exact immutable compilations
used for admission. Neither creates a gameplay instance.

On admission failure, original source bytes and a diagnostic remain available, with no
partial runnable working set or default replacement. Repair the source and re-admit it;
a diagnostic workspace cannot prepare Save.

## Make a validated edit

1. Read the current document revision from `WorkingSet::documents()`.
2. Pass revision preconditions for every existing document the transaction may change.
3. Use `transact_checked` for a multi-document edit or `patch` for one source operation.
4. Supply a validator that calls `kedra_scene::assets::compile_catalog` over the entire
   candidate with the host's types, policies and loader.
5. Publish or project the accepted candidate only after success, as a separate host step.

The built-in transaction checks resolve every candidate graph. The supplied validator
adds typed component/reference validation; graph validity alone is not sufficient.
A rejected edit or stale revision leaves source values, saved revisions and history
unchanged. Successful edits use monotonic revisions and clear redo history.

`undo_checked` and `redo_checked` also validate before publication. Restoring old content
creates a new revision; it does not roll the revision clock back. Conflicts with externally
changed affected content reject history restoration without consuming the history entry.
`dirty(id)` compares current and acknowledged saved revisions, not a gameplay World diff.

### Propose fields without committing

`kedra_authoring::properties::PropertySession` separates a typed field candidate from accepted
working sources. Begin with a current compilation and an explicit containing-document target;
use `propose` for one field or `propose_batch` for related fields, supplying complete typed
catalog validation. `candidate` produces a separately validated catalog for an isolated preview.
`accept` rechecks revisions and commits the changed fields in one undoable transaction.
`cancel` or dropping the session discards proposals without modifying working sources or redo.
The editor's Transform and embedded UiCaption adapters consume these sessions; other reflected
fields and Save are not enabled. Read the
[property candidate contract](../../contributing/contracts/source-authoring.md#typed-property-candidates)
for ownership, capability limits and rejection behavior.

### Choosing the edit owner

To specialize one nested prefab, patch its containing source scope. To change a shared
source, use the explicit Apply operations described in
[Composition](composition.md#apply-revert-and-unpack).
`apply_field` and `apply_component` need source/destination revision guards and an exact
current compilation. `revert_field` and `revert_component` remove originating overrides.
`unpack_complete` returns source-address remappings; `last_mapping` carries mappings for
selection restoration through undo/redo. These are tool-building operations, not UI steps.

## Prepare Save; do not write yet

New documents need an unused source path via `assign_path` or `assign_new_path`.
Paths are workspace-relative and must retain their native asset-path meaning, with the
matching `.scene.ron` or `.prefab.ron` suffix. Absolute paths, traversal, backslashes and
native source/label delimiters such as `:` and `#` are rejected. Unicode and spaces are
not a reason to rewrite a filename into a percent-encoded URI.

`Workspace::prepare_save` validates sources and returns one immutable `CommitRequest`.
Selecting a document does not guarantee a single-file save: the group includes coupled
transactions and required dirty dependencies/document links. The proposed post-commit
catalog is compiled independently of unrelated unsaved edits.

The request pins an operation UUID, expected workspace generation and source inventory,
unchanged-file/asset checks, and exact coupled write bytes with revisions and prior hashes.
It can include `catalog.prefabs.ron`, whose references use the workspace namespace and
source paths. Exact serialized document/catalog bytes must satisfy native loader bounds.
Identity/path collisions fail rather than choosing a source to overwrite.

## Commit and acknowledge

The host writer must compare the request's generation, inventory and hashes against its
actual source baseline, commit the coupled bytes with its durability guarantees, and
return a correlated `Receipt`. This writer is an integration responsibility; none is
installed by `kedra_authoring`. A prepared request has not saved anything to disk.

`Workspace::acknowledge` checks the operation, next generation, complete file membership,
paths, document IDs, revisions and exact content hashes. Only a matching receipt advances
the baseline. If another local edit occurred while Save was pending, the receipt records
the submitted bytes without marking that newer revision saved.

Only one request may be pending. If the writer response is lost, inspect `pending()` and
retry or reconcile that same operation and bytes. A timeout is not proof of refusal.
Call `refused(operation)` only after conclusive refusal; it releases the pending request
without marking edits saved. Do not generate a second operation to resolve an ambiguous
first write.

## Reconcile external changes

`keep_working` is an explicit conflict decision: retain dirty working documents against a
newly reviewed source baseline, admit clean source changes and validate the merged result.
It cannot run with a pending Save, change the namespace or reuse another source's path.
It performs no write. There is no automatic last-writer-wins policy.

Gameplay components, editor camera movement and rendered preview entities are never Save
inputs. Authoring transaction atomicity, filesystem commit durability and deferred ECS
mutation are different guarantees. For runtime ownership, see [Lifecycle](lifecycle.md).

## Repository references

For exact Save request/receipt APIs, see the [persistence implementation][persistence]
and [authoring contract suite][contracts]. The [Glossary](../glossary.md) distinguishes
source revisions, working sets and Save receipts.

[persistence]:
  <https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/src/persistence.rs>
[contracts]:
  <https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/tests/contracts.rs>
