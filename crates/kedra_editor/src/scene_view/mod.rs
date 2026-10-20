//! Read-only projection. Document authority and rendered ownership never coincide.
mod asset_target;
pub(crate) mod binding;
mod bounds;
pub(crate) mod camera;
#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod canvas_tests;
#[cfg(test)]
mod control_tests;
mod gpu;
mod lifecycle;
mod models;
mod projection;
mod selection;
#[cfg(test)]
mod selection_tests;
mod stage;
#[cfg(test)]
mod tests;
mod ui_marker;
#[cfg(test)]
mod ui_selection_tests;
use bevy::prelude::*;
use kedra_docking::model::ViewId;
use kedra_scene::{
    composition::ResolvedGraph, instance::CompiledPrefab, reflection::ComponentPolicies,
};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) fn install(app: &mut App) {
    gpu::install(app);
    camera::install(app);
    ui_marker::install(app);
    app.add_plugins(bevy::dev_tools::infinite_grid::InfiniteGridPlugin);
    app.init_resource::<ComponentPolicies>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let result = kedra_scene::spatial::register(
        &mut registry.write(),
        &mut app.world_mut().resource_mut::<ComponentPolicies>(),
    );
    if let Err(error) = result {
        app.world_mut()
            .resource_mut::<kedra_ui::authored::components::UiFailure>()
            .0 = Some(error.to_string());
    }
}

pub(crate) struct View {
    pub root: Entity,
    viewport: Entity,
    render: Entity,
    information: Entity,
    heading: Entity,
    controls: Vec<Entity>,
    projection_label: Entity,
    lighting_label: Entity,
    lighting_button: Entity,
    grid_button: Entity,
    marker: selection::Marker,
    compiled: Option<Arc<CompiledPrefab>>,
    projection: Option<projection::Projection>,
    lifecycle: lifecycle::Lifecycle,
    elapsed: f32,
    raw_model: bool,
}
impl View {
    pub fn mount(
        world: &mut World,
        parent: Entity,
        id: ViewId,
        title: &str,
    ) -> Result<Self, String> {
        let view = crate::views::mount(
            world,
            parent,
            crate::views::document(0xe5f0af0978be48c88cc1ddd65951350c),
            &format!("panel/{id:?}"),
        )?;
        crate::workspace::bind_scene_controls(world, &view, id)?;
        world
            .entity_mut(
                view.element("viewport")
                    .map_err(|error| error.to_string())?,
            )
            .insert(bevy::input_focus::tab_navigation::TabIndex(0));
        world
            .entity_mut(view.element("render").map_err(|error| error.to_string())?)
            .insert(bevy::picking::Pickable::IGNORE);
        world
            .get_mut::<Text>(view.element("heading").map_err(|e| e.to_string())?)
            .ok_or("Scene heading missing")?
            .0 = format!("{title} · Read-only");
        let result = Self {
            root: view.root,
            viewport: view.element("viewport").map_err(|e| e.to_string())?,
            render: view.element("render").map_err(|e| e.to_string())?,
            information: view.element("information").map_err(|e| e.to_string())?,
            heading: view.element("heading").map_err(|e| e.to_string())?,
            controls: ["projection", "lighting", "grid", "views"]
                .into_iter()
                .map(|name| view.element(name).map_err(|error| error.to_string()))
                .collect::<Result<_, _>>()?,
            projection_label: view
                .element("projection-label")
                .map_err(|error| error.to_string())?,
            lighting_label: view
                .element("lighting-label")
                .map_err(|error| error.to_string())?,
            grid_button: view.element("grid").map_err(|error| error.to_string())?,
            lighting_button: view
                .element("lighting")
                .map_err(|error| error.to_string())?,
            marker: default(),
            compiled: None,
            projection: None,
            lifecycle: default(),
            elapsed: 0.,
            raw_model: false,
        };
        result.controls_enabled(world, false);
        Ok(result)
    }
    pub fn prepare(
        &mut self,
        world: &mut World,
        graph: &Arc<ResolvedGraph>,
        source: &kedra_authoring::persistence::SourceSnapshot,
        layer: usize,
    ) {
        self.retire(world);
        self.compiled = None;
        self.elapsed = 0.;
        self.raw_model = false;
        let result = (|| {
            self.lifecycle.begin()?;
            projection::supported(graph)?;
            let delivery=world.get_resource::<binding::Delivery>().ok_or("Scene View needs a verified delivery. Build assets with --namespace game, then relaunch this project with --project-assets PATH. Open never builds assets.")?;
            let manager = world.resource::<crate::model::ProjectManager>();
            let root = &manager.project.as_ref().ok_or("No project is open")?.assets;
            delivery.matches(root, source)?;
            let registry = world.resource::<AppTypeRegistry>().clone();
            let mut loader = binding::Loader {
                server: world.resource::<AssetServer>().clone(),
                source,
                delivery,
                error: None,
            };
            let compiled = CompiledPrefab::compile(
                (**graph).clone(),
                &registry.read(),
                world.resource::<ComponentPolicies>(),
                &mut loader,
                &BTreeMap::new(),
            )
            .map_err(|e| e.to_string())?;
            if let Some(error) = loader.error {
                return Err(error);
            }
            self.compiled = Some(Arc::new(compiled));
            self.projection = Some(projection::Projection::pending(layer));
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => self.message(world, "Loading Scene View resources…", false),
            Err(error) => self.message(world, &error, true),
        }
    }
    pub fn message(&mut self, world: &mut World, text: &str, failed: bool) {
        if failed {
            self.lifecycle.fail(text.into());
        }
        self.controls_enabled(world, false);
        if let Some(mut value) = world.get_mut::<Text>(self.information) {
            value.0 = text.into();
        }
        kedra_ui::docking::class(world, self.information, "hidden", false);
    }
    pub fn tick(&mut self, world: &mut World, visible: bool, dpr: f32) -> Result<(), String> {
        // Early dependency/host exits must not retain last frame's enabled tools.
        self.controls_enabled(world, false);
        self.lifecycle.resized();
        let result = self.tick_projection(world, visible, dpr);
        if let Err(error) = &result {
            self.message(world, error, true);
        } else {
            self.controls_enabled(world, visible && self.navigation(world).is_some());
        }
        result
    }
    fn controls_enabled(&self, world: &mut World, enabled: bool) {
        for button in &self.controls {
            let disabled = world
                .get::<bevy::ui::InteractionDisabled>(*button)
                .is_some();
            if enabled && disabled {
                world
                    .entity_mut(*button)
                    .remove::<bevy::ui::InteractionDisabled>();
            } else if !enabled && !disabled {
                world
                    .entity_mut(*button)
                    .insert(bevy::ui::InteractionDisabled);
            }
        }
    }
    fn tick_projection(
        &mut self,
        world: &mut World,
        visible: bool,
        dpr: f32,
    ) -> Result<(), String> {
        if matches!(
            self.lifecycle.phase,
            lifecycle::Phase::Failed | lifecycle::Phase::Retired
        ) {
            return Ok(());
        }
        let Some(compiled) = self.compiled.clone() else {
            return Ok(());
        };
        if !compiled
            .asset_status(world.resource::<AssetServer>())
            .map_err(|e| e.to_string())?
        {
            self.elapsed += world
                .get_resource::<Time<Real>>()
                .map_or(0., Time::delta_secs);
            if self.elapsed > 30. {
                return Err(
                    "Scene View resources timed out; accepted sources and Hierarchy were kept"
                        .into(),
                );
            }
            return Ok(());
        }
        let rect = kedra_ui::docking::rect(world, self.viewport);
        let spatial_source = compiled
            .graph()
            .nodes
            .values()
            .any(|node| node.components.keys().any(|key| key.starts_with("scene.")));
        let stage = if spatial_source {
            let Some(stage) = stage::ready(world)? else {
                self.elapsed += world
                    .get_resource::<Time<Real>>()
                    .map_or(0., Time::delta_secs);
                if self.elapsed > 30. {
                    return Err("Editor lighting stage resources timed out".into());
                }
                return Ok(());
            };
            Some(stage)
        } else {
            None
        };
        let Some(projection) = self.projection.as_mut() else {
            return Ok(());
        };
        if !projection.mounted() {
            projection.mount(world, compiled)?;
            if let Some(stage) = stage
                && let Err(error) = projection.mount_stage(world, stage, self.raw_model)
            {
                projection.retire(world);
                return Err(error);
            }
            self.lifecycle.instantiated();
        }
        let size = rect.map(|r| r.size() * dpr).unwrap_or(Vec2::ZERO);
        projection.update(world, self.render, size, visible)?;
        if let Some(camera) = projection.camera() {
            let orthographic = matches!(
                world.get::<bevy::camera::Projection>(camera),
                Some(bevy::camera::Projection::Orthographic(_))
            );
            let caption = if orthographic {
                "Orthographic"
            } else {
                "Perspective"
            };
            if let Some(mut label) = world
                .get_mut::<Text>(self.projection_label)
                .filter(|label| label.0 != caption)
            {
                label.0 = caption.into();
            }
            let inspection = projection.inspection.is_some_and(|entity| {
                world.get::<Visibility>(entity) == Some(&Visibility::Inherited)
            });
            let caption = if inspection { "Editor" } else { "Lit" };
            kedra_ui::docking::class(world, self.lighting_button, "editor-lighting", inspection);
            if let Some(mut label) = world
                .get_mut::<Text>(self.lighting_label)
                .filter(|label| label.0 != caption)
            {
                label.0 = caption.into();
            }
            kedra_ui::docking::class(
                world,
                self.grid_button,
                "selected",
                projection.grid.is_some_and(|entity| {
                    world.get::<Visibility>(entity) != Some(&Visibility::Hidden)
                }),
            );
        }
        if projection.frame_ready(world) {
            self.lifecycle.frame();
            kedra_ui::docking::class(world, self.information, "hidden", true);
        } else {
            self.lifecycle.resized();
            kedra_ui::docking::class(world, self.information, "hidden", false);
        }
        Ok(())
    }
    pub fn retire(&mut self, world: &mut World) {
        self.marker.clear(world);
        self.lifecycle.retire();
        self.controls_enabled(world, false);
        if let Some(mut p) = self.projection.take() {
            p.retire(world);
        }
    }
    pub fn bound(&self) -> bool {
        self.lifecycle.bound()
    }
    pub(crate) fn navigation(&self, world: &World) -> Option<Entity> {
        if self.lifecycle.phase != lifecycle::Phase::Ready {
            return None;
        }
        self.projection
            .as_ref()
            .and_then(|projection| projection.camera())
            .filter(|camera| world.get::<camera::Navigation>(*camera).is_some())
    }
    pub(crate) fn area(&self, world: &World) -> Option<Rect> {
        kedra_ui::docking::rect(world, self.render)
    }
    pub(crate) fn pick(
        &self,
        world: &mut World,
        point: Vec2,
    ) -> Result<Option<kedra_scene::document::NodeAddress>, String> {
        let projection = self.projection.as_ref().ok_or("Projection missing")?;
        let area = self.area(world).ok_or("Viewport geometry missing")?;
        selection::pick(world, projection, point, area)
    }
    pub(crate) fn select(
        &mut self,
        world: &mut World,
        selected: Option<&kedra_scene::document::NodeAddress>,
    ) -> Result<(), String> {
        if let Some(projection) = &self.projection
            && projection.mounted()
        {
            self.marker.update(world, projection, selected)?;
        }
        Ok(())
    }
    pub(crate) fn focus(&self) -> Entity {
        self.viewport
    }
    pub(crate) fn grid(&mut self, world: &mut World) {
        if self.navigation(world).is_none() {
            return;
        }
        if let Some(grid) = self
            .projection
            .as_ref()
            .and_then(|projection| projection.grid)
            && let Some(mut visibility) = world.get_mut::<Visibility>(grid)
        {
            *visibility = if *visibility == Visibility::Hidden {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    pub(crate) fn light(&mut self, world: &mut World, enabled: bool) {
        if self.navigation(world).is_none() {
            return;
        }
        if let Some(projection) = self.projection.as_mut()
            && let Err(error) = projection.set_inspection(world, enabled)
        {
            self.message(world, &error, true);
        }
    }
    pub(crate) fn snapshot(&self, world: &World) -> serde_json::Value {
        let Some(camera) = self
            .projection
            .as_ref()
            .and_then(|projection| projection.camera())
        else {
            return serde_json::json!({"ready":false});
        };
        let transform = world.get::<Transform>(camera);
        let navigation = world.get::<camera::Navigation>(camera);
        let orthographic = matches!(
            world.get::<bevy::camera::Projection>(camera),
            Some(bevy::camera::Projection::Orthographic(_))
        );
        serde_json::json!({"ready":self.lifecycle.phase==lifecycle::Phase::Ready,"spatial":navigation.is_some(),"translation":transform.map(|transform|transform.translation.to_array()),"rotation":transform.map(|transform|transform.rotation.to_array()),"pivot":navigation.map(|navigation|navigation.pivot.to_array()),"speed":navigation.map(|navigation|navigation.speed),"orthographic":orthographic})
    }
    pub fn prepare_asset(
        &mut self,
        world: &mut World,
        path: &str,
        source: &kedra_authoring::persistence::SourceSnapshot,
        layer: usize,
    ) -> Result<(), String> {
        self.retire(world);
        let graph = match asset_target::rig(world, path) {
            Ok(graph) => graph,
            Err(error) => {
                self.message(world, &error, true);
                return Err(error);
            }
        };
        self.prepare(world, &graph, source, layer);
        self.raw_model = path.ends_with(".glb");
        if let Some(mut text) = world.get_mut::<Text>(self.heading) {
            text.0 = format!(
                "{path} · {} · Read-only",
                if path.ends_with(".glb") {
                    "Static model / inspection light"
                } else {
                    "Image"
                }
            );
        }
        Ok(())
    }
}
