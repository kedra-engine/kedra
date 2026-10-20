//! Scene View's authored image labels must match its actual processor profile.
use bevy::asset::meta::{AssetAction, AssetMeta};
use kedra_assets::images::ImageVariantsLoader;
use kedra_import::processors::SvgToImageVariants;
#[test]
fn grid_icon_has_the_four_authored_density_subassets() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor/icons");
    for name in [
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
        let meta: AssetMeta<ImageVariantsLoader, SvgToImageVariants> =
            ron::de::from_bytes(&std::fs::read(root.join(format!("{name}.svg.meta")))?)?;
        let AssetAction::Process {
            processor,
            settings,
        } = meta.asset
        else {
            return Err("Grid icon requires a density package".into());
        };
        assert_eq!(processor, "kedra_import::processors::SvgToImageVariants");
        assert_eq!(settings.densities, [1, 2, 3, 4]);
        assert!(!std::fs::read(root.join(format!("{name}.svg")))?.is_empty());
    }
    Ok(())
}
