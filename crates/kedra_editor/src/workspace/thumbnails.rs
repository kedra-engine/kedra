//! Project authorization and cache placement for the shared passive thumbnail owner.
use super::*;
use kedra_preview::thumbnails::{Kind, Thumbnails};
use std::any::TypeId;

#[derive(Resource, Default)]
struct State {
    cache: Thumbnails,
    layer: Option<usize>,
    revision: String,
    admitted: bool,
    store: Option<kedra_cache::Store>,
    diagnostic: Option<String>,
    project_root: Option<std::path::PathBuf>,
}
pub(super) fn reserved_layer(world: &World) -> usize {
    world
        .get_resource::<State>()
        .and_then(|state| state.layer)
        .unwrap_or_default()
}
pub(super) fn poll(world: &mut World, workspace: &mut EditorWorkspace) {
    if !world.contains_resource::<State>() {
        world.init_resource::<State>();
    }
    world.resource_scope(|world, mut state: Mut<State>| {
        if state.layer.is_none() {
            let Some(layer) = workspace.panels.scene_layer.checked_add(1) else {
                return;
            };
            workspace.panels.scene_layer = layer;
            state.layer = Some(layer);
        }
        workspace.panels.scene_layer = workspace
            .panels
            .scene_layer
            .max(state.layer.unwrap_or_default());
        let project = world
            .get_resource::<crate::model::ProjectManager>()
            .and_then(|manager| manager.project.clone());
        if let Some(project) = &project
            && world.contains_resource::<kedra_authoring_assets::Delivery>()
        {
            workspace
                .documents
                .ensure_source(world, project.assets.clone());
        }
        let source = workspace.documents.scene_source();
        let mut revision = String::new();
        if let (Some(project), Some(source), Some(delivery)) = (
            &project,
            source,
            world.get_resource::<kedra_authoring_assets::Delivery>(),
        ) {
            let mut bytes = project.manifest.id.to_string();
            for asset in &source.assets {
                bytes.push_str(&asset.path);
                bytes.push('\0');
                bytes.push_str(&asset.hash);
            }
            for (path, file) in &delivery.manifest.files {
                bytes.push_str(path);
                bytes.push('\0');
                bytes.push_str(&file.sha256);
            }
            revision = kedra_assets::delivery::digest(bytes.as_bytes());
            if revision != state.revision || state.project_root.as_ref() != Some(&project.root) {
                let admission = delivery.matches_resources(&project.assets, source);
                state.admitted = admission.is_ok();
                if let Err(error) = admission {
                    warn!("Asset thumbnails unavailable: {error}");
                }
                state.store = kedra_cache::Store::project(&project.root, default()).ok();
                state.revision = revision.clone();
                state.project_root = Some(project.root.clone());
            }
        }
        if revision.is_empty() || !state.admitted {
            revision.clear();
        }
        let mut demand = Vec::new();
        for panel in workspace.panels.browsers.values() {
            demand.extend(panel.thumbnail_demand(world, &revision));
        }
        demand.retain(|(image, badge, request)| {
            let Some(source) = source else {
                return false;
            };
            let Some(delivery) = world.get_resource::<kedra_authoring_assets::Delivery>() else {
                return false;
            };
            let Ok(path) = bevy::asset::AssetPath::try_parse(&request.path) else {
                return false;
            };
            let kind = match request.kind {
                Kind::Image => TypeId::of::<Image>(),
                Kind::Model => TypeId::of::<bevy::gltf::Gltf>(),
            };
            let accepted = delivery.accepts(source, kind, &path)
                && source
                    .assets
                    .iter()
                    .find(|a| format!("game://{}", a.path) == request.path)
                    .is_some_and(|a| a.bytes <= 16 * 1024 * 1024);
            if !accepted {
                crate::asset_browser::thumbnails::show(world, *image, *badge, None);
            }
            accepted
        });
        let requests: Vec<_> = demand.iter().map(|(_, _, r)| r.clone()).collect();
        let store = state.store.clone();
        let layer = state.layer.unwrap_or_default();
        state.cache.update(world, &requests, layer, store.as_ref());
        crate::asset_browser::thumbnails::apply(world, &demand, &state.cache);
        if state.cache.needs_frames()
            && let Some(mut wake) = world.get_resource_mut::<kedra_wakeup::WakeDemand>()
        {
            wake.active();
        }
        if let Some(error) = state.cache.disk_error().map(str::to_owned)
            && state.diagnostic.as_ref() != Some(&error)
        {
            warn!("Thumbnail disk cache unavailable: {error}");
            state.diagnostic = Some(error);
        }
    });
}

pub(super) fn drain(world: &mut World) {
    if world.contains_resource::<EditorWorkspace>() || !world.contains_resource::<State>() {
        return;
    }
    let finished = world.resource_scope(|world, mut state: Mut<State>| {
        let layer = state.layer.unwrap_or_default();
        state.cache.update(world, &[], layer, None);
        !state.cache.busy()
    });
    if finished {
        world.remove_resource::<State>();
    }
}
