//! Editor-owned product source contracts, not native interaction or delivery readiness.
use bevy::{
    asset::{AssetApp, AssetMetaCheck, AssetPlugin},
    prelude::*,
};
use kedra_scene::{
    assets::{CatalogLoader, compile_catalog},
    composition::Catalog,
    document::{DocumentKind, PrefabDocument},
    reflection::ComponentPolicies,
};
use kedra_ui::{ElementId, authored::AuthoredUiPlugin};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor")
}

fn read_source(path: &str) -> Result<String> {
    let relative = path
        .strip_prefix("editor://")
        .ok_or("Product source requires an explicit editor path")?;
    Ok(std::fs::read_to_string(source_root().join(relative))?)
}

// This integration target cannot use the editor's private view helpers. Keep only
// source admission here: the native bounded catalog parser and document composer,
// with path membership checked before deriving each current document identity.
struct Sources {
    paths: Vec<String>,
    catalog: Catalog,
}

impl Sources {
    fn load(path: &str) -> Result<Self> {
        let paths = CatalogLoader::parse(&read_source(path)?)?;
        let mut catalog = Catalog::default();
        for path in &paths {
            catalog.insert(PrefabDocument::parse(&read_source(path)?)?)?;
        }
        Ok(Self { paths, catalog })
    }

    fn document(&self, path: &str) -> Result<&PrefabDocument> {
        if !self.paths.iter().any(|member| member == path) {
            return Err(format!("Product source is not a catalog member: {path}").into());
        }
        let current = PrefabDocument::parse(&read_source(path)?)?;
        let document = self
            .catalog
            .documents
            .get(&current.id)
            .ok_or("Catalog document identity missing")?;
        if document != &current {
            return Err(format!("Product source differs from catalog snapshot: {path}").into());
        }
        Ok(document)
    }
}

#[test]
fn startup_sources_compile_with_loading_keys_and_real_editor_surface_without_splash() -> Result<()>
{
    let sources = Sources::load("editor://scenes/startup.prefabs.ron")?;
    let mut app = App::new();
    app.register_asset_source(
        "editor",
        bevy::asset::io::AssetSourceBuilder::new(|| {
            Box::new(bevy::asset::io::file::FileAssetReader::new(source_root()))
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
    ))
    .init_asset::<Font>()
    .init_asset::<Image>()
    .init_resource::<AppTypeRegistry>()
    .add_plugins(AuthoredUiPlugin);
    kedra_ui::docking::scene::register(&mut app)?;
    // Complete Flair registration before compilation starts asynchronous CSS reads.
    app.finish();
    app.cleanup();
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_none()
    );
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = compile_catalog(
        &sources.catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
    )?;
    assert_eq!(compiled.len(), sources.catalog.documents.len());

    let section = sources.document("editor://prefabs/property-section.prefab.ron")?;
    let section = compiled
        .get(&section.id)
        .ok_or("Property section compilation missing")?;
    let section_world = section.authoring_world();
    let toggle = section
        .source_entities()
        .values()
        .copied()
        .find(|entity| {
            section_world
                .get::<ElementId>(*entity)
                .is_some_and(|id| id.0 == "toggle")
        })
        .ok_or("Property disclosure missing")?;
    let order = section_world
        .get::<Children>(toggle)
        .ok_or("Property header children missing")?
        .iter()
        .filter_map(|entity| {
            section_world
                .get::<ElementId>(entity)
                .map(|id| id.0.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        ["down", "right", "title"],
        "Canonical section disclosure precedes its title; source UUID sorting must not move it to the trailing edge"
    );

    let loading = sources.document("editor://scenes/loading.scene.ron")?;
    let snapshot = compiled
        .get(&loading.id)
        .ok_or("Loading compilation missing")?;
    assert!(snapshot.has_dependencies());
    let keys = snapshot
        .source_entities()
        .values()
        .filter_map(|entity| snapshot.authoring_world().get::<ElementId>(*entity))
        .map(|id| id.0.as_str())
        .collect::<Vec<_>>();
    for key in [
        "loading.name",
        "loading.path",
        "loading.phase",
        "loading.log_toggle",
        "loading.failure_log_toggle",
        "loading.retry",
    ] {
        assert_eq!(
            keys.iter().filter(|candidate| **candidate == key).count(),
            1,
            "{key}"
        );
    }

    let editor = sources.document("editor://scenes/editor.scene.ron")?;
    assert_eq!(editor.kind, DocumentKind::Scene);
    let snapshot = compiled
        .get(&editor.id)
        .ok_or("Editor compilation missing")?;
    let root = snapshot.source_entities()[&snapshot.graph().root];
    assert_eq!(
        snapshot
            .authoring_world()
            .get::<ElementId>(root)
            .map(|id| id.0.as_str()),
        Some("editor.canvas")
    );
    let initial = snapshot
        .authoring_world()
        .get::<kedra_ui::docking::scene::InitialLayout>(root)
        .ok_or("Editor initial docking layout missing")?
        .workspace()?;
    assert_eq!(initial.snapshot().hosts.len(), 1);
    assert_eq!(initial.snapshot().views.len(), 4);
    assert!(initial.snapshot().hidden.is_empty());
    // Reject splash sources by their current semantic contract, not a retired UUID.
    for path in &sources.paths {
        assert!(!path.to_ascii_lowercase().contains("splash"), "{path}");
    }
    for snapshot in compiled.values() {
        let document = &sources.catalog.documents[&snapshot.graph().document];
        assert!(!document.title.to_ascii_lowercase().contains("splash"));
        for entity in snapshot.source_entities().values() {
            if let Some(id) = snapshot.authoring_world().get::<ElementId>(*entity) {
                assert!(!id.0.to_ascii_lowercase().contains("splash"), "{}", id.0);
            }
        }
    }
    Ok(())
}

#[test]
fn debug_admission_preserves_production_docking_chrome_and_rail_order() -> Result<()> {
    let sources = Sources::load("editor://scenes/debug.prefabs.ron")?;
    // Resolve the actual debug admission set without pretending that shared UI
    // policies alone can compile editor-owned diagnostic component schemas.
    // Admission includes every diagnostic source, not just docking. Resolve all
    // documents through the production composer to enforce kind/base/nesting contracts.
    for path in &sources.paths {
        sources
            .catalog
            .resolve(sources.document(path)?.id)
            .map_err(|error| format!("{path}: {error}"))?;
    }
    let group = sources.catalog.resolve(
        sources
            .document("editor://prefabs/dock-group.prefab.ron")?
            .id,
    )?;
    let rail = group
        .nodes
        .values()
        .find(|node| node.name == "Rail")
        .ok_or("Rail missing")?;
    let names = rail
        .children
        .iter()
        .map(|address| group.nodes[address].name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["Tabs", "Menu"],
        "Authored order must survive identity-sorted composition"
    );

    let tab = sources
        .catalog
        .resolve(sources.document("editor://prefabs/dock-tab.prefab.ron")?.id)?;
    let root = tab.nodes.get(&tab.root).ok_or("Tab root missing")?;
    assert_eq!(
        root.children.len(),
        2,
        "Only label and close slot belong over the native rounded tab; no rectangular top-light child"
    );
    assert_eq!(
        root.children
            .iter()
            .map(|address| tab.nodes[address].name.as_str())
            .collect::<Vec<_>>(),
        ["Draggable label", "Close"]
    );
    Ok(())
}
