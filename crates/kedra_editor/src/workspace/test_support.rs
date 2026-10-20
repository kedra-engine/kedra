//! Small authored catalog for headless production-path tests, without native windows.
use super::*;
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
    let documents: &[(u128, &[&str])] = &[
        (
            0x19a00000000040008000000000000001,
            &["label", "editor", "diagnostic"],
        ),
        (0x19a10000000040008000000000000001, &["value"]),
        (
            0x19a20000000040008000000000000001,
            &["title", "toggle", "body"],
        ),
        (
            0x19a30000000040008000000000000001,
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
            0x11e00000000040008000000000000001,
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
            0x11e10000000040008000000000000001,
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
        (0xc0100000000040008000000000000001, &["caption"]),
        (0xafbe6799efde42319e46e7d3ecfe626b, &["name"]),
        (
            0xe5f0af0978be48c88cc1ddd65951350c,
            &[
                "heading",
                "information",
                "viewport",
                "render",
                "projection",
                "select-menu",
                "lighting",
                "overlays",
                "grid",
                "views",
                "select-tool",
                "orbit",
                "zoom",
                "pan",
                "frame",
                "navigation",
                "tool-rail",
                "axis-x",
                "axis-y",
                "axis-z",
                "axis-neg-x",
                "axis-neg-y",
                "axis-neg-z",
            ],
        ),
        (0xc0600000000040008000000000000001, &[]),
        (
            0xc0900000000040008000000000000001,
            &[
                "window.title",
                "window.minimize",
                "window.maximize",
                "window.close",
                "window.maximize_glyph",
                "window.restore_glyph",
            ],
        ),
        (0xd0700000000040008000000000000001, &["glyph"]),
        (0x1a95bec32a484041a1ee60754f991938, &["empty"]),
        (
            0xe7b184e5b0914db1ae27374ff6cec785,
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
            0xd0a00000000040008000000000000001,
            &["entry", "name", "kind", "folder", "badge"],
        ),
        (
            0x36cb6fa882bd4e418ad446972848b7ee,
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
        (0xd0800000000040008000000000000001, &["title", "glyph"]),
        (
            0xd0400000000040008000000000000001,
            &["heading", "information", "draft"],
        ),
        (
            0xd0100000000040008000000000000001,
            &["tabs", "body", "menu"],
        ),
        (
            0xd0200000000040008000000000000001,
            &["label", "title", "close"],
        ),
        (0xd0300000000040008000000000000001, &["divider"]),
        (
            0xd0500000000040008000000000000001,
            &["stash", "host", "status", "panels", "reset"],
        ),
    ];
    let mut catalog = Catalog::default();
    for (id, keys) in documents {
        let mut doc = PrefabDocument::new("Headless workspace test");
        doc.id = crate::views::document(*id);
        let root = doc.root;
        doc.nodes[0]
            .components
            .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
        doc.nodes[0]
            .components
            .insert("ui.classes".into(), ComponentData::ron(1, "(names:[])")?);
        if *id == 0xc0600000000040008000000000000001 {
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
            if *id == 0x11e00000000040008000000000000001 {
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
            if *key == "value" {
                components.insert(
                    "ui.editable".into(),
                    ComponentData::ron(1, "(autofocus:false,select_all:false)")?,
                );
            }
            doc.nodes.push(SourceNode {
                id: kedra_scene::document::NodeId::new(),
                name: (*key).into(),
                parent: Some(root.into()),
                components,
            });
        }
        catalog.insert(doc)?;
    }
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

pub(super) fn wait<T: Send + 'static>(task: &bevy::tasks::Task<T>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !task.is_finished() {
        assert!(
            std::time::Instant::now() < deadline,
            "Isolated I/O task did not finish"
        );
        std::thread::yield_now();
    }
}
