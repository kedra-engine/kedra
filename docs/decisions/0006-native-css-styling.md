# ADR 0006: Native CSS styling

Status: styling contract in use; native platform acceptance is separate.

## Decision

Use pinned Bevy Flair 0.9.0 with Bevy 0.20 and no default features. Its property
registry, parser, selectors, variables, imports and cascade own styling. Bevy
UI/Taffy owns layout. Kedra does not implement another CSS parser or layout engine.
The `bevy_flair_style` source override adds conservative subject class/ID
rejection before stock matching and isolates inline-only changes from relational
selector invalidation. Provenance, lifecycle and verification boundaries are
defined by [ADR 0043](0043-css-subject-prefilter.md).

`kedra_ui::styles::StyleAssetsPlugin` installs headless parsing/property assets.
`StylesPlugin` adds live cascade/animation systems. The importer uses the headless
plugin; runtime has no importer or editor dependency.

Source-world policies admit inert `UiStylesheet` and `UiClasses`. Hooked Flair
components exist only in the live hydrated view. Prefabs own structure and declared
behavior; features own text/input values, selected/disabled state and typed actions.
CSS supplies layout, typography, outlines, colors and visual states. Initial native
Node fields apply only where CSS supplies no property. Dynamic layout uses one
declared Flair inline-property path, not competing Node-field writers.

Authored image variants own density-selected image handles and tint. `UiSpin` owns
spinner rotation. Other CSS/system writers must not race those owned properties.
Native Bevy widgets own activation, editing, focus and modal navigation.

Workbench values and classes live in external CSS. Games choose their own sheets,
classes and custom reflected properties. Nested `Styled` changes rule ownership,
not the entity hierarchy. Each independent theme boundary defines its values under
an explicit `.theme`; `:root` is not an arbitrary nested theme selector.

Installed CSS requires strict `ReturnError` loader metadata and a 64 KiB source
bound. Parse, import and dependency failures reject publication rather than shipping
partial valid rules. Dynamic variables need tests in the states that use them.
Flair supports a CSS subset, not every browser property or shorthand.

## Startup lifecycle

[ADR 0021](0021-windowless-startup.md) defines windowless boot verification and
catalog loading. Styling readiness does not imply a native surface or rendered
frame. Startup failures report diagnostics without an embedded replacement UI.

## Alternatives and cost

A custom RON theme/cascade duplicates selectors, inheritance and property
resolution. Feathers themes do not supply the required full styling path. Flair's
declared compatibility is supplemented by integration tests of the supported subset.
Its CSS stack includes exact-version MPL dependencies; licensing, upstream
duplicates and the bounded parser advisory exception remain governed by
[ADR 0007](0007-dependency-policy.md) and [third-party obligations](../third-party.md).

## Verification and limits

Headless tests cover imported variables, native geometry, interaction states,
nested themes, retained text/editor state, strict dependency failures and installed
source-free loading. Boot lifecycle tests cover delivery/catalog readiness and
failure separately from renderer readiness.

Physical input, CSS performance under representative HUD/editor load, cold
first-visible-frame latency, Retina/mixed-DPI behavior and platform-specific pixel
fidelity require native evidence. Bevy initialization precedes the first useful
window; renderer readback does not certify OS presentation time.
See [startup](../site/en/contributing/contracts/startup.md).
