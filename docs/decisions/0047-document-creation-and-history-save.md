# ADR 0047: Document creation through source history and Save

Status: implemented source/editor profile; owner review and native acceptance pending.

## Decision

Create documents through the existing authoring workspace, not by generating files
from an editor button. `source::create_document` validates an isolated candidate,
reserves its exact source-relative path and publishes one named `WorkingSet`
transaction. Scenes and prefabs share this mechanism. The editor's New menu offers
empty Scene and Prefab documents, without a game-specific template or runtime.

Document/node UUIDs are allocated once and retained by Undo/Redo. A rejected path,
identity, compilation or cancelled name form publishes neither history nor source bytes.
Existing documents and their ordered history remain in the same workspace. The editor
uses an editor-owned name form in the current Assets folder and ordinary
document tabs, Hierarchy, Inspector and Scene View for presentation. Creation invokes
the existing Save writer before opening a clean tab; subsequent edits require explicit Save.
Publication failure retains the pending identity for retry without opening a successful tab.
Existing filenames refuse without an overwrite flow. Parent directories
must exist. Retiring the project retires its pending creation work.

## History and Save

Creation is an undoable source operation under ADR 0038. Undo before first Save removes
the working document; Redo restores its exact identity, content and reserved path.
Undo after Save leaves a dirty source removal. Only a subsequent explicit Save publishes
that removal. Redo can recreate the same source identity. A removed document's retained
tab cannot expose stale source nodes or a live preview. Discard restores the acknowledged
workspace, including path ownership. Dirty includes changed content and removed saved
documents, independently of revisions and open views.

History survives Save. Receipts acknowledge pinned content only; newer edits, including
Undo during an in-flight creation Save, remain dirty. Selection/focus attachments use
existing `HistoryId` entries, not another Undo stack or runtime callbacks.

`prepare_document_save` prepares complete coupled/dependency groups, including new
documents and removals. It validates the actual post-commit graph and pins exact bytes,
presence/absence and unchanged source/asset checks. Inventory changes require an
index-free source scope. Runtime catalogs remain ordinary admitted assets; they are
not implicitly rewritten or changed by document creation. `prepare_existing_save`
retains its explicitly inventory-preserving API contract.

`commit_documents` extends the existing filesystem lease, request/receipt and
`fs-transaction` 0.4.0 journal adapter. New names use exclusive publication, never
overwrite. Recovery remains explicit through `kedra recover-save` and
`recover_documents`; Open never performs recovery writes. The replacement-only writer
retains its narrower capability contract.

The pinned transaction library moves removed files aside for rollback. The restricted
storage port maps only its exact bounded removal-backup names into the existing private
sibling staging directory. Backups cannot become authored inventory members. Hash and
operation grants apply to backup restoration/removal too. This port depends on the
pinned upstream naming convention, as the journal allocation guard depends on its codec;
upgrades require real rollback/recovery tests. No new journal, scanner or transaction
framework is introduced.

## Owners, preview and limits

Native input, settlement and project lifetime belong to the editor; source candidates,
history and Save preparation to `kedra_authoring`; publication/recovery to
`kedra_authoring_fs`. These are extensions of cohesive existing owners and need no
new crate or dependency. Runtime dependency boundaries remain intact.

`kedra_authoring_assets::compile_preview` admits resource-free graphs without a project
delivery. Any asset request without a delivery refuses the candidate. An explicit
delivery still undergoes existing source/identity/hash checks. Initial presentation and
accepted-value refresh share this compilation path. Source validity, dependency readiness
and actual native frames remain separate guarantees.

Managed asset deletion with sidecars, directory creation, rename/move, import, runtime
catalog editing, Play and unsaved-session recovery remain separate capabilities.
Removal with a sidecar refuses rather than deleting metadata without an asset-owner
protocol. New scene does not implement visual node/component creation.

Authoring tests own identity/path/history and receipts; filesystem tests own exclusive
publication, conflicts, coupled rollback and private-backup recovery. Editor tests own
activation, discard, asynchronous Save/reopen and retained history. Resource admission
tests own resource-free preview and missing-delivery refusal. Native input/frames,
physical power loss and other-platform behavior require separate acceptance evidence.
