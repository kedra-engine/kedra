//! Demand-driven, session-local static thumbnails. Hosts authorize paths/revisions
//! and reserve an exclusive render layer. No filesystem access or gameplay runs.
use bevy::{
    asset::LoadState,
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    gltf::Gltf,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use kedra_wakeup::{Task, Wakeup};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
mod disk;
#[cfg(test)]
mod tests;

/// Fixed output bounds: at most 128 RGBA8 images, each 288² pixels (CPU + GPU
/// residency at most 81 MiB), plus one temporary render target/capture. Native
/// source dependencies are separate from this output budget.
pub const SIZE: u32 = 288;
const CAPACITY: usize = 128;

/// Native provider selected only after host source/delivery admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Rasterized native image.
    Image,
    /// Audited passive native GLTF scene.
    Model,
}

/// Host-authorized immutable request. Revision must include referenced assets and
/// scope identity. Changing it revokes cached results without changing source data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Full source-qualified logical path, with a real native subasset label if needed.
    pub path: String,
    /// Host's source/dependency revision, including source scope identity.
    pub revision: String,
    /// Admitted native asset type.
    pub kind: Kind,
}
impl Request {
    fn key(&self) -> (String, String) {
        (
            self.revision.clone(),
            format!("{:?}:{}", self.kind, self.path),
        )
    }
}

struct Cached {
    image: Option<Handle<Image>>,
    error: Option<String>,
    used: u64,
}
enum Loading {
    Image(Handle<Image>),
    Model(Handle<Gltf>),
}
struct Job {
    request: Request,
    loading: Loading,
    root: Option<Entity>,
    target: Option<Handle<Image>>,
    capture: Option<Entity>,
    frames: u32,
    elapsed: f32,
    result: Arc<Mutex<Option<Image>>>,
    conversion: Option<Task<Result<Image, String>>>,
}

/// One explicitly ticked thumbnail owner. Call `update` even with empty demand
/// while `busy` is true: cancelled native loads/captures must finish retirement.
/// A stalled capture blocks further admission rather than leaking unbounded work.
#[derive(Resource, Default)]
pub struct Thumbnails {
    cache: BTreeMap<(String, String), Cached>,
    job: Option<Job>,
    clock: u64,
    disk: Option<disk::Disk>,
    disk_error: Option<String>,
}
impl Thumbnails {
    /// Borrow a completed snapshot. Cloned handles are consumer pins: callers must
    /// drop them when withdrawing demand; retaining them exceeds this owner's budget.
    pub fn image(&self, request: &Request) -> Option<Handle<Image>> {
        self.cache.get(&request.key()).and_then(|v| v.image.clone())
    }
    /// A bounded failure is retained until eviction or revision replacement.
    pub fn error(&self, request: &Request) -> Option<&str> {
        self.cache
            .get(&request.key())
            .and_then(|v| v.error.as_deref())
    }
    /// Native preparation or capture still requires ticking, even after cancellation.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.disk.is_some()
    }
    /// Active-frame demand ends after a stalled native job's deadline. Its slot
    /// remains retained; low-frequency host polling can still observe completion.
    pub fn needs_frames(&self) -> bool {
        self.job.as_ref().is_some_and(|job| job.elapsed < 30.)
    }

    /// Latest persistent-cache I/O diagnostic. Rendering remains available when
    /// cache storage is unavailable; the host may surface this once in diagnostics.
    pub fn disk_error(&self) -> Option<&str> {
        self.disk_error.as_deref()
    }

    /// Replace demand in priority order. Only the first 128 distinct requests are
    /// admitted, one native load/render at a time. `layer` is host-reserved and
    /// must not overlap other cameras, lights or models. Hidden consumers withdraw
    /// demand. Failures affect only their request, never another preview or source.
    pub fn update(
        &mut self,
        world: &mut World,
        demand: &[Request],
        layer: usize,
        store: Option<&kedra_cache::Store>,
    ) {
        self.clock = self.clock.saturating_add(1);
        let mut wanted = Vec::new();
        for request in demand {
            if wanted.len() == CAPACITY {
                break;
            }
            if !wanted.contains(&request) {
                wanted.push(request);
            }
        }
        for request in &wanted {
            if let Some(cached) = self.cache.get_mut(&request.key()) {
                cached.used = self.clock;
            }
        }
        if let Some(mut disk) = self.disk.take() {
            if let Some(result) = disk.task.poll() {
                match result {
                    Ok(Some(image)) if wanted.contains(&&disk.request) => {
                        self.cache.insert(
                            disk.request.key(),
                            Cached {
                                image: Some(world.resource_mut::<Assets<Image>>().add(image)),
                                error: None,
                                used: self.clock,
                            },
                        );
                    }
                    other => {
                        if let Err(error) = other {
                            self.disk_error = Some(error);
                        }
                        if disk.reading && wanted.contains(&&disk.request) {
                            self.start(world, disk.request);
                        }
                    }
                }
            } else {
                self.disk = Some(disk);
                return;
            }
        }
        if let Some(mut job) = self.job.take() {
            let result = job.tick(world, layer, wanted.contains(&&job.request));
            match result {
                Ok(None) => self.job = Some(job),
                outcome => {
                    if wanted.contains(&&job.request) {
                        let (image, error) = match outcome {
                            Ok(Some(image)) => {
                                if let Some(store) = store {
                                    self.disk = Some(disk::Disk::write(
                                        world,
                                        job.request.clone(),
                                        store.clone(),
                                        image.clone(),
                                    ));
                                }
                                (Some(world.resource_mut::<Assets<Image>>().add(image)), None)
                            }
                            Err(error) => (None, Some(error)),
                            _ => unreachable!(),
                        };
                        self.cache.insert(
                            job.request.key(),
                            Cached {
                                image,
                                error,
                                used: self.clock,
                            },
                        );
                    }
                    job.retire(world);
                }
            }
        }
        if self.busy() {
            return;
        }
        let Some(request) = wanted.iter().find(|r| !self.cache.contains_key(&r.key())) else {
            return;
        };
        if self.cache.len() >= CAPACITY {
            let victim = self
                .cache
                .iter()
                .filter(|(key, _)| !wanted.iter().any(|r| r.key() == **key))
                .min_by_key(|(_, value)| value.used)
                .map(|(key, _)| key.clone());
            let Some(victim) = victim else {
                return;
            };
            self.cache.remove(&victim);
        }
        if let Some(store) = store {
            self.disk = Some(disk::Disk::read(world, (*request).clone(), store.clone()));
        } else {
            self.start(world, (*request).clone());
        }
    }
    fn start(&mut self, world: &World, request: Request) {
        let server = world.resource::<AssetServer>();
        let loading = match request.kind {
            Kind::Image => Loading::Image(server.load(request.path.clone())),
            Kind::Model => Loading::Model(
                server
                    .load_builder()
                    .with_settings::<bevy::gltf::GltfLoaderSettings>(|s| {
                        s.load_cameras = false;
                        s.load_lights = false;
                        s.load_animations = false;
                    })
                    .load(request.path.clone()),
            ),
        };
        self.job = Some(Job {
            request,
            loading,
            root: None,
            target: None,
            capture: None,
            frames: 0,
            elapsed: 0.,
            result: default(),
            conversion: None,
        });
    }
}

impl Job {
    fn tick(
        &mut self,
        world: &mut World,
        layer: usize,
        interested: bool,
    ) -> Result<Option<Image>, String> {
        self.elapsed += world.resource::<Time>().delta_secs();
        if let Some(task) = &mut self.conversion {
            return task.poll().transpose();
        }
        if self.capture.is_some() {
            if let Some(result) = self
                .result
                .lock()
                .map_err(|_| "Thumbnail capture lock unavailable")?
                .take()
            {
                // Bevy readbacks are CPU-only. A thumbnail is also a UI texture:
                // publish it to the render world while retaining pixels for disk I/O.
                let mut result = result;
                result.asset_usage = bevy::asset::RenderAssetUsages::default();
                return Ok(Some(result));
            }
            // Never replace a still in-flight capture on a timer.
            return Ok(None);
        }
        let server = world.resource::<AssetServer>();
        let id = match &self.loading {
            Loading::Image(h) => h.id().untyped(),
            Loading::Model(h) => h.id().untyped(),
        };
        if let Some(LoadState::Failed(error)) = server.get_load_state(id) {
            return Err(error.to_string());
        }
        if !server.is_loaded_with_dependencies(id) {
            // Keep the sole loading slot charged until native loading actually ends.
            return Ok(None);
        }
        if !interested {
            return Err("Thumbnail demand was withdrawn".into());
        }
        if self.elapsed > 30. {
            return Err("Thumbnail preparation exceeded 30 seconds".into());
        }
        match &self.loading {
            Loading::Image(handle) => {
                let image = world
                    .resource::<Assets<Image>>()
                    .get(handle)
                    .ok_or("Thumbnail image unavailable")?;
                if image.width() > 4096 || image.height() > 4096 {
                    return Err("Thumbnail image exceeds 4096 pixels per axis".into());
                }
                let image = image.clone();
                self.conversion = Some(Wakeup::for_world(world).spawn(
                    bevy::tasks::AsyncComputeTaskPool::get(),
                    async move {
                        let reduced = image
                            .try_into_dynamic()
                            .map_err(|e| e.to_string())?
                            .thumbnail(SIZE, SIZE);
                        Ok(Image::from_dynamic(reduced, true, default()))
                    },
                ));
                Ok(None)
            }
            Loading::Model(handle) => {
                if self.root.is_none() {
                    let gltf = world
                        .resource::<Assets<Gltf>>()
                        .get(handle)
                        .ok_or("Thumbnail model unavailable")?;
                    let scene = gltf
                        .default_scene
                        .as_ref()
                        .or(gltf.scenes.first())
                        .cloned()
                        .ok_or("Model has no native scene")?;
                    let root = world
                        .spawn((Transform::IDENTITY, Visibility::Inherited))
                        .id();
                    self.root = Some(root);
                    super::models::mount(world, root, &scene, layer, 4095)?;
                    let (min, max) = super::bounds::measure(world, root)?
                        .ok_or("Model has no visible geometry")?;
                    let center = (min + max) * 0.5;
                    let radius = (max - min).length().max(0.01) * 0.5;
                    let target =
                        world
                            .resource_mut::<Assets<Image>>()
                            .add(Image::new_target_texture(
                                SIZE,
                                SIZE,
                                TextureFormat::Rgba8UnormSrgb,
                                None,
                            ));
                    world.spawn((
                        Camera3d::default(),
                        Camera {
                            order: -2,
                            clear_color: Color::NONE.into(),
                            ..default()
                        },
                        RenderTarget::Image(target.clone().into()),
                        Tonemapping::Reinhard,
                        Projection::Perspective(PerspectiveProjection {
                            near: radius * 0.01,
                            far: radius * 10.,
                            ..default()
                        }),
                        Transform::from_translation(
                            center + Vec3::new(1., 0.7, 1.2).normalize() * radius * 3.5,
                        )
                        .looking_at(center, Vec3::Y),
                        RenderLayers::layer(layer),
                        AmbientLight {
                            brightness: 300.,
                            ..default()
                        },
                        ChildOf(root),
                    ));
                    world.spawn((
                        DirectionalLight {
                            illuminance: 7000.,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(1., 2., 3.).looking_at(Vec3::ZERO, Vec3::Y),
                        RenderLayers::layer(layer),
                        ChildOf(root),
                    ));
                    self.target = Some(target);
                }
                self.frames = self.frames.saturating_add(1);
                if self.frames >= 12 && super::gpu::ready(world)? {
                    let result = self.result.clone();
                    let capture = world
                        .spawn(Screenshot::image(
                            self.target.clone().ok_or("Thumbnail target missing")?,
                        ))
                        .observe(move |event: On<ScreenshotCaptured>| {
                            if let Ok(mut result) = result.lock() {
                                *result = Some(event.image.clone());
                            }
                        })
                        .id();
                    self.capture = Some(capture);
                }
                Ok(None)
            }
        }
    }
    fn retire(self, world: &mut World) {
        if let Some(root) = self.root {
            world.despawn(root);
        }
        if let Some(capture) = self.capture {
            let _ = world.despawn(capture);
        }
    }
}
