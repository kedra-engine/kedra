# Kedra native editor UI profile

## Product and authority

This profile covers the desktop game-development editor, not a game HUD, website
or documentation theme. Rust, Bevy UI and bevy_flair render the editor. Native text
uses Bevy/Parley. Source transactions and Save remain host-owned.

The existing [Figma file](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/Kedra)
defines visual targets; production sources define current behavior and limitations.
Do not treat Figma descriptions of implementation status as current code facts.
Do not introduce React, HTML, egui or another renderer into the native editor.

## Design system

- Tokens: `assets/editor/styles/tokens.css`. Shared controls:
  `assets/editor/prefabs/property-*.prefab.ron`,
  `assets/editor/styles/properties.css` and `crates/kedra_ui/src/fields/`.
- Inspector composition: `crates/kedra_inspector/src/`; editor source/lifecycle
  adapter: `crates/kedra_editor/src/inspector/`. Explicitly authorized Luau drawers
  compose the same controls through `kedra_inspector_luau`.
- Inter regular/medium/semibold, body 12px, property labels/captions 11px,
  16px text line. JetBrains Mono is reserved for explicit code/numeric identities.
- Controls are 22px, section headers 24px; related rows use 2px gaps, groups 4px.
  Inspector content uses 12px horizontal padding. Labels occupy 88px.
- Property labels and their controls share one horizontal row, including narrow
  panels. Vector axes remain inline. Payloads and collection details may expand
  below their owning control; ordinary labels do not become stacked captions.
- Values use recessed wells; commands and variant selectors use their kit treatment.
  Do not style every value as a raised button or every choice as a text input.
- Use semantic dark surfaces, subtle borders, 2px control corners and purple
  interaction/focus tokens. Color content is data, not a replacement UI accent.
- Icons are existing 16px SVG assets. Do not substitute emoji or text glyphs.
- Base Inspector owns component headers and disclosure. A drawer owns only its body.
  Read-only values remain copyable; invalid drafts remain visible. Modified property
  and unsaved document are distinct states.
- Follow the repository's [product UI content rule](../../AGENTS.md#product-ui-content):
  no persistent tips or explanatory captions inside product screens or their mockups.

## Canonical design references

- Inspector grouped composition at 320px:
  [131:20213](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/Kedra?node-id=131-20213).
- Same composition at 480px:
  [131:20388](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/Kedra?node-id=131-20388).
- Page `175:45355`, **05 · Kit / Value editors**, contains existing reusable
  variant, optional, length, collection, map-entry, group, color, reference,
  matrix, identifier and unsupported-value designs. Reuse these before drawing
  alternatives or creating duplicate canonical components.
- VariantEditor `139:23668`; LengthEditor `139:23697`;
  OptionalEditor `139:23718`; CollectionEditor `139:24345`;
  ColorValueEditor `139:24377`; PropertyRow `11:142`;
  ColorField `11:437`; Vector3 `27:635`.

## Real data and capabilities

Use native reflected values and the host's existing source policies, not a parallel
Inspector schema. Real examples include `Primitive::Cuboid { size }` / `Sphere { radius }`,
`Surface` colors/optional textures/metallic/roughness, `UiCaption` text/justify/wrap,
`UiImage` density entries and tint, `UiClasses.names`, and native `Node` layout values.
See `crates/kedra_scene/src/spatial.rs`,
`crates/kedra_ui/src/authored/components.rs` and `tests/fixtures/test_project/`.

Discovery does not grant construction, mutation or persistence. Variant switching,
optional creation, collection insertion/removal/reorder and map/set changes need
valid native construction and host validation. Missing capabilities are explicit,
not fabricated defaults. CSS ownership and authored values remain distinct.

## Design delivery and acceptance

The existing Figma kit is the reusable starting template. New structural proposals
belong in a clearly marked review section in that file; preserve accepted canonical
components until approval. Show alternatives together, including 320px/480px,
empty/populated, invalid, read-only and long-content states. No tracker tasks,
publishing, commits or canonical replacement without owner authorization.

Generated screenshots, local galleries, briefs and test evidence belong under
`target/`, not repository documentation. Durable accepted contracts belong with
their production owner or an ADR. This profile does not assert native acceptance.

Run `cargo xtask check` and the relevant native route, for example:

```sh
cargo xtask test -- features --only inspector --output target/test/inspector-controls
```

The output path must be fresh. Inspect actual native frames and test real input,
label/control alignment, clipping, collapse, focus, validation, Undo/Redo and Save.
Figma/server renders, synthetic geometry and RPC state do not prove native usability.
