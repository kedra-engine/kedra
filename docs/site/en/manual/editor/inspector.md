# Inspector

Inspector displays the selected source node's effective authored component values.
Select a node in [Hierarchy](hierarchy.md) or through supported mesh picking in
[Scene View](scene-view.md). Source inspection does not require a matching renderer
delivery or a successful preview frame.

The built-in Transform, Primitive, Surface and Sun presentations, embedded Luau UiCaption
drawer and supported scalar UI fields are editable. The default reflected view remains
read-only. Values are not live gameplay state,
and computed UI layout is
not substituted for authored layout. Inspector does not enumerate renderer helpers or
compiler-generated components merely because they exist in a preview.

## Read a selected node

To assign lighting, select the scene asset in Assets or its root in Hierarchy.
The **Environment** field is present even when no profile is assigned. Open its
picker and choose a project `.environment.ron` asset; save the scene with Ctrl/Cmd+S.
Save follows the selected scene asset even when a different document tab is active.
To create a profile, use **Create → Environment** in the Assets context menu, then
edit and save the selected profile in Inspector. New profiles are available in the
scene picker immediately, without rebuilding a runtime delivery.

Selecting a Scene/Prefab asset in Assets inspects its composed root without requiring
a document tab to be activated. Selecting an existing `.environment.ron` asset
shows its Environment root in the same Inspector: background color, ambient color,
ambient brightness and schema version. Version must remain `1`; colors must be finite
normalized values and brightness must be between `0` and `1000000`.

Environment text fields accept with Enter; invalid values keep their draft and
diagnostic. Accepted changes use source Undo/Redo and remain in memory until
Ctrl/Cmd+S or **Save**. The header's `*` marks unsaved changes.
Switching selection does not discard accepted changes; unresolved drafts retain the
previous Inspector target. Scene lighting updates from the shared working profile;
**Editor lighting** continues to use the editor's independent studio environment.
Save preserves the asset identity and does not rebuild a runtime delivery.

The header shows the selected node's name. Component sections use registered
source keys or a built-in presentation title, with nested properties exposed through disclosures.
Expand a component/value tree to inspect its fields; collapse it to reduce clutter.
Scroll the property body to reach additional sections.

The reflected presentation supports primitive/string values, named structs, tuples,
tuple structs, active enum variants and bounded collection structure. Numbers preserve
their native precision. Built-in Vec2/Vec3/Vec4 presentations show axes inline. Transform
uses Position / Rotation / Scale rows, with quaternion rotation displayed in degrees using
Bevy's YXZ convention. Viewing a rotation never rewrites it.
Default reflected view exposes the native structure instead of these conveniences.
Unknown/opaque shapes and missing capabilities show their type and a reason rather than
invented editable controls. Collections show a bounded first segment, not pagination.

Other property text uses native read-only selection where supported. Selecting text is not
permission to type a new value. Complete OS clipboard, copy/paste and IME support are
not certified; do not rely on the panel as a complete-value export tool.

## Edit Transform without writing files

Select a source node with `scene.transform` and use its Position / Rotation / Scale
axes. Valid changes update the working document immediately; **Enter** explicitly
requests validation and ends the input gesture. **Escape** cancels only unaccepted
text, not changes already published. Incomplete, nonfinite or out-of-policy values retain the exact
draft with a diagnostic and do not create source history. Untouched axes remain intact.
Rotation edits use degrees in YXZ order and persist a quaternion in the working source.
Consecutive accepted changes in one input gesture form one source Undo operation.
Enter, Escape and leaving the field end that gesture.

Choose **Undo source edit** / **Redo source edit** in the group or Panels menu.
Outside text fields and inside settled live fields, Cmd/Ctrl+Z and Shift+Cmd/Ctrl+Z
also use source history. Invalid/incomplete text keeps local text Undo. Resolve
pending drafts first. Accepted changes/history refresh Inspector and a supported Scene
View without resetting the preview camera. A changed target does not silently discard
a draft: Inspector retains that context until Enter/Escape. Hide/float retains its inputs.
Tab and pointer blur end the gesture; they do not force invalid text into the source.

**Changes remain in memory until Save.** Closing with unsaved work opens a dialog with
**Save and exit**, **Exit without saving** and **Cancel**. Unaccepted drafts must be
settled before saving; unresolved publication must be retried before discarding.
Exit without saving restores the latest acknowledged source baseline and retires drafts.
Forced termination/crash recovery of unsaved edits is not supported.

## Edit primitive, surface and light values

Existing `scene.primitive` values expose Cuboid dimensions as inline XYZ controls and
Sphere radius as a number. `scene.surface` exposes color, metallic and roughness;
`scene.sun` exposes color and illuminance. Enter/Escape, source history and Save follow
the same rules as Transform. Invalid dimensions and out-of-range material/light values
retain a diagnostic without changing the source or preview.

Color uses a swatch and `#RRGGBB` / `#RRGGBBAA` input, including alpha. Hex input is
sRGB; native sRGB/linear storage is preserved. Accepting unchanged display text preserves
the exact original channels rather than quantizing them. HDR and other color spaces
remain available in default view without clamping. Click the swatch to expand exact
R/G/B/Alpha fields, a named palette and native sRGB/linear storage selection. Channel
amounts use 0..=1 in the selected native space; presets preserve alpha and storage.
Color/dimension/material/light changes refresh the retained native assets
without replacing the camera. Labels and value controls remain on the same row.

The Primitive **Variant** menu switches Cuboid/Sphere with valid default dimensions;
choosing the current variant preserves its exact payload. Resource wells show copyable
logical paths, not editable runtime handle IDs. Supported image/stylesheet fields offer
a searchable picker and typed Asset Browser drop with a matching verified delivery.
Model/Environment use retained preview refresh and source Undo/Redo. Dropping a model
with one advertised native scene selects that scene; multiple scenes require the picker.
Cross-window drops and loading-failure timing still have limited native acceptance.

## Edit collections

Classes, image densities and shadows provide bottom **+ / −** controls, item selection
and grip reorder. Alt+Up/Down reorders the focused grip. Valid item fields update the
source immediately; structural changes validate the complete list before publication.
There is no Apply/Cancel. Invalid text remains visible without changing source;
classes must remain unique and contain no whitespace. Escape restores the last
accepted field value; source Undo restores an accepted editing gesture.

Unfinished collections block source Undo/Redo, Save, target following and Close even
when Inspector is hidden. Image density entries copy an existing valid image; Add is
disabled without a compatible construction/resource choice. An empty image-list
constructor first asks for a typed resource; it never inserts a dummy handle. Existing
`UiImage` source validation still requires a nonempty list. Source Undo/Redo restores
known item selection/focus for the last 32 local structural changes; older mappings
can reset selection. Long-list autoscroll acceptance remains incomplete.

## Edit UiCaption through Luau

The built-in `ui.caption` presentation is composed by an explicitly embedded Luau
script, using the same native property rows and inputs as Rust drawers. **Text** edits
a single-line string (up to 4096 characters), **Alignment** selects a native variant
such as `Left`, `Center` or `Right`, and **Wrap** uses a native checkbox. Opening or
cancelling a choice menu does not edit the source; selecting a value validates it
immediately. Accept or cancel any text draft before using another control.

Enter/Escape, source Undo/Redo, draft retention, default view and explicit all-source
discard follow the same rules as Transform. Accepted edits update supported UI-only
and mixed Scene Views without changing the preview camera. Editing a nested prefab
caption creates an override in the opened document; it does not edit the prefab file.

Luau selects field composition; Rust parses exact native types, checks revisions and
validates/accepts source changes. Script failure shows a diagnostic with read-only
partial fields and default-view access. Multiline/unsupported fields also remain
available in default view. Project Open does not discover or run project scripts.
Project drawers, hot reload, dynamic scripted layout and Play are not provided.

## Edit UI values

Supported scalar fields in authored layout, classes, images, background/border,
shadows, ordering, outlines and inert behavior flags use the same typed source
validation and history. Bool values use native checkboxes. Enums offer complete
variants only when native defaults or explicit constructors are available; active
payload fields remain inspectable. Supported collection entries use the same immediate
validated source editing as scalar fields.

Layout groups native fields into Size, Position, Flex & alignment, Spacing, Grid and
Advanced. Size starts expanded. Lengths keep amount/unit controls on one row;
Auto has no editable amount. Source history and pending-draft rules apply to groups
and unit changes. Searching reveals matching fields without losing collapse state.

Passive Scene View updates layout, classes, captions, images and visual components
without enabling buttons, autofocus, modal ownership or source observation IDs.
The serving preview remains intact while source-refresh dependencies load. Failed
dependencies refuse refresh. Cross-window drops and long-list autoscroll still require
broader native acceptance.

## Save active document

Use **Save** in the main menu or header, or **Cmd/Ctrl+S**, including
while a settled property input owns focus. **Save All** / **Cmd/Ctrl+Shift+S** saves dirty
documents and environment assets, stopping on the first refusal. Save writes accepted working changes, never
a pending draft or live preview state. Accept with Enter or cancel with Escape first.
The active document receives the write, not a different target retained by a locked Inspector.
An asterisk marks unsaved source revisions; progress and errors appear in workspace feedback.

Save writes the active document's complete coupled existing-source group after checking
hashes and inventory. Each file replacement is atomic; a durable journal makes the group
recoverable. It does not create catalogs/sidecars, build assets or create/delete documents.
External changes cause refusal without losing working changes or source history.
There is no automatic overwrite or external-merge decision.

New edits accepted during Save remain dirty after the pinned revision is acknowledged.
If publication may have happened but cannot be confirmed, use **Retry Save**: the same
operation and exact bytes are retained; close/discard stay blocked until reconciliation.
After successful Save, Discard restores that saved baseline, not the original project Open.
After process interruption, an unfinished Save blocks Open. Explicitly run
`kedra recover-save <source-root>` (the project's assets directory), then reopen.
Recovery completes the pinned Save; it does not restore unsaved drafts or editor history.
Save does not clear source Undo/Redo. A fresh editor session still needs a matching delivery.

## Use the group menu

Inspector actions live in its dock group's **…** menu when Inspector is active.
There is no implemented component-editing context menu.

| Action | Result |
| --- | --- |
| Search properties | Open and focus the property search field. |
| Lock Inspector | Keep inspecting the current source target while selection changes elsewhere. |
| Unlock Inspector | Resume following the active document's selection. |
| Use default reflected view | Bypass specialized presentations and show reflected structure. |
| Use custom presentations | Restore registered presentation dispatch where available. |
| Toggle source details | Show/hide the source path and interaction context. |

Lock is offered only when there is a target, or when unlocking an existing lock.
It pins identity, not a frozen copy of values. It neither locks the document against
changes nor changes the selected node in Hierarchy. If the target becomes unavailable,
the panel reports that fact rather than retargeting another node with the same name.

The source panel installs built-in Rust math/Transform presentations. The custom-presentation
menu does not load project plugins or guarantee a specialized widget for each component.
Default reflected view is an inspection fallback, not a source-format conversion.
Component headers and nested full-width custom sections use the same collapse/expand behavior
as ordinary reflected groups. Collapsing retains native controls; default/custom mode retains
the section's collapsed state. Custom drawers which create separate recursive trees must route
their own inner-tree actions. Project-defined Inspector extensions are not available.

## Search without changing selection

Choose **Search properties**, or press Ctrl/Cmd+F with focus in Inspector. Search retains
the selected object's identity and component/ancestor context. It does not select a
different object or edit values. Close with the search bar's close action, or Escape
while focus is in the panel; closing clears the query.

Search matches property labels and displayed value previews without case sensitivity,
within the inspection budgets. It does not search every value in the project.

Search has local Undo/Redo (Cmd+Z / Shift+Cmd+Z on macOS, Ctrl+Z / Shift+Ctrl+Z on
other desktop hosts). This changes the query, never document history. The field accepts
at most 256 Unicode scalar values and rejects controls and Unicode line separators. Expansion,
search, lock and default/custom presentation mode are local to this Inspector view.
Hide/Reopen, Float/Redock and maximize retain the panel rather than mounting another
Inspector with an independent source workspace.

## Example: inspect an effective override

For fixture setup, follow [your first document](../getting-started/first-document.md).
The [composition example](../scenes/composition.md) explains the override used here.

Open `assets/Inspector/Main.prefab.ron` from the repository's test project:

1. Hierarchy initially selects **Main**. Its `ui.editable` component describes source
   settings; it does not make this Inspector an editable field.
2. Select the nested **Repeated name** node. Expand `ui.caption` as needed.
3. Its `text` value is `Override 日本語`, not the child's original `Inherited 日本語`.
   Inspector reads the effective composition, including the instance override.
4. Choose **Lock Inspector**, then open `Inspector/Other.prefab.ron` in Assets.
   The active document changes, but Inspector keeps **Repeated name**.
5. Search for `text` to narrow properties without losing the locked target.
6. Choose **Unlock Inspector** to follow the active document again. The other fixture's
   caption value is `Other source`.

The fixture retains Unicode text exactly. Rendering each glyph depends on available
fonts; font and IME behavior still need testing.

## Empty, unavailable and limited presentations

| State | Meaning / next step |
| --- | --- |
| Select a node guidance | Open a document and select its root/child; raw asset previews supply no source target. |
| No authored components | The selected source node is valid but has no component values to show. |
| Target unavailable | The bound source scope, compilation, revision or node is no longer usable. |
| No properties match | Clear or change the property search; selection is unchanged. |
| Search incomplete / presentation limit | Some values are omitted by inspection budgets; results are not exhaustive. |
| Reference representation limitation | A symbolic source reference cannot yet be faithfully presented; do not infer authored `None` or a persistent runtime entity number. |

The current presentation limits are depth 32, 128 collection entries, 2048 discovered
rows per rebuild and 4096 Unicode scalar values per value preview. Truncation does not
modify sources and is not a complete copyable value. Filtering shares these limits.

The header **+** opens Add component; a component section's **…** offers Remove component.
These operations, reference pickers and source Save are
available for eligible targets. Unsupported reflected fields remain read-only;
the editor does not expose the full authoring API's prefab Apply/Revert workflow.
For source transaction and persistence boundaries, read
[Source editing and Save](../scenes/source-editing.md).
