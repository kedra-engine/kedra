//! Native pipeline readiness bridge; no second render scheduler or shader compiler.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::{CachedPipelineState, PipelineCache},
    },
};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct Status {
    pending: bool,
    error: Option<String>,
}
#[derive(Resource, Clone, Default)]
pub(super) struct Gpu(Arc<Mutex<Status>>);
pub(super) fn install(app: &mut App) {
    let status = Gpu::default();
    app.insert_resource(status.clone());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(status)
            .add_systems(Render, record.in_set(RenderSystems::Cleanup));
    }
}
fn record(cache: Res<PipelineCache>, status: Res<Gpu>) {
    if let Ok(mut status) = status.0.lock() {
        status.pending = cache.pipelines().any(|p| {
            matches!(
                p.state,
                CachedPipelineState::Queued | CachedPipelineState::Creating(_)
            )
        });
        status.error = cache.pipelines().find_map(|p| {
            if let CachedPipelineState::Err(error) = &p.state {
                Some(format!("Shared renderer pipeline failed: {error}"))
            } else {
                None
            }
        });
    }
}
pub(super) fn ready(world: &World) -> Result<bool, String> {
    let Some(gpu) = world.get_resource::<Gpu>() else {
        return Ok(true);
    };
    let status = gpu
        .0
        .lock()
        .map_err(|_| "Scene View renderer status is unavailable")?;
    if let Some(error) = &status.error {
        return Err(error.clone());
    }
    Ok(!status.pending)
}
