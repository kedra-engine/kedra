# Styling, layout and native windows

Kedra uses pinned Bevy Flair 0.9.0 for CSS parsing, selectors, variables, imports, cascade
and style animations. Bevy 0.20 owns native UI layout, text, widgets and rendering.
This is native retained UI, not a webview or a second Kedra layout engine.

To integrate styling and native windows into your application, use the interfaces in the
[Rust API guide](../../api/index.md) and the [shared UI contract (English)][ui-contract].
For using panels in the editor, see [editor panel controls](../editor/index.md).

## Connect structure and style

Authored prefabs supply structure and behavior; `ui.stylesheet` points to a delivered CSS
asset, and `ui.classes` supplies rule-matching classes. CSS supplies layout, typography,
colors and interaction visuals. Initial `ui.layout` Node fields can supply values that
CSS does not own. Games choose their own sheets, fonts and custom registered properties;
the shared UI plugin does not silently inherit the editor's theme.

This CSS excerpt is from `assets/editor/styles/controls.css`:

```css
.button {
    height:var(--control-height); min-height:var(--control-height); min-width:80px; padding:0 6px;
    align-items:center; justify-content:center; flex-shrink:0;
    border-radius:var(--control-radius); background-color:transparent;
}
.button:hover { background-color:var(--hover); }
.button:active { background-color:var(--pressed); }
.button:disabled { background-color:var(--panel); }
```

It is not a complete stylesheet: its custom properties come from the selected theme
scope. `assets/editor/prefabs/button.prefab.ron` supplies the native button, label and
decorative focus-ring node. The focus ring explicitly sets
`should_block_lower:false,is_hoverable:false`; transparent paint alone does not make a
native overlay pass pointer input through.

Treat Flair as a supported CSS subset, not a browser compatibility promise. Use existing
project sheets and the pinned property registry as reference rather than inventing a
shader language or assuming every web shorthand works. For upstream APIs, see
[Bevy Flair 0.9.0](https://docs.rs/bevy_flair/0.9.0/bevy_flair/).

## Prepare styles without creating a window

`StyleAssetsPlugin`, installed after `AssetPlugin`, registers stylesheet assets, native
properties and parsing for headless consumers. `StylesPlugin` adds live cascade and
animation systems; `AuthoredUiPlugin` includes it. Finish native property registration
before requesting stylesheets. Custom properties belong to the host that defines them.

Installed CSS requires `CssStyleLoaderErrorMode::ReturnError` in native loader metadata;
the upstream default is recovery with warnings, not Kedra's strict publication contract.
The delivery workflow bounds CSS sources to 64 KiB and rejects parse/import/dependency
failures. A valid stylesheet asset still does not prove that every dynamic variable
evaluates correctly in every widget state. See [supported formats](../assets/supported-formats.md)
and [delivery diagnosis](../assets/troubleshooting.md).

To inspect supported presentation in the editor, follow
[Scene View resource preparation](../editor/scene-view.md).

Nested stylesheet boundaries use explicit theme classes and define their own variables.
Do not treat `:root` as a selector for an arbitrary nested subtree. Avoid competing
writers: no per-frame Node updates for CSS-owned properties, no CSS rotation animation
on a `UiSpin` node, and no second image writer racing authored density selection.
The existing docking helpers use a declared Flair inline-property path for dynamic
geometry instead of a parallel cascade.

## Density, motion and retained values

`ui.image` contains one to eight positive, strictly increasing density variants. Mounting
chooses the first variant whose density covers the requested density; subsequent native
target measurements can change that choice. Missing coverage or a disappeared image
reports failure while retaining the previous image, not a rescaled placeholder. See
[image variant processing](../assets/supported-formats.md).

`UiSpin` uses real time, independently of gameplay clocks. A paused spin or
`UiMotion.reduced` keeps rotation at zero. This is not a general animation editor or
automatic reduced-motion control for every CSS animation.

Shared `fields::{PropertyRow, SectionView, ValueView}` build retained presentations from
host-supplied authored templates. Rows own label/editor/diagnostic slots; sections can
collapse without replacing value widgets. `FieldAccess::ReadOnly` prevents mutation
while preserving native selection/copy. These controls do not own reflection, document
validation or persistence, and do not turn the read-only Inspector into an editor.

## Native controls and window scope

Serving buttons use Bevy activation; text fields use native `EditableText`/`TextInput`,
focus and tab navigation. `NativeInputPlugin` scopes keyboard/wheel bubbling and tab
traversal to the control's window and active modal boundary. Authored UI installs the
adapter automatically only when Bevy's input-dispatch and text-input plugins are already
present; other native hosts install it explicitly after those plugins.

Technical UI roots need `UiTargetCamera` and `TabGroup`, with a live window-targeted
camera. Hidden/disabled controls are excluded from navigation. IME enablement, events
and cursor position are routed to the focused field's owner. Headless event tests do not
certify physical keyboards, OS IME, mixed-DPI monitors or platform-specific input behavior.

Window headers use `kedra_ui::window_chrome`: authored slots borrow content, and explicit
`Client`, `Drag` and `NativeControls` regions determine input ownership. A disabled client
control still blocks a drag region. `Header::unbind` parks borrowed content before the
header is retired; directly despawning it bypasses this lifecycle. Invalid, stale,
foreign-window or cyclic attachments are rejected without moving existing content.

The editor's `kedra_desktop` adapter owns native window/camera/root sets and routes chrome
commands to an explicit window. It does not make `kedra_ui` create windows. Native Close
requests remain distinct from feature close approval or Save. Docking and native surface
ownership do not grant authority over project sources. See the
[editor workspace](../editor/index.md) for user-facing controls.

CSS performance, native text fidelity, first visible frames and platform behavior require
their own native verification; source inspection cannot establish those results.
Filesystem hot reload and transactional live stylesheet replacement are not supported.

[ui-contract]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_ui/README.md
