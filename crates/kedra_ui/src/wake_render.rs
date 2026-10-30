//! Render/readback work without a completion seam uses explicit active demand.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::PipelineCache,
        view::screenshot::{Captured, Capturing, Screenshot},
    },
};
use kedra_wakeup::{WakeDemand, WakeSystems};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Resource, Clone, Default)]
struct Pipelines(Arc<AtomicBool>);

pub(crate) fn install(app: &mut App) {
    let pending = Pipelines::default();
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render
        .insert_resource(pending.clone())
        .add_systems(Render, collect.after(RenderSystems::Render));
    app.insert_resource(pending)
        .init_resource::<WakeDemand>()
        .add_systems(Last, demand.before(WakeSystems::Apply));
}

fn collect(cache: Res<PipelineCache>, pending: Res<Pipelines>) {
    pending.0.store(
        cache.waiting_pipelines().next().is_some(),
        Ordering::Release,
    );
}

type PendingReadbacks = (Or<(With<Screenshot>, With<Capturing>)>, Without<Captured>);

fn demand(
    pending: Res<Pipelines>,
    captures: Query<(), PendingReadbacks>,
    mut demand: ResMut<WakeDemand>,
) {
    if pending.0.load(Ordering::Acquire) || !captures.is_empty() {
        demand.active();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn inflight_readback_and_pipeline_hints_stop_after_native_completion() -> Result<(), String> {
        let mut world = World::new();
        world.init_resource::<Pipelines>();
        world.init_resource::<WakeDemand>();
        // An owner can cancel its logical request while Capturing still retains
        // the native recipient. Continue frames until actual native completion.
        let capture = world.spawn(Capturing).id();
        world.run_system_once(demand).map_err(|e| e.to_string())?;
        assert_ne!(world.resource::<WakeDemand>(), &WakeDemand::default());
        world.insert_resource(WakeDemand::default());
        world.entity_mut(capture).insert(Captured);
        world.run_system_once(demand).map_err(|e| e.to_string())?;
        assert_eq!(world.resource::<WakeDemand>(), &WakeDemand::default());
        world
            .resource::<Pipelines>()
            .0
            .store(true, Ordering::Release);
        world.run_system_once(demand).map_err(|e| e.to_string())?;
        assert_ne!(world.resource::<WakeDemand>(), &WakeDemand::default());
        world.insert_resource(WakeDemand::default());
        world
            .resource::<Pipelines>()
            .0
            .store(false, Ordering::Release);
        world.despawn(capture);
        world.run_system_once(demand).map_err(|e| e.to_string())?;
        assert_eq!(world.resource::<WakeDemand>(), &WakeDemand::default());
        Ok(())
    }
}
