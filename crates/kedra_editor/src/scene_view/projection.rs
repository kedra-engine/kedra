//! Native scene materialization and view-owned render targets; no gameplay systems.
#[cfg(test)]
mod lifecycle_tests;
mod readbacks;
mod ui_canvas;
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use kedra_scene::{
    composition::ResolvedGraph,
    instance::{CompiledPrefab, PrefabInstance},
    spatial::{Model, Primitive, Sun, Surface},
};
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn supported(graph: &ResolvedGraph) -> Result<(), String> {
    if graph.ports.values().any(|required| *required) {
        return Err("Scene View cannot supply external ports from the serving editor".into());
    }
    for node in graph.nodes.values() {
        let ui = node.components.contains_key("ui.layout");
        if ui && node.components.keys().any(|key| key.starts_with("scene.")) {
            return Err(format!(
                "{}: native UI and spatial rendering data require separate nodes",
                node.name
            ));
        }
        if !ui && node.components.keys().any(|key| key.starts_with("ui.")) {
            return Err(format!(
                "{}: UI presentation data requires ui.layout",
                node.name
            ));
        }
        if node
            .parent
            .as_ref()
            .is_some_and(|parent| graph.nodes[parent].components.contains_key("ui.layout"))
            && !ui
        {
            return Err(format!(
                "{}: native spatial/non-layout nodes cannot descend from a UI layout",
                node.name
            ));
        }
        for key in node.components.keys() {
            if !matches!(
                key.as_str(),
                "scene.transform"
                    | "scene.environment"
                    | "scene.primitive"
                    | "scene.surface"
                    | "scene.sun"
                    | "scene.model"
                    | "ui.layout"
                    | "ui.caption"
                    | "ui.stylesheet"
                    | "ui.classes"
                    | "ui.id"
                    | "ui.image"
                    | "ui.button"
                    | "ui.editable"
                    | "ui.modal"
                    | "ui.spin"
                    | "ui.background"
                    | "ui.border"
                    | "ui.shadow"
                    | "ui.z"
                    | "ui.global_z"
                    | "ui.outline"
            ) {
                return Err(format!(
                    "Scene View does not render {key} on {}. UI preview and additional spatial schemas are separate slices; source navigation remains available.",
                    node.name
                ));
            }
        }
        if node.components.contains_key("scene.primitive")
            != node.components.contains_key("scene.surface")
        {
            return Err(format!(
                "{}: scene.primitive and scene.surface must be supplied together",
                node.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod readback_tests {
    use super::*;
    #[test]
    fn stale_target_and_retired_camera_readbacks_cannot_publish_readiness() {
        let mut world = World::new();
        let current = TargetVersion {
            size: UVec2::splat(2),
            epoch: 2,
        };
        let camera = world.spawn(current).id();
        let read = world
            .spawn(Readback {
                camera,
                version: TargetVersion {
                    epoch: 1,
                    ..current
                },
            })
            .observe(captured)
            .id();
        let image = || Image::new_target_texture(2, 2, TextureFormat::Rgba8UnormSrgb, None);
        world.trigger(ScreenshotCaptured {
            entity: read,
            image: image(),
        });
        world.flush();
        assert!(world.get::<Confirmed>(camera).is_none());
        world.entity_mut(read).insert(Readback {
            camera,
            version: current,
        });
        world.trigger(ScreenshotCaptured {
            entity: read,
            image: image(),
        });
        world.flush();
        assert!(world.get::<Confirmed>(camera).is_some());
        world.entity_mut(camera).despawn();
        world.trigger(ScreenshotCaptured {
            entity: read,
            image: image(),
        });
        world.flush();
        assert!(world.get_entity(camera).is_err());
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct TargetVersion {
    size: UVec2,
    epoch: u64,
}
#[derive(Component)]
struct Confirmed(TargetVersion);
#[derive(Component)]
struct Readback {
    camera: Entity,
    version: TargetVersion,
}
fn captured(event: On<ScreenshotCaptured>, reads: Query<&Readback>, mut commands: Commands) {
    let Ok(read) = reads.get(event.entity) else {
        return;
    };
    if event.image.width() != read.version.size.x
        || event.image.height() != read.version.size.y
        || event.image.data.as_ref().is_none_or(Vec::is_empty)
    {
        return;
    }
    let camera = read.camera;
    let version = read.version;
    commands.queue(move |world: &mut World| {
        if world.get::<TargetVersion>(camera) == Some(&version) {
            world.entity_mut(camera).insert(Confirmed(version));
        }
    });
}

pub(super) struct Projection {
    pub(super) nodes: BTreeMap<kedra_scene::document::NodeAddress, Entity>,
    layer: usize,
    owner: Option<Entity>,
    camera: Option<Entity>,
    target: Option<Handle<Image>>,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<StandardMaterial>>,
    size: UVec2,
    epoch: u64,
    readback: Option<Entity>,
    frames: u32,
    canvas: Option<UVec2>,
    ui_canvas: Option<ui_canvas::UiCanvas>,
    ui_nodes: Vec<Entity>,
    passive: Option<kedra_ui::authored::view::PassiveView>,
    pub(super) grid: Option<Entity>,
    pub(super) inspection: Option<Entity>,
    scene_environment: Option<kedra_scene::environment::Environment>,
    studio_environment: Option<kedra_scene::environment::Environment>,
    scene_lights: Vec<Entity>,
}
impl Projection {
    pub fn pending(layer: usize) -> Self {
        Self {
            nodes: BTreeMap::new(),
            layer,
            owner: None,
            camera: None,
            target: None,
            meshes: Vec::new(),
            materials: Vec::new(),
            size: UVec2::ZERO,
            epoch: 0,
            readback: None,
            frames: 0,
            canvas: None,
            ui_canvas: None,
            ui_nodes: Vec::new(),
            passive: None,
            grid: None,
            inspection: None,
            scene_environment: None,
            studio_environment: None,
            scene_lights: Vec::new(),
        }
    }
    pub fn mounted(&self) -> bool {
        self.owner.is_some()
    }
    pub fn mount(
        &mut self,
        world: &mut World,
        compiled: Arc<CompiledPrefab>,
    ) -> Result<(), String> {
        let result = self.mount_inner(world, compiled);
        if result.is_err() {
            self.retire(world);
        }
        result
    }
    fn mount_inner(
        &mut self,
        world: &mut World,
        compiled: Arc<CompiledPrefab>,
    ) -> Result<(), String> {
        let owner = world.spawn((Transform::IDENTITY, Visibility::Hidden)).id();
        self.owner = Some(owner);
        let has_ui = compiled
            .authoring_world()
            .iter_entities()
            .any(|entity| entity.contains::<kedra_ui::authored::components::UiLayout>());
        let has_spatial = compiled
            .graph()
            .nodes
            .values()
            .any(|node| node.components.keys().any(|key| key.starts_with("scene.")));
        if has_ui && !has_spatial {
            self.canvas = Some(UVec2::new(1280, 720));
            if compiled.graph().nodes.len() == 1
                && let Some(image) = compiled
                    .authoring_world()
                    .iter_entities()
                    .find_map(|entity| entity.get::<kedra_ui::authored::components::UiImage>())
            {
                let handle = &image
                    .variants
                    .first()
                    .ok_or("Image preview has no variant")?
                    .image;
                let image = world
                    .resource::<Assets<Image>>()
                    .get(handle)
                    .ok_or("Image dependency unavailable")?;
                let natural = Vec2::new(image.width() as f32, image.height() as f32);
                self.canvas = Some(
                    (natural * (2048. / natural.max_element()).min(1.))
                        .ceil()
                        .as_uvec2()
                        .max(UVec2::ONE),
                );
            }
        }
        let root = if has_ui {
            let passive =
                kedra_ui::authored::view::UiView::mount_passive(world, &compiled, Some(owner), 1.)
                    .map_err(|e| e.to_string())?;
            let root = passive.root;
            self.passive = Some(passive);
            root
        } else {
            compiled
                .instantiate(world, Some(owner), &BTreeMap::new())
                .map_err(|e| e.to_string())?
        };
        let nodes = world
            .get::<PrefabInstance>(root)
            .ok_or("Scene instance mapping missing")?
            .nodes
            .clone();
        let mut count = nodes.len();
        if has_ui {
            self.ui_nodes = nodes.values().copied().collect();
        }
        if nodes
            .values()
            .filter(|entity| world.get::<Primitive>(**entity).is_some())
            .count()
            > 256
        {
            return Err(
                "Scene View supports at most 256 generated primitive meshes per view".into(),
            );
        }
        for entity in nodes.values() {
            if world.get::<Transform>(*entity).is_none()
                && world
                    .get::<kedra_ui::authored::components::UiLayout>(*entity)
                    .is_none()
            {
                world.entity_mut(*entity).insert(Transform::IDENTITY);
            }
            world
                .entity_mut(*entity)
                .insert((Visibility::Inherited, RenderLayers::layer(self.layer)));
            if let Some(primitive) = world.get::<Primitive>(*entity).cloned() {
                let mesh = kedra_scene::rendering::mesh(&primitive);
                let surface = world
                    .get::<Surface>(*entity)
                    .ok_or("Primitive has no surface")?
                    .clone();
                let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
                let material = world
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(kedra_scene::rendering::material(&surface));
                world
                    .entity_mut(*entity)
                    .insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
                self.meshes.push(mesh);
                self.materials.push(material);
            }
            if let Some(sun) = world.get::<Sun>(*entity).cloned() {
                world
                    .entity_mut(*entity)
                    .insert(kedra_scene::rendering::sun(&sun));
                self.scene_lights.push(*entity);
            }
            if let Some(model) = world.get::<Model>(*entity).cloned() {
                count +=
                    super::models::mount(world, *entity, &model.scene, self.layer, 4096 - count)?;
            }
        }
        // Calculate initial finite framing through the composed parent ordering;
        // native Transform propagation remains authoritative during rendering.
        let bounds = super::bounds::measure(world, root)?;
        let (minimum, maximum) =
            bounds.unwrap_or((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)));
        let center = if minimum.is_finite() {
            (minimum + maximum) / 2.
        } else {
            Vec3::ZERO
        };
        let radius = if minimum.is_finite() {
            ((maximum - minimum).length() / 2.).max(0.5)
        } else {
            2.
        };
        let camera = world
            .spawn((
                Camera3d::default(),
                RenderTarget::None { size: UVec2::ONE },
                // Explicit no-LUT curve: the editor's minimal renderer feature set
                // does not ship the default camera's TonyMcMapFace LUT package.
                kedra_scene::rendering::tonemapping(),
                bevy::camera::Projection::Perspective(PerspectiveProjection {
                    far: 16_000_000.,
                    ..default()
                }),
                Camera {
                    is_active: false,
                    order: -1,
                    clear_color: Color::srgb_u8(36, 43, 52).into(),
                    ..default()
                },
                Transform::from_translation(
                    center + Vec3::new(1., 0.7, 1.2).normalize() * radius * 3.5,
                )
                .looking_at(center, Vec3::Y),
                RenderLayers::layer(self.layer),
                AmbientLight {
                    brightness: 0.,
                    ..default()
                },
                ChildOf(owner),
            ))
            .id();
        self.camera = Some(camera);
        let environment = nodes
            .values()
            .find_map(|entity| world.get::<kedra_scene::environment::SceneEnvironment>(*entity))
            .map(|reference| {
                world
                    .resource::<Assets<kedra_scene::environment::Environment>>()
                    .get(&reference.asset)
                    .cloned()
                    .ok_or("Scene environment dependency is unavailable")
            })
            .transpose()?;
        kedra_scene::rendering::environment(world, camera, environment.as_ref())
            .map_err(|error| error.to_string())?;
        self.scene_environment = environment;
        if has_spatial || !has_ui {
            super::camera::attach(world, camera, bounds);
            self.grid = Some(
                world
                    .spawn((
                        bevy::dev_tools::infinite_grid::InfiniteGrid,
                        RenderLayers::layer(self.layer),
                        ChildOf(owner),
                    ))
                    .id(),
            );
        }
        if has_ui && has_spatial {
            self.ui_canvas = Some(ui_canvas::UiCanvas::mount(
                world,
                owner,
                self.layer,
                &self.ui_nodes,
            ));
        }
        for entity in nodes.values().filter(|_| self.ui_canvas.is_none()) {
            if world
                .get::<kedra_ui::authored::components::UiLayout>(*entity)
                .is_some()
            {
                world.entity_mut(*entity).insert(UiTargetCamera(camera));
            }
        }
        self.nodes = nodes;
        Ok(())
    }
    pub(super) fn mount_stage(
        &mut self,
        world: &mut World,
        compiled: Arc<CompiledPrefab>,
        enabled: bool,
    ) -> Result<(), String> {
        let owner = self.owner.ok_or("Scene ownership missing")?;
        let root = compiled
            .instantiate(world, Some(owner), &BTreeMap::new())
            .map_err(|e| e.to_string())?;
        // Record ownership before any fallible hydration; retirement reclaims it.
        self.inspection = Some(root);
        let nodes = world
            .get::<PrefabInstance>(root)
            .ok_or("Stage instance mapping missing")?
            .nodes
            .clone();
        self.studio_environment = nodes
            .values()
            .find_map(|entity| world.get::<kedra_scene::environment::SceneEnvironment>(*entity))
            .map(|reference| {
                world
                    .resource::<Assets<kedra_scene::environment::Environment>>()
                    .get(&reference.asset)
                    .cloned()
                    .ok_or("Editor stage environment dependency unavailable")
            })
            .transpose()?;
        for entity in nodes.values() {
            world
                .entity_mut(*entity)
                .insert((Visibility::Inherited, RenderLayers::layer(self.layer)));
            if let Some(source) = world.get::<Sun>(*entity).cloned() {
                world
                    .entity_mut(*entity)
                    .insert(kedra_scene::rendering::sun(&source));
            }
        }
        self.set_inspection(world, enabled)
    }
    pub(super) fn set_inspection(
        &mut self,
        world: &mut World,
        enabled: bool,
    ) -> Result<(), String> {
        let camera = self.camera.ok_or("Scene camera missing")?;
        let stage = self
            .inspection
            .ok_or("Editor lighting stage is unavailable")?;
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Scene target generation exhausted")?;
        kedra_scene::rendering::environment(
            world,
            camera,
            if enabled {
                self.studio_environment.as_ref()
            } else {
                self.scene_environment.as_ref()
            },
        )
        .map_err(|e| e.to_string())?;
        for entity in &self.scene_lights {
            if let Some(mut visibility) = world.get_mut::<Visibility>(*entity) {
                *visibility = if enabled {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
        }
        if let Some(mut visibility) = world.get_mut::<Visibility>(stage) {
            *visibility = if enabled {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        self.epoch = epoch;
        if let Some(readback) = self.readback.take() {
            readbacks::cancel(world, readback);
        }
        world.entity_mut(camera).remove::<Confirmed>();
        if self.size != UVec2::ZERO {
            world.entity_mut(camera).insert(TargetVersion {
                size: self.size,
                epoch,
            });
        }
        self.frames = 0;
        Ok(())
    }
    pub fn update(
        &mut self,
        world: &mut World,
        viewport: Entity,
        size: Vec2,
        visible: bool,
    ) -> Result<(), String> {
        let camera = self.camera.ok_or("Scene camera missing")?;
        for entity in &self.ui_nodes {
            if let Some(error) = world
                .get::<kedra_ui::authored::components::PassiveUi>(*entity)
                .and_then(|local| local.error.as_ref())
            {
                return Err(format!("Passive UI projection failed: {error}"));
            }
        }
        let active = visible && size.is_finite() && size.min_element() >= 1.;
        world
            .get_mut::<Camera>(camera)
            .ok_or("Scene camera retired")?
            .is_active = active;
        world
            .get_mut::<Visibility>(self.owner.ok_or("Scene ownership missing")?)
            .ok_or("Scene owner retired")?
            .set_if_neq(if active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        if let Some(passive) = &self.passive {
            for root in &passive.ui_roots {
                if let Some(mut visibility) = world.get_mut::<Visibility>(*root) {
                    *visibility = if active {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
        if !active {
            self.release_target(world);
            world.entity_mut(viewport).remove::<ImageNode>();
            return Ok(());
        }
        let scale = (2048. / size.max_element()).min(1.);
        let viewport_size = size;
        let target_size = self
            .canvas
            .unwrap_or_else(|| (size * scale).ceil().as_uvec2().max(UVec2::ONE));
        let fit = (size / target_size.as_vec2()).min_element();
        let extent = if self.canvas.is_none() {
            Vec2::splat(100.)
        } else {
            target_size.as_vec2() * fit / size * 100.
        };
        if let Some(mut node) = world.get_mut::<Node>(viewport) {
            node.width = Val::Percent(extent.x);
            node.height = Val::Percent(extent.y);
            node.left = Val::Percent((100. - extent.x) / 2.);
            node.top = Val::Percent((100. - extent.y) / 2.);
        }
        let size = target_size;
        if size != self.size {
            let pixels = readbacks::retained_pixels(world)
                + readbacks::retiring_pixels(world, self.target.as_ref())
                + world
                    .query::<(Entity, &TargetVersion)>()
                    .iter(world)
                    .filter(|(entity, _)| *entity != camera)
                    .map(|(_, target)| u64::from(target.size.x) * u64::from(target.size.y))
                    .sum::<u64>();
            let ui_pixels = self
                .ui_canvas
                .as_ref()
                .map_or(0, ui_canvas::UiCanvas::pending_pixels);
            if pixels + u64::from(size.x) * u64::from(size.y) + ui_pixels > 16_777_216 {
                return Err(
                    "Visible Scene View targets exceed the shared 64 MiB color-target budget"
                        .into(),
                );
            }
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or("Scene target generation exhausted")?;
            if let Some(readback) = self.readback.take() {
                readbacks::cancel(world, readback);
            }
            let mut image =
                Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
            image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            let target = world.resource_mut::<Assets<Image>>().add(image);
            world
                .entity_mut(camera)
                .insert((
                    RenderTarget::Image(target.clone().into()),
                    TargetVersion {
                        size,
                        epoch: self.epoch,
                    },
                ))
                .remove::<Confirmed>();
            world.entity_mut(viewport).insert((
                ImageNode::new(target.clone()),
                bevy::picking::Pickable::IGNORE,
            ));
            if let Some(old) = self.target.replace(target) {
                readbacks::release_image(world, old);
            }
            self.size = size;
            self.frames = 0;
        }
        self.frames = self.frames.saturating_add(1);
        let pipelines_ready = super::gpu::ready(world)?;
        if let Some(canvas) = self.ui_canvas.as_mut() {
            canvas.update(world, viewport, viewport_size, pipelines_ready)?;
        }
        if !self.frame_ready(world)
            && self.readback.is_none()
            && self.frames >= 12
            && pipelines_ready
        {
            readbacks::admit(world)?;
            let version = TargetVersion {
                size: self.size,
                epoch: self.epoch,
            };
            self.readback = Some(
                world
                    .spawn((
                        Screenshot::image(self.target.clone().ok_or("Scene target missing")?),
                        Readback { camera, version },
                        readbacks::SceneCapture,
                    ))
                    .observe(captured)
                    .id(),
            );
        }
        if !self.frame_ready(world) && self.frames > 600 {
            return Err("Scene View renderer readback timed out; sources were kept".into());
        }
        Ok(())
    }
    pub(super) fn camera(&self) -> Option<Entity> {
        self.camera
    }
    pub(super) fn layer(&self) -> usize {
        self.layer
    }
    pub fn frame_ready(&self, world: &World) -> bool {
        self.ui_canvas
            .as_ref()
            .is_none_or(|canvas| canvas.ready(world))
            && self
                .camera
                .and_then(|camera| world.get::<Confirmed>(camera))
                .is_some_and(|confirmed| {
                    confirmed.0
                        == TargetVersion {
                            size: self.size,
                            epoch: self.epoch,
                        }
                })
    }
    pub fn retire(&mut self, world: &mut World) {
        self.release_target(world);
        if let Some(passive) = self.passive.take() {
            passive.retire(world);
        }
        if let Some(entity) = self.owner.take()
            && let Ok(entity) = world.get_entity_mut(entity)
        {
            entity.despawn();
        }
        for mesh in self.meshes.drain(..) {
            world.resource_mut::<Assets<Mesh>>().remove(mesh.id());
        }
        for material in self.materials.drain(..) {
            world
                .resource_mut::<Assets<StandardMaterial>>()
                .remove(material.id());
        }
    }
    fn release_target(&mut self, world: &mut World) {
        if let Some(canvas) = self.ui_canvas.as_mut() {
            canvas.release(world);
        }
        if let Some(entity) = self.readback.take() {
            readbacks::cancel(world, entity);
        }
        if let Some(target) = self.target.take() {
            readbacks::release_image(world, target);
        }
        if let Some(camera) = self.camera
            && world.entities().contains(camera)
        {
            world
                .entity_mut(camera)
                .insert(RenderTarget::None { size: UVec2::ONE })
                .remove::<(Confirmed, TargetVersion)>();
        }
        self.size = UVec2::ZERO;
        self.frames = 0;
    }
}
