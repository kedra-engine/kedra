//! Explicit opt-in native hydration. No global scene discovery or gameplay schedule.
use crate::{
    environment::Environment,
    spatial::{Primitive, Sun, Surface},
};
use bevy::prelude::*;

/// Convert supported inert primitive data to a native mesh. Callers retain/retire
/// the asset handle; this function allocates no world entities or shared assets.
pub fn mesh(source: &Primitive) -> Mesh {
    match source {
        Primitive::Cuboid { size } => Mesh::from(Cuboid::from_size(*size)),
        Primitive::Sphere { radius } => Sphere::new(*radius).mesh().uv(32, 16),
    }
}
/// Native material value; texture handles retain their normal Bevy ownership.
pub fn material(source: &Surface) -> StandardMaterial {
    StandardMaterial {
        base_color: source.color,
        base_color_texture: source.texture.clone(),
        metallic: source.metallic,
        perceptual_roughness: source.roughness,
        ..default()
    }
}
/// Supported directional-light profile. Shadows are explicitly unsupported in
/// this initial shared profile, rather than silently differing between hosts.
pub fn sun(source: &Sun) -> DirectionalLight {
    DirectionalLight {
        color: source.color,
        illuminance: source.illuminance,
        shadow_maps_enabled: false,
        ..default()
    }
}
/// Explicit shared no-LUT camera default. Hosts may replace it with native Bevy
/// settings; matching images then requires matching those camera settings too.
pub fn tonemapping() -> bevy::core_pipeline::tonemapping::Tonemapping {
    bevy::core_pipeline::tonemapping::Tonemapping::Reinhard
}
/// Apply a validated environment only to the specified existing camera. None
/// means black background/zero ambient, not studio fallback. No global resources
/// or other cameras change. Call explicitly to replace/refresh, not every frame;
/// gameplay remains free to drive the resulting native camera components.
/// Validation/retired-camera errors leave the destination untouched.
pub fn environment(
    world: &mut World,
    camera: Entity,
    source: Option<&Environment>,
) -> crate::Result<()> {
    if let Some(source) = source {
        source.validate()?;
    }
    if world.get::<Camera>(camera).is_none() {
        return Err(crate::failure(
            "environment destination camera is unavailable",
        ));
    }
    let (background, color, brightness) = source.map_or((Color::BLACK, Color::WHITE, 0.), |s| {
        (s.background, s.ambient_color, s.ambient_brightness)
    });
    world
        .get_mut::<Camera>(camera)
        .ok_or_else(|| crate::failure("environment destination camera retired"))?
        .clear_color = background.into();
    world.entity_mut(camera).insert(AmbientLight {
        color,
        brightness,
        ..default()
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_native_camera_bindings_do_not_touch_each_other_or_global_light()
    -> crate::Result<()> {
        let mut world = World::new();
        world.insert_resource(GlobalAmbientLight {
            brightness: 42.,
            ..default()
        });
        let a = world.spawn(Camera::default()).id();
        let b = world.spawn(Camera::default()).id();
        let mut source = Environment {
            version: 1,
            background: Color::WHITE,
            ambient_color: Color::WHITE,
            ambient_brightness: 200.,
        };
        environment(&mut world, a, Some(&source))?;
        environment(&mut world, b, None)?;
        assert_eq!(
            world.get::<AmbientLight>(a).map(|v| v.brightness),
            Some(200.)
        );
        assert_eq!(world.get::<AmbientLight>(b).map(|v| v.brightness), Some(0.));
        assert_eq!(world.resource::<GlobalAmbientLight>().brightness, 42.);
        source.ambient_brightness = f32::NAN;
        assert!(environment(&mut world, a, Some(&source)).is_err());
        assert_eq!(
            world.get::<AmbientLight>(a).map(|v| v.brightness),
            Some(200.)
        );
        world.entity_mut(a).despawn();
        assert!(environment(&mut world, a, None).is_err());
        assert_eq!(world.get::<AmbientLight>(b).map(|v| v.brightness), Some(0.));
        Ok(())
    }
}
