//! External CSS assets and native Flair styling, shared by editor and games.
//!
//! No CSS parser, selector engine, cascade or parallel layout implementation is
//! maintained here. Authored sources hold inert classes/handles; live hydration
//! installs Flair's native components after scene compilation succeeds.
use bevy::{asset::AssetApp, prelude::*};
use bevy_flair::{
    core::{ImplComponentPropertiesPlugin, PropertyRegistryPlugin},
    parser::FlairCssParserPlugin,
    style::{StyleBlock, StyleSheet},
};

/// Register stylesheet assets, properties and the maintained CSS parser without
/// installing presentation systems. Importers can validate assets headlessly.
/// Native metadata must select `CssStyleLoaderErrorMode::ReturnError`.
pub struct StyleAssetsPlugin;
impl Plugin for StyleAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<StyleSheet>()
            .init_asset::<StyleBlock>()
            .add_plugins((
                PropertyRegistryPlugin,
                ImplComponentPropertiesPlugin,
                FlairCssParserPlugin,
            ));
    }
}

/// Install the shared native cascade and animation systems. Custom visual
/// properties must be explicitly registered by their owning presentation plugin.
pub struct StylesPlugin;
impl Plugin for StylesPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StyleAssetsPlugin>() {
            app.add_plugins(StyleAssetsPlugin);
        }
        app.add_plugins((
            bevy_flair::style::FlairStylePlugin,
            bevy_flair::style::FlairDefaultStyleAnimationsPlugin,
        ));
        app.add_message::<bevy::window::WindowEvent>()
            .add_message::<bevy::window::RequestRedraw>();
    }
}
