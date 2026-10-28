//! Retain native GLTF metadata only for the selected source's admitted delivery
//! members. Scene paths come from loaded native handles, never guessed Scene0 labels.
use bevy::gltf::Gltf;
use bevy::prelude::*;
#[cfg(test)]
mod tests;
#[derive(Component, Default)]
struct Catalog {
    handles: Vec<Handle<Gltf>>,
    choices: Vec<String>,
}
/// Enumerate real GLTF scene labels under supplied admitted parents. Retains native
/// metadata on the explicit owner; loading grants no authoring rights. Returns
/// choices and whether they changed, without assuming a Scene0 label.
pub fn choices(world: &mut World, root: Entity, paths: &[String]) -> (Vec<String>, bool) {
    let mut catalog = world.entity_mut(root).take::<Catalog>().unwrap_or_default();
    catalog.handles.retain(|handle| {
        handle
            .path()
            .is_some_and(|path| paths.contains(&path.to_string()))
    });
    if let Some(server) = world.get_resource::<AssetServer>()
        && world.contains_resource::<Assets<Gltf>>()
    {
        for path in paths {
            if !catalog.handles.iter().any(|handle| {
                handle
                    .path()
                    .is_some_and(|current| current.to_string() == *path)
            }) {
                catalog.handles.push(
                    server
                        .load_builder()
                        .with_settings::<bevy::gltf::GltfLoaderSettings>(|settings| {
                            settings.load_cameras = false;
                            settings.load_lights = false;
                            settings.load_animations = false;
                        })
                        .load::<Gltf>(path.clone()),
                );
            }
        }
    }
    let mut choices = Vec::new();
    if let Some(assets) = world.get_resource::<Assets<Gltf>>() {
        for handle in &catalog.handles {
            let Some(gltf) = assets.get(handle) else {
                continue;
            };
            for scene in &gltf.scenes {
                let Some(path) = scene.path() else {
                    continue;
                };
                // A native label must still belong to this exact admitted parent.
                if handle
                    .path()
                    .is_some_and(|parent| parent.without_label() == path.without_label())
                    && path.label().is_some()
                {
                    choices.push(path.to_string());
                }
            }
        }
    }
    choices.sort();
    choices.dedup();
    let changed = choices != catalog.choices;
    catalog.choices = choices.clone();
    world.entity_mut(root).insert(catalog);
    (choices, changed)
}
