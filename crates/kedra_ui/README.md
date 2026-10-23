# kedra_ui

Runtime-layer authored UI hydration, native CSS styling, stable observation IDs
and renderer-derived geometry. Shared by editor and independent game-style hosts;
no dependency on editor, importer or authoring writer. Bevy owns layout, widgets,
text editing and rendering; pinned Bevy Flair owns CSS parsing and cascade.

Install `styles::StyleAssetsPlugin` after AssetPlugin for headless CSS assets and
property registration. `styles::StylesPlugin` adds live styling and animations.
`authored::AuthoredUiPlugin` includes live styling and registers inert source
component policies. Native hosts must also install Bevy UI/text/input-focus,
widget, picking and tab-navigation plugins as appropriate; this crate does not
create a window or install a renderer. No default font or editor theme is supplied.

`UiView::mount_scene` is the native UI adapter for an explicitly selected
`kedra_scene::startup::StartupScenePlugin`. Scene-owned `ui.id` and `ui.screen`
values remain unchanged for live observation; passive views strip both identities.
The host still supplies its camera/window and explicitly authorizes scripts.

Authored `ui.pickable` version 1 directly uses Bevy's `Pickable` component. Set
`should_block_lower:false,is_hoverable:false` on decorative overlays such as focus
rings: transparent paint alone does not remove a node from native pointer picking.
The ordinary authored root owns its lifetime; passive projections always ignore
picking regardless of this source policy.

Optional `text-capture` exposes `text_capture::capture` for explicit native QA
only with debug assertions; release excludes the module even if the feature is set.
The host supplies a live authored root and its complete source-address inventory;
no listener or remote method is installed by this feature. Measurements cover
Text/EditableText layout, loaded font bytes, effective typography, clipping and
independent shaping coverage. Full/shape paint state belongs to that root and
retires with it; restore full paint before releasing a live scope. Caret, selection,
layout and child icons are not suppressed. Unready assets/layout return `ready:false`;
unsupported modes or changes to the isolated text inventory/style fail closed.
This adds no dependency on QA tooling, editor or authoring writers.

Install `tooltip::TooltipPlugin` once and attach `Tooltip("Hint".into())` to any
native UI node. Shared behavior owns the 350ms dwell, cursor-relative placement,
window-edge fitting and cleanup on pointer departure, click, hiding or retirement.
The enclosing stylesheet supplies `.tooltip`, including its
font; native text layout measures and wraps hints up to 320 logical pixels.
Bubbles never capture pointer input or focus. This public component exists so
features supply text without implementing their own hover timer, overlay or theme.

Fully native authored hosts also install `native_input::NativeInputPlugin`
automatically when Bevy's input-dispatch and text-input plugins are already present.
Other native hosts can install it explicitly after those plugins. Give every
technical UI root `UiTargetCamera` and `TabGroup`, with a live window-targeted
camera. The adapter scopes keyboard/wheel bubbling and native tab traversal to
that window, excludes hidden/disabled controls, and routes IME events, enablement
and cursor position to the focused field's owner. Bevy still owns widgets, tab
ordering and text editing. Headless schema/asset hosts do not install this adapter.
These contracts have headless event/schedule regressions, not physical OS/IME
certification.

Keyboard and IME share the active modal boundary, including before delayed focus
handoff. Tab traversal retains Bevy's nested nonmodal-group order while excluding
other windows' controls for the synchronous traversal.

`authored::view::UiView::mount` consumes an immutable compiled source, an explicit
optional owner, local bindings and supported density. Invalid source/dependencies
fail before live allocation. A successful root owns all hydrated descendants and
its compilation. Retire it by despawning the hierarchy. Features bind content,
input state and classes to local IDs without reconstructing the source tree.

`docking::Projection::sync` validates skin bindings and stages missing chrome before
changing published attachments or focus. Failed preparation retires only staged
mounts; successful sync retains existing content/chrome identities. Consumers own
the host/stash/content lifetimes and must not retire or reparent projection-owned
entities from external ECS observers while synchronization is running.
Live and candidate split projections use Bevy's unrounded flex layout so nested
odd-width allocations fit their host instead of accumulating physical-pixel
rounding overflow. Drop-admission bounds and minimum-size checks are unchanged.

Hosts may explicitly call `docking::scene::register` after AuthoredUiPlugin and
before PrefabPlugin to admit inert `ui.docking` source data. `InitialLayout` wraps
the existing bounded JSON Snapshot schema and validates it before publication.
Its `workspace()` creates an independent layout lineage; it never selects native
controllers, opens windows or mounts content. Hosts own restore-key interpretation,
source-catalog admission, native resources and cleanup on binding failure.

`UiStylesheet` and `UiClasses` are side-effect-free source data. Live hydration
installs Flair components. CSS declares styling; native Node data can initialize
properties not supplied by CSS. Never add a second per-frame writer for a CSS-owned
property. Authored density variants own their image handles and `UiSpin` owns its
rotation; do not also animate that rotation in CSS. Nested stylesheets use explicit
theme classes and define their independent variables rather than treating :root
as a subtree selector. Games register custom Flair properties themselves.

Installed CSS requires strict native loader settings. Parse/import failures reject
delivery publication. Dynamic variable evaluation, native input, performance and
frame fidelity require state-specific verification. No filesystem hot-reload or
transactional live stylesheet replacement protocol is claimed by this integration.

See [authored scenes](../../docs/scenes.md) and
[ADR 0006](../../docs/decisions/0006-native-css-styling.md).
