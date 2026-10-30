//! Small authored catalog for headless production-path tests, without native windows.
use super::*;
mod host;
pub(super) use host::{box_at, setup};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, PrefabDocument},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn project_manager(
    project: kedra_project::Project,
    history: Option<std::path::PathBuf>,
) -> Result<crate::model::ProjectManager, String> {
    let mut manager = crate::model::ProjectManager::default();
    manager.model.history_writable = history.is_some();
    manager.model.history_path = history;
    let generation = manager
        .begin(crate::domain::Intent::Open(project.root.clone()))
        .ok_or("Project request was rejected")?;
    if !manager.complete(generation, Ok(project)) || !manager.enter_editor() {
        return Err("Validated test project was not admitted".into());
    }
    Ok(manager)
}

pub(super) fn sources(world: &mut World) -> TestResult {
    let icon = Handle::<Image>::Uuid(uuid::Uuid::new_v4(), std::marker::PhantomData);
    world
        .resource_mut::<Assets<Image>>()
        .insert(icon.id(), Image::default())?;
    let documents: &[(&str, &[&str])] = &[
        ("editor://prefabs/browser-icon.prefab.ron", &["icon"]),
        (
            "editor://prefabs/property-row.prefab.ron",
            &["label", "editor", "diagnostic"],
        ),
        ("editor://prefabs/property-value.prefab.ron", &["value"]),
        (
            "editor://prefabs/property-vector.prefab.ron",
            &[
                "axis-x", "value-x", "axis-y", "value-y", "axis-z", "value-z", "axis-w", "value-w",
            ],
        ),
        (
            "editor://prefabs/property-section.prefab.ron",
            &["title", "toggle", "body"],
        ),
        (
            "editor://prefabs/inspector.prefab.ron",
            &[
                "name",
                "identity",
                "source",
                "message",
                "toolbar",
                "search",
                "close-search",
                "entries",
                "track",
                "thumb",
                "locked",
            ],
        ),
        (
            "editor://prefabs/hierarchy.prefab.ron",
            &[
                "search",
                "toolbar",
                "close-search",
                "filter",
                "search-icon",
                "hint",
                "placeholder",
                "entries",
                "canvas",
                "track",
                "thumb",
                "message",
                "empty",
                "status",
            ],
        ),
        (
            "editor://prefabs/hierarchy-row.prefab.ron",
            &[
                "select",
                "toggle",
                "name",
                "down",
                "right",
                "scene",
                "object",
                "object-selected",
                "folder",
                "prefab",
            ],
        ),
        ("editor://prefabs/button.prefab.ron", &["caption"]),
        ("editor://prefabs/browser-crumb.prefab.ron", &["name"]),
        (
            "editor://prefabs/source-document.prefab.ron",
            &[
                "heading",
                "information",
                "viewport",
                "render",
                "projection",
                "move",
                "hand",
                "rotate",
                "scale",
                "snap",
                "settings",
                "lighting",
                "views",
                "orbit",
                "frame",
                "navigation",
                "transform-tools",
                "tool-rail",
                "axis-x",
                "axis-y",
                "axis-z",
                "axis-neg-x",
                "axis-neg-y",
                "axis-neg-z",
            ],
        ),
        ("editor://prefabs/window-frame.prefab.ron", &[]),
        (
            "editor://prefabs/titlebar.prefab.ron",
            &[
                "window.title",
                "window.minimize",
                "window.maximize",
                "window.close",
                "window.maximize_glyph",
                "window.restore_glyph",
                "window.tabs",
                "window.menu",
            ],
        ),
        ("editor://prefabs/dock-guide.prefab.ron", &["glyph"]),
        ("editor://prefabs/empty-panel.prefab.ron", &["empty"]),
        (
            "editor://prefabs/asset-browser.prefab.ron",
            &[
                "toolbar",
                "back",
                "forward",
                "up",
                "refresh",
                "open",
                "root",
                "folders",
                "entries",
                "location",
                "message",
                "status",
                "grid",
                "list",
                "filter",
                "clear",
                "search",
                "search-placeholder",
                "search-icon",
                "new",
                "import",
                "breadcrumbs",
                "tile-canvas",
                "tree-status",
                "tooltip",
                "tooltip-host",
                "count",
                "status-clip",
                "content-scrollbar",
                "content-thumb",
                "tree-scrollbar",
                "tree-thumb",
            ],
        ),
        (
            "editor://prefabs/asset-source.prefab.ron",
            &["entry", "name", "kind", "folder", "badge"],
        ),
        (
            "editor://prefabs/browser-folder.prefab.ron",
            &[
                "toggle",
                "navigate",
                "name",
                "right",
                "down",
                "folder",
                "selected-folder",
            ],
        ),
        (
            "editor://prefabs/dock-ghost.prefab.ron",
            &["title", "glyph"],
        ),
        (
            "editor://prefabs/dock-content.prefab.ron",
            &["heading", "information", "draft"],
        ),
        (
            "editor://prefabs/dock-group.prefab.ron",
            &["tabs", "body", "menu"],
        ),
        (
            "editor://prefabs/dock-tab.prefab.ron",
            &["label", "title", "close"],
        ),
        ("editor://prefabs/dock-split.prefab.ron", &["divider"]),
        (
            "editor://prefabs/dock-workspace.prefab.ron",
            &["stash", "host", "status", "panels", "reset"],
        ),
    ];
    let mut catalog = Catalog::default();
    let mut native_documents = Vec::new();
    for (path, keys) in documents {
        let mut doc = PrefabDocument::new("Headless workspace test");
        let root = doc.root;
        doc.nodes[0]
            .components
            .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
        doc.nodes[0]
            .components
            .insert("ui.classes".into(), ComponentData::ron(1, "(names:[])")?);
        if matches!(
            *path,
            "editor://prefabs/button.prefab.ron" | "editor://prefabs/browser-icon.prefab.ron"
        ) {
            doc.nodes[0]
                .components
                .insert("ui.button".into(), ComponentData::ron(1, "()")?);
        }
        if *path == "editor://prefabs/window-frame.prefab.ron" {
            let sheet = kedra_ui::authored::components::UiStylesheet {
                asset: Handle::default(),
            };
            let data = world.resource::<ComponentPolicies>().encode(
                "ui.stylesheet",
                &sheet,
                &world.resource::<AppTypeRegistry>().read(),
                &BTreeMap::new(),
            )?;
            doc.nodes[0].components.insert("ui.stylesheet".into(), data);
        }
        for key in *keys {
            let mut components = BTreeMap::from([
                ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
                (
                    "ui.id".into(),
                    ComponentData::ron(1, &format!("(\"{key}\")"))?,
                ),
            ]);
            if *key == "icon" && *path == "editor://prefabs/browser-icon.prefab.ron" {
                components.insert(
                    "ui.image".into(),
                    world.resource::<ComponentPolicies>().encode(
                        "ui.image",
                        &kedra_ui::authored::components::UiImage {
                            variants: vec![kedra_ui::authored::components::UiImageDensity {
                                density: 1.,
                                image: icon.clone(),
                            }],
                            color: Color::WHITE,
                        },
                        &world.resource::<AppTypeRegistry>().read(),
                        &BTreeMap::new(),
                    )?,
                );
            }
            if *path == "editor://prefabs/hierarchy.prefab.ron" {
                components.insert("ui.classes".into(), ComponentData::ron(1, "(names:[])")?);
            }
            if [
                "caption",
                "window.title",
                "title",
                "heading",
                "information",
                "status",
                "name",
                "kind",
                "location",
                "message",
                "badge",
                "tree-status",
                "tooltip",
                "count",
                "empty",
                "source",
                "label",
                "diagnostic",
                "value",
            ]
            .contains(key)
            {
                components.insert(
                    "ui.caption".into(),
                    ComponentData::ron(1, "(text:\"Test\",justify:Left,wrap:false)")?,
                );
            }
            if *key == "value"
                || (*key == "search" && *path == "editor://prefabs/inspector.prefab.ron")
            {
                components.insert(
                    "ui.editable".into(),
                    ComponentData::ron(1, "(autofocus:false,select_all:false)")?,
                );
            }
            if *key == "search" && *path == "editor://prefabs/inspector.prefab.ron" {
                components.insert(
                    "ui.caption".into(),
                    ComponentData::ron(1, "(text:\"\",justify:Left,wrap:false)")?,
                );
            }
            doc.nodes.push(SourceNode {
                id: kedra_scene::document::NodeId::new(),
                name: (*key).into(),
                parent: Some(root.into()),
                components,
            });
        }
        native_documents.push(((*path).to_string(), doc.clone()));
        catalog.insert(doc)?;
    }
    let editor = PrefabDocument::parse(include_str!(
        "../../../../assets/editor/scenes/editor.scene.ron"
    ))?;
    native_documents.push(("editor://scenes/editor.scene.ron".into(), editor.clone()));
    catalog.insert(editor)?;
    let registry = world.resource::<AppTypeRegistry>().clone();
    let compiled = catalog
        .documents
        .keys()
        .map(|id| {
            CompiledPrefab::compile(
                catalog.resolve(*id)?,
                &registry.read(),
                world.resource::<ComponentPolicies>(),
                &mut world.resource::<AssetServer>(),
                &BTreeMap::new(),
            )
            .map(|p| (*id, Arc::new(p)))
        })
        .collect::<kedra_scene::Result<BTreeMap<_, _>>>()?;
    for prefab in compiled.values() {
        let source = prefab.authoring_world();
        for entity in prefab.source_entities().values() {
            if let Some(sheet) = source.get::<kedra_ui::authored::components::UiStylesheet>(*entity)
            {
                world
                    .resource_mut::<Assets<bevy_flair::style::StyleSheet>>()
                    .insert(sheet.asset.id(), bevy_flair::style::StyleSheet::empty())?;
            }
        }
    }
    let mut sources = kedra_scene::assets::PrefabSources::default();
    sources.handle = crate::views::test_support::materialize(world, native_documents)?;
    sources.publish(catalog, compiled)?;
    sources.dependencies_ready = true;
    world.insert_resource(sources);
    world.init_resource::<UiScale>();
    let state = world.resource::<EditorWorkspace>();
    let (window, root) = (state.window, state.root);
    world.insert_resource(kedra_desktop::DesktopSurface {
        window: Some(window),
        root: Some(root),
        ..default()
    });
    world
        .entity_mut(root)
        .insert(bevy_flair::style::components::Styled::new(Handle::default()));
    Ok(())
}

pub(super) fn visible(world: &mut World) {
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<Node>>()
        .iter(world)
        .collect();
    for entity in entities {
        world
            .entity_mut(entity)
            .insert(InheritedVisibility::VISIBLE);
    }
}

pub(super) fn step(world: &mut World, position: Vec2, button: Option<bool>) {
    world.resource_mut::<ButtonInput<MouseButton>>().clear();
    if let Some(pressed) = button {
        if pressed {
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
        } else {
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
        }
    }
    let window = world.resource::<EditorWorkspace>().window;
    world.write_message(bevy::window::CursorMoved {
        window,
        position,
        delta: None,
    });
    // Run the real cursor message collector, then the exclusive production tick.
    let mut schedule = Schedule::default();
    schedule.add_systems(input::cursor);
    schedule.run(world);
    tick(world);
}

pub(super) fn wait<T: Send + 'static>(task: &kedra_wakeup::Task<T>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !task.is_finished() {
        assert!(
            std::time::Instant::now() < deadline,
            "Isolated I/O task did not finish"
        );
        std::thread::yield_now();
    }
}
