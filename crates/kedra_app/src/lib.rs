//! Shared application setup for editor and independent games.
//! Native assets and Luau remain available in release. Diagnostic remote services,
//! injected input, capture and finite test programs exist only with debug assertions.
#[cfg(debug_assertions)]
pub mod capture;
#[cfg(debug_assertions)]
mod debug;
#[cfg(debug_assertions)]
pub mod input;
pub mod launch;
#[cfg(debug_assertions)]
mod methods;
#[cfg(debug_assertions)]
pub mod observe;
#[cfg(debug_assertions)]
pub mod operations;
#[cfg(debug_assertions)]
mod performance;
#[cfg(debug_assertions)]
pub mod scripting;
#[cfg(all(test, debug_assertions))]
mod tests;
#[cfg(debug_assertions)]
pub mod transport;
use bevy::prelude::*;
#[cfg(debug_assertions)]
pub use debug::{AppSession, CommandCatalog, RemoteAppExt, invalid, params};

/// Common application services; creates neither windows nor a renderer.
/// Debug builds also install inert diagnostic services. No listener is opened
/// until explicitly requested through the shared launch configuration.
pub struct KedraAppPlugin;
impl Plugin for KedraAppPlugin {
    fn build(&self, app: &mut App) {
        let wakeup = kedra_wakeup::Wakeup::for_world(app.world());
        app.insert_resource(wakeup);
        if !app.is_plugin_added::<kedra_assets::KedraAssetsPlugin>() {
            app.add_plugins(kedra_assets::KedraAssetsPlugin);
        }
        app.add_plugins(kedra_script::LuauPlugin);
        #[cfg(debug_assertions)]
        debug::install(app);
    }
    fn finish(&self, app: &mut App) {
        // Preserve a supplied host waker; refresh an early headless default once
        // the rest of the application's plugins have installed the native proxy.
        let wakeup = kedra_wakeup::Wakeup::for_world(app.world());
        app.insert_resource(wakeup);
    }
}
