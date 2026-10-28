# Ordered native text input

Runtime-layer `kedra_text_input` owns opt-in native operation admission, clipboard
sequencing and local Undo/Redo. It knows no Inspector, authoring source, Save or
filesystem. `kedra_ui::NativeInputPlugin` installs it and forwards eligible history
shortcuts before native focused dispatch. Inspector search and supported editable
property fields consume this operation layer through `kedra_ui`.

## Native boundary

Bevy `TextInput` retains keyboard/pointer/focus behavior and rendering. Parley owns
the only mutable text buffer, selection, Unicode operations, layout and composition.
Navigation delegates to upstream `TextEdit::apply`; no second text engine is installed.

`TextSession::attach` admits the current buffer with a 1..=4096 Unicode scalar limit.
Controls, tabs, CR/LF and Unicode line/paragraph separators are refused, not removed
or normalized. Selection replacements are checked before deleting their contents.
This profile owns its filter: do not combine it with `EditableTextFilter`, whose
private upstream closure cannot be inspected by a replacement processor. Unmanaged
inputs keep their existing behavior.

Native edits transfer into the session before Bevy's batch processor. The session
owns pending clipboard reads; later operations wait behind the same read, fetched
once. Refused insertion/clipboard operations preserve the buffer and selection and
emit `InputEvent`, without source/clipboard contents in diagnostics.

`request(Read(id))` captures text exactly between preceding and subsequent native
edits. This is **not acceptance**. A refused mutation blocks reads of the previous
value until correction or explicit reset. Binding identity, generation checks,
validation and acknowledgment are consumer responsibilities. Do not write sources
from an unvalidated read notification.

Local history stores native Parley states at **mutation boundaries**, never one
snapshot per frame, with at most 32 undo/redo entries in total. Separate inserts in
one frame remain separate steps; typing coalescence is not implemented. Preedit
does not create history; one IME commit forms one step. Cancelled/rejected composition
restores the native pre-composition state, including the original selection.
Restoration reapplies current host styles, scale, alignment and viewport width.
Restored layout generations are checked against Bevy's last rendered generation;
an alias is invalidated through the native layout API so old glyphs cannot survive
a correct buffer undo.

Retain both the native input and session across host moves. Reset history after a
consumer acknowledgment. `replace` validates a host-supplied string before changing
local state; it is not a source acceptance operation. `discard` revokes local queues,
paste and history, never
reverses a source transaction. Retiring the entity drops its work. Detected external
text replacement revokes queued work instead of appending a pending paste into a
new binding. Explicitly discard before rebinding even if both strings are identical.
Use native edits before the managed PostUpdate processor and the session API for
control requests; do not apply or enqueue raw native edits between this processor
and Bevy's stock processor.
`validate_value` applies the session's exact admission limit without mutation.
`reset_history` synchronously starts an acknowledged epoch while preserving the native
selection; it refuses unsettled work or exhausted generations without changing state.
Explicit replacement/discard and history epochs advance the local generation;
exhaustion refuses work rather than wrapping a token. Consumers must also reject
late notifications for a retired/replaced binding.

## Provenance and dependency rationale

Adapted portions come from **Bevy 0.20.0**, crate-release VCS revision
`b3bdd75becaffe9a16677cf5bae5bd6d198f052b`:

- `crates/bevy_text/src/editing.rs`: ordered clipboard barrier/processing pattern;
- `crates/bevy_text/src/text_edit.rs`: selection-aware insertion and cursor reveal;
- `crates/bevy_ui_widgets/src/text_input.rs`: logical/physical shortcut matching.

Upstream: <https://github.com/bevyengine/bevy>. Adapted files identify provenance;
`LICENSE-MIT` retains upstream's MIT permission text. Upstream offers MIT OR
Apache-2.0; adapted portions are redistributed under its MIT option. New Kedra code
uses the workspace's MIT OR Apache-2.0 license. Review exact APIs and copied portions
when upgrading Bevy/Parley.

The copy is limited to the operation layer, not Parley, widget rendering or the
whole native input module. Bevy's public batch processor lacks per-operation
results, local undo and interleaved read boundaries; a per-frame observer cannot
supply these guarantees. `bevy_simple_text_input` is deprecated, while
`bevy_ui_text_input` uses Cosmic Text instead of the pinned Parley pipeline.
Neither is a compatible replacement. The upstream undo goal is tracked at
<https://github.com/bevyengine/bevy/issues/23354>.

Direct `parley = 0.11.1` and `bevy_clipboard = 0.20.0` dependencies name packages
already resolved by Bevy 0.20.0, adding no new text engine or platform backend.
Features do not enable system clipboard access.

## Verification and limits

```sh
cargo test -p kedra_text_input --locked
cargo xtask test -- features --only inspector,inspector-fixture --output target/test/text-history
```

Headless contracts exercise native selection, ordered reads, refusal, text history,
IME grouping/cancellation, clipboard readiness/ownership and access/binding loss.
They use the canonical test project's licensed Inter font. Native Inspector scenarios
also exercise search Undo/Redo and retained host transfer using injected input and
frames, not physical OS input.

This is the operation layer, not the complete editable TextField.
`kedra_ui::fields` adapts ordered reads and history epochs to live/Enter/Escape and
blur settlement, including grouped vector gestures and consumer acknowledgment.
The editor separately validates source proposals, owns source history and coordinates
cross-view drafts; those capabilities do not belong to this crate.
Search keeps its existing acceptance policy. Physical
IME, OS clipboard failures, screen readers and other desktop platforms need separate
acceptance. Web/mobile is not certified. Pending clipboard timeout and bounded total
queue/layout/history memory remain unresolved; scalar/entry ceilings are not a total
allocation bound. These operation contracts alone do not authorize source editing.
