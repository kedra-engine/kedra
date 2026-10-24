# Inspector

Inspector displays the selected source node's effective authored component values.
Select a node in [Hierarchy](hierarchy.md) or through supported mesh picking in
[Scene View](scene-view.md). Source inspection does not require a matching renderer
delivery or a successful preview frame.

The panel is read-only. Values are not live gameplay state, and computed UI layout is
not substituted for authored layout. Inspector does not enumerate renderer helpers or
compiler-generated components merely because they exist in a preview.

## Read a selected node

The header shows the selected node's name. Component sections use registered
source keys, with nested properties exposed through disclosures where available.
Expand a component/value tree to inspect its fields; collapse it to reduce clutter.
Scroll the property body to reach additional sections.

The reflected presentation supports primitive/string values, named structs, tuples,
tuple structs, active enum variants and bounded collection structure. Numbers preserve
their native precision; rotation is not automatically converted into Euler angles.
Unknown/opaque shapes and missing capabilities show their type and a reason rather than
invented editable controls. Collections show a bounded first segment, not pagination.

Property text uses native read-only selection where supported. Selecting text is not
permission to type a new value. Complete OS clipboard, copy/paste and IME support are
not certified; do not rely on the panel as a complete-value export tool.

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
| Toggle source details | Show/hide the source path and read-only source context. |

Lock is offered only when there is a target, or when unlocking an existing lock.
It pins identity, not a frozen copy of values. It neither locks the document against
changes nor changes the selected node in Hierarchy. If the target becomes unavailable,
the panel reports that fact rather than retargeting another node with the same name.

The source panel currently uses the default editor registry: the custom-presentation
menu does not load project plugins or guarantee a specialized widget for each component.
Default reflected view is an inspection fallback, not a source-format conversion.
The panel forwards expand/collapse actions only for top-level component trees. Nested custom
presentations must handle their own inner-tree actions. Project-defined Inspector extensions
are not available.

## Search without changing selection

Choose **Search properties**, or press Ctrl/Cmd+F with focus in Inspector. Search retains
the selected object's identity and component/ancestor context. It does not select a
different object or edit values. Close with the search bar's close action, or Escape
while focus is in the panel; closing clears the query.

Search matches property labels and displayed value previews without case sensitivity,
within the inspection budgets. It does not search every value in the project.

The field accepts at most 256 characters and rejects control characters. Expansion,
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

There are no enabled Add component, Reset, Apply/Revert, asset picker/location, field
editing, undo/redo for source changes, or Save actions. To understand the distinction
between source-editing APIs and editor capabilities, read
[Source editing and Save](../scenes/source-editing.md).
