# Source authoring contracts

`kedra_authoring` validates source transactions and prepares Save requests. The introductory
workflow is [Source editing and Save](../../manual/scenes/source-editing.md); runtime graph
ownership belongs to [Scene runtime contracts](scene-runtime.md). These APIs do not implement
visual editing, editor Save, Play or a filesystem writer.

## Working documents and history

`kedra_authoring::workspace::WorkingSet` owns one mutable source per document identity,
monotonic revisions, acknowledged saved revisions and transaction history. It never owns a
captured gameplay World. Opening a duplicate identity must not replace a working document.

Every changed existing document requires an exact revision precondition. Transactions build
an independent candidate, resolve every graph and invoke the supplied validator before
publication. Hosts should use `kedra_scene::assets::compile_catalog` on the entire candidate
with their actual type registry, component policies and asset loader: graph validity alone
does not validate typed fields or dependencies. An edit/validator error or stale guard leaves
working values, saved revisions and history unchanged. Successful nonempty edits advance the
revision clock and clear redo history; a no-op does not consume history.

Undo/redo validate the candidate and require unchanged affected source content. Conflicts do
not consume history. Restored content gets a new monotonic revision, not an old timestamp.
Unrelated documents and instance links remain intact. `last_mapping` records source-address
selection remapping; undo uses the inverse Unpack map and redo restores the forward map.
Dirty state compares authored content with the acknowledged saved content, independently
of monotonic revisions. Undo to saved content is clean. History retains changed-document
before/after deltas, not full unaffected catalogs. `HistoryPolicy` defaults to 256 operations
and 64 MiB encoded payload; oldest entries are evicted and oversized operations refuse
without publication. This is not a total allocator-memory guarantee.

## Scoped edit transactions

`kedra_authoring::transaction::EditTransaction` stages an isolated catalog with explicit
grants for 1..=128 admitted document identities. It pins the complete working revision
inventory and working-set lineage. Foreign/deserialized workspaces, intervening source
edits, Undo/Redo, admission and Save acknowledgement invalidate the lease. Staging cannot
change inventory, document identities or ungranted documents. Commit performs complete
host validation and publishes through the existing `WorkingSet` history; cancel/drop
never publishes. Failed staging/validation preserves the previous candidate.

`transact_named` and `transact_grouped_named` attach bounded action labels. `undo_entry`
and `redo_entry` expose stable lineage-local `HistoryId`, label and affected documents.
Grouping retains the first entry identity/label; Undo/Redo restores recorded sources,
not callbacks or scripts. A validated no-op closes its lease without consuming Redo.
Commit, filesystem Save and Git commit remain separate operations. See
[editor integration](editor/source-documents.md#authorized-script-transactions) and
[ADR 0038](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0038-editor-edit-transactions.md).

## Typed property candidates

`kedra_authoring::properties::PropertySession` owns a source-bound candidate over an exact
immutable compilation. Its target is the opened document, full scoped node address and
canonical component key; edits of nested prefab nodes create containing-document overrides,
not implicit Apply to the prefab source. The host owns session/view generations and revokes
access when its source scope retires. The session is not a UI input buffer or a permission token.

Begin captures the destination and transitive composition revisions. Propose checks exact
native represented types and Bevy field paths, encodes typed values with the existing reference
serializer, and validates an isolated working-set candidate. Batch proposals validate related
fields together, without rejecting a valid final state because an intermediate field is invalid.
Hosts must supply complete typed catalog compilation at proposal, candidate and acceptance;
graph-only validation is insufficient. Missing paths, wrong types, overlapping proposals,
stale dependency revisions and host rejection preserve working sources and the previous candidate.

At most 32 disjoint proposals / 64 KiB of encoded values are retained. Indexed collection/struct
paths and structural commands are unsupported. Child edits inside an existing parent-field or
whole-component override in the same layer require editing that existing override; implicit
override expansion is not implemented. Entity-reference pickers and new asset dependency
admission require their own host integration, not live entity handles supplied as values.

Candidate construction and cancellation never change working sources, history or Save baselines.
Accept rechecks revisions and host validation, publishing one transaction for all changed fields.
No-op acceptance preserves redo. Acceptance closes the session; rejection leaves its candidate
available for inspection or correction, without automatic rebasing. The editor's native field
interaction, projection refresh, settlement and Save are not implemented by this library API.

## Exact compilation for Apply and Unpack

Apply/Revert operate on source values and originating override owners, including nested
instance-site scopes. Apply moves an effective field/component to its actual source and
removes its originating override atomically. Invalid source/destination, missing overrides,
shadowed destinations or unrepresentable references reject the whole edit. The originating
effective value must remain unchanged; introducing an untracked native asset dependency in
snapshot recompilation is an error, not permission to invent a loader.

Apply and complete scoped Unpack require a compilation matching the current origin and every
transitive composition source **exactly**, including metadata and shadowed fields. Matching
only visible values or supplying new revision guards with an old compilation is insufficient.
Unrelated documents outside that transitive set do not invalidate a snapshot.

Unpack selects a nonempty complete nesting scope and materializes its subtree, descendants,
references and effective child order into source-owned nodes. It remaps identities explicitly,
removes the selected composition relationship and preserves other linked instances. Symbolic
optional-port intent must survive: an unbound source port is not rewritten to runtime `None`.
Failures preserve source/history; success returns the selection map. These operations never
serialize a gameplay instance. See
[composition ownership](../../manual/scenes/composition.md#apply-revert-and-unpack).

## Baseline admission and path ownership

`kedra_authoring::persistence::Workspace` retains the exact host-supplied `SourceSnapshot`:
namespace, workspace generation, source bytes and hashes, asset inventory and optional explicit
catalog baseline. The host reads sources; the runtime performs no filesystem discovery.
`open_compiled` returns the exact admission compilations, not live instances; subsequent edits
do not update those snapshots.

Admission validates namespace, unique source identities/paths, hashes, document path meaning,
asset inventory, catalog references and complete typed compilation. On failure, original
bytes and diagnostics remain available for repair, with no partial runnable working set or
default scene. A diagnostic workspace cannot prepare Save.

Document paths must be portable workspace-relative paths with their exact native meaning and
matching `.scene.ron`/`.prefab.ron` suffix. Absolute paths, traversal, empty/dot segments,
backslashes, NUL and source/label delimiters such as `:` and `#` fail; percent-encoding must
not silently select a different filename. Preserve Unicode and spaces. `assign_path` reserves
an unused path for a new document, not a managed rename, and is forbidden during pending Save.
`assign_new_path` may derive an unused path from a nonempty supported title.

An admitted catalog must contain exactly the declared documents. Namespace-qualified source
paths and supported UUID addresses are accepted, but two references resolving to one document
are rejected. Proposed saved catalogs use sorted namespace-qualified source paths. Persistent
UUID identity is not replaced by a path. Reconciliation and Save independently reject
document/asset/reserved-catalog path collisions, including restored conflicting session state;
neither chooses which source to overwrite.

## Prepare exact coupled bytes

`prepare_save` admits one immutable `CommitRequest`, not a disk write. A selected document's
group includes transaction coupling and required dirty composition dependencies/document
links. The actual post-commit catalog is compiled separately using submitted changes plus
the acknowledged baseline, not unrelated unsaved edits.

Admission serializes and checks the **exact** proposed document/catalog bytes through native
readers, including pretty-print expansion and their 64 KiB bounds. Empty/oversized native
catalogs and duplicate targets fail before a pending request is installed. Missing paths,
unsupported managed deletion, invalid dirty sources and nothing-to-save remain explicit
errors. A preparation failure leaves working values, baseline and existing pending bytes
unchanged.

The request pins an operation UUID, expected workspace generation, complete source inventory,
unchanged-source/asset hash checks (including catalog presence/absence), exact coupled
write/delete representations, document revisions and expected prior hashes or absence.
A host writer owns filesystem compare-and-swap, durability and actual publication. It must
check those read preconditions and commit the coupled bytes; preparing or acknowledging an
in-memory request does not itself perform I/O. The separate `kedra_authoring_fs` adapter
implements guarded recoverable replacement of an existing document group. Its
`prepare_existing_save` profile pins the complete group while preserving admitted inventory/catalog.
Full canonical `prepare_save` still needs a writer supporting catalog/inventory changes.

## Lost responses, refusal and receipts

Only one request may be pending. A lost response must retry/reconcile the same operation and
exact bytes from `pending()`, not regenerate a request or assume timeout means refusal.
`refused(operation)` releases only a known, conclusively refused operation without marking
working edits saved. A wrong operation, malformed receipt or reconciliation attempt while
Save is pending preserves the pending operation and state.

`acknowledge` validates operation correlation, exactly the next generation, complete unique
file membership, paths, document IDs, submitted revisions and exact content hashes/deletions
before updating the source baseline. A matching receipt records the submitted bytes, not
newer local edits; saved revisions and content checkpoints record the pinned submission
even when newer working edits exist. The last acknowledged operation can be acknowledged again without another state
transition. This idempotence is not a second write or authority to accept unrelated receipts.

## Reconcile without writing

`keep_working` is an explicit host conflict decision against a newly reviewed baseline.
It retains dirty working documents, admits clean external changes/removals, validates the
merged complete catalog and path ownership, then updates the baseline. It cannot change the
namespace or run with pending Save. A reused dirty path, invalid incoming baseline, failed
compilation or ownership conflict leaves the original workspace unchanged. A collision-free
external rename may retain dirty content under the newly admitted path. There is no automatic
last-writer-wins overwrite and reconciliation performs no write.

Authoring transaction atomicity, filesystem durability and deferred ECS mutation are different
guarantees. Gameplay state, preview entities and runtime camera changes are never Save inputs.

## Source owners and focused verification

Exact public interfaces remain in [workspace][workspace], [overrides][overrides],
[unpack][unpack] and [persistence][persistence], with generated signatures available through
the [Rust API guide](../../api/index.md#rust-api-guide).
Do not duplicate a hand-maintained API list.
[ADR 0005 (English)][adr] retains the ownership decision.

The [authoring contract target][contracts] tests transactions and history, routing edits to
their source owners, freshness and Unpack semantics. [Freshness tests][freshness] assert stale
Apply/Unpack rejection without consuming history, including transitive metadata changes hidden
from effective values.
[Persistence guards][guards] cover exact byte boundaries, pretty expansion and unchanged state
on collision; [protocol tests][protocol] cover malformed receipts, same-operation restoration,
refusal and pending reconciliation; [native path tests][paths] protect literal path meaning.
Use the [testing guide (English)][testing] for commands and broader gate policy.

```sh
cargo test -p kedra_authoring --locked
```

[workspace]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/src/workspace.rs
[overrides]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/src/overrides.rs
[unpack]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/src/unpack.rs
[persistence]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/src/persistence.rs
[adr]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0005-authored-scenes.md
[contracts]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/tests/contracts.rs
[freshness]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/tests/contracts/freshness.rs
[guards]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/tests/contracts/persistence_guards.rs
[protocol]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/tests/contracts/persistence_protocol.rs
[paths]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_authoring/tests/contracts/native_paths.rs
[testing]: testing.md
