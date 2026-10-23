//! Pending authored views are owned by their native parent and retire with it.
use crate::model::*;
use bevy::prelude::*;
use kedra_scene::{
    assets::{PrefabCatalogAsset, PrefabSources},
    document::PrefabDocument,
    instance::CompiledPrefab,
};
use kedra_ui::authored::{components::UiFailure, view::UiView};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Frame(kedra_desktop::SurfaceKind),
    Caption(kedra_desktop::SurfaceKind),
    Projects,
    Loading,
    Editor,
    Entry(EntryMode),
}
#[derive(Component)]
struct Pending {
    kind: Kind,
    started: Duration,
    sequence: u64,
}
#[derive(Resource, Default)]
pub(crate) struct ViewClock(u64);

#[cfg(test)]
pub(crate) mod test_support;

/// Resolve an already loaded native asset, never initiate a load or invent identity.
/// Both native catalog membership/materialization and published compilation are required.
pub(crate) fn compiled(world: &World, path: &str) -> Result<Arc<CompiledPrefab>, String> {
    let asset_path = bevy::asset::AssetPath::try_parse(path)
        .map_err(|error| format!("Invalid authored asset path: {error}"))?;
    if !matches!(asset_path.source(), bevy::asset::io::AssetSourceId::Name(_))
        || asset_path.label().is_some()
    {
        return Err(
            "Authored documents require a full source-qualified asset path without a label".into(),
        );
    }
    let sources = world.resource::<PrefabSources>();
    if !sources.ready() {
        return Err("Authored sources are not ready".into());
    }
    let handle = world
        .resource::<AssetServer>()
        .get_handle::<PrefabDocument>(asset_path)
        .ok_or_else(|| format!("Authored asset is not loaded: {path}"))?;
    let catalog = world
        .resource::<Assets<PrefabCatalogAsset>>()
        .get(&sources.handle)
        .ok_or("Authored native catalog is not materialized")?;
    if !catalog
        .documents
        .iter()
        .any(|member| member.id() == handle.id())
    {
        return Err(format!(
            "Authored asset is not a native catalog member: {path}"
        ));
    }
    let document = world
        .resource::<Assets<PrefabDocument>>()
        .get(&handle)
        .ok_or_else(|| format!("Authored asset is not materialized: {path}"))?;
    let snapshot = sources.snapshot();
    if snapshot.catalog.documents.get(&document.id) != Some(document) {
        return Err(format!(
            "Authored asset differs from the published source revision: {path}"
        ));
    }
    snapshot
        .compiled
        .get(&document.id)
        .cloned()
        .ok_or_else(|| format!("Authored asset has no published compilation: {path}"))
}

pub(crate) fn mount_asset(
    world: &mut World,
    parent: Entity,
    path: &str,
    scope: &str,
) -> Result<UiView, String> {
    let compiled = compiled(world, path)?;
    mount_compiled(world, parent, &compiled, scope)
}

pub(crate) fn request(world: &mut World, parent: Entity, kind: Kind) -> Result<Entity, String> {
    if !world.entities().contains(parent) {
        return Err("view parent was retired".into());
    }
    let sequence = world.resource::<ViewClock>().0;
    world.resource_mut::<ViewClock>().0 = sequence
        .checked_add(1)
        .ok_or("view request counter exhausted")?;
    let started = world.resource::<Time<Real>>().elapsed();
    Ok(world
        .spawn((
            ChildOf(parent),
            Pending {
                kind,
                started,
                sequence,
            },
        ))
        .id())
}

fn mount_compiled(
    world: &mut World,
    parent: Entity,
    compiled: &Arc<CompiledPrefab>,
    scope: &str,
) -> Result<UiView, String> {
    let mut ancestor = Some(parent);
    let mut target = None;
    while let Some(entity) = ancestor {
        if let Some(camera) = world.get::<UiTargetCamera>(entity) {
            if let Some(bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(
                window,
            ))) = world.get::<bevy::camera::RenderTarget>(camera.0)
            {
                target = Some(*window);
            }
            break;
        }
        ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    let window = target
        .or_else(|| world.resource::<kedra_desktop::DesktopSurface>().window)
        .and_then(|entity| world.get::<Window>(entity))
        .ok_or("Authored view has no owned native window")?;
    let density = window.scale_factor() * world.resource::<UiScale>().0;
    UiView::mount(
        world,
        compiled,
        Some(parent),
        &BTreeMap::new(),
        scope,
        density,
    )
    .map_err(|e| e.to_string())
}

pub(crate) fn hydrate(world: &mut World) {
    if let Some(error) = world.resource::<UiFailure>().0.clone() {
        fail(world, error);
        return;
    }
    let mut requests = world
        .query::<(Entity, &ChildOf, &Pending)>()
        .iter(world)
        .map(|(entity, parent, request)| {
            (
                request.sequence,
                entity,
                parent.parent(),
                request.kind,
                request.started,
            )
        })
        .collect::<Vec<_>>();
    requests.sort_by_key(|request| request.0);
    for (_, entity, parent, kind, started) in requests {
        if !world.entities().contains(entity) || !world.entities().contains(parent) {
            continue;
        }
        let sources = world.resource::<PrefabSources>();
        let failure = sources.error.clone().or_else(|| {
            (!sources.ready()
                && world
                    .resource::<Time<Real>>()
                    .elapsed()
                    .saturating_sub(started)
                    > Duration::from_secs(30))
            .then(|| "Authored startup sources did not become ready within 30 seconds".into())
        });
        if let Some(error) = failure {
            world.despawn(entity);
            fail(world, error);
            return;
        }
        if !sources.ready() {
            continue;
        }
        let path = match kind {
            Kind::Frame(_) => "editor://prefabs/window-frame.prefab.ron",
            Kind::Caption(
                kedra_desktop::SurfaceKind::Boot | kedra_desktop::SurfaceKind::Loading,
            ) => "editor://prefabs/loading-titlebar.prefab.ron",
            Kind::Caption(kedra_desktop::SurfaceKind::Editor) => {
                "editor://prefabs/editor-titlebar.prefab.ron"
            }
            Kind::Caption(_) => "editor://prefabs/titlebar.prefab.ron",
            Kind::Projects => "editor://scenes/projects.scene.ron",
            Kind::Loading => "editor://scenes/loading.scene.ron",
            Kind::Editor => "editor://scenes/editor.scene.ron",
            Kind::Entry(EntryMode::Create) => "editor://scenes/create-project.scene.ron",
            Kind::Entry(EntryMode::Open) => "editor://scenes/open-project.scene.ron",
        };
        let result = mount_asset(world, parent, path, "").and_then(|view| {
            let result = match kind {
                Kind::Frame(surface) => {
                    world.resource_mut::<kedra_desktop::DesktopSurface>().frame = Some(view.root);
                    let body = match surface {
                        kedra_desktop::SurfaceKind::Boot => {
                            return Err("Editor startup has no splash surface".into());
                        }
                        kedra_desktop::SurfaceKind::Projects => Kind::Projects,
                        kedra_desktop::SurfaceKind::Loading => Kind::Loading,
                        kedra_desktop::SurfaceKind::Editor => Kind::Editor,
                    };
                    request(world, view.root, Kind::Caption(surface))
                        .and_then(|_| request(world, view.root, body))
                        .map(|_| ())
                }
                Kind::Caption(_) => crate::chrome::bind(world, &view),
                Kind::Projects => crate::screens::bind_projects(world, &view),
                Kind::Loading => crate::loading::bind(world, &view),
                Kind::Editor => {
                    world
                        .entity_mut(view.root)
                        .insert(kedra_ui::ScreenId("editor".into()));
                    crate::workspace::bind(world, &view)
                }
                Kind::Entry(mode) => crate::entry::bind(world, &view, mode, entity),
            };
            if result.is_err() {
                world.despawn(view.root);
            }
            result
        });
        world.despawn(entity);
        if let Err(error) = result {
            fail(world, error);
            return;
        }
    }
}

pub(crate) fn fail(world: &mut World, error: String) {
    error!("Authored startup view failed: {error}");
    world.resource_mut::<UiFailure>().0 = Some(error);
    world.write_message(AppExit::error());
}

#[cfg(test)]
#[path = "views/path_tests.rs"]
mod path_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_ui_resource_failures_exit_even_when_no_view_request_is_pending() {
        let mut world = World::new();
        world.insert_resource(UiFailure(Some("unsupported image density".into())));
        world.init_resource::<Messages<AppExit>>();
        hydrate(&mut world);
        assert_eq!(world.resource::<Messages<AppExit>>().len(), 1);
    }
    #[test]
    fn pending_views_retire_with_parent_and_reject_stale_parents_and_counter_exhaustion() {
        let mut world = World::new();
        world.init_resource::<ViewClock>();
        world.init_resource::<Time<Real>>();
        let parent = world.spawn_empty().id();
        let pending = request(&mut world, parent, Kind::Projects).unwrap_or_else(|e| panic!("{e}"));
        world.despawn(parent);
        assert!(!world.entities().contains(pending));
        assert!(request(&mut world, parent, Kind::Projects).is_err());
        let parent = world.spawn_empty().id();
        world.resource_mut::<ViewClock>().0 = u64::MAX;
        let before = world.entities().len();
        assert!(request(&mut world, parent, Kind::Projects).is_err());
        assert_eq!(before, world.entities().len());
    }
}
