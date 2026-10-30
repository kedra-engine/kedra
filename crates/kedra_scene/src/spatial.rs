//! Opt-in inert spatial schemas. Registration never installs a renderer or gameplay.
use crate::{Result, failure, reflection::ComponentPolicies};
use bevy::world_serialization::WorldAsset;
use bevy::{prelude::*, reflect::TypeRegistry};

/// Bounded native primitive specification; consumers materialize native Bevy meshes.
#[derive(Component, Clone, Reflect)]
#[reflect(Component, Default)]
pub enum Primitive {
    /// Positive full extents in local scene units.
    Cuboid {
        /// Full local extents, each in (0, 10000].
        size: Vec3,
    },
    /// Positive radius; tessellation is presentation-owned.
    Sphere {
        /// Local radius in (0, 10000].
        radius: f32,
    },
}
impl Default for Primitive {
    fn default() -> Self {
        Self::Cuboid { size: Vec3::ONE }
    }
}

/// Opaque standard-surface data, separate from hooked renderer components.
/// Texture handles retain native dependency/subasset semantics. No live material is saved.
#[derive(Component, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct Surface {
    /// Linear or sRGB native color; all channels must be finite and within 0..=1.
    pub color: Color,
    /// Optional base-color image from the explicitly admitted asset scope.
    pub texture: Option<Handle<Image>>,
    /// Native metallic factor in 0..=1.
    pub metallic: f32,
    /// Native perceptual roughness in 0..=1.
    pub roughness: f32,
}
impl Default for Surface {
    fn default() -> Self {
        Self {
            color: Color::srgb(0.8, 0.8, 0.8),
            texture: None,
            metallic: 0.,
            roughness: 0.7,
        }
    }
}

/// Directional illumination, interpreted along the node's native local -Z axis.
/// Shadows are deferred; inserting this source data executes no renderer hooks.
#[derive(Component, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct Sun {
    /// Finite color channels in 0..=1.
    pub color: Color,
    /// Illuminance in lux, 0..=1000000.
    pub illuminance: f32,
}
impl Default for Sun {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            illuminance: 10000.,
        }
    }
}

/// A static native model scene. A viewer must not activate imported cameras,
/// lights, skins or animations without a separate supported-behavior contract.
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct Model {
    /// Native scene subasset, for example `game://ship.glb#Scene0`.
    pub scene: Handle<WorldAsset>,
}

/// Authored perspective camera settings. This inert component owns no window or
/// render target; hosts choose when to activate a camera. Scene View keeps its
/// navigation camera independent of authored cameras.
#[derive(Component, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct SceneCamera {
    /// Vertical field of view in radians, strictly between 0 and pi.
    pub fov: f32,
    /// Positive near clipping distance, smaller than `far`.
    pub near: f32,
    /// Far clipping distance, finite and at most 1000000 scene units.
    pub far: f32,
}
impl Default for SceneCamera {
    fn default() -> Self {
        Self {
            fov: std::f32::consts::FRAC_PI_4,
            near: 0.1,
            far: 1000.,
        }
    }
}
fn camera(value: &dyn PartialReflect) -> Result<()> {
    let v = SceneCamera::from_reflect(value).ok_or_else(|| failure("invalid camera"))?;
    if !v.fov.is_finite()
        || !(0.0..std::f32::consts::PI).contains(&v.fov)
        || v.fov == 0.
        || !v.near.is_finite()
        || !v.far.is_finite()
        || v.near <= 0.
        || v.far <= v.near
        || v.far > 1_000_000.
    {
        return Err(failure(
            "camera requires a finite field of view in (0, pi) and 0 < near < far <= 1000000",
        ));
    }
    Ok(())
}

fn color(value: Color) -> bool {
    value
        .to_linear()
        .to_f32_array()
        .into_iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
}
fn transform(value: &dyn PartialReflect) -> Result<()> {
    let v = Transform::from_reflect(value).ok_or_else(|| failure("invalid spatial transform"))?;
    if !v.translation.is_finite()
        || v.translation.abs().max_element() > 1_000_000.
        || !v.scale.is_finite()
        || v.scale.abs().max_element() > 10_000.
        || !v.rotation.is_finite()
        || (v.rotation.length_squared() - 1.).abs() > 0.001
    {
        return Err(failure(
            "spatial transform requires finite bounded values and a unit quaternion",
        ));
    }
    Ok(())
}
fn primitive(value: &dyn PartialReflect) -> Result<()> {
    let v = Primitive::from_reflect(value).ok_or_else(|| failure("invalid primitive"))?;
    let size = match v {
        Primitive::Cuboid { size } => size,
        Primitive::Sphere { radius } => Vec3::splat(radius),
    };
    if !size.is_finite() || size.min_element() <= 0. || size.max_element() > 10_000. {
        return Err(failure(
            "primitive dimensions must be finite and in 0..=10000, excluding zero",
        ));
    }
    Ok(())
}
fn surface(value: &dyn PartialReflect) -> Result<()> {
    let v = Surface::from_reflect(value).ok_or_else(|| failure("invalid surface"))?;
    if !color(v.color)
        || !v.metallic.is_finite()
        || !(0.0..=1.0).contains(&v.metallic)
        || !v.roughness.is_finite()
        || !(0.0..=1.0).contains(&v.roughness)
    {
        return Err(failure(
            "surface requires finite color, metallic and roughness in 0..=1",
        ));
    }
    Ok(())
}
fn sun(value: &dyn PartialReflect) -> Result<()> {
    let v = Sun::from_reflect(value).ok_or_else(|| failure("invalid sun"))?;
    if !color(v.color)
        || !v.illuminance.is_finite()
        || !(0.0..=1_000_000.0).contains(&v.illuminance)
    {
        return Err(failure(
            "sun requires finite color and illuminance in 0..=1000000",
        ));
    }
    Ok(())
}

/// Register version-1 spatial data in the existing policy/Bevy registry.
/// Call once per policy set; duplicate registration returns an error. These
/// schemas have authoring/preview consumers but add no projection or simulation;
/// authored camera activation remains host-owned.
/// Invalid dimensions, transforms or lighting fail compilation before live writes.
pub fn register(types: &mut TypeRegistry, policies: &mut ComponentPolicies) -> Result<()> {
    policies.register::<crate::environment::SceneEnvironment>(
        types,
        "scene.environment",
        1,
        |_| Ok(()),
    )?;
    policies.register::<Transform>(types, "scene.transform", 1, transform)?;
    policies.register::<Primitive>(types, "scene.primitive", 1, primitive)?;
    policies.register::<Surface>(types, "scene.surface", 1, surface)?;
    policies.register::<Sun>(types, "scene.sun", 1, sun)?;
    policies.register::<SceneCamera>(types, "scene.camera", 1, camera)?;
    policies.register::<Model>(types, "scene.model", 1, |_| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_is_inert_and_validators_reject_failure_paths() -> Result<()> {
        let mut types = TypeRegistry::default();
        let mut policies = ComponentPolicies::default();
        register(&mut types, &mut policies)?;
        assert!(register(&mut types, &mut policies).is_err());
        assert!(camera(&SceneCamera::default()).is_ok());
        for invalid in [
            SceneCamera {
                fov: f32::NAN,
                ..default()
            },
            SceneCamera {
                near: 0.,
                ..default()
            },
            SceneCamera {
                near: 10.,
                far: 1.,
                ..default()
            },
        ] {
            assert!(camera(&invalid).is_err());
        }
        assert!(primitive(&Primitive::Sphere { radius: 0. }).is_err());
        assert!(
            primitive(&Primitive::Cuboid {
                size: Vec3::splat(f32::NAN)
            })
            .is_err()
        );
        assert!(primitive(&Primitive::Sphere { radius: 1. }).is_ok());
        assert!(transform(&Transform::from_rotation(Quat::from_xyzw(0., 0., 0., 0.))).is_err());
        assert!(transform(&Transform::IDENTITY).is_ok());
        assert!(
            surface(&Surface {
                color: Color::WHITE,
                texture: None,
                metallic: f32::NAN,
                roughness: 0.5
            })
            .is_err()
        );
        assert!(
            sun(&Sun {
                color: Color::WHITE,
                illuminance: -1.
            })
            .is_err()
        );
        Ok(())
    }
}
