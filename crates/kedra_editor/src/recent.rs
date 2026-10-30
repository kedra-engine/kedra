//! Read-only availability checks and retained selection for the recent-project view.
use crate::model::*;
use bevy::{prelude::*, tasks::IoTaskPool};
use bevy_flair::style::components::ClassList;
use kedra_ui::ElementId;
use kedra_ui::authored::view::UiView;
use kedra_wakeup::Task;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Resource, Default)]
pub(crate) struct RecentView {
    pub selected: Option<PathBuf>,
    roots: Option<Vec<PathBuf>>,
    missing: BTreeSet<PathBuf>,
    task: Option<Task<(Vec<PathBuf>, BTreeSet<PathBuf>)>>,
    host: Option<(Entity, Entity, Entity)>,
    rows: BTreeMap<PathBuf, UiView>,
}

pub(crate) fn invalidate(world: &mut World) {
    world.resource_mut::<RecentView>().roots = None;
    let mut view = world.resource_mut::<RecentView>();
    view.host = None;
    view.rows.clear();
}

pub(crate) fn poll(
    mut view: ResMut<RecentView>,
    mut manager: ResMut<ProjectManager>,
    wakeup: Option<Res<kedra_wakeup::Wakeup>>,
) {
    if manager.phase() != Phase::Projects {
        return;
    }
    let wakeup = wakeup.as_deref().cloned().unwrap_or_default();
    let roots: Vec<_> = manager
        .history
        .entries()
        .iter()
        .map(|e| e.root.clone())
        .collect();
    if view.roots.as_ref() != Some(&roots) {
        view.roots = Some(roots.clone());
        view.task = Some(wakeup.spawn(IoTaskPool::get(), async move {
            let missing = roots
                .iter()
                .filter(|root| !root.join(kedra_project::MANIFEST_FILE).is_file())
                .cloned()
                .collect();
            (roots, missing)
        }));
    }
    let result = view.task.as_mut().and_then(Task::poll);
    if let Some((checked, missing)) = result {
        view.task = None;
        if view.roots.as_ref() == Some(&checked) {
            view.missing = missing;
            if !view
                .selected
                .as_ref()
                .is_some_and(|p| checked.contains(p) && !view.missing.contains(p))
            {
                view.selected = checked.iter().find(|p| !view.missing.contains(*p)).cloned();
            }
            manager.list_dirty = true;
        }
    }
}

pub(crate) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let host = view.element("projects.rows").map_err(|e| e.to_string())?;
    let empty = view.element("projects.empty").map_err(|e| e.to_string())?;
    let message = view
        .element("projects.empty_message")
        .map_err(|e| e.to_string())?;
    world.resource_mut::<RecentView>().host = Some((host, empty, message));
    world.resource_mut::<ProjectManager>().list_dirty = true;
    Ok(())
}

pub(crate) fn sync(world: &mut World) {
    if world.resource::<State<AppPhase>>().get() != &AppPhase::Projects
        || !world.resource::<ProjectManager>().list_dirty
    {
        return;
    }
    let Some((host, empty, message)) = world.resource::<RecentView>().host else {
        return;
    };
    if !world.entities().contains(host) {
        return;
    }
    let manager = world.resource::<ProjectManager>();
    let filter = manager.filter.to_lowercase();
    let entries = manager
        .history
        .entries()
        .iter()
        .filter(|entry| {
            entry.name.to_lowercase().contains(&filter)
                || entry
                    .root
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&filter)
        })
        .cloned()
        .collect::<Vec<_>>();
    let history_empty = manager.history.entries().is_empty();
    let selected = world.resource::<RecentView>().selected.clone();
    let missing = world.resource::<RecentView>().missing.clone();
    let mut rows = std::mem::take(&mut world.resource_mut::<RecentView>().rows);
    let desired = entries
        .iter()
        .map(|entry| entry.root.clone())
        .collect::<BTreeSet<_>>();
    rows.retain(|path, view| {
        if desired.contains(path) {
            true
        } else {
            world.despawn(view.root);
            false
        }
    });
    let mut order = Vec::new();
    let result = (|| -> Result<(), String> {
        for (index, entry) in entries.iter().enumerate() {
            if !rows.contains_key(&entry.root) {
                let view = crate::views::mount_asset(
                    world,
                    host,
                    "editor://prefabs/project-row.prefab.ron",
                    &format!("projects.item.{}", uuid::Uuid::new_v4()),
                )?;
                let contract = ["name", "path", "missing", "open", "locate"]
                    .into_iter()
                    .try_for_each(|key| view.element(key).map(|_| ()));
                if let Err(error) = contract {
                    world.despawn(view.root);
                    return Err(error.to_string());
                }
                rows.insert(entry.root.clone(), view);
            }
            let view = &rows[&entry.root];
            let name = view.element("name").map_err(|e| e.to_string())?;
            let path = view.element("path").map_err(|e| e.to_string())?;
            let recovery = view.element("missing").map_err(|e| e.to_string())?;
            let open = view.element("open").map_err(|e| e.to_string())?;
            let locate = view.element("locate").map_err(|e| e.to_string())?;
            world
                .get_mut::<Text>(name)
                .ok_or("project name binding is not native Text")?
                .0 = entry.name.clone();
            world
                .get_mut::<Text>(path)
                .ok_or("project path binding is not native Text")?
                .0 = display_path(&entry.root);
            let is_selected = selected.as_ref() == Some(&entry.root);
            let is_missing = missing.contains(&entry.root);
            world.entity_mut(view.root).insert((
                Action::Select(entry.root.clone()),
                ElementId::new(format!("projects.row.{index}")),
                crate::screens::ManagedControl,
            ));
            if is_selected {
                world.entity_mut(view.root).insert(bevy::ui::Selected);
            } else {
                world.entity_mut(view.root).remove::<bevy::ui::Selected>();
            }
            world.entity_mut(open).insert((
                Action::OpenRecent(entry.root.clone()),
                ElementId::new(format!("projects.open.{index}")),
                crate::screens::ManagedControl,
            ));
            world.entity_mut(locate).insert((
                Action::Locate(entry.root.clone()),
                ElementId::new(format!("projects.locate.{index}")),
                crate::screens::ManagedControl,
            ));
            for (entity, visible) in [(recovery, is_missing), (open, is_selected && !is_missing)] {
                let mut classes = world
                    .get_mut::<ClassList>(entity)
                    .ok_or("project row has no authored style classes")?;
                if classes.contains("hidden") == visible {
                    classes.toggle("hidden");
                }
            }
            order.push(view.root);
        }
        world.entity_mut(host).replace_children(&order);
        let mut classes = world
            .get_mut::<ClassList>(empty)
            .ok_or("empty view classes missing")?;
        if classes.contains("hidden") == entries.is_empty() {
            classes.toggle("hidden");
        }
        world
            .get_mut::<Text>(message)
            .ok_or("empty message is not native Text")?
            .0 = if history_empty {
            "No recent projects"
        } else {
            "No matching projects"
        }
        .into();
        Ok(())
    })();
    world.resource_mut::<RecentView>().rows = rows;
    if let Err(error) = result {
        crate::views::fail(world, error);
    } else {
        world.resource_mut::<ProjectManager>().list_dirty = false;
    }
}

pub(crate) fn display_path(path: &std::path::Path) -> String {
    display_path_from(
        path,
        directories::UserDirs::new().as_ref().map(|d| d.home_dir()),
    )
}

fn display_path_from(path: &std::path::Path, home: Option<&std::path::Path>) -> String {
    if let Some(relative) = home.and_then(|home| path.strip_prefix(home).ok()) {
        if relative.as_os_str().is_empty() {
            "~".into()
        } else {
            format!("~/{}", relative.display())
        }
    } else {
        path.display().to_string()
    }
}

#[cfg(test)]
#[path = "recent/lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Retained authored row regression assertions"
    )]
    use super::*;
    use std::path::Path;
    #[test]
    fn home_abbreviation_uses_path_components_not_a_text_prefix() {
        let home = Path::new("/users/test");
        assert_eq!(
            display_path_from(Path::new("/users/test/Проекты 日本語"), Some(home)),
            "~/Проекты 日本語"
        );
        assert_eq!(
            display_path_from(Path::new("/users/test-other/game"), Some(home)),
            "/users/test-other/game"
        );
        assert_eq!(display_path_from(home, Some(home)), "~");
    }

    #[test]
    fn filtering_retains_rows_without_colliding_descendant_ids() {
        use kedra_scene::{
            assets::PrefabSources, composition::Catalog, document::PrefabDocument,
            reflection::ComponentPolicies,
        };
        use kedra_ui::authored::{AuthoredUiPlugin, components::UiStylesheet};
        let mut app = App::new();
        crate::views::test_support::install(&mut app);
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .add_plugins(kedra_scene::assets::SceneAssetPlugin)
            .init_asset::<Font>()
            .init_asset::<Image>()
            .init_resource::<AppTypeRegistry>()
            .add_plugins(AuthoredUiPlugin)
            .init_resource::<ProjectManager>()
            .init_resource::<RecentView>()
            .init_resource::<UiScale>()
            .insert_resource(State::new(AppPhase::Projects));
        let mut row = PrefabDocument::parse(include_str!(
            "../../../assets/editor/prefabs/project-row.prefab.ron"
        ))
        .unwrap();
        // Images are irrelevant to identity; retain the actual authored hierarchy,
        // semantic IDs, nested action instances and text bindings.
        for node in &mut row.nodes {
            node.components.remove("ui.image");
        }
        let mut catalog = Catalog::default();
        let mut button = PrefabDocument::parse(include_str!(
            "../../../assets/editor/prefabs/button.prefab.ron"
        ))
        .unwrap();
        for node in &mut button.nodes {
            node.components.remove("ui.image");
        }
        let native_documents = vec![
            (
                "editor://prefabs/project-row.prefab.ron".to_string(),
                row.clone(),
            ),
            (
                "editor://prefabs/button.prefab.ron".to_string(),
                button.clone(),
            ),
        ];
        catalog.insert(row).unwrap();
        catalog.insert(button).unwrap();
        let compiled = {
            let registry = app.world().resource::<AppTypeRegistry>();
            let mut loader = app.world().resource::<AssetServer>();
            kedra_scene::assets::compile_catalog(
                &catalog,
                &registry.read(),
                app.world().resource::<ComponentPolicies>(),
                &mut loader,
            )
            .unwrap()
        };
        let mut sources = PrefabSources::default();
        sources.handle =
            crate::views::test_support::materialize(app.world_mut(), native_documents).unwrap();
        sources.publish(catalog, compiled).unwrap();
        sources.dependencies_ready = true;
        app.insert_resource(sources);
        let window = app.world_mut().spawn(Window::default()).id();
        app.insert_resource(kedra_desktop::DesktopSurface {
            window: Some(window),
            ..default()
        });
        let host = app
            .world_mut()
            .spawn(UiStylesheet { asset: default() })
            .id();
        let empty = app.world_mut().spawn(ClassList::default()).id();
        let message = app.world_mut().spawn(Text::default()).id();
        app.world_mut().resource_mut::<RecentView>().host = Some((host, empty, message));
        let directory = tempfile::tempdir().unwrap();
        let a = kedra_project::create(directory.path(), "Alpha").unwrap();
        let b = kedra_project::create(directory.path(), "Beta").unwrap();
        app.world_mut()
            .resource_mut::<ProjectManager>()
            .history
            .remember(&b);
        app.world_mut()
            .resource_mut::<ProjectManager>()
            .history
            .remember(&a);
        let mut retained = None;
        for filter in ["no-match", "Beta", "", "Beta", ""] {
            let mut manager = app.world_mut().resource_mut::<ProjectManager>();
            manager.filter = filter.into();
            manager.list_dirty = true;
            sync(app.world_mut());
            let mut ids = BTreeSet::new();
            for id in app.world_mut().query::<&ElementId>().iter(app.world()) {
                assert!(ids.insert(id.0.clone()), "duplicate identity {}", id.0);
            }
            if let Some(row) = app.world().resource::<RecentView>().rows.get(&b.root) {
                if let Some(retained) = retained {
                    assert_eq!(row.root, retained);
                } else {
                    retained = Some(row.root);
                }
            }
            assert!(!app.world().resource::<ProjectManager>().list_dirty);
        }
    }
}
