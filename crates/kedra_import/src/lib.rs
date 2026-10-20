//! Native Bevy processing and immutable checked deliveries, shared by game
//! projects and the editor. No renderer, window, editor or scene dependency.
mod build;
pub mod processors;
mod verification;
use bevy::prelude::*;
pub use build::{build, initialize};

/// Registers Kedra processors; Bevy owns their scheduling and transactions.
pub struct KedraImportPlugin;
impl Plugin for KedraImportPlugin {
    fn build(&self, app: &mut App) {
        use bevy::asset::AssetApp;
        app.register_asset_processor(processors::SvgToImage)
            .register_asset_processor(processors::SvgToImageVariants);
    }
}
