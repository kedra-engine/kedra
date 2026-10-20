//! UI icon consumers request labeled native subassets, not a single PNG output.
use bevy::asset::meta::{AssetAction, AssetMeta};
use kedra_assets::images::ImageVariantsLoader;
use kedra_import::processors::SvgToImageVariants;
#[test]
fn browser_icon_sidecars_supply_every_declared_ui_density() -> Result<(), Box<dyn std::error::Error>>
{
    let icons = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor/icons");
    for name in [
        "back",
        "down",
        "filter",
        "grid",
        "list",
        "right",
        "up",
        "search",
        "folder",
        "folder-selected",
    ] {
        let data = std::fs::read(icons.join(format!("browser-{name}.svg.meta")))?;
        let meta: AssetMeta<ImageVariantsLoader, SvgToImageVariants> = ron::de::from_bytes(&data)?;
        let AssetAction::Process {
            processor,
            settings,
        } = meta.asset
        else {
            panic!("Browser icons must be processed density packages")
        };
        assert_eq!(
            processor, "kedra_import::processors::SvgToImageVariants",
            "{name}"
        );
        assert_eq!(settings.densities, [1, 2, 3, 4], "{name}");
    }
    Ok(())
}
