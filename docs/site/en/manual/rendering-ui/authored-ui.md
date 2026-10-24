# Authored UI

Authored UI uses the same scene/prefab documents as other Kedra content. Sources describe
a retained tree of native layout, text, images, styles and declared widget behavior.
Hosts bind actions and current values to that tree rather than rebuilding it every frame.

The editor uses external authored sources and CSS for startup forms, repeated rows, frames,
captions and dock chrome. It uses the shared UI integration rather than a separate manual layout.

**Hydration** means turning inert source data into its native presentation: for example,
`UiLayout` becomes Bevy `Node`, and a serving `UiButton` becomes a native widget.
Compilation keeps these hooked presentation components out of the source world.
Hydration is not script execution, a source edit or proof of a rendered frame.

## What a document can describe

All shared keys below use schema version 1:

| Source key | Role after hydration |
| --- | --- |
| `ui.layout` | Native Bevy `Node` data, wrapped to remain inert in sources |
| `ui.caption` | Initial text, justification and wrapping; typography comes from CSS |
| `ui.stylesheet`, `ui.classes` | Flair stylesheet dependency and explicit classes |
| `ui.image` | Ordered raster-density variants and semantic tint |
| `ui.button` | Native pointer/keyboard button in a serving view |
| `ui.editable` | Single-line native text input; requires caption content |
| `ui.modal` | Native dialog or barrier behavior in a serving view |
| `ui.spin` | Real-time UI rotation with pause/reduced-motion handling |
| `ui.id`, `ui.screen` | Serving observation identities, not persistent node addresses |
| `ui.background`, `ui.border`, `ui.shadow`, `ui.outline` | Native visual data |
| `ui.z`, `ui.global_z` | Native stacking data |
| `ui.pickable` | Bevy pointer-picking policy |

This is an admitted schema, not an alternate markup language. Unknown native layout
fields and nonfinite numeric values fail validation. `UiLayout` retains native Bevy `Node`
default-field semantics. Text, buttons and images require
layout on their node. Authored classes are unique, nonempty, whitespace-free names:
at most 64 per component, at most 256 bytes each.

### Example: a styled HUD root

This is the root `components` map excerpt from
`tests/fixtures/test_project/assets/Hud.prefab.ron`, not a complete prefab:

```ron
"ui.classes": (
    version: 1,
    value: (names:["hud"]),
),
"ui.layout": (
    version: 1,
    value: (node:()),
),
"ui.stylesheet": (
    version: 1,
    value: (asset:Path("game://preview.css")),
),
```

The same fixture contains a button captioned `Launch — inert in preview` and a field
with `autofocus:true`. In Scene View neither becomes interactive. A production host
must deliberately choose serving hydration and bind the action before a button means
anything application-specific.

## Mount and retain a serving view

Install `AuthoredUiPlugin` after the appropriate Bevy asset/UI/text/input-focus,
widget, picking and tab-navigation plugins. It includes live styling but creates no
window or renderer and supplies no default font or editor theme. Select sources through
the [loaded catalog and revision](../scenes/lifecycle.md#instantiate-a-prefab-explicitly),
using qualified logical paths, not hardcoded document UUIDs.

`UiView::mount` takes a compiled source, optional parent, local external bindings,
observation scope and positive finite density. The serving root needs `ui.layout`.
Stylesheets and selected image variants must already be materialized. Text needs an
explicit stylesheet scope; mounting a caption without one fails before live allocation.

If mounting fails, handle the error; there is no manual-layout substitute or lower-density
fallback. Serialized values cannot execute code or arbitrary remote operations. Bind typed
actions and domain data to the retained native components in your feature code.

The returned root owns the hierarchy and exact immutable compilation. Despawning its
hierarchy retires the view. `UiView::element` resolves required local semantic keys;
`UiView::text` accepts a text node or exactly one direct text label, not a guessed child.
Missing/ambiguous bindings are errors. Feature code updates native values and classes
while retaining text-editing state and focus; it does not rewrite the source document.

`UiView::mount_scene` is the startup-scene adapter for serving UI. It preserves authored
`ui.id` and `ui.screen`; `mount` can namespace element IDs with `scope/key` while local
lookup remains `key`. Element IDs must be unique within a native window; duplicate keys
inside one instance fail mounting. Persistent source node addresses and temporary Bevy
entity handles remain different identities.

## Live controls and passive previews

`UiView::mount_passive` uses the same compiled data for isolated presentation, including
UI subtrees in mixed spatial graphs. It strips `ElementId` and `ScreenId`, ignores
picking, renders editable captions as ordinary text and installs no button, autofocus,
text-input or modal behavior. Scripts and host-specific schemas gain no authority from
this path. Visual styling and supported presentation, including UI spin, remain separate
from interaction authorization.

Text in each detached UI tree requires a valid stylesheet scope within that tree.
A sheet on an external parent or beyond a non-layout ancestor cannot supply passive text
styling. The host assigns explicit target cameras and retires the detached trees with the
source instance; `PassiveView::retire` handles that ownership.

`UiReady` means native UI hydration completed, not GPU/frame readiness. Resource failures
on passive images are recorded locally in `PassiveUi`, not the serving application's
global `UiFailure`. This keeps a failed project preview from impersonating or disabling
the editor's live UI. For actual input and window scope, continue with
[styling and layout](styling-and-layout.md#native-controls-and-window-scope).

## Repository references

The [UI integration contract][ui] gives plugin and mount prerequisites.
For hydration, authored UI and passive UI terminology, see the [Glossary](../glossary.md).

[ui]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_ui/README.md
