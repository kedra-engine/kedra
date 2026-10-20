//! Signed Y-up camera orientation, independent of scene objects and source axes.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    X,
    Y,
    Z,
    NegX,
    NegY,
    NegZ,
}
pub(super) struct Endpoint {
    pub axis: Axis,
    pub center: Vec2,
    pub depth: f32,
}
impl Axis {
    pub(crate) const ALL: [Self; 6] = [
        Self::X,
        Self::Y,
        Self::Z,
        Self::NegX,
        Self::NegY,
        Self::NegZ,
    ];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::X => "axis-x",
            Self::Y => "axis-y",
            Self::Z => "axis-z",
            Self::NegX => "axis-neg-x",
            Self::NegY => "axis-neg-y",
            Self::NegZ => "axis-neg-z",
        }
    }
    pub(super) fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
            Self::NegX => Vec3::NEG_X,
            Self::NegY => Vec3::NEG_Y,
            Self::NegZ => Vec3::NEG_Z,
        }
    }
    pub(super) fn command(self, opposite: bool) -> super::camera::Command {
        use super::camera::Command::*;
        match (self, opposite) {
            (Self::X, false) | (Self::NegX, true) => Right,
            (Self::X, true) | (Self::NegX, false) => Left,
            (Self::Y, false) | (Self::NegY, true) => Top,
            (Self::Y, true) | (Self::NegY, false) => Bottom,
            (Self::Z, false) | (Self::NegZ, true) => Front,
            (Self::Z, true) | (Self::NegZ, false) => Back,
        }
    }
}
pub(super) fn project(rotation: Quat) -> Option<[Endpoint; 6]> {
    if !rotation.is_finite() || (rotation.length_squared() - 1.).abs() > 0.001 {
        return None;
    }
    let mut endpoints = Axis::ALL.map(|axis| {
        let local = rotation.inverse() * axis.vector();
        Endpoint {
            axis,
            // Front reference 186:3056: endpoint centers at 14 / 48 / 82px.
            center: Vec2::splat(48.) + Vec2::new(local.x, -local.y) * 34.,
            depth: local.z,
        }
    });
    endpoints.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    Some(endpoints)
}
pub(crate) fn align(world: &mut World, camera: Entity, axis: Axis) -> Result<(), String> {
    let pose = world.get::<Transform>(camera).ok_or("Camera retired")?;
    let opposite = pose.back().dot(axis.vector()) > 0.9999;
    super::camera::command(world, camera, axis.command(opposite))
}

pub(super) struct Widget {
    endpoints: [(Axis, Entity); 6],
    lines: [Entity; 3],
}
impl Widget {
    pub(super) fn mount(
        world: &mut World,
        view: &kedra_ui::authored::view::UiView,
    ) -> Result<Self, String> {
        let parent = view.element("orbit").map_err(|e| e.to_string())?;
        let mut endpoints = Vec::new();
        for axis in Axis::ALL {
            let entity = view.element(axis.name()).map_err(|e| e.to_string())?;
            endpoints.push((axis, entity));
        }
        // Only the projected line geometry is generated. Authored endpoints keep
        // their exact exported artwork, text styling, identities and focus state.
        let lines = [
            Color::srgb_u8(240, 163, 163),
            Color::srgb_u8(164, 207, 166),
            Color::srgb_u8(208, 139, 255),
        ]
        .map(|color| {
            world
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(47),
                        top: px(47.5),
                        width: px(2),
                        height: px(1),
                        ..default()
                    },
                    BackgroundColor(color.with_alpha(0.65)),
                    UiTransform::default(),
                    ZIndex(1),
                    bevy::picking::Pickable::IGNORE,
                    ChildOf(parent),
                ))
                .id()
        });
        Ok(Self {
            endpoints: endpoints
                .try_into()
                .map_err(|_| "Six axis endpoints required")?,
            lines,
        })
    }
    pub(super) fn update(&self, world: &mut World, rotation: Quat) {
        let Some(endpoints) = project(rotation) else {
            return;
        };
        for (rank, endpoint) in endpoints.iter().enumerate() {
            let Some((_, entity)) = self
                .endpoints
                .iter()
                .find(|(axis, _)| *axis == endpoint.axis)
            else {
                continue;
            };
            let next = UiTransform::from_translation(Val2::px(
                endpoint.center.x - 48.,
                endpoint.center.y - 48.,
            ));
            if world.get::<UiTransform>(*entity) != Some(&next) {
                world.entity_mut(*entity).insert(next);
            }
            let z = ZIndex(rank as i32 + 2);
            if world.get::<ZIndex>(*entity) != Some(&z) {
                world.entity_mut(*entity).insert(z);
            }
        }
        for (index, axis) in [Axis::X, Axis::Y, Axis::Z].into_iter().enumerate() {
            let Some(endpoint) = endpoints.iter().find(|e| e.axis == axis) else {
                continue;
            };
            let delta = endpoint.center - Vec2::splat(48.);
            let next = UiTransform {
                scale: Vec2::new(delta.length(), 1.),
                rotation: Rot2::radians(delta.y.atan2(delta.x)),
                ..default()
            };
            if world.get::<UiTransform>(self.lines[index]) != Some(&next) {
                world.entity_mut(self.lines[index]).insert(next);
            }
        }
    }
}
