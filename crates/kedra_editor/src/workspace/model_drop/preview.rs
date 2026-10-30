//! Transient model placement shares static preview materialization, never source history.
use super::*;
#[derive(Resource)]
struct Preview {
    owner: Entity,
    viewport: Entity,
    path: String,
    model: Handle<Gltf>,
    root: Option<Entity>,
    lift: f32,
    position: Option<Vec3>,
}
pub(super) fn drop_position(
    world: &World,
    owner: Entity,
    viewport: Entity,
    path: &str,
) -> Result<Vec3, String> {
    world
        .get_resource::<Preview>()
        .filter(|p| p.owner == owner && p.viewport == viewport && p.path == path)
        .and_then(|p| p.position)
        .ok_or("Wait for the placement preview and choose a surface.".into())
}
fn retire(world: &mut World, preview: Preview) {
    if let Some(root) = preview.root
        && let Ok(entity) = world.get_entity_mut(root)
    {
        entity.despawn();
    }
}
pub(super) fn tick(world: &mut World) {
    let active = crate::drag::active(world);
    let sample = world
        .get_resource::<input::CursorSample>()
        .map(|s| (s.window, s.position));
    let old = world.remove_resource::<Preview>();
    let Some((owner, crate::drag::DragPayload::Assets(assets))) = active else {
        if let Some(old) = old {
            retire(world, old);
        }
        return;
    };
    let Some((window, point)) = sample else {
        if let Some(old) = old {
            retire(world, old);
        }
        return;
    };
    if assets.len() != 1
        || !assets[0].path().ends_with(".glb")
        || !world.contains_resource::<EditorWorkspace>()
    {
        if let Some(old) = old {
            retire(world, old);
        }
        return;
    }
    let path = assets[0].path().to_owned();
    world.resource_scope(|world, state: Mut<EditorWorkspace>| {
        let destination = state.panels.scenes.iter().find(|(view, scene)| {
            inspector_visible(&state, **view)
                && scene.area(world).is_some_and(|r| r.contains(point))
                && kedra_ui::native_input::owner(world, scene.focus()) == window
        });
        let Some((_, scene)) = destination else {
            if let Some(old) = old {
                retire(world, old);
            }
            return;
        };
        let viewport = scene.focus();
        let old = match old {
            Some(old) if old.owner == owner && old.viewport == viewport && old.path == path => {
                Some(old)
            }
            Some(old) => {
                retire(world, old);
                None
            }
            None => None,
        };
        let mut preview = if let Some(old) = old {
            old
        } else {
            let model = world
                .resource::<AssetServer>()
                .load_builder()
                .with_settings::<bevy::gltf::GltfLoaderSettings>(|s| {
                    s.load_cameras = false;
                    s.load_lights = false;
                    s.load_animations = false;
                })
                .load::<Gltf>(path.clone());
            Preview {
                owner,
                viewport,
                path,
                model,
                root: None,
                lift: 0.,
                position: None,
            }
        };
        preview.position = None;
        let result = (|| {
            let (position, layer) = scene.placement(world, point)?;
            if preview.root.is_none() {
                let gltf = world
                    .resource::<Assets<Gltf>>()
                    .get(&preview.model)
                    .ok_or("Loading model")?;
                let model = gltf
                    .default_scene
                    .clone()
                    .or_else(|| (gltf.scenes.len() == 1).then(|| gltf.scenes[0].clone()))
                    .ok_or("Choose a model scene")?;
                let root = world.spawn((Transform::IDENTITY, Visibility::Hidden)).id();
                let result = kedra_preview::models::mount(world, root, &model, layer, 4096)
                    .and_then(|_| kedra_preview::bounds::measure(world, root));
                let bounds = match result {
                    Ok(bounds) => bounds,
                    Err(error) => {
                        world.despawn(root);
                        return Err(error);
                    }
                };
                preview.root = Some(root);
                preview.lift = bounds.map_or(0., |(lo, _)| -lo.y);
                let mut children = vec![root];
                while let Some(e) = children.pop() {
                    if let Some(c) = world.get::<Children>(e) {
                        children.extend(c.iter());
                    }
                    world
                        .entity_mut(e)
                        .insert(crate::scene_view::PlacementPreview);
                }
            }
            let position = position + Vec3::Y * preview.lift;
            world
                .entity_mut(preview.root.ok_or("Preview missing")?)
                .insert((Transform::from_translation(position), Visibility::Inherited));
            preview.position = Some(position);
            Ok::<_, String>(())
        })();
        if result.is_err()
            && let Some(root) = preview.root
            && let Ok(mut e) = world.get_entity_mut(root)
        {
            e.insert(Visibility::Hidden);
        }
        world.insert_resource(preview);
    });
}
