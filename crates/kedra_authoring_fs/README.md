# kedra_authoring_fs

Authoring-layer filesystem adapter for a document group's Save request. The
editor is its real consumer; runtime, native fields, Luau and the transport-independent
`kedra_authoring` crate do not acquire filesystem write authority.

`commit_existing(root, request)` accepts 1..=128 existing scene/prefab replacements.
It rejects catalog writes, creation and deletion. The host uses `prepare_existing_save` to pin
the unchanged admitted inventory without generating or rewriting a source catalog.
The full `prepare_save` protocol remains available for other explicitly implemented writers.

`commit_documents` accepts complete document groups with creation/replacement/removal,
prepared by `prepare_document_save`. New paths publish exclusively. Parents must exist;
catalogs and sidecar writes remain unsupported. Removal of a source with an existing
sidecar refuses. Exact retries and recovery use the same request/receipt and lease.
Undo/Redo acts on working sources, never directly on this writer. See
[ADR 0047](../../docs/decisions/0047-document-creation-and-history-save.md).

## Publication and conflicts

`commit_environment(root, request, baseline)` replaces one existing `.environment.ron`
asset using the same lease, journal, exact retry and recovery. The `SourceAsset`
baseline pins its identity under the writer lease. Environment validation replaces
document parsing for this operation; its sidecar is preserved. The caller acknowledges
the receipt through `kedra_authoring::environment::Edits`, updating the source inventory.

The writer reuses native parsing, `walkdir`, SHA-256, standard File locking,
`tempfile` replacement and pinned `fs-transaction` 0.4.0 journal/rollback/replay.
Kedra owns bounded request admission and exact-byte grants, not another journal engine.

- Portable request paths retain their exact Unicode/native meaning. Symlink/special
  files and ancestors, missing/extra sources/assets, hash mismatches and duplicate or
  incomplete preconditions reject. Inventory/depth/byte work has explicit bounds.
- A stable regular sibling lease `<source-root-name>.kedra-save.lock` coordinates
  cooperating writers. It is intentionally retained outside the asset inventory;
  it contains no document state or persistent lookup identities.
- A private sibling staging directory preserves the original permissions and keeps
  crash-orphaned temporary bytes outside authored inventory. Bytes are synced before
  replacement. Removal rollback backups also remain in this private directory, through
  the pinned transaction library's exact operation-bound backup names. The
  original read-only source is never made writable. Sidecars/catalogs are not touched.
- `tempfile::persist` atomically replaces each existing name on the same filesystem.
  File flushes open with write access on Windows, without truncation or permission changes;
  POSIX uses read access. Windows has no equivalent POSIX directory-sync step.
  The transaction engine journals the group and flushes publication/rollback before
  retiring its journal. Physical power-loss durability is not certified by process tests.
- An exact already-published retry returns a correlated receipt without rewriting
  bytes. The host still calls `Workspace::acknowledge`, never marks arbitrary revisions
  saved, and must retain pinned bytes after an uncertain completion.

`SaveError::uncertain` distinguishes failures that may follow replacement from definite
pre-publication refusal. A retry of an already uncertain host operation must remain
pending if any new conflict prevents reconciliation. Timeout is not proof of refusal.
This adapter is not a hostile-writer sandbox: advisory locks and two hash checks cannot
prevent malicious namespace replacement between the final check and rename. Remote or
nonstandard filesystem durability, physical power loss and other-platform native behavior
need separate acceptance. Simultaneous visibility to arbitrary filesystem readers is not claimed.

`read_guard(root, pending)` holds the shared admission lease. An interrupted group
blocks normal admission; only its exact pinned Save may inspect it for retry. Open
never writes recovery data. `recover_documents(root)` explicitly rolls the saved intent
forward; the native command is `kedra recover-save <source-root>`. Foreign/changed
bytes, sidecar preconditions and journal operations outside the request refuse replay.
Recovery files live beside the root and must remain paired with it. The private staging
directory can retain crash-orphaned source bytes; automatic pruning is not provided.

The editor additionally reuses its existing complete admission reader to verify asset
identity/sidecars before writing. Save updates the acknowledged source baseline, not
the original verified renderer-delivery provenance, and does not rebuild assets.

## Asset folders

`entries::apply` provides explicit folder Create and file/folder Rename/Delete under the
same exclusive source lease as Save. It uses directory operations, recovery directories and
the existing bounded source reader. The callback admits the complete proposed inventory
before mutation; the editor checks its current baseline and compiles dependencies through
`kedra_authoring::source::reconcile`. No alternate path registry or transaction engine is
introduced. Rename also moves between existing directories, preserving bytes and sidecar UUIDs.
File/sidecar pairs use the existing `fs-transaction` journal; directory moves use one native rename.
The entry storage adapter delegates transaction I/O to `StdFs`, but uses Save's
write-capable file flush on Windows: upstream 0.4.0 opens read-only handles that
cannot call `FlushFileBuffers`. Directory-flush limitations remain unchanged;
no journal format, rollback algorithm or relaxed durability mode is introduced.
Moves into the source itself or its descendants, and file-type changes, refuse.
Existing destinations, root mutation, linked paths and pending Save recovery refuse.

Delete detaches the entry into a retained `.kedra-deleted-*` sibling of
the asset root. This recovery copy is outside the runtime/source inventory; automatic
cleanup and in-editor restoration are not implemented. Entry operations are immediate
filesystem edits, separate from document Save and document Undo/Redo. They do not rewrite
logical references, explicit catalogs or immutable deliveries; admission refuses changes
that would break source dependencies. Case-only renames can require an intermediate name
on case-insensitive filesystems. The existing advisory/non-hostile-writer limits apply.
An interrupted file/sidecar transaction blocks ordinary source admission. Its journal
has no document Save intent: `recover-save` refuses it, preserving records for explicit
filesystem inspection. Automatic entry-transaction restart recovery is not implemented.

## Verification

Public contracts exercise real Unicode files, permissions, exact retries, conflicts,
inventory changes, symlink redirection, lease contention and invalid requests. Private
contracts cover real-file groups, guarded rollback, interrupted intent/replay and refusal
of foreign journal operations. `kedra_authoring` owns preparation/receipt/newer-revision
safety. Editor tests
own asynchronous completion, pending retry/discard/close policy and retained history.
The Inspector native scenario owns actual Cmd/Ctrl+S, draft refusal and source writes
on a complete private fixture, including an external-write conflict.
See [ADR 0039](../../docs/decisions/0039-existing-source-save-groups.md) for dependency
selection and the remaining multi-document native/power-loss acceptance limits.
