# ADR 0006: Native CSS styling and staged splash

Status: styling and boot contract in use; native platform acceptance is separate.

## Decision

Use pinned Bevy Flair 0.9.0 with Bevy 0.20 and no default features. Its property
registry, parser, selectors, variables, imports and cascade own styling. Bevy
UI/Taffy owns layout. Kedra does not implement another CSS parser or layout engine.

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

## Boot readiness

Engine boot uses a small explicit catalog and a normal 640×360 native surface.
Only the bounded manifest is read up front; bootstrap reads still verify hashes.
Complete delivery verification starts on the I/O pool after native creation,
hydration and a nonblank splash renderer readback. The full catalog and successful
verification are prerequisites for project workflow.

A single native readback fences splash readiness; it does not compare with Figma
or retry blank pixels until matching. Opt-in automation uses the shared capture
writer. Bootstrap failures report diagnostics and exit, without an embedded
replacement UI. No fake percentage or artificial boot minimum is used.

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
source-free loading. Bootstrap tests distinguish manifest-only reading from full
verification and reject blank readbacks.

Physical input, CSS performance under representative HUD/editor load, cold
first-visible-frame latency, Retina/mixed-DPI behavior and platform-specific pixel
fidelity require native evidence. Bevy initialization precedes the splash;
readback does not certify OS presentation time. See [startup](../startup.md).
