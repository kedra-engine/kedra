# ADR 0037: Existing-document Save through a guarded filesystem adapter

Status: implemented; owner review pending. Group publication/recovery is governed by
[ADR 0039](0039-existing-source-save-groups.md). New documents, managed rename/delete
and complete settlement remain outside this profile.

## Decision and boundaries

Keep canonical preparation and acknowledgment in `kedra_authoring::persistence`.
`prepare_existing_save` shares its native compilation, coupling, revision and exact-byte
checks with full Save, but preserves the admitted inventory and catalog. Do not silently
split a coupled transaction or create a catalog as a side effect of saving one field.

The independent authoring-layer `kedra_authoring_fs` owns actual filesystem replacement,
with the editor as a real consumer. It depends only on existing maintained/native APIs:
native document validation, `walkdir`, `tempfile`, SHA-256, standard file locks and the
transaction dependency selected by ADR 0039. Runtime
and the transport-independent authoring model remain free of filesystem write authority.
Keeping this mechanism in the editor would conflate physical publication with UI lifecycle.
Do not introduce another registry, walker or generic transaction framework.

The host explicitly authorizes Save of the active project document, rejects unsettled
Inspector drafts, pins one request and runs bounded filesystem work off the UI thread.
It acknowledges only the submitted revision after successful publication; newer accepted
edits and source history remain intact. Save has visible progress/error/success feedback
and dirty tab markers. Cmd/Ctrl+S works in settled native text fields without accepting a
draft. Save All and implicit Enter/blur acceptance are not part of this command.

## Safety and outcomes

The writer checks exact source/asset hashes and complete membership before group publication
and recovery. It refuses source/ancestor/lease symlinks, special files, read-only source files,
unsupported writes and inventory changes. A regular stable sibling lease coordinates
cooperating editor instances/readers. `tempfile` replacement preserves permissions;
the group writer owns durable intent, journal and explicit recovery under ADR 0039.

This is hash-guarded local publication, not a hostile-filesystem CAS primitive. Advisory
locks do not isolate uncooperative writers from the final check/rename interval. Non-Unix
directory durability and other filesystems/platforms are not certified by native macOS
acceptance. There is no automatic external overwrite or recovery of unsaved working history.
An interrupted group may contain old/new whole files until explicitly recovered; ordinary
Kedra admission refuses that state rather than treating it as a complete source snapshot.

A known refusal releases the pinned operation without changing working values/history.
An uncertain result retains the exact operation/bytes and blocks discard/ordinary close.
Retry rechecks preconditions and acknowledges exact already-published bytes without another
write; a conflict during uncertain reconciliation never means the original Save was refused.

Save advances the source baseline used by subsequent edits/discard. The original renderer
admission snapshot remains pinned separately: saving a source is not an asset rebuild or
permission to substitute unverified dependencies. Existing retained previews continue to
show accepted working values. A new editor session still requires matching delivery data.

## Verification ownership

Authoring owns preserved-catalog/group preparation and acknowledgment of newer
revisions. The filesystem adapter owns real I/O, permissions, exact retries, lease/content
conflicts and unsupported/symlink rejection. Editor tests own async acknowledgment, source
history and pending outcomes. The existing Inspector native route verifies the actual
shortcut/menu, unsettled-input refusal, disk bytes, saved baseline/discard and conflict
preservation on an isolated copy. Compilation/RPC alone is not native acceptance.
