//! One ordinary application's fixed source admission and startup composition.
//! Resource locations and debug scene overrides cannot select native capabilities.
pub(crate) const CATALOG: &str = "application://catalogs/native.prefabs.ron";
pub(crate) const CATALOG_SOURCE: &str = include_str!("../catalogs/native.prefabs.ron");
pub(crate) const STARTUP_SCENE: &str = "game://Native/switch.scene.ron";

pub(crate) fn register_schema(app: &mut bevy::prelude::App) -> kedra_scene::Result<()> {
    kedra_ui::docking::scene::register(app)
}
