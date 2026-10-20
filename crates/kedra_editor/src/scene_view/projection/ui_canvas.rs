//! Mixed scenes retain a fixed passive UI canvas independently of the 3D target.
use super::*;

pub(super) struct UiCanvas {
    pub(super) camera: Entity,
    pub(super) target: Option<Handle<Image>>,
    image: Option<Entity>,
    pub(super) readback: Option<Entity>,
    epoch: u64,
    frames: u32,
}
impl UiCanvas {
    pub(super) const SIZE: UVec2 = UVec2::new(1280, 720);

    pub(super) fn mount(world: &mut World, owner: Entity, layer: usize, nodes: &[Entity]) -> Self {
        let camera = world
            .spawn((
                Camera2d,
                Camera {
                    is_active: false,
                    order: -2,
                    clear_color: Color::NONE.into(),
                    ..default()
                },
                RenderTarget::None { size: UVec2::ONE },
                RenderLayers::layer(layer),
                ChildOf(owner),
            ))
            .id();
        for entity in nodes {
            if world
                .get::<kedra_ui::authored::components::UiLayout>(*entity)
                .is_some()
            {
                world.entity_mut(*entity).insert(UiTargetCamera(camera));
            }
        }
        Self {
            camera,
            target: None,
            image: None,
            readback: None,
            epoch: 0,
            frames: 0,
        }
    }

    pub(super) fn pending_pixels(&self) -> u64 {
        if self.target.is_none() {
            u64::from(Self::SIZE.x) * u64::from(Self::SIZE.y)
        } else {
            0
        }
    }

    pub(super) fn update(
        &mut self,
        world: &mut World,
        parent: Entity,
        size: Vec2,
        pipelines_ready: bool,
    ) -> Result<(), String> {
        if self.target.is_none() {
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or("UI canvas generation exhausted")?;
            let mut image = Image::new_target_texture(
                Self::SIZE.x,
                Self::SIZE.y,
                TextureFormat::Rgba8UnormSrgb,
                None,
            );
            image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            let target = world.resource_mut::<Assets<Image>>().add(image);
            world.entity_mut(self.camera).insert((
                RenderTarget::Image(target.clone().into()),
                TargetVersion {
                    size: Self::SIZE,
                    epoch: self.epoch,
                },
            ));
            self.target = Some(target);
        }
        world
            .get_mut::<Camera>(self.camera)
            .ok_or("UI canvas camera retired")?
            .is_active = true;
        let extent =
            Self::SIZE.as_vec2() * (size / Self::SIZE.as_vec2()).min_element() / size * 100.;
        let node = Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(extent.x),
            height: Val::Percent(extent.y),
            left: Val::Percent((100. - extent.x) / 2.),
            top: Val::Percent((100. - extent.y) / 2.),
            ..default()
        };
        if let Some(image) = self.image {
            world.entity_mut(image).insert(node);
        } else {
            self.image = Some(
                world
                    .spawn((
                        node,
                        ImageNode::new(self.target.clone().ok_or("UI target unavailable")?),
                        bevy::picking::Pickable::IGNORE,
                        ZIndex(1),
                        ChildOf(parent),
                    ))
                    .id(),
            );
        }
        self.frames = self.frames.saturating_add(1);
        if !self.ready(world) && self.readback.is_none() && self.frames >= 12 && pipelines_ready {
            readbacks::admit(world)?;
            self.readback = Some(
                world
                    .spawn((
                        Screenshot::image(self.target.clone().ok_or("UI target unavailable")?),
                        Readback {
                            camera: self.camera,
                            version: TargetVersion {
                                size: Self::SIZE,
                                epoch: self.epoch,
                            },
                        },
                        readbacks::SceneCapture,
                    ))
                    .observe(captured)
                    .id(),
            );
        }
        Ok(())
    }

    pub(super) fn ready(&self, world: &World) -> bool {
        self.target.is_some()
            && world
                .get::<Confirmed>(self.camera)
                .is_some_and(|confirmed| {
                    confirmed.0
                        == TargetVersion {
                            size: Self::SIZE,
                            epoch: self.epoch,
                        }
                })
    }

    pub(super) fn release(&mut self, world: &mut World) {
        if let Some(readback) = self.readback.take() {
            readbacks::cancel(world, readback);
        }
        if let Some(entity) = self.image.take()
            && let Ok(entity) = world.get_entity_mut(entity)
        {
            entity.despawn();
        }
        if let Some(target) = self.target.take() {
            readbacks::release_image(world, target);
        }
        if world.entities().contains(self.camera) {
            world
                .entity_mut(self.camera)
                .insert(RenderTarget::None { size: UVec2::ONE })
                .remove::<(TargetVersion, Confirmed)>();
            if let Some(mut camera) = world.get_mut::<Camera>(self.camera) {
                camera.is_active = false;
            }
        }
        self.frames = 0;
    }
}
