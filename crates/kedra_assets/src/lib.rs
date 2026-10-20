//! Runtime-side asset delivery. No editor, processor or authoring writer.
//!
//! Bevy owns loading, dependency tracking and subasset labels. Kedra adds stable
//! source identity and integrity checks; hashes detect corruption, not trust.
pub mod data;
pub mod delivery;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;
pub mod images;
#[cfg(feature = "model-support")]
pub mod model;
mod source;
pub use source::register_source;

#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod tests;

/// Runtime assets shared by games and the editor. The host installs Bevy's
/// asset/image plugins and registers its delivery sources before this plugin.
pub struct KedraAssetsPlugin;
impl bevy::prelude::Plugin for KedraAssetsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        use bevy::asset::AssetApp;
        app.init_asset::<images::ImageVariants>()
            .init_asset_loader::<images::ImageVariantsLoader>()
            .init_asset::<data::DataAsset>()
            .init_asset_loader::<data::DataLoader>();
    }
}
