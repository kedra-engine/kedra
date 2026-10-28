# ADR 0039: Recoverable existing-source Save groups

Status: implemented local-filesystem profile; owner review pending. Physical power-loss,
cross-platform native and multi-document native UI acceptance remain uncertified.

## Decision

Save the active document's complete coupled/dependency group using the existing
`prepare_existing_save`/receipt protocol. Preserve admitted source inventory/catalog,
UUIDs, sidecars, working history and newer edits. Creation, deletion, managed moves,
Save All and recovery of unsaved editor drafts/history remain outside this profile.

Ordinary files cannot be replaced simultaneously. `kedra_authoring_fs` coordinates
cooperating writers and admission readers with the existing sibling advisory lease.
Readers hold a shared lease across source discovery; writers hold an exclusive lease.
Uncooperative tools may observe intermediate whole files; this is not database-style
isolation, a hostile-filesystem sandbox or atomic visibility to arbitrary readers.

The writer durably records the exact request plus captured sidecar read preconditions
in `<source-root-name>.kedra-save.request.ron` before publication. `fs-transaction`
owns the write-ahead journal, rollback and replay algorithms. Its named journal lives
beside the root, outside authored asset inventory. Kedra validates recovery operations
against the pinned source replacements before replay; unknown/foreign operations,
changed assets/sidecars or non-old/non-new target bytes refuse without overwriting them.

The restricted storage adapter only admits exact old/new bytes for existing targets
and its journal. Both normal replacement and rollback reuse `tempfile` atomic
replacement and preserve permissions. Temporary files live in the private sibling
`<source-root-name>.kedra-save.staging` directory (0700 on Unix), so interruption cannot
turn them into apparent authored assets. Crash-orphaned temporaries are not automatically
deleted; they may retain source bytes. Keep recovery records with their source root;
do not move/rename only the root while recovery is pending or sync them across machines.

Known pre-publication refusal or a confirmed durable rollback leaves sources unchanged.
An uncertain result keeps the request and blocks ordinary admission, discard and close.
Retry Save reconciles that same request. After process restart, explicitly run
`kedra recover-save <source-root>` to finish the pinned Save before reopening. Open never
executes recovery writes. Replay rolls forward; later external conflicts require manual
resolution and preserve the evidence. This is recovery of an authorized Save, not auto-Save.

## Dependency choice

Pin `fs-transaction` 0.4.0 (MIT OR Apache-2.0, Rust 1.88 minimum, no default dependencies).
Its ordinary-file change sets, rollback and journal codec match this responsibility;
it remains maintained by Diaryx and used by its products, but is a young, low-adoption
dependency and remains a review risk. Inspect its exact Cargo-resolved source and keep
Kedra failure-path tests; upstream proof claims are not Kedra acceptance evidence.

`atomicwrites` and `atomic-write-file` provide individual atomic replacements, not a
recoverable group. `tempfile` already supplies our replacement primitive. Database or
virtual-filesystem solutions would change ordinary authored source access. Reusing the
maintained transaction engine avoids a custom journal, parser or generic transaction
framework. The small adapter supplies Kedra-specific grants, bounds and preconditions;
the upstream default storage is insufficient for these restrictions and private staging.
The pinned journal codec has no caller-supplied allocation budget. A fixed-header
operation-count guard enforces Kedra's 128-operation ceiling before invoking it;
the upstream codec still owns format, checksum and operation parsing. Dependency
upgrades must revalidate that header boundary and the actual encoded recovery fixtures.

## Verification ownership

Authoring contracts own complete-group preparation and rejection of partial receipts.
The filesystem adapter owns real-file replacement, rollback through its restricted port,
recovery correlation, external/sidecar conflicts and shared-reader exclusion. Documents
owns asynchronous group acknowledgment, retained newer revisions and common history.
The real recovery CLI has an externally bounded process test. Existing native Inspector
Save checks cover the unchanged command/input path; a complete multi-document native
scenario, physical power interruption, remote filesystems and other platforms require
separate acceptance, not an inference from these headless contracts.
