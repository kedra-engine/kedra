//! Read-only projection. Document authority and rendered ownership never coincide.
mod asset_target;
use kedra_authoring_assets as binding;
use kedra_preview::{bounds, models};
pub(crate) mod camera;
pub(crate) mod camera_motion;
#[cfg(test)]
mod camera_motion_tests;
#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod canvas_tests;
#[cfg(test)]
mod control_tests;
mod controls;
use kedra_preview::gpu;
mod lifecycle;
mod manipulation;
pub(crate) mod orientation;
#[cfg(test)]
mod orientation_tests;
mod projection;
mod selection;
pub(crate) use selection::PlacementPreview;
#[cfg(test)]
mod selection_tests;
mod source_refresh;
mod stage;
#[cfg(test)]
pub(crate) mod test_support;
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
use std::sync::Arc;

#[derive(Component)]
pub(crate) struct Viewport;

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
    controls: controls::Controls,
    marker: selection::Marker,
    compiled: Option<Arc<CompiledPrefab>>,
    pending_source: Option<Arc<CompiledPrefab>>,
    projection: Option<projection::Projection>,
    lifecycle: lifecycle::Lifecycle,
    elapsed: f32,
    raw_model: bool,
    source_removed: bool,
    source_layer: usize,
    requested_source: Option<Arc<ResolvedGraph>>,
}
impl View {
    pub(crate) fn align(
        &self,
        world: &mut World,
        camera: Entity,
        window: Entity,
        target: camera::Command,
    ) -> Result<(), String> {
        camera_motion::request(world, camera, window, target)?;
        // Input follows the normal view tick. Instant/reduced-motion requests
        // must refresh endpoints before this frame's UI layout and extraction.
        self.controls.orientation(world, camera);
        Ok(())
    }
    pub fn mount(
        world: &mut World,
        parent: Entity,
        id: ViewId,
        title: &str,
    ) -> Result<Self, String> {
        let view = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/source-document.prefab.ron",
            &format!("panel/{id:?}"),
        )?;
        crate::workspace::bind_scene_controls(world, &view, id)?;
        world
            .entity_mut(
                view.element("viewport")
                    .map_err(|error| error.to_string())?,
            )
            .insert((bevy::input_focus::tab_navigation::TabIndex(0), Viewport));
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
            controls: controls::Controls::mount(world, &view)?,
            marker: default(),
            compiled: None,
            pending_source: None,
            projection: None,
            lifecycle: default(),
            elapsed: 0.,
            raw_model: false,
            source_removed: false,
            source_layer: 0,
            requested_source: None,
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
        self.source_removed = false;
        self.source_layer = layer;
        self.requested_source = Some(graph.clone());
        let result = (|| {
            self.lifecycle.begin()?;
            projection::supported(graph)?;
            let manager = world.resource::<crate::model::ProjectManager>();
            let root = &manager.project.as_ref().ok_or("No project is open")?.assets;
            let registry = world.resource::<AppTypeRegistry>().clone();
            let compiled = binding::compile_preview(
                (**graph).clone(),
                &registry.read(),
                world.resource::<ComponentPolicies>(),
                world.resource::<AssetServer>(),
                source,
                root,
                world.get_resource::<binding::Delivery>(),
            )?;
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
    pub(crate) fn remove_source(&mut self, world: &mut World) {
        if !self.source_removed {
            self.retire(world);
            self.compiled = None;
            self.source_removed = true;
            self.message(world, "Document removed", false);
        }
    }
    pub fn tick(&mut self, world: &mut World, visible: bool, dpr: f32) -> Result<(), String> {
        // Readiness is not inactivity. A serving viewport must keep its normal
        // render/update loop even after resources and its first frame are ready.
        // Until project execution is isolated, this conservatively keeps the
        // shared App continuous; no individual effect owns a wake obligation.
        if visible
            && (self.compiled.is_some() || self.pending_source.is_some())
            && !matches!(
                self.lifecycle.phase,
                lifecycle::Phase::Failed | lifecycle::Phase::Retired
            )
            && let Some(mut demand) = world.get_resource_mut::<kedra_wakeup::WakeDemand>()
        {
            demand.continuous();
        }
        if self.pending_source.is_some() {
            if visible
                && let Some(mut demand) = world.get_resource_mut::<kedra_wakeup::WakeDemand>()
            {
                demand.active();
            }
            return Ok(());
        }
        // Early dependency/host exits must not retain last frame's enabled tools.
        self.controls_enabled(world, false);
        self.lifecycle.resized();
        let result = self.tick_projection(world, visible, dpr);
        if let Err(error) = &result {
            self.message(world, error, true);
        } else {
            let ready = visible && self.lifecycle.phase == lifecycle::Phase::Ready;
            self.controls_enabled(world, ready);
            if ready {
                self.controls
                    .spatial_enabled(world, self.navigation(world).is_some());
            }
        }
        self.controls.layout(
            world,
            self.root,
            self.viewport,
            visible && self.navigation(world).is_some(),
        );
        if visible
            && matches!(
                self.lifecycle.phase,
                lifecycle::Phase::Dependencies | lifecycle::Phase::Frame
            )
            && let Some(mut demand) = world.get_resource_mut::<kedra_wakeup::WakeDemand>()
        {
            demand.active();
        }
        result
    }
    fn controls_enabled(&self, world: &mut World, enabled: bool) {
        self.controls.enabled(world, enabled);
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
            .any(|node| node.components.keys().any(|key| key.starts_with("scene.")))
            || !compiled
                .graph()
                .nodes
                .values()
                .any(|node| node.components.contains_key("ui.layout"));
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
            self.controls.orientation(world, camera);
            let orthographic = matches!(
                world.get::<bevy::camera::Projection>(camera),
                Some(bevy::camera::Projection::Orthographic(_))
            );
            self.controls.selected(world, "projection", orthographic);
            if let Some(entity) = self.controls.entity("projection-label")
                && let Some(mut text) = world.get_mut::<Text>(entity)
            {
                text.0 = if orthographic {
                    "Orthographic"
                } else {
                    "Perspective"
                }
                .into();
            }
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
        self.pending_source = None;
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
    pub(crate) fn placement(
        &self,
        world: &mut World,
        point: Vec2,
    ) -> Result<(Vec3, usize), String> {
        let projection = self.projection.as_ref().ok_or("Projection missing")?;
        if self.navigation(world).is_none() {
            return Err("Scene View is not ready".into());
        }
        let area = self
            .area(world)
            .filter(|area| area.contains(point))
            .ok_or("Pointer outside Scene View")?;
        Ok((
            selection::placement(world, projection, point, area)?,
            projection.layer(),
        ))
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
    pub(crate) fn control(&self, name: &str) -> Option<Entity> {
        self.controls.entity(name)
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
    pub(crate) fn toggle_overlays(&mut self, world: &mut World) {
        if self.lifecycle.phase == lifecycle::Phase::Ready {
            self.marker.set_visible(world, !self.marker.is_visible());
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
    #[cfg(any(debug_assertions, test))]
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
        let editor_lighting = self
            .projection
            .as_ref()
            .and_then(|projection| projection.inspection)
            .is_some_and(|entity| world.get::<Visibility>(entity) == Some(&Visibility::Inherited));
        serde_json::json!({
            "ready": self.lifecycle.phase == lifecycle::Phase::Ready,
            "spatial": navigation.is_some(),
            "translation": transform.map(|pose| pose.translation.to_array()),
            "rotation": transform.map(|pose| pose.rotation.to_array()),
            "pivot": navigation.map(|navigation| navigation.pivot.to_array()),
            "speed": navigation.map(|navigation| navigation.speed),
            "orthographic": orthographic,
            "overlays": self.marker.is_visible(),
            "editor_lighting": editor_lighting,
            "ambient_brightness": world.get::<AmbientLight>(camera).map(|light| light.brightness),
            "aligning": camera_motion::active(world, camera),
            "source_spatial": self.spatial_values(world),
            "source_transforms": self.projection.as_ref().map(|projection| projection.nodes.iter().filter_map(|(address, entity)| {
                self.compiled.as_ref()?.graph().nodes.get(address)?.components.get("scene.transform")?;
                let value = world.get::<Transform>(*entity)?;
                Some(serde_json::json!({"node":address,"translation":value.translation.to_array(),"rotation":value.rotation.to_array(),"scale":value.scale.to_array()}))
            }).collect::<Vec<_>>()),
            "source_captions": self.projection.as_ref().map(|projection| projection.nodes.iter().filter_map(|(address, entity)| {
                self.compiled.as_ref()?.graph().nodes.get(address)?.components.get("ui.caption")?;
                let text = world.get::<Text>(*entity)?;
                let layout = world.get::<TextLayout>(*entity)?;
                Some(serde_json::json!({"node":address,"text":text.0,"justify":format!("{:?}",layout.justify),"wrap":layout.linebreak != bevy::text::LineBreak::NoWrap}))
            }).collect::<Vec<_>>()),
            "source_ui_layouts": self.projection.as_ref().map(|projection| projection.nodes.iter().filter_map(|(address, entity)| {
                self.compiled.as_ref()?.graph().nodes.get(address)?.components.get("ui.layout")?;
                let node = world.get::<Node>(*entity)?;
                let rect = world.get::<ComputedNode>(*entity).zip(world.get::<UiGlobalTransform>(*entity)).map(|(node, transform)| kedra_ui::logical_rect(node, transform));
                Some(serde_json::json!({"node":address,"width":format!("{:?}",node.width),"height":format!("{:?}",node.height),"rect":rect}))
            }).collect::<Vec<_>>()),
            "source_ui_collections": self.projection.as_ref().map(|projection| projection.nodes.iter().filter_map(|(address, entity)| {
                let classes = world.get::<kedra_ui::authored::components::UiClasses>(*entity)?;
                let native = world.get::<bevy_flair::style::components::ClassList>(*entity)?;
                Some(serde_json::json!({"node":address,"classes":classes.names,"classes_applied":classes.names.iter().all(|name| native.contains(name.clone()))}))
            }).collect::<Vec<_>>()),
            "source_ui_images": self.projection.as_ref().map(|projection| projection.nodes.iter().filter_map(|(address, entity)| {
                let image = world.get::<kedra_ui::authored::components::UiImage>(*entity)?;
                let rendered = world.get::<ImageNode>(*entity)?;
                Some(serde_json::json!({"node":address,"densities":image.variants.iter().map(|entry|entry.density).collect::<Vec<_>>(),"images":image.variants.iter().map(|entry|entry.image.path().map(ToString::to_string)).collect::<Vec<_>>(),"rendered_image":rendered.image.path().map(ToString::to_string)}))
            }).collect::<Vec<_>>()),
            "source_ui_shadows": self.projection.as_ref().map(|projection| projection.nodes.iter().filter_map(|(address, entity)| {
                let shadows = world.get::<BoxShadow>(*entity)?;
                Some(serde_json::json!({"node":address,"values":shadows.0.iter().map(|shadow|serde_json::json!({"x":format!("{:?}",shadow.x_offset),"y":format!("{:?}",shadow.y_offset),"spread":format!("{:?}",shadow.spread_radius),"blur":format!("{:?}",shadow.blur_radius),"color":shadow.color.to_srgba().to_f32_array()})).collect::<Vec<_>>()}))
            }).collect::<Vec<_>>()),
        })
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
