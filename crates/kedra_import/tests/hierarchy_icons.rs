//! Hierarchy state exports are density packages consumed by authored native UI.
use bevy::asset::meta::{AssetAction, AssetMeta};
use kedra_assets::images::ImageVariantsLoader;
use kedra_import::processors::SvgToImageVariants;
#[test]
fn hierarchy_state_icons_have_all_authored_density_labels() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor/icons");
    for name in ["scene", "node", "node-selected", "link", "close"] {
        let path = root.join(format!("hierarchy-{name}.svg.meta"));
        let source = std::fs::read(path)?;
        let meta: AssetMeta<ImageVariantsLoader, SvgToImageVariants> =
            ron::de::from_bytes(&source)?;
        let AssetAction::Process {
            processor,
            settings,
        } = meta.asset
        else {
            panic!("Hierarchy icons require density packages")
        };
        assert_eq!(
            processor, "kedra_import::processors::SvgToImageVariants",
            "{name}"
        );
        assert_eq!(settings.densities, [1, 2, 3, 4], "{name}");
        assert!(!std::fs::read(root.join(format!("hierarchy-{name}.svg")))?.is_empty());
    }
    Ok(())
}
