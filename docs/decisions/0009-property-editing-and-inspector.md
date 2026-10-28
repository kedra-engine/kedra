# ADR 0009: Property editing, shared fields and inspector ownership

Status: full editing contract proposed for owner review. Shared `kedra_ui::fields`,
`kedra_inspector` and the source-bound read-only Inspector exist; see the
[current profile](../site/en/contributing/contracts/editor/inspector.md).
Typed source property candidates exist in `kedra_authoring::properties`; they do not
implement native input buffers, preview rendering, cross-view settlement or filesystem
Save. The native Transform adapter consumes these candidates with explicit Enter/Escape,
in-memory publication, renderer refresh and source Undo/Redo. General field/blur settlement,
Save and general project-defined drawers remain unimplemented. The embedded UiCaption
Luau profile is specified by [ADR 0036](0036-scoped-luau-inspector-drawers.md).
Semantic names below specify the full
requirements beyond that narrow profile.

## Context and scope

[Stage 1](../roadmap.md#1-scene-and-prefab-authoring) needs a reusable editing
contract, not independent controls which happen to mutate components. Build it
in small verified increments without a temporary MVP architecture. The first
real authoring consumer will inspect Kedra's own UI scenes and prefabs in an
independent preview. This does not authorize live modification of the running
editor shell, project code execution or a general extension-loading framework.

Existing foundations remain authoritative:

- `kedra_scene` owns source schemas, composition and compilation over Bevy reflection.
- `kedra_authoring::workspace::WorkingSet` owns validated source transactions and
  undo/redo. `transact_checked` already rejects stale document revisions and
  avoids history entries for unchanged sources.
- `kedra_authoring::persistence::Workspace` owns the Save protocol, not disk writes.
- `kedra_ui` owns authored retained UI, native input adaptation and Flair styling.
- `kedra_docking` owns placement, never document edits or close approval.

The working-set transaction API commits a complete accepted edit. Property sessions
separately retain cancellable typed candidates, with exact dependency revisions and
one accepted transaction. Native gesture/input integration and candidate preview
rendering remain unimplemented. Do not simulate them by appending an undo entry per
pointer movement or by rewinding shared history on Escape.

## Behavioral goals

Kedra's property editing contract must provide:

- Source-bound edits integrated with validation, undo/redo and prefab overrides.
- Independent input buffers which survive source refresh without losing drafts.
- Explicit acceptance boundaries for typed input, including invalid-input and
  focus-scope behavior.
- Type-specific and field-specific presentation over the same editing protocol.
- Reusable controls with subfield editing, read-only state and focus recovery.
- Cancellable isolated previews without provisional source or history mutation.

The decisions below assign these responsibilities and specify how to verify them;
the contract does not depend on another editor's behavior.

## Decision

### 1. Separate four kinds of state

| State | Owner | Meaning |
| --- | --- | --- |
| Input buffer | Shared field interaction model | Exact text, caret/selection and uncommitted input; may be incomplete |
| Edit session | Authoring property adapter | Target, baseline revisions, proposed typed changes and validation result |
| Working source | Existing authoring workspace | Last accepted document revision and undo/redo history |
| Rendered preview | Preview host | Disposable projection of an identified validated candidate or working revision |

Save serializes working sources, never input buffers, provisional candidates or
rendered entities. A pending input indication is distinct from document dirty.
An unchanged value may change text formatting without changing source revision.
The current revision-based dirty semantics remain unchanged; content-equivalent
undo to a saved value is not newly promised to clear dirty.

Native Bevy text input remains the authority for caret, text selection, clipboard
and composition. Local text undo is a requirement, not a currently available
Bevy 0.20 feature; [ADR 0010](0010-text-field-interaction.md) records that upstream
gap and the implementation gate. The field adapter must not maintain a competing
text editor. Retention captures the native buffer state through supported APIs;
if faithful restoration is unavailable, keep the retained widget alive rather
than silently reconstructing a lossy replacement.

### 2. Give every edit an explicit source address and lifetime

An authoring binding identifies the owning workspace session, destination document,
existing persistent node/instance address, canonical component key/schema version
and reflected field path. View identity and edit-session generation additionally
identify the interaction. Entity IDs, display labels and row positions are not
property identity. Reuse existing node addresses, override paths and Bevy reflected
paths; do not invent another path language or parser.

The binding distinguishes the displayed effective value from its edit destination:
local source, instance override or explicitly opened prefab source. An edit of an
instance never silently becomes an edit of its prefab source. Inherited values,
override presence, read-only reasons and unsupported representations are separate
facts. Reset to a type default, Revert override and Apply to source are distinct
commands, not three names for assigning the visible value.

At Begin, capture the working destination revision, required transitive source
revisions, represented type and schema, baseline value and session generation.
Revalidate the entire candidate catalog through existing policies before acceptance.
Changes to any captured dependency invalidate the session, even when the visible
field happens to look unchanged. Do not automatically rebase the proposed write.

Late results carry session, generation and candidate sequence. A result for a
retired view, older candidate or different document cannot publish or refocus UI.
Deletion, schema change and loss of write permission make an active edit conflicted
or unavailable; retain its buffer for inspection/copy, never resurrect its target.
Idle controls refresh from sources without emitting edit events. Editing controls
are not overwritten by polling, reflection refresh or their own value echo.

### 3. Use one semantic interaction protocol

Fields report Begin, Propose, Accept and Cancel intents; these are semantic names,
not a frozen Rust API. Native widget notifications are adapted into this protocol.
Input text may be incomplete, syntactically invalid, typed-but-domain-invalid or
valid. Authority separately tracks pending validation, rejection and conflict.
Do not collapse those facts into a single visual-state enum.

| Interaction | Required behavior |
| --- | --- |
| Type in a single-line text/number field | Retain exact buffer; no source write per keystroke |
| Enter outside IME composition | Parse and request one validated acceptance |
| Tab/Shift-Tab or pointer focus leaving the field scope | Accept valid changed input once; normal focus traversal continues |
| Incomplete/invalid input loses field focus | Retain buffer and explanation; no default, clamp or source mutation |
| Escape while editing | Cancel the session and provisional preview; restore current authoritative value |
| Checkbox activation or confirmed enum choice | One validated discrete transaction |
| Picker hover/navigation | No source transaction; confirmation accepts, cancellation restores |
| Numeric drag, slider or continuous picker gesture | Candidate preview during the gesture; one transaction at acceptance |
| Pointer capture loss during a gesture | Cancel, unless normal gesture completion was already acknowledged |
| OS window deactivation | Suspend text entry without acceptance; cancel an active pointer gesture |
| Dock move/float without closing the view | Preserve view, binding, buffer and edit-session identity |
| Presentation-only refresh | Preserve active input, focus and caret; no new Begin or Accept |

Focus scope includes a field's owned popup and compound control children. Opening
a picker or moving among vector axes is not automatically leaving that scope.
Each vector-axis edit retains the untouched axes; explicitly grouped vector
operations accept atomically. A vector editor declares its acceptance boundary
rather than emitting whole-vector replacements from stale per-axis copies.
Multiline fields use Enter for newline and the platform command modifier plus
Enter for acceptance. Popup Escape and IME handling take precedence over field
Escape; an event consumed by one owner must not also close a document or undo.

Text history and source history are different. While editing text, native text
Undo/Redo consumes its shortcuts without falling through at an empty text history.
Outside text editing, document Undo/Redo first resolves any active edit. Menu
commands for document history follow the same rule. A continuous gesture is
cancelled before a history command runs; invalid text requires explicit resolution.

Moving to a different inspected target, changing inspector lock target, hiding or
closing a view, document close/reload, and application exit must settle its sessions:

1. Accept valid pending edits in deterministic interaction order before proceeding.
2. If validation fails, becomes stale or a buffer is invalid, keep that context and
   offer Continue editing or Discard pending input and continue. Do not silently
   navigate away or discard the draft. No unconditional focus-stealing loop.
3. Explicit discard removes only the affected input/candidate, not accepted changes.

Save also settles sessions bound to its affected documents, across inspector views.
If any cannot settle, do not start the Save and report where resolution is needed.
Settling several independent fields is not an all-or-nothing batch: already accepted
edits stay in history if a later field fails, and the UI reports that result.
The authoring host serializes settlement and stops on a conflict; it does not
silently retry against a newer revision. Sessions are retained in memory, not
promised recoverable after a crash. Cross-view draft recovery needs a later decision.

### 4. Validate before publication; preview without document mutation

```text
native widget input -> shared field intent -> source-bound edit session
  -> typed candidate -> source/policy/catalog validation
    -> optional isolated preview (not dirty, not saved, not history)
    -> Accept + exact revision checks -> WorkingSet transaction
      -> accepted source projection + document history
```

A drag is baseline-to-final, not a series of committed values merged by a timer.
Use an independent candidate catalog/compilation for provisional display. Initial
text entry may be delayed with preview only after acceptance; continuous controls
must not ship until they have cancellable preview and one-step history.
Only the final successful source transaction clears redo and advances revision.
No-op acceptance and cancellation do neither. Failed final validation retains the
buffer and working source, removes provisional display, and reports the rejection.

The host owns at most one provisional candidate for a given preview surface.
Starting another gesture first settles the previous interaction. Independent
views may hold drafts; concurrent commits use the same revision guards, not
last-writer-wins. Disjoint-field automatic rebasing is deferred.

Source acceptance and renderer readiness are separate. If an accepted source
cannot be rendered, keep that source/history and report the preview failure.
Never label old pixels as the current revision. Pending/failed projection is
visible; recovery must not rewrite sources or pretend that Save failed.

### 5. Keep the field kit reusable and preserve native types

The shared kit supplies text, numeric and discrete interaction building blocks,
then compound vector/color/reference controls. It knows values, formatting,
constraints and interaction lifecycle, not scene selection, document Save,
prefab provenance or authoring history. Applications can reuse it without
depending on editor or authoring crates.

Keep typed values and Bevy type information. Do not funnel all numbers through
`f32`, turn enum variants into display strings or introduce a second universal
property schema. Use standard Rust parsing/formatting and Bevy reflection; no
expression evaluator, custom RON parser or inspector-owned serializer.

Numeric rules for the first implementation:

- Parse into the exact declared numeric type, with checked range/overflow behavior.
  Empty input, `-` and unfinished exponents are buffers, not zero.
- Support decimal entry and floating-point exponent notation where applicable.
  Use `.` as the explicit decimal separator and no grouping separators initially;
  reject ambiguous localized syntax rather than interpreting it by machine locale.
- Do not silently clamp typed out-of-range input. A slider's visual range is not
  necessarily a domain bound. Domain validation remains authoritative.
- Keep unit suffixes separate from editable text. Unit conversion, if supplied by
  a specialized editor, is explicit and checked; arbitrary expressions are deferred.
- Display rounding must not quantize an untouched source. On entering edit mode,
  expose a round-trip representation; commit compares semantic typed values.
- Respect schema validation for nonfinite numbers; do not silently sanitize them.
  Reject nonfinite continuous-gesture results before preview or acceptance.

Text retains Unicode and significant whitespace. Only an explicit domain rule
may normalize input. IME preedit is not an accepted string; Enter used to confirm
composition must not also accept the property. Native clipboard and accessibility
semantics must remain intact across floating hosts.

Read-only fields explain why and permit inspecting/copying supported values.
Disabled controls communicate temporary unavailability. Unsupported fields remain
visible with their type/reason and safe bounded source display; never invent a
default editable value. Mixed is a presentation capability, not an empty value:
multi-object writes are deferred until atomic target-set semantics are specified.

### 6. Separate responsibilities across package boundaries

| Package/module | Layer | Responsibility and consumer |
| --- | --- | --- |
| `kedra_ui::fields` (existing read-only presentation; editing proposed) | runtime | Reusable field interactions and authored/native projection; consumed by inspector and an independent field fixture |
| `kedra_authoring::properties` (typed candidates implemented; native binding planned) | authoring | Source targets, revision-pinned typed field batches and candidate/accept/cancel semantics over existing workspace/policies; native inspector binding adapter remains planned |
| `kedra_inspector` (existing read-only crate; edits proposed) | editor | Generic Bevy-reflected value discovery, default and specialized editors, property rows and proposed consumer-neutral edit intents; consumed by an independent value fixture and the editor inspector panel |
| `kedra_editor` | editor | Source-binding adapter, document/selection/view lifecycle, preview host, command routing and Save orchestration |
| Existing source workspace and persistence modules | authoring | Accepted source/history and Save request/receipt authority, not widgets or filesystem publication |

Intended production edges:

```text
kedra_editor -> kedra_inspector -> kedra_ui
             -> kedra_authoring -> kedra_scene
kedra_ui -> kedra_scene            (existing authored UI dependency)
```

`kedra_inspector` must not depend directly on `kedra_authoring`, `kedra_editor`,
desktop windows or a filesystem writer. It uses Bevy reflection and a narrow
consumer-supplied value/edit boundary; source documents are one consumer, not its
property schema. Existing authored-UI dependencies do not authorize source writes.
Editor-owned selection and command capabilities are supplied through narrow inputs,
not a global editor service locator. Headless domain models remain ordinary Rust
values; Bevy entities live in presentation adapters.

The inspector crate has real panel/value consumers and inherits the checked
layer/lint/license rules. Keep the kit in the existing UI crate, rather than
creating a crate per control or a forwarding facade. A later extraction needs a
demonstrated independent responsibility; do not scaffold it now.
Do not add a generic inspector-core crate, second reflection registry, scheduler,
DI container or proprietary UI description language.

### 7. Design extension seams without loading project code

Discover value structure from Bevy `TypeRegistry` and reflected values, not a
Kedra-only list of UI components. The generic default editor handles supported
reflected structure for native Bevy, Kedra and project/library types alike. A
custom editor improves presentation; it is not a requirement to inspect a type.
See [ADR 0011](0011-universal-reflected-inspector.md) for this distinction.

The source adapter separately enforces the existing component allowlist and
serialization/lifecycle contracts. Reflection availability is not permission to
persist arbitrary runtime data or execute hooks. Editor-only presentation metadata
refers to Bevy identities; it does not register new serialized schema identities
or make an unregistered component authorable.

Use deterministic editor selection: explicit field specialization, then exact
type specialization, then supported reflected structure, then visible unsupported
fallback. Duplicate registrations at one precedence are diagnostic errors, not
registration-order wins. Component-level inspectors compose the same field edits.
Labels, units, help, ranges and presentation grouping do not authorize mutation.
Neither a custom drawer nor built-in control can bypass candidate validation,
revision checks, history or Save by writing live components.

First use internal ordinary Rust registration with built-in consumers. Project
extension loading, compatibility, trust, retirement and any stable public API
remain stage 3 decisions. No dynamic loading or execution on project open follows.

### 8. Edit the editor as data, not as an active control surface

Open existing editor UI prefab/scene sources into a working authoring workspace.
Inspect and render a separate instance with explicit document provenance. Do not
attach live editor command handlers, filesystem capabilities or automation scopes
to the preview. Preview controls must not execute editor actions or steal input
from the surrounding editor. Interactive preview requires an explicit later mode.
The running shell stays pinned to its current delivered assets.

Save uses explicitly opened writable sources, never installed delivery/cache paths.
It must not rewrite a built delivery in place. Seeing saved shell changes in a
subsequent launch requires the existing asset build/verify pipeline; restart alone
does not rebuild assets. Hot application to the serving editor is deferred.

Layout component data and CSS ownership stay distinct. A scene property editor
does not silently write computed layout into sources or inline a stylesheet value.
Provide provenance/read-only guidance for stylesheet-owned appearance. A visual
CSS authoring tool, stylesheet conflict handling and coupled scene/CSS Save need
a separate design before implementation.

## Reuse and dependency rationale

Use pinned Bevy 0.20 native text/focus/IME, widgets and reflection, existing Flair
styling, RON serialization and Kedra's authored UI. Bevy's [headless widgets][bevy-widgets]
already propose externally managed values. Retain their event/focus ownership;
do not replace the text engine or maintain a second focus tree.

Evaluate [FeathersNumberInput][bevy-number] before implementing numeric interaction:
it supports typed values, text and dragging, and final/nonfinal notifications.
Its documented value variants are `f32`, `f64`, `i32`, `i64`; that is not universal
numeric coverage. Reinserting values replaces its text buffer, so blindly echoing
source values does not satisfy incomplete-input retention. Its scene-component
construction and styling also need an authored/Flair compatibility check.
Prefer adaptation where those contracts can be retained; otherwise reuse native
TextInput and standard parsers with narrowly scoped field-session policy. This
ADR does not yet select an additional dependency or authorize copying its code.

Bevy/Feathers are maintained upstream and MIT OR Apache-2.0, matching the pinned
engine. Adding Feathers features still requires a resolved dependency, license,
advisory and platform review. No external editor code is imported.
An egui-based inspector would add a second rendering/input stack and does not
satisfy authored native/Flair composition; it is not the production adapter.
Kedra-specific source bindings, revision policy and override destinations remain
custom because generic widgets do not own these project contracts.

## Figma contract

Use the [current UI-kit source map](../site/en/contributing/contracts/ui/design-provenance.md),
not a new parallel widget style:

- `49:536`: TextField masters; `51:1497`: state examples.
- `27:560`: NumberField masters; `51:1557`: state examples.
- Both provide Default, Focus, Invalid, Mixed, ReadOnly, Disabled, Hover and Editing.
- Current Inspector: Flat compact A `131:20213`/`131:20388`; the original
  `108:8790` composition remains a source reference for component/property grouping.

Focus/hover, value validity, access mode, mixed value and source provenance are
orthogonal facts, not mutually exclusive domain states. For example, invalid
input can still have keyboard focus. Candidate-pending/conflicted input, rejection
details, settlement prompts and preview failures need reviewed compositions;
the eight examples are not evidence that these lifecycle states are designed.
Resolve missing compositions before shipping the affected behavior. Required
offline references remain independent Figma exports, never application pixels.

## Incremental delivery and verification

Each increment has its own reviewed behavior, source changes, regression coverage
and applicable native acceptance. This is dependency order, not authorization to
implement every increment together:

1. **Field interaction specification and upstream fit.** Use
   [ADR 0010](0010-text-field-interaction.md) for the single-line text field.
   Resolve the upstream text-undo gap, input quotas, native IME and Feathers
   adaptation with small fixtures. Approve the
   missing error/settlement compositions. No inspector framework scaffold.
2. **Shared TextField.** Independent native fixture exercises input, Unicode,
   composition, clipboard, focus, rejection, cancellation and retained host moves.
   The fixture's accepted value is an ordinary model, not a fake source writer.
3. **Shared NumberField.** Exact-type parsing, incomplete buffers, bounds and
   precision; include cancellable preview and one-operation semantics before drag
   is enabled. Then add discrete and compound controls as separate increments.
4. **Authoring property sessions.** Headless source-bound candidates over an actual
   UI prefab. Prove stale transitive revisions, failed validation, no-ops and
   cancellation preserve sources, history, redo and save baseline.
5. **Editable Inspector composition.** Extend the existing read-only crate and panel;
   discover registered fields, select editors deterministically and show unsupported
   fields. Use the same kit, not inspector-only copies of the first controls.
6. **Own-source document integration.** Hierarchy/selection, independent preview,
   document commands and cross-view settlement. Add source-owned filesystem Save
   only with its separately reviewed durability/conflict implementation. Reopen
   saved documents; build/verify a new delivery before testing shell changes.

Model/adaptor tests must cover: one history entry per accepted interaction; local
text versus document Undo; invalid blur; IME Enter; Escape with a popup; stale
completion; concurrent views; Save settlement; target deletion; no-op precision;
read-only transition; vector subfield preservation; and projection failure after
source acceptance. Rejection paths must assert unchanged sources and history,
not merely an error string. Dependency-layer tests forbid runtime-to-inspector
and inspector-to-editor/writer edges.

Native verification needs real frames and input for focus, text selection/caret,
popup ownership, floating windows, DPI, clipboard and IME on claimed platforms.
Injected input and headless tests are necessary but not physical-input acceptance.
Run `cargo xtask check` and relevant feature/integration checks for each code slice;
retain generated native evidence under `target/`.

## Consequences and remaining decisions

This keeps source editing reusable, the inspector replaceable and the field kit
usable outside the editor. It costs explicit session/preview handling and more
error-state design than direct reflected World mutation. Full candidate validation
may be expensive; measure on bounded real documents before introducing incremental
compilation. Coalescing provisional display work may skip obsolete previews, never
validation at final acceptance or unexplained failing checks.

Before implementation, approve buffer/session resource limits and diagnostics;
prove the native widget APIs needed by the retained-input contract; and review
settlement UX and extension metadata shape. Full document lifecycle, filesystem
durability, collections/structural edits, multi-object editing, asset/color pickers,
CSS authoring and editable 3D viewport tools need their own focused specifications. This
ADR defines the common boundaries, not completion of those features.

[bevy-widgets]: https://docs.rs/bevy/0.20.0/bevy/ui_widgets/index.html
[bevy-number]: https://docs.rs/bevy/0.20.0/bevy/feathers/controls/struct.FeathersNumberInput.html
