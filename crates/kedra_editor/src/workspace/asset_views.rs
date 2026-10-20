//! Raw asset viewers retain asset identity, never manufacture an authoring session.
use super::*;
use kedra_docking::model::View;
pub(super) fn publish(
    world: &mut World,
    state: &mut EditorWorkspace,
    path: std::path::PathBuf,
) -> Result<(), String> {
    let key = path.to_str().ok_or("Asset path must be Unicode")?;
    if !key.ends_with(".png") && !key.ends_with(".glb") {
        return Err("Asset preview supports PNG and static GLB only".into());
    }
    let source = state
        .documents
        .scene_source()
        .ok_or("No admitted source inventory")?
        .clone();
    let asset = source
        .assets
        .iter()
        .find(|asset| asset.path == key)
        .ok_or("Asset not admitted")?;
    if asset.id.is_none() {
        return Err(
            "Asset preview requires initialized source metadata and a matching delivery".into(),
        );
    }
    if let Some(id) = state.layout.snapshot().views.iter().find_map(|(id, v)| {
        (v.kind == "asset-preview" && v.restore_key.as_deref() == Some(key)).then_some(*id)
    }) {
        if state.layout.snapshot().hidden.contains_key(&id) {
            state
                .layout
                .execute(Command::Show {
                    view: id,
                    destination: super::document_views::content_destination(state),
                })
                .map_err(|e| e.to_string())?;
        }
        if let Some(scene) = state.panels.scenes.get_mut(&id)
            && !scene.bound()
        {
            state.panels.scene_layer = state
                .panels
                .scene_layer
                .checked_add(1)
                .ok_or("Scene layer exhausted")?;
            scene.prepare_asset(world, key, &source, state.panels.scene_layer)?;
        }
        return apply(world, state, Command::Activate(id));
    }
    let id = ViewId::new();
    let target = super::document_views::content_destination(state);
    let mut candidate = state
        .layout
        .snapshot()
        .clone()
        .into_workspace()
        .map_err(|e| e.to_string())?;
    candidate
        .execute(Command::Open {
            id,
            view: View {
                kind: "asset-preview".into(),
                restore_key: Some(key.into()),
            },
            destination: target,
        })
        .map_err(|e| e.to_string())?;
    let mut scene = crate::scene_view::View::mount(world, state.stash, id, key)?;
    state.panels.scene_layer = state
        .panels
        .scene_layer
        .checked_add(1)
        .ok_or("Scene layer exhausted")?;
    if let Err(error) = scene.prepare_asset(world, key, &source, state.panels.scene_layer) {
        scene.retire(world);
        world.entity_mut(scene.root).despawn();
        return Err(error);
    }
    if let Err(error) = state.renderer.register(id, scene.root) {
        scene.retire(world);
        world.entity_mut(scene.root).despawn();
        return Err(error);
    }
    state.panels.scenes.insert(id, scene);
    state.titles.insert(id, key.into());
    state.layout = candidate;
    state.documents.active = None;
    state.preferences.dirty = true;
    redraw(world, state)
}
