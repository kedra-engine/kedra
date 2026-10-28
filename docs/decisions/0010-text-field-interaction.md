# ADR 0010: Shared single-line text field interaction

Status: full editable interaction proposed for owner review under
[ADR 0009](0009-property-editing-and-inspector.md). The shared field module already
supplies retained read-only presentations for the
[Inspector](../site/en/contributing/contracts/editor/inspector.md);
that subset does not implement this editable acceptance/history protocol.
Opt-in native text history and ordered operations exist in `kedra_text_input`, first
consumed by Inspector search. Editable acceptance, rejection feedback and accessibility
still have unresolved integration requirements. `kedra_ui::fields::TextAcceptance`
implements ordered candidate capture, identity/baseline checks, acknowledgment,
rejection, external refresh and cancellation over the native session. `fields::TextField`
connects Enter/Escape in eligible native windows and freezes mutation during a consumer
response. The independent `inspector-fixture` ordinary-string consumer uses the existing
kit field and property diagnostic; it rejects empty text without editing project data.
The native Transform source adapter also consumes this protocol with staged authoring
acceptance and explicit document history. Tab/blur settlement, owned popups and complete
pending/conflict presentation remain unimplemented.
Do not claim the complete field ready by omitting them.

## Scope and ownership

Build the editable single-line string contract in the existing `kedra_ui::fields`
module, with an independent native field consumer before editable source binding.
The source-bound read-only Inspector already consumes its presentation controls.
Do not create a text-field crate, inspector framework or runtime-to-authoring
dependency for the editable field.

The native widget owns Unicode editing, caret/selection, composition and scrolling.
An ordinary Rust interaction model owns acceptance state, interaction generation
and diagnostics. The consumer supplies accepted values and handles acceptance
requests; it owns domain validation and any source transaction. Presentation binds
these values to retained authored UI and CSS. No document IDs, filesystem writer
or prefab policy belong to the reusable field.

This field is not a code editor, multiline editor, password input, search control
or renaming workflow. Those may compose the same native input and shared protocol,
but their acceptance, normalization and command policies need separate decisions.
Do not automatically change existing startup-form acceptance policy when adding this field.

## Pinned native capabilities and gaps

Use the actual Bevy 0.20 source/API, not assumptions about generic native inputs:

| Area | Existing mechanism | Consequence |
| --- | --- | --- |
| Buffer and selection | `EditableText` wraps a Parley editor | Retain it; do not shadow its caret or text mutations |
| Event timing | `TextInput` queues edits; `EditableTextSystems` applies them later | Enter/blur must not read pre-edit text |
| Clipboard ordering | `pending_paste` can hold later edits behind an unresolved paste | Acceptance waits for the ordered queue or is explicitly cancelled |
| Composition | `is_composing()` and queued IME edits | Check applied and queued composition, including same-batch input |
| Local Undo/Redo | Not implemented in the pinned text input | Required upstream/integration work, not an available capability |
| Length | `max_characters` counts Unicode scalar values | Not bytes or graphemes; direct `set_text` bypasses this guard |
| Rejected input | Character/length checks may reject without a UI-facing result | Visible refusal needs a verified integration seam |
| Single line | `allow_newlines = false` suppresses keyboard newline insertion | It does not by itself reject newlines pasted or set programmatically |
| Read-only | `TextReadWriteMode::ReadOnly` allows selection and copy | Keep it separate from temporary disabled state |
| Escape | Native keyboard handler collapses selection, clears focus and propagates | Field cancellation must use the original target and prevent outer dismissal |
| Accessibility | TextInput supplies a role; text editing accessibility is incomplete | Role presence alone is not screen-reader acceptance |

Current Kedra hydration uses `UiEditable`, `UiCaption`, `TextInput`, `EditableText`,
`TextCursorStyle` and optional `SelectAllOnFocus`. Reuse that authored/native path.
Current `authored::style::editing` sets native read-only mode when
`InteractionDisabled` or `FieldAccess` is present. `FieldAccess::ReadOnly` supplies
an independent host-declared access reason. Extend that shared mode authority for
future editing rather than installing a competing writer. Current access mode does
not imply the proposed draft/acceptance protocol exists.

`NativeInputPlugin` already adapts keyboard, focus and IME to owned windows. Field
integration must compose it, not dispatch a second keyboard stream. The editor's
existing deferred form commands demonstrate checking pending native edits and
generation before submission; extract only reusable lifecycle policy, not startup
form dependencies into the runtime kit.

The current desktop application's resolved feature graph enables `bevy_clipboard`
but not `system_clipboard`. Its internal clipboard service is not proof of OS
clipboard integration. Native-fit work must explicitly review/enforce the required
desktop feature and dependency/license impact, then test actual OS copy/paste.
This ADR does not change Cargo features or claim existing OS clipboard acceptance.

## Decision

### 1. Content and initial resource policy

A field presents exactly one string, or an explicit unavailable/mixed presentation.
Empty is an actual string, not missing or mixed. Preserve Unicode normalization,
case, leading/trailing spaces and significant whitespace. Placeholder or mixed
labels are presentation and never enter the editable buffer or acceptance payload.
No automatic trimming, spelling correction, capitalization or path normalization.

For the first single-line profile, propose these explicit safety limits:

- At most 4096 Unicode scalar values per admitted editable buffer. UTF-8 text is
  therefore at most 16 KiB, excluding storage/layout/history overhead.
- At most 32 retained unsettled field sessions per consumer host. A host is the
  consumer's owning session, not an OS window; floating a view does not reset it.
- Domain consumers may set a smaller scalar limit. Raising the shared ceiling
  requires a measured/profile-specific decision, not an unbounded default.

These are proposed product limits, not measurements or source-file limits. Native
history/layout/clipboard/preedit allocations are not bounded by that arithmetic.
The native-fit gate must establish their limits or document the remaining resource
boundary before readiness. Do not claim a total memory bound from buffer length.

An insertion, selection replacement, paste or IME commit exceeding the admitted
limit is rejected as a whole, with visible feedback. Never truncate a grapheme,
partially paste, or delete the selection before refusing its replacement. Native
navigation and deletion remain possible after refusal. Admit programmatic values
through the same guard before calling native setters; `set_text` is not a validator.

Reject CR, LF, line/paragraph separators (U+2028/U+2029), tab and C0/C1 control
characters in the single-line profile. Do not remove them silently. Combining
marks, joiners and bidirectional text are not rejected merely for being non-ASCII.
Malformed or oversized supplied source values stay intact in the consumer source;
show an explicit bounded read-only presentation rather than truncating and making
the displayed prefix editable. A future multiline/specialized editor can handle
values this profile cannot represent.

At retained-session capacity, a new editing attempt leaves existing drafts intact
and explains that one must be resolved. Never evict the oldest invalid draft.
Diagnostic presentation is bounded and does not interpolate whole source/clipboard
contents into logs. Actual document byte/serialization limits are validated later
by authoring; a string fitting this control is not proof that its document can Save.

### 2. Binding and interaction state

The consumer supplies a stable binding key, accepted string, opaque baseline token,
access mode/reason and string constraints. Native entities are adapter-local.
Begin is caused by the first attempted mutation, not hover, focus, selection or
copy. No-op commands and rejected native insertions do not create source history.

Track these independent facts instead of one flattened state enum:

- Access: editable, read-only or disabled.
- Focus/hover, native pending work and composition.
- Baseline token, local buffer generation and whether accepted text differs.
- Validation: unchecked, pending, accepted, rejected or stale.
- Request generation and optional acceptance request identifier.

Do not duplicate native text every frame as another writable model. A captured
candidate string is an immutable message for a particular buffer generation.
The retained widget remains authoritative for the active local buffer.

### 3. Focus and selection behavior

- Pointer entry places the caret at the native hit-tested position. Double click
  selects a word and drag extends selection through native behavior.
- Keyboard Tab entry selects all for replacement. Returning from an owned popup,
  window reactivation, validation feedback or dock transfer preserves selection.
  Do not use an unconditional select-all-on-every-focus policy.
- Focusing/copying a read-only value is allowed. Disabled fields leave tab order;
  their reason remains available through the surrounding property presentation.
- Long single-line text scrolls horizontally to reveal the caret. Do not replace
  the editable string with an ellipsized display string.
- Reflow, search filtering, section refresh and host transfer must not reconstruct
  an actively retained widget. Hiding/filtering away its context first settles it
  under ADR 0009; a search must not destroy invisible drafts.

Field scope is explicit and includes its owned popup/context menu. A focus change
within that scope does not imply acceptance. Ordinary focus movement outside the
scope requests acceptance only for a changed buffer; a selection/copy-only visit
is not an edit. OS deactivation suspends instead of accepting. Classify the cause
of departure before settling; a raw native FocusLost event is insufficient.

### 4. Acceptance and cancellation

| Action | Result |
| --- | --- |
| Enter while editable and not composing | Request acceptance of the settled current buffer; keep focus |
| Held/repeated Enter | At most one in-flight request for that buffer generation |
| Tab/Shift-Tab | Request acceptance if changed, then use normal native navigation |
| Pointer leaving field scope | Request acceptance if changed; do not force focus back on rejection |
| Escape, without an owning popup or composition | Cancel local session, restore current accepted text and consume cancellation |
| Acceptance returns success | Adopt the acknowledged value/token; clear pending-input indication |
| Acceptance returns rejection | Retain exact buffer/caret and show the reason |
| Consumer token changes while editing | Retain draft, mark stale, prohibit automatic reapply |
| Consumer token changes while idle | Refresh accepted presentation without emitting a user edit |
| Access becomes read-only/disabled mid-edit | Freeze and retain draft, cancel pending acceptance where possible; never accept on blur |

Restoring current accepted text on Cancel is deliberate: if another view changed
the value, do not restore an obsolete baseline into the document. Cancellation
never writes a reverse source transaction. A programmatic refresh cannot masquerade
as acceptance of a user edit.

After Enter succeeds, a subsequent mutation starts a fresh session against the new
token even while focus stays in the same field. A later blur without further
mutation cannot create a duplicate operation. A no-op candidate completes without
advancing source history or dirty state.

For asynchronous acceptance, lock text mutation for that candidate while retaining
selection/copy and visible pending status. Before any authoritative publication,
the consumer checks that the request/session/baseline is still current. Cancel
retires that request; if publication already won, retain the accepted value and
do not promise rollback. Stale completions may report their actual outcome but
cannot overwrite a newer buffer or refocus a retired view. No unrestricted task
gets a mutable document reference from a field callback.

### 5. Native event ordering and IME

```text
window-owned input -> native edit queue -> native text edits applied
  -> settled buffer snapshot -> acceptance intent -> consumer validation/commit
    -> acknowledgment -> retained presentation update
```

Capture Enter/blur requests with the original binding, generation and an input-order
boundary. Process them after preceding native text edits, not immediately in the
keyboard callback. If those edits or paste remain queued, keep the request pending;
do not submit a previous valid string while the final typed character is still in
the native queue. Preceding queued edits belong to this request; subsequent edits
must not silently enter its candidate. The native-fit fixture must prove this
boundary or identify the required upstream seam. Retire requests on target loss
or a superseding interaction instead of accepting an unintended candidate.

IME preedit belongs to composition, not validation or source preview. Enter,
Escape and Tab consumed by composition must not also accept, cancel or navigate
the property. Include queued Preedit/Commit events in the same-batch checks:
`is_composing()` alone is not sufficient before native edits run. An IME commit
updates the buffer; it is not automatically property acceptance. Losing OS focus
must not synthesize an IME commit. Retain committed text; composition cancellation
follows the native backend and is not promised to survive a cross-window transfer.

Copy failures leave selection/buffer intact and report failure. Cut deletes only
after clipboard write succeeds. Paste errors, forbidden characters and over-limit
replacement retain the previous buffer/selection and show feedback. Pending paste
cannot later insert into a replacement binding after Cancel or retirement. The
existing native queue stays the ordering authority; do not fetch clipboard text a
second time to guess whether the native paste succeeded.

### 6. Local history and the selected native adaptation

While an editable text field owns input, command Undo/Redo belongs to local text
history even when it is empty. It must never fall through to document history.
Accepted text creates a consumer transaction only at the acceptance boundary;
keystrokes, caret movements, failed validation and local undo create no such entry.
Crossing an acceptance boundary starts a new local-history epoch so local undo
cannot reverse an already accepted source operation behind the consumer's back.
Document Undo remains an explicit host command after settlement under ADR 0009.

The pinned Bevy text editing module explicitly lists Undo/Redo as unimplemented;
its `TextEdit` actions and keyboard mapping have no Undo/Redo operation. Keeping a
native editor alive is therefore not sufficient to meet this requirement.

Prefer an upstream native-input history implementation or a maintained compatible
integration which preserves the existing text engine. Before selecting either,
evaluate Unicode/IME grouping, selection restoration, cut/paste ordering, bounded
history, license, maintenance, dependency cost and all claimed desktop targets.
The selected incremental implementation is a Kedra-owned copy of the operation
layer, not a Bevy fork or a second text engine. Runtime-layer `kedra_text_input`
adapts insertion, clipboard sequencing and shortcut matching from Bevy 0.20.0;
native widgets and Parley retain interaction, rendering and buffer ownership.
Its [crate contract](../../crates/kedra_text_input/README.md) records exact source
provenance, licensing, compatibility rationale and remaining limits. Direct
clipboard/Parley dependencies reuse versions resolved by the pinned Bevy release;
no toolchain or system-clipboard feature change is selected.

History captures native states at individual mutation boundaries, never by polling
one snapshot per frame. There are at most 32 entries; preedit is excluded and an
IME commit forms one step. Native selection is restored without reconstructing byte
carets. Typing coalescence is not implemented. The first product consumer is Inspector
search, not authoring fields; other inputs retain their existing behavior. Clipboard
timeout, total allocation bounds, acceptance/blur policy and reviewed error states
still block the complete shared TextField. This adaptation relaxes none of those
requirements and does not authorize source editing by itself.

### 7. Visual and accessible presentation

Use existing TextField masters `49:536` and examples `51:1497` from the
[UI-kit source map](../site/en/contributing/contracts/ui/design-provenance.md).
The original example is 172 by 26 logical units;
current compact presentation uses the shared 22px control-height token. Width
participates in native layout rather than being fixed in all uses.
Use CSS tokens for typography, surfaces, borders and state styling. Do not bake
those values into the interaction model or add a manual text-layout path.

| Figma example | Semantic interpretation |
| --- | --- |
| Default | Accepted value with no current interaction |
| Hover | Pointer affordance, not Begin or validation |
| Focus | Keyboard focus independent of value validity |
| Editing | Native caret/selection and current local buffer |
| Invalid | Retained rejected buffer with an explanatory message |
| Mixed | Multiple values, not the literal editable string `Mixed` |
| ReadOnly | Inspectable/copyable value with a reason |
| Disabled | Temporarily unavailable interaction |

Figma states are examples, not exclusive states: retain visible focus on an invalid
field. Do not communicate rejection, pending work or read-only access by color
alone. Provide a persistent associated label and diagnostic text; placeholder is
not the accessible name. Use native text direction and caret geometry. Keyboard
users must reach diagnostics and settlement actions without hover or a focus loop.

Missing reviewed compositions include focused-invalid, pending acceptance, stale
draft, over-limit/paste refusal, access loss during editing and context-settlement
actions. Screen-reader value/selection/edit announcements need a native integration
check beyond role assignment. Do not claim these compositions or accessibility
behaviors exist merely because the eight baseline examples exist.

## Implementation boundaries and acceptance gates

Use coherent modules within `kedra_ui::fields`: ordinary interaction state,
native-input binding, and authored presentation. These are responsibilities, not
a requirement for a fixed file/API layout. Extend existing hydration/mode handling
where necessary; do not run parallel authorities or add field policy to startup.
Schema changes to serialized authored components need explicit version handling;
do not reinterpret existing `UiEditable` documents silently.

Work proceeds in separate reviewable slices:

1. **Native fit and gaps.** Demonstrate queue/blur/IME/Escape ordering, native
   clipboard features, cause-sensitive selection, access-mode ownership and visible
   input refusal in an isolated fixture. Resolve the text-history strategy before
   declaring the complete control implementable. No source editor is needed yet.
2. **Interaction model.** Test generation/acknowledgment/cancellation, pending input,
   token conflict, session capacity and value echo with a consumer-owned string.
3. **Native authored control.** Bind the model to existing input, CSS and semantic
   observation IDs. Preserve native state through refresh and floating host moves.
4. **Complete field acceptance.** Supply local history, reviewed missing states and
   applicable accessibility integration; verify actual physical input and frames.
5. **Source-bound consumer.** Connect the accepted-string protocol to authoring
   sessions separately. Test one accepted edit equals one source transaction.

Required regression scenarios include final-character-plus-Enter, Enter-plus-blur,
Escape-plus-blur without an accidental acceptance, async refusal/cancellation,
same-batch preedit/Enter, IME commit without property acceptance, invalid blur then
Tab, Unicode combining/emoji/bidirectional selection, cut failure, newline paste,
exact/over-limit selection replacement, oversized supplied source, repeated source
echo, stale token, view deletion, read-only transition and retained host transfer.
Assert accepted value, exact draft, selection, consumer operation count and source
history where applicable, not only rendered text.

Native evidence must identify platform, actual physical versus injected input,
IME/clipboard availability and unsupported cases. Headless checks do not certify
caret, text selection, screen readers or OS IME. Keep generated evidence under
`target/`; run `cargo xtask check` and relevant feature/integration checks for
implementation slices. This ADR alone supplies no native acceptance evidence.

## Consequences

The shared field can serve many inspectors without knowing their documents. Strict
acceptance and retained input avoid data loss, but require explicit event ordering,
consumer acknowledgment and more native integration than a styled text box.
The current upstream gaps are real prerequisites, not permission to reduce the
quality contract. Multiline input, search/rename policies, numeric parsing and
property-editor registration remain separate specifications.

## API references

- [Pinned editable buffer API](https://docs.rs/bevy/0.20.0/bevy/text/struct.EditableText.html)
- [Pinned text editing capabilities and limitations](https://docs.rs/bevy_text/0.20.0/src/bevy_text/editing.rs.html)
- [Pinned text actions](https://docs.rs/bevy/0.20.0/bevy/text/enum.TextEdit.html)
- [Pinned native input widget](https://docs.rs/bevy/0.20.0/bevy/ui_widgets/struct.TextInput.html)
