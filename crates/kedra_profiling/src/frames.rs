//! Explicit update identity; optional rendering consumes the exact extracted value.
use bevy::prelude::*;

#[derive(Resource, Clone, Default)]
#[cfg_attr(
    feature = "render",
    derive(bevy::render::extract_resource::ExtractResource)
)]
#[cfg_attr(feature = "render", extract_app(bevy::render::RenderApp))]
struct UpdateIdentity(u64);

pub(super) fn install(app: &mut App) {
    app.init_resource::<UpdateIdentity>()
        .add_systems(First, update);
    #[cfg(feature = "render")]
    if app.get_sub_app(bevy::render::RenderApp).is_some() {
        app.add_plugins(bevy::render::extract_resource::ExtractResourcePlugin::<
            UpdateIdentity,
        >::default());
        app.sub_app_mut(bevy::render::RenderApp).add_systems(
            bevy::render::Render,
            render
                .after(bevy::render::RenderSystems::ExtractCommands)
                .before(bevy::render::RenderSystems::PrepareAssets),
        );
    }
}

fn update(mut identity: ResMut<UpdateIdentity>) {
    identity.0 = identity.0.wrapping_add(1);
    super::backend::markers::frame("kedra.update", identity.0);
}

#[cfg(feature = "render")]
fn render(identity: Res<UpdateIdentity>) {
    super::backend::markers::frame("kedra.render_cpu", identity.0);
}

#[cfg(all(test, feature = "render"))]
mod tests {
    use super::*;
    use bevy::{
        ecs::schedule::{ScheduleLabel, SystemSet},
        render::{Render, RenderApp, RenderSystems},
    };

    #[test]
    fn render_consumes_the_extracted_update_after_deferred_commands() {
        use bevy::render::extract_plugin::ExtractPlugin;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ExtractPlugin::<RenderApp>::new(
            |_, _| {},
            Render::base_schedule,
            Render.intern(),
            RenderSystems::ExtractCommands.intern(),
            RenderSystems::PostCleanup.intern(),
        ));
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
        install(&mut app);
        // This runs the actual extraction schedule and its deferred resource insertion,
        // not a synthetic timestamp match or a GPU/render acceptance substitute.
        for expected in 1..=3 {
            app.update();
            assert_eq!(app.world().resource::<UpdateIdentity>().0, expected);
            assert_eq!(
                app.sub_app(RenderApp)
                    .world()
                    .resource::<UpdateIdentity>()
                    .0,
                expected
            );
        }
        app.sub_app_mut(RenderApp)
            .world_mut()
            .remove_resource::<UpdateIdentity>();
        app.update();
        assert_eq!(
            app.sub_app(RenderApp)
                .world()
                .resource::<UpdateIdentity>()
                .0,
            4
        );
    }
}
