//! Production editor icons supply the density packages consumed by authored UI.
use bevy::asset::meta::{AssetAction, AssetMeta};
use kedra_assets::images::ImageVariantsLoader;
use kedra_import::processors::SvgToImageVariants;

#[test]
fn editor_icons_supply_every_authored_density_label() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor/icons");
    for name in [
        "browser-back",
        "browser-down",
        "browser-filter",
        "browser-grid",
        "browser-list",
        "browser-right",
        "browser-up",
        "browser-search",
        "browser-folder",
        "browser-folder-selected",
        "hierarchy-scene",
        "hierarchy-node",
        "hierarchy-node-selected",
        "hierarchy-link",
        "hierarchy-close",
        "scene-grid",
        "scene-grid-selected",
        "scene-down",
        "scene-settings",
        "scene-eye",
        "scene-sun",
        "scene-cube",
        "scene-select",
        "scene-zoom",
        "scene-pan",
        "scene-frame",
        "scene-orbit-disc",
        "scene-axis-neg-x",
        "scene-axis-neg-y",
        "scene-axis-neg-z",
        "scene-axis-x",
        "scene-axis-y",
        "scene-axis-z",
    ] {
        let data = std::fs::read(root.join(format!("{name}.svg.meta")))
            .unwrap_or_else(|error| panic!("{name}: cannot read metadata: {error}"));
        let meta: AssetMeta<ImageVariantsLoader, SvgToImageVariants> = ron::de::from_bytes(&data)
            .unwrap_or_else(|error| panic!("{name}: cannot decode metadata: {error}"));
        let AssetAction::Process {
            processor,
            settings,
        } = meta.asset
        else {
            panic!("{name}: editor icons require processed density packages");
        };
        assert_eq!(
            processor, "kedra_import::processors::SvgToImageVariants",
            "{name}"
        );
        assert_eq!(settings.densities, [1, 2, 3, 4], "{name}");
        let svg = std::fs::read(root.join(format!("{name}.svg")))
            .unwrap_or_else(|error| panic!("{name}: cannot read SVG: {error}"));
        assert!(!svg.is_empty(), "{name}: SVG source must not be empty");
    }
}
