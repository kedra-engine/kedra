//! Native Browser-to-Hierarchy/Scene placement retains the original source destination.
use super::*;
use crate::documents::inspection::Target;
use bevy::{gltf::Gltf, picking::events::PointerDragDrop};
mod preview;

#[derive(Resource)]
struct Pending {
    target: Target,
    expected: BTreeMap<kedra_scene::document::DocumentId, u64>,
    model: Handle<Gltf>,
    started: std::time::Instant,
    position: Option<Vec3>,
}
pub(super) fn install(app: &mut App) {
    app.add_observer(dropped)
        .add_systems(Update, (poll, preview::tick));
}
fn dropped(
    mut event: On<PointerDragDrop>,
    bindings: Query<&crate::hierarchy::Binding>,
    viewports: Query<(), With<crate::scene_view::Viewport>>,
    mut commands: Commands,
) {
    let destination = event.event_target();
    let hierarchy = bindings.get(destination).ok().and_then(|binding| {
        let crate::hierarchy::Action::Select(node) = &binding.action else {
            return None;
        };
        Some((binding.view, binding.document?, node.clone()))
    });
    if hierarchy.is_none() && !viewports.contains(destination) {
        return;
    };
    let owner = event.dropped;
    event.propagate(false);
    commands.queue(move |world: &mut World| {
        let Some(crate::drag::DragPayload::Assets(assets)) = crate::drag::payload(world, owner)
        else {
            return;
        };
        if assets.len() != 1 {
            return;
        }
        let path = assets[0].path().to_owned();
        if !world.contains_resource::<EditorWorkspace>()
            || hierarchy.as_ref().is_some_and(|(view, document, _)| {
                !crate::workspace::node_drag_available(world, *view, *document)
            })
        {
            return;
        }
        world.resource_scope(|world, state: Mut<EditorWorkspace>| {
            let result = (|| {
                if world.contains_resource::<Pending>()
                    || super::settlement::source_edits(world, &state)
                {
                    return Err("Finish the pending source operation first.".to_owned());
                }
                let target = if let Some((_, document, node)) = hierarchy {
                    state.documents.target(document, node)
                } else {
                    state
                        .panels
                        .scenes
                        .iter()
                        .find(|(view, scene)| {
                            scene.focus() == destination && inspector_visible(&state, **view)
                        })
                        .and_then(|(view, _)| {
                            state
                                .documents
                                .opened
                                .iter()
                                .find(|(_, opened)| opened.view == *view)
                        })
                        .and_then(|(document, _)| state.documents.root_target(*document))
                }
                .ok_or("Model parent unavailable")?;
                let position = if viewports_marker(world, destination) {
                    Some(preview::drop_position(world, owner, destination, &path)?)
                } else {
                    None
                };
                let delivery = world
                    .get_resource::<kedra_authoring_assets::Delivery>()
                    .ok_or("Model placement requires a verified project delivery")?;
                let root = &world
                    .resource::<crate::model::ProjectManager>()
                    .project
                    .as_ref()
                    .ok_or("No project")?
                    .assets;
                delivery.matches_resources(
                    root,
                    state
                        .documents
                        .scene_source()
                        .ok_or("Source baseline missing")?,
                )?;
                let parsed = bevy::asset::AssetPath::try_parse(&path).map_err(|e| e.to_string())?;
                if parsed.label().is_some()
                    || parsed.path().extension().is_none_or(|ext| ext != "glb")
                    || !delivery
                        .manifest
                        .assets
                        .iter()
                        .any(|asset| parsed.path() == std::path::Path::new(&asset.path))
                {
                    return Err("Choose a GLB model from the admitted project delivery.".into());
                }
                let handle = world
                    .resource::<AssetServer>()
                    .load_builder()
                    .with_settings::<bevy::gltf::GltfLoaderSettings>(|settings| {
                        settings.load_cameras = false;
                        settings.load_lights = false;
                        settings.load_animations = false;
                    })
                    .load::<Gltf>(path);
                world.insert_resource(Pending {
                    target,
                    expected: state.documents.revisions(),
                    model: handle,
                    started: std::time::Instant::now(),
                    position,
                });
                Ok(())
            })();
            if let Err(error) = result {
                notice(world, &state, &error);
            }
        });
    });
}
fn poll(world: &mut World) {
    let Some(pending) = world.remove_resource::<Pending>() else {
        return;
    };
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    let scene = world
        .resource::<Assets<Gltf>>()
        .get(&pending.model)
        .map(|gltf| {
            gltf.default_scene
                .clone()
                .or_else(|| (gltf.scenes.len() == 1).then(|| gltf.scenes[0].clone()))
                .ok_or("Model needs one scene or a declared default scene".to_owned())
        });
    let Some(scene) = scene else {
        if pending.started.elapsed().as_secs() < 30
            && !matches!(
                world
                    .resource::<AssetServer>()
                    .load_state(pending.model.id()),
                bevy::asset::LoadState::Failed(_)
            )
        {
            world.insert_resource(pending);
        } else {
            world.resource_scope(|world, state: Mut<EditorWorkspace>| {
                notice(world, &state, "Model loading failed or timed out.")
            });
        }
        return;
    };
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result: Result<(), String> = (|| {
            let model = scene?;
            if super::settlement::source_edits(world, &state) {
                return Err("Finish pending source edits before model placement.".into());
            }
            let delivery = world
                .get_resource::<kedra_authoring_assets::Delivery>()
                .ok_or("Project delivery retired")?;
            let source = state
                .documents
                .scene_source()
                .ok_or("Source baseline retired")?;
            let root = &world
                .resource::<crate::model::ProjectManager>()
                .project
                .as_ref()
                .ok_or("Project retired")?
                .assets;
            delivery.matches_resources(root, source)?;
            if !delivery.accepts(
                source,
                std::any::TypeId::of::<bevy::world_serialization::WorldAsset>(),
                model.path().ok_or("Model source path missing")?,
            ) {
                return Err("Model scene is outside the admitted delivery.".into());
            }
            let context = super::history_context::capture(world, &state);
            let registry = world.resource::<AppTypeRegistry>().clone();
            state.documents.create_model(
                &pending.target,
                &pending.expected,
                &model,
                pending.position,
                &registry.read(),
                world.resource::<kedra_scene::reflection::ComponentPolicies>(),
            )?;
            super::history_context::accepted(world, &mut state, context);
            Ok(())
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}
fn viewports_marker(world: &World, entity: Entity) -> bool {
    world.get::<crate::scene_view::Viewport>(entity).is_some()
}
