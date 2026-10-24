# ADR 0015: Inspector tool panel and target binding

Status: full editing/settlement contract proposed for owner review. The connected
[read-only panel](../site/en/contributing/contracts/editor/inspector.md)
implements source following, lock, search and
shared reflected presentations, not editable sessions or Save. The dockable panel
contract is distinct from the universal reflected editor in
[ADR 0011](0011-universal-reflected-inspector.md) and field interaction in
[ADR 0009](0009-property-editing-and-inspector.md).

## Purpose and responsibility

Inspector presents the active document's selected node, its components and their
reflected values. It composes default/type/field editors from existing
`kedra_inspector` and controls from `kedra_ui::fields`. It is not a Kedra-UI-only
inspector, another source workspace or an unrestricted mutable World editor.

The editor-owned panel adapter resolves document/node addresses and source
capabilities, then supplies consumer-neutral bindings to the inspector crate.
The panel does not parse RON, write files or build a second reflection registry.
The authoring adapter validates and acknowledges proposed changes. Renderer
entities never become persistent field targets.

## Docking contract

Register Inspector as an independent retained tool view in existing docking.
It can occupy any group, split, maximize, float and redock; there is no reserved
right sidebar. Native field roots, caret, selection, drafts and popup ownership
survive moves without remounting the editor tree.

Close settles field sessions, then hides the tool without closing its inspected
document. Reopening reveals the same view. Floating-window Close follows feature
approval, not unconditional tool-host hiding. Context menus and search/lock actions
use the shared panel-action family and feature toolbar, not custom window chrome.

## Following and locking a target

Initially the unlocked panel follows the document coordinator's active-document
selection. Browser entry selection is not a node target and does not silently
switch modes. Component sections are not independent mutable selections.

Changing document/node target first settles affected sessions. If a field is invalid,
stale or awaiting uncertain acceptance, retain the old target and show the pending
navigation decision. Do not rebuild the inspector and lose its buffer. Merely
refreshing a value or reselecting the same node preserves retained controls.

A lock pins the source-scope/document/node identity, not a snapshot of field values.
Locked Inspector continues showing current authoritative changes to that node while
other tools navigate elsewhere. Lock/unlock settles affected edits before changing
binding. Deleting the target shows Target unavailable and preserves a conflicted
buffer for inspection/copy; it does not retarget a same-named node.

Closing the target's last document view must include this panel's sessions in
settlement. If the user retains the dirty document in session, the locked inspector
may remain attached. Explicit document/session retirement leaves an unavailable
target presentation; never keep a mutable orphan workspace alive inside Inspector.

## Presentation and edit destination

Show the node identity, document/source context and available components. The default
editor handles supported reflected fields for native Bevy, Kedra and other registered
types. Type specializations improve usability; raw/default view stays available.
Do not require a bespoke inspector for a newly reflected component.

For source editing, display whether a value belongs to local source, an instance
override or inherited prefab data. Edit destination is explicit before acceptance.
Read-only/unsupported values retain type and reason. Missing reflected capability,
missing source policy and missing UI field editor are different diagnostics.
Computed/runtime-only values do not silently become editable source fields.

Component expansion is view-local state. Property search filters presentation,
preserving labels/component context; hiding an actively edited field requires
settlement before unmounting it. View-wide pending-input and diagnostic indication
remains visible if a rejected field is outside the viewport. Search itself does
not create authoring history.

Add/remove component, Reset, Apply/Revert, asset pickers, multi-selection and custom
component actions are separately verified commands, not implied by rendering the
component header. Display only available actions, with explicit reasons where
useful. Disabled validation must never be the route to editable arbitrary types.

## Figma and verification

Use the current [UI-kit](../site/en/contributing/contracts/ui/design-provenance.md)
Flat compact A compositions `131:20213`
(320px) and `131:20388` (480px), with 40px selection identity, 24px section headers
and 22px controls. Original composition `108:8790`, search `108:8936` and visibility
`108:9034` retain their source-reference role for grouping and interaction examples,
not current compact geometry. The sample Transform/Mesh/Material arrangement is
not a fixed schema or limit on what Inspector discovers. These are recorded source
identities, not a claim of live Figma verification.

Review no-selection, locked/unavailable target, unregistered capabilities, pending
target change, rejected/conflicted input and narrow/floating layouts before the
affected interaction ships. Native field/control acceptance remains required even
when the reflected model tests pass.

Test generic and specialized editors against identical source validation/history;
unchanged target refresh without remount; source changes while locked; deletion;
cancelled navigation; hidden invalid fields; pending acceptance on close; and float/
redock/hide/reopen with exact draft retention. An independent reflected-value fixture
must remain a consumer of the generic inspector, without depending on this panel.

Implement source target adapter and read-only composition before editable fields,
then enable supported edits one field family at a time. Keep a separate feature
module for panel lifecycle rather than adding editor ownership to the inspector
crate. Run repository and applicable native checks; no implementation is authorized
solely by this proposed specification.
