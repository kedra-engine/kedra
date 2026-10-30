//! Native Bevy processing and immutable checked deliveries, shared by game
//! projects and the editor. Uses shared scene data without a renderer, window or
//! editor dependency.
mod build;
pub mod ingest;
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
