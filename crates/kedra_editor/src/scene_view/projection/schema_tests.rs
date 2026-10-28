//! Admission uses real production sources; hydration assertions are CPU-only.
use super::*;
use bevy::asset::{
    AssetApp, AssetLoader, AssetMetaCheck, AssetPlugin, LoadContext,
    io::{
        Reader,
        memory::{Dir, MemoryAssetReader},
    },
};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, PrefabDocument},
    reflection::ComponentPolicies,
};
use kedra_ui::authored::{AuthoredUiPlugin, components::*, view::UiView};
type TestResult = Result<(), Box<dyn std::error::Error>>;

// Match the existing startup CPU fixture: real asset loading and labeled density
// subassets, without claiming SVG rasterization or native rendering acceptance.
#[derive(Default, TypePath)]
struct CpuIcons;
impl AssetLoader for CpuIcons {
    type Asset = kedra_assets::images::ImageVariants;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        _: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        Ok(kedra_assets::images::ImageVariants {
            images: (1..=4)
                .map(|density| context.add_labeled_asset(format!("{density}x"), Image::default()))
                .collect(),
        })
    }
    fn extensions(&self) -> &[&str] {
        &["svg"]
    }
}

fn app() -> App {
    let memory = Dir::default();
    memory.insert_asset_text(
        std::path::Path::new("preview.css"),
        ".text { color:white; }",
    );
    memory.insert_asset_text(
        std::path::Path::new("icons/browser-right.svg"),
        include_str!("../../../../../assets/editor/icons/browser-right.svg"),
    );
    for (path, source) in [
        (
            "icons/plus.svg",
            include_str!("../../../../../assets/editor/icons/plus.svg"),
        ),
        (
            "icons/minus.svg",
            include_str!("../../../../../assets/editor/icons/minus.svg"),
        ),
        (
            "icons/dock-grip.svg",
            include_str!("../../../../../assets/editor/icons/dock-grip.svg"),
        ),
    ] {
        memory.insert_asset_text(std::path::Path::new(path), source);
    }
    let mut app = App::new();
    app.register_asset_source(
        "editor",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(MemoryAssetReader {
                root: memory.clone(),
            })
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
    .init_asset::<kedra_assets::images::ImageVariants>()
    .init_asset_loader::<CpuIcons>()
    .init_resource::<AppTypeRegistry>()
    .add_plugins(AuthoredUiPlugin);
    app.finish();
    app.cleanup();
    app
}

fn compile(app: &App, document: PrefabDocument) -> Result<Arc<CompiledPrefab>, kedra_scene::Error> {
    let mut catalog = Catalog::default();
    let id = document.id;
    catalog.insert(document)?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    Ok(Arc::new(CompiledPrefab::compile(
        catalog.resolve(id)?,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
        &BTreeMap::new(),
    )?))
}

#[test]
fn production_controls_admit_decorative_picking_but_mount_only_passive_ui() -> TestResult {
    for source in [
        include_str!("../../../../../assets/editor/prefabs/button.prefab.ron"),
        include_str!("../../../../../assets/editor/prefabs/field.prefab.ron"),
        include_str!("../../../../../assets/editor/prefabs/browser-icon.prefab.ron"),
    ] {
        let mut app = app();
        let mut document = PrefabDocument::parse(source)?;
        assert!(
            document
                .nodes
                .iter()
                .any(|n| n.components.contains_key("ui.pickable"))
        );
        // Give standalone controls their required explicit stylesheet. CPU assets
        // verify hydration, not delivery loading, rasterization or native input.
        let sheet = app
            .world()
            .resource::<AssetServer>()
            .load("editor://preview.css");
        let policies = app.world().resource::<ComponentPolicies>();
        document.nodes[0].components.insert(
            "ui.stylesheet".into(),
            policies.encode(
                "ui.stylesheet",
                &UiStylesheet { asset: sheet },
                &app.world().resource::<AppTypeRegistry>().read(),
                &BTreeMap::new(),
            )?,
        );
        for (key, value) in [
            ("ui.id", "(\"preview-element\")"),
            ("ui.screen", "(\"preview-screen\")"),
            ("ui.pickable", "(should_block_lower:true,is_hoverable:true)"),
        ] {
            document.nodes[0]
                .components
                .insert(key.into(), ComponentData::ron(1, value)?);
        }
        let compiled = compile(&app, document)?;
        supported(compiled.graph())?;
        let graph = compiled.graph().clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !compiled.asset_status(app.world().resource::<AssetServer>())? {
            assert!(
                std::time::Instant::now() < deadline,
                "CPU preview assets did not load"
            );
            app.update();
            std::thread::yield_now();
        }
        let passive = UiView::mount_passive(app.world_mut(), &compiled, None, 1.)?;
        let nodes = &app
            .world()
            .get::<PrefabInstance>(passive.root)
            .ok_or("instance missing")?
            .nodes;
        for entity in nodes.values().copied() {
            let world = app.world();
            assert!(world.get::<kedra_ui::ElementId>(entity).is_none());
            assert!(world.get::<kedra_ui::ScreenId>(entity).is_none());
            assert!(world.get::<bevy::ui_widgets::Button>(entity).is_none());
            assert!(world.get::<bevy::ui_widgets::TextInput>(entity).is_none());
            assert!(world.get::<bevy::input_focus::AutoFocus>(entity).is_none());
            assert!(
                world
                    .get::<bevy::input_focus::tab_navigation::TabIndex>(entity)
                    .is_none()
            );
            assert!(
                world
                    .get::<bevy::ui_widgets::SelectAllOnFocus>(entity)
                    .is_none()
            );
            assert!(world.get::<PassiveUi>(entity).is_some());
            let picking = world
                .get::<bevy::picking::Pickable>(entity)
                .ok_or("picking missing")?;
            assert!(!picking.is_hoverable && !picking.should_block_lower);
        }
        assert!(app.world().resource::<UiFailure>().0.is_none());
        assert_eq!(compiled.graph(), &graph);
        let source_root = compiled.source_entities()[&graph.root];
        assert!(
            compiled
                .authoring_world()
                .get::<kedra_ui::ScreenId>(source_root)
                .is_some()
        );
        assert!(
            compiled
                .authoring_world()
                .get::<bevy::picking::Pickable>(source_root)
                .is_some_and(|picking| picking.is_hoverable && picking.should_block_lower)
        );
        passive.retire(app.world_mut());
    }
    Ok(())
}

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct HostBehavior;

#[test]
fn registered_host_behavior_and_unknown_ui_keys_never_gain_preview_authority() -> TestResult {
    let mut app = app();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    app.world_mut()
        .resource_mut::<ComponentPolicies>()
        .register::<HostBehavior>(&mut registry.write(), "ui.host_behavior", 1, |_| Ok(()))?;
    let mut source = PrefabDocument::new("Not passive behavior");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    source.nodes[0]
        .components
        .insert("ui.host_behavior".into(), ComponentData::ron(1, "()")?);
    let compiled = compile(&app, source.clone())?;
    assert!(supported(compiled.graph()).is_err_and(|error| error.contains("ui.host_behavior")));
    assert!(app.world().resource::<UiFailure>().0.is_none());
    source.nodes[0].components.remove("ui.host_behavior");
    source.nodes[0]
        .components
        .insert("ui.unknown".into(), ComponentData::ron(1, "()")?);
    assert!(compile(&app, source).is_err());
    assert!(!kedra_ui::authored::supports_passive_component(
        "ui.docking"
    ));
    Ok(())
}
