//! Shared Kedra application services. Editors and game projects install this
//! same plugin; automation is inert until a host explicitly opens a local port.
//! BRP owns request dispatch; no reflection-based world mutation is exposed.
pub mod capture;
pub mod input;
mod methods;
pub mod observe;
pub mod operations;
#[cfg(test)]
mod tests;
pub mod transport;
use bevy::{ecs::system::IntoSystem, prelude::*};
use bevy_remote::{BrpResult, RemoteMethodSystemId, RemoteMethods, RemotePlugin};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Process identity used to scope client handles and command receipts.
#[derive(Resource)]
pub struct AppSession {
    /// Random per-process session, not a source document identity.
    pub id: String,
}
/// Machine-readable method inventory. Features register only implemented methods.
#[derive(Resource, Default)]
pub struct CommandCatalog(pub BTreeMap<String, Value>);

/// Default shared engine host services; it does not create a window or renderer.
pub struct KedraAppPlugin;
impl Plugin for KedraAppPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<kedra_assets::KedraAssetsPlugin>() {
            app.add_plugins(kedra_assets::KedraAssetsPlugin);
        }
        app.add_plugins(RemotePlugin::default());
        // Never publish BRP's generic spawn/mutate/despawn/source-bypass surface.
        app.insert_resource(RemoteMethods::new())
            .insert_resource(AppSession {
                id: uuid::Uuid::new_v4().to_string(),
            })
            .init_resource::<CommandCatalog>()
            .init_resource::<operations::Operations>()
            .add_plugins((input::InputPlugin, capture::CapturePlugin));
        app.register_remote(
            "app.describe",
            "Discover implemented command schemas",
            json!({}),
            methods::describe,
        );
        app.register_remote(
            "app.observe",
            "Observe real windows, UI, focus and input state",
            json!({}),
            observe::observe,
        );
        app.register_remote(
            "app.operation",
            "Read an asynchronous operation receipt",
            json!({"id":"UUID"}),
            methods::operation,
        );
        app.register_remote(
            "app.capture",
            "Capture a rendered native window",
            json!({"session":"app.describe.session","id":"UUID idempotency key","window":"app.observe.windows[].id"}),
            capture::request,
        );
        app.register_remote(
            "app.input",
            "Inject pointer/keyboard events through Bevy",
            json!({"session":"app.describe.session","id":"UUID idempotency key","window":"app.observe.windows[].id","input":{"kind":["click","move","button","modifier","drag","wheel","key","text"],"click":"exactly one of element or position:[x,y]","move":"position:[x,y]","button":"pressed:boolean, left pointer button","modifier":"modifier:shift|control|alt|super, pressed:boolean; explicitly release after held gestures","drag":"from:[x,y],to:[x,y]","wheel":"delta:[x,y] in pixels","key":"documented chord name","text":"UTF-8, at most 16 KiB"}}),
            input::request,
        );
        app.register_remote(
            "app.resize",
            "Resize before next-frame camera calculation",
            json!({"session":"app.describe.session","id":"UUID idempotency key","window":"app.observe.windows[].id","width":"logical pixels, 320..3840","height":"logical pixels, 320..3840","dpr":"optional test override, 1..4"}),
            input::resize,
        );
    }
}

/// Register native BRP systems without exposing their World through reflection.
pub trait RemoteAppExt {
    /// Register one feature-owned method and its parameter contract. Duplicate
    /// names are a programmer error; this is not a remote plugin-loading API.
    fn register_remote<M>(
        &mut self,
        name: &str,
        description: &str,
        schema: Value,
        system: impl IntoSystem<In<Option<Value>>, BrpResult, M> + 'static,
    ) -> &mut Self;
}
impl RemoteAppExt for App {
    fn register_remote<M>(
        &mut self,
        name: &str,
        description: &str,
        schema: Value,
        system: impl IntoSystem<In<Option<Value>>, BrpResult, M> + 'static,
    ) -> &mut Self {
        assert!(
            !self
                .world()
                .resource::<CommandCatalog>()
                .0
                .contains_key(name),
            "Duplicate remote command {name}"
        );
        let id = self.world_mut().register_system(system);
        self.world_mut()
            .resource_mut::<RemoteMethods>()
            .insert(name, RemoteMethodSystemId::Instant(id));
        self.world_mut().resource_mut::<CommandCatalog>().0.insert(
            name.into(),
            json!({"description":description,"params":schema}),
        );
        self
    }
}

/// Typed request parsing with strict unknown-field rejection in request types.
pub fn params<T: for<'de> serde::Deserialize<'de>>(
    value: Option<Value>,
) -> Result<T, bevy_remote::BrpError> {
    serde_json::from_value(value.unwrap_or_else(|| json!({}))).map_err(invalid)
}
/// Client validation error; internal paths/secrets must not be embedded in it.
pub fn invalid(message: impl ToString) -> bevy_remote::BrpError {
    bevy_remote::BrpError {
        code: -32602,
        message: message.to_string(),
        data: None,
    }
}
