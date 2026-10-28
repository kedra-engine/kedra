# ADR 0038: Editor edit transactions and script authoring

Status: implemented admitted-document profile; owner review pending. Native acceptance
of explicit project-script launch and multi-document Save remains pending.

## Decision

All accepted authoring mutations use one project working-set transaction/history
boundary. Inspector, future hierarchy/gizmo tools and explicitly authorized editor
scripts are clients, not history owners. Commit publishes working sources and one
named Undo entry; it is neither filesystem Save nor a Git commit.

`kedra_authoring` owns isolated source candidates, revision/lineage checks, atomic
multi-document commit and source history. Keep this cohesive responsibility with
`WorkingSet`; do not introduce a second journal or arbitrary World-mutating undo
callbacks. Native property sessions continue through that same publication route.

Each history entry has a stable operation identity and label. Gesture grouping keeps
the original entry identity. Restoring source never executes the original script:
its code, runtime or panel may no longer exist. Errors/no-ops preserve Redo; failed
validation leaves the candidate available but publishes nothing. Dropping an open
candidate cancels it. Source revisions remain monotonic across Undo/Redo.

The editor grants a finite source scope and owns settlement, current project lifetime,
typed catalog validation and publication. A script receives only scoped edit methods;
it cannot obtain a mutable World, write files, Save, load arbitrary project code or
implicitly apply an instance override to a prefab source. Every resumed mutation and
commit rechecks lifetime/revisions. Persistent source addresses are not UI entities.

The Luau bridge has an independent responsibility: adapt an authorized native edit
lease to the existing `kedra_script` runtime. It belongs in `kedra_authoring_luau`,
not Inspector or the runtime. Use existing mlua/serde/Bevy reflection, not another VM,
value language, parser, scheduler or source registry. The editor supplies its real
backend. Explicit project-script loading is available; automatic execution remains unavailable.

Trusted Program bindings install `kedra_editor::edit_scripts::install` with exact
source-qualified path grants. The existing read-only debug scripting API acquires no
authoring capability. The host reuses Documents compilation/publication; typed values
reuse PropertySession, and native property gestures use named working-set history.
The Lua profile exposes nodes, reflected property requests, renames, node removal,
reparenting, component removal, commit and cancel. Each staged request must be valid; temporarily invalid cross-field
batches are not exposed. Explicit successful commits survive later script failures.

## Boundaries and follow-up

The first backend edits admitted scene/prefab working sources. Generic filesystem
effects, managed asset moves/deletions and project-settings transactions need their
owners' explicit reversible protocols; this ADR does not make arbitrary I/O undoable.
Text-local Undo remains separate; source history and Save never consume invalid text.

Editor-context attachments are keyed by transaction identity, with logical selection/focus
and value-free collection mappings. Closed panels do not prevent source Undo. Attachments
are pruned against retained source history; no value-matching journal is maintained.
Dirty compares authored content with pinned acknowledged content, independently of the
monotonic revision clock. History stores affected-document before/after deltas, bounded by
256 operations / 64 MiB encoded payload by default. Oversized operations refuse before
publication; policy limits describe encoded bytes, not total allocator use.

One explicit `--editor-script Editor/path.luau` invocation requires an explicit project
and repeated `--edit-source game://path` grants. The editor snapshots bounded regular
Editor/ sources, using existing standard Luau modules and runtime budgets. Project Open
alone executes nothing. Source loading is not a sandbox against concurrent hostile
filesystem replacement. Status is observable and native hosts can cancel queued/running
invocations; no arbitrary remote eval or persistent project trust is introduced.

## Verification

Authoring contracts own scoped isolation, stale/foreign rejection, multi-document
atomicity, named history, grouping and no-op/Redo preservation. The editor owns typed
property semantics, instance overrides and publication into its actual Documents.
Real authorized Luau contracts exercise commit, exception/drop, cancellation and
stale resumption without source writes before commit or filesystem writes afterwards.
Existing native Inspector scenarios guard the first UI client's interaction contract.
