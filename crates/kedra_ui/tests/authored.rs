//! The shared UI consumer uses actual native Node/widgets and the common scene compiler.
use bevy::{
    asset::{AssetApp, AssetPlugin},
    input_focus::tab_navigation::TabGroup,
    prelude::*,
};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, NodeId, PrefabDocument},
    instance::CompiledPrefab,
    reflection::{ComponentPolicies, ReferenceContext},
};
use kedra_ui::{
    ElementId,
    authored::{AuthoredUiPlugin, components::*, view::UiView},
};
use std::{collections::BTreeMap, sync::Arc};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn setup() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    app
}

fn compile(app: &App, document: PrefabDocument) -> Result<Arc<CompiledPrefab>> {
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
fn native_layout_serde_defaults_are_not_an_alternate_ui_language() -> Result<()> {
    let app = setup();
    assert!(
        app.world().resource::<UiFailure>().0.is_none(),
        "{:?}",
        app.world().resource::<UiFailure>().0
    );
    let mut source = PrefabDocument::new("Native panel");
    source.nodes[0].components.insert(
        "ui.layout".into(),
        ComponentData::ron(
            1,
            "(node:(width:Percent(100.0),height:Px(24.0),min_width:Px(0.0)))",
        )?,
    );
    source.nodes[0]
        .components
        .insert("ui.id".into(), ComponentData::ron(1, "(\"panel\")")?);
    let snapshot = compile(&app, source)?;
    let node = snapshot
        .authoring_world()
        .get::<UiLayout>(snapshot.source_entities()[&snapshot.graph().root])
        .ok_or("layout missing")?;
    assert_eq!(node.node.width, percent(100));
    assert_eq!(node.node.height, px(24));
    Ok(())
}

#[test]
fn nested_ui_instances_do_not_inject_a_parallel_spatial_transform_hierarchy() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Non-spatial UI instance");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    let snapshot = compile(&app, source)?;
    let outer = UiView::mount(
        app.world_mut(),
        &snapshot,
        None,
        &BTreeMap::new(),
        "outer",
        1.,
    )?;
    let inner = UiView::mount(
        app.world_mut(),
        &snapshot,
        Some(outer.root),
        &BTreeMap::new(),
        "inner",
        1.,
    )?;
    for root in [outer.root, inner.root] {
        assert!(app.world().get::<Node>(root).is_some());
        assert!(app.world().get::<Transform>(root).is_none());
        assert!(app.world().get::<GlobalTransform>(root).is_none());
    }
    Ok(())
}

#[test]
fn passive_projection_never_installs_focus_actions_modal_ownership_or_observation_ids() -> Result<()>
{
    let mut app = setup();
    let mut document = PrefabDocument::new("Passive button and modal");
    document.nodes[0].components = BTreeMap::from([
        ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
        ("ui.id".into(), ComponentData::ron(1, "(\"passive\")")?),
        ("ui.button".into(), ComponentData::ron(1, "()")?),
        ("ui.modal".into(), ComponentData::ron(1, "Dialog")?),
    ]);
    let snapshot = compile(&app, document)?;
    let graph = snapshot.graph().clone();
    let view = UiView::mount_passive(app.world_mut(), &snapshot, None, 1.)?;
    let root = app.world().entity(view.root);
    assert!(root.get::<Node>().is_some());
    assert!(root.get::<ElementId>().is_none());
    assert!(root.get::<bevy::ui_widgets::Button>().is_none());
    assert!(root.get::<bevy::input_focus::AutoFocus>().is_none());
    assert!(root.get::<TabGroup>().is_none());
    assert!(
        root.get::<bevy::input_focus::tab_navigation::TabIndex>()
            .is_none()
    );
    assert_eq!(snapshot.graph(), &graph);
    Ok(())
}

#[test]
fn passive_mixed_graph_detaches_ui_roots_and_retires_both_native_forests() -> Result<()> {
    let mut app = setup();
    let mut document = PrefabDocument::new("Spatial root with UI");
    let ui = NodeId::new();
    document.nodes.push(SourceNode {
        id: ui,
        name: "UI subtree".into(),
        parent: Some(document.root.into()),
        components: BTreeMap::from([
            ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
            ("ui.button".into(), ComponentData::ron(1, "()")?),
        ]),
    });
    let snapshot = compile(&app, document)?;
    let owner = app.world_mut().spawn(Transform::IDENTITY).id();
    let passive = UiView::mount_passive(app.world_mut(), &snapshot, Some(owner), 1.)?;
    assert_eq!(passive.ui_roots.len(), 1);
    let ui = passive.ui_roots[0];
    assert!(
        app.world().get::<ChildOf>(ui).is_none(),
        "Native UI must not inherit a spatial/serving parent"
    );
    assert!(app.world().get::<Transform>(ui).is_none());
    assert_eq!(
        app.world()
            .get::<ChildOf>(passive.root)
            .ok_or("Spatial root parent missing")?
            .parent(),
        owner
    );
    passive.retire(app.world_mut());
    passive.retire(app.world_mut());
    assert!(app.world().get_entity(ui).is_err());
    assert!(app.world().get_entity(passive.root).is_err());
    assert!(app.world().get_entity(owner).is_ok());
    Ok(())
}

#[test]
fn misspelled_layout_and_nonfinite_values_are_rejected() -> Result<()> {
    let app = setup();
    for data in ["(node:(widht:Px(24.0)))", "(node:(width:Px(NaN)))"] {
        let mut source = PrefabDocument::new("Invalid panel");
        source.nodes[0]
            .components
            .insert("ui.layout".into(), ComponentData::ron(1, data)?);
        assert!(compile(&app, source).is_err(), "{data}");
    }
    Ok(())
}

#[test]
fn duplicate_semantic_keys_fail_before_live_allocation() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Ambiguous panel");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    source.nodes[0]
        .components
        .insert("ui.id".into(), ComponentData::ron(1, "(\"duplicate\")")?);
    let child = NodeId::new();
    source.nodes.push(SourceNode {
        id: child,
        name: "Child".into(),
        parent: Some(source.root.into()),
        components: source.nodes[0].components.clone(),
    });
    let snapshot = compile(&app, source)?;
    let before = app.world().entities().count_spawned();
    assert!(UiView::mount(app.world_mut(), &snapshot, None, &BTreeMap::new(), "", 1.).is_err());
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}

#[test]
fn missing_stylesheet_and_non_ui_root_fail_before_any_live_allocation() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Missing stylesheet");
    let snapshot = compile(&app, source.clone())?;
    let before = app.world().entities().count_spawned();
    assert!(UiView::mount(app.world_mut(), &snapshot, None, &BTreeMap::new(), "", 1.).is_err());
    assert_eq!(app.world().entities().count_spawned(), before);
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    let sheet = UiStylesheet {
        asset: app.world().resource::<AssetServer>().load("missing.css"),
    };
    let data = app.world().resource::<ComponentPolicies>().encode(
        "ui.stylesheet",
        &sheet,
        &app.world().resource::<AppTypeRegistry>().read(),
        &BTreeMap::new(),
    )?;
    source.nodes[0]
        .components
        .insert("ui.stylesheet".into(), data);
    let snapshot = compile(&app, source)?;
    let error = UiView::mount(app.world_mut(), &snapshot, None, &BTreeMap::new(), "", 1.)
        .err()
        .ok_or("missing stylesheet was accepted")?;
    assert!(
        error
            .to_string()
            .contains("stylesheet dependency is not materialized")
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}

#[test]
fn text_without_a_stylesheet_scope_fails_without_partial_allocation() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Unstyled text");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    source.nodes[0].components.insert(
        "ui.caption".into(),
        ComponentData::ron(1, "(text:\"No implicit font\",justify:Left,wrap:false)")?,
    );
    let snapshot = compile(&app, source)?;
    let before = app.world().entities().len();
    let error = UiView::mount(app.world_mut(), &snapshot, None, &BTreeMap::new(), "", 1.)
        .err()
        .ok_or("unstyled text was accepted")?;
    assert!(error.to_string().contains("explicit stylesheet scope"));
    assert_eq!(before, app.world().entities().len());
    Ok(())
}

#[test]
fn missing_runtime_and_unsupported_density_are_explicit_mount_errors() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Panel");
    source.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    let snapshot = compile(&app, source)?;
    let before = app.world().entities().count_spawned();
    assert!(
        UiView::mount(
            app.world_mut(),
            &snapshot,
            None,
            &BTreeMap::new(),
            "",
            f32::NAN
        )
        .is_err()
    );
    app.world_mut().remove_resource::<UiFailure>();
    assert!(UiView::mount(app.world_mut(), &snapshot, None, &BTreeMap::new(), "", 1.).is_err());
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}

#[test]
fn native_dialog_group_buttons_and_scope_keys_come_from_source() -> Result<()> {
    let mut app = setup();
    let mut source = PrefabDocument::new("Dialog");
    source.nodes[0].components.insert(
        "ui.layout".into(),
        ComponentData::ron(1, "(node:(width:Px(640.0),height:Px(400.0)))")?,
    );
    source.nodes[0]
        .components
        .insert("ui.id".into(), ComponentData::ron(1, "(\"dialog\")")?);
    source.nodes[0]
        .components
        .insert("ui.modal".into(), ComponentData::ron(1, "Dialog")?);
    let snapshot = compile(&app, source)?;
    let view = UiView::mount(
        app.world_mut(),
        &snapshot,
        None,
        &BTreeMap::new(),
        "fixture",
        1.,
    )?;
    assert!(
        app.world()
            .get::<TabGroup>(view.root)
            .is_some_and(|g| g.modal)
    );
    assert!(
        app.world()
            .get::<bevy::ui_widgets::ModalDialog>(view.root)
            .is_some()
    );
    assert_eq!(view.element("dialog")?, view.root);
    assert_eq!(
        app.world()
            .get::<ElementId>(view.root)
            .ok_or("identity missing")?
            .0,
        "fixture/dialog"
    );
    assert!(view.element("missing").is_err());
    Ok(())
}

#[test]
fn asset_handles_are_encoded_by_bevy_not_an_embedded_font_or_image_fallback() -> Result<()> {
    let app = setup();
    let asset = app
        .world()
        .resource::<AssetServer>()
        .load("game://styles/menu.css");
    let sheet = UiStylesheet { asset };
    let types = app.world().resource::<AppTypeRegistry>();
    let data = app.world().resource::<ComponentPolicies>().encode(
        "ui.stylesheet",
        &sheet,
        &types.read(),
        &BTreeMap::new(),
    )?;
    let mut loader = app.world().resource::<AssetServer>();
    let mut context = ReferenceContext {
        entities: &BTreeMap::new(),
        ports: &BTreeMap::new(),
        prefix: &[],
        loader: &mut loader,
    };
    let decoded = app.world().resource::<ComponentPolicies>().decode(
        "ui.stylesheet",
        &data,
        &types.read(),
        &mut context,
    )?;
    assert_eq!(
        UiStylesheet::from_reflect(decoded.as_ref())
            .ok_or("stylesheet reference missing")?
            .asset
            .path(),
        sheet.asset.path()
    );
    Ok(())
}

#[test]
fn delivered_startup_source_uses_the_same_native_ui_component_policies() -> Result<()> {
    let app = setup();
    let catalog = startup_catalog()?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = kedra_scene::assets::compile_catalog(
        &catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
    )?;
    let snapshot = compiled
        .values()
        .find(|p| p.graph().document.0 == uuid::Uuid::from_u128(0xb20d50db11804528b8a75ffb53ad9b35))
        .ok_or("loading compilation missing")?;
    assert!(snapshot.has_dependencies());
    let source = snapshot.authoring_world();
    let keys = snapshot
        .source_entities()
        .values()
        .filter_map(|entity| source.get::<ElementId>(*entity))
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
    Ok(())
}

#[test]
fn all_declared_startup_documents_compile_atomically_through_shared_policies() -> Result<()> {
    let app = setup();
    let catalog = startup_catalog()?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = kedra_scene::assets::compile_catalog(
        &catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
    )?;
    assert_eq!(compiled.len(), catalog.documents.len());
    Ok(())
}

#[test]
fn startup_catalog_contains_first_real_surface_without_a_splash_document() -> Result<()> {
    let app = setup();
    let catalog = startup_catalog()?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = kedra_scene::assets::compile_catalog(
        &catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
    )?;
    assert!(
        !compiled.contains_key(&kedra_scene::document::DocumentId(uuid::Uuid::from_u128(
            0xc0b00000000040008000000000000003
        )))
    );
    assert!(
        compiled.contains_key(&kedra_scene::document::DocumentId(uuid::Uuid::from_u128(
            0xc0300000000040008000000000000001
        )))
    );
    Ok(())
}

fn startup_catalog() -> Result<Catalog> {
    source_catalog("scenes/startup.prefabs.ron")
}

#[test]
fn dock_fixture_catalog_uses_shared_chrome_without_editor_screens_or_workspace_shell() -> Result<()>
{
    let app = setup();
    let catalog = source_catalog("fixtures/docking.prefabs.ron")?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = kedra_scene::assets::compile_catalog(
        &catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
    )?;
    assert_eq!(compiled.len(), catalog.documents.len());
    for id in [
        0xd0100000000040008000000000000001,
        0xd0200000000040008000000000000001,
        0xd0300000000040008000000000000001,
        0xd0700000000040008000000000000001,
        0xd0800000000040008000000000000001,
        0xd0900000000040008000000000000001,
    ] {
        assert!(
            compiled.contains_key(&kedra_scene::document::DocumentId(uuid::Uuid::from_u128(
                id
            )))
        );
    }
    let group = compiled
        .get(&kedra_scene::document::DocumentId(uuid::Uuid::from_u128(
            0xd0100000000040008000000000000001,
        )))
        .ok_or("Dock group missing")?;
    let rail = group
        .graph()
        .nodes
        .values()
        .find(|node| node.name == "Rail")
        .ok_or("Rail missing")?;
    let names: Vec<_> = rail
        .children
        .iter()
        .map(|address| group.graph().nodes[address].name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Tabs", "Menu"],
        "Authored order must survive identity-sorted composition"
    );
    let tab = compiled
        .get(&kedra_scene::document::DocumentId(uuid::Uuid::from_u128(
            0xd0200000000040008000000000000001,
        )))
        .ok_or("Dock tab missing")?;
    let tab_root = tab
        .graph()
        .nodes
        .get(&tab.graph().root)
        .ok_or("Tab root missing")?;
    assert_eq!(
        tab_root.children.len(),
        2,
        "Only label and close slot belong over the native rounded tab; no rectangular top-light child"
    );
    for id in [
        0xd0500000000040008000000000000001,
        0xd0400000000040008000000000000001,
        0xc0300000000040008000000000000001,
    ] {
        assert!(
            !compiled.contains_key(&kedra_scene::document::DocumentId(uuid::Uuid::from_u128(
                id
            )))
        );
    }
    Ok(())
}

fn source_catalog(path: &str) -> Result<Catalog> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor");
    let paths: Vec<String> = ron::from_str(&std::fs::read_to_string(root.join(path))?)?;
    let mut catalog = Catalog::default();
    for path in paths {
        let relative = path
            .strip_prefix("editor://")
            .ok_or("startup fixture requires an explicit editor source")?;
        catalog.insert(PrefabDocument::parse(&std::fs::read_to_string(
            root.join(relative),
        )?)?)?;
    }
    Ok(catalog)
}

#[path = "authored/hydration.rs"]
mod hydration;

#[path = "authored/assets.rs"]
mod hydration_assets;

#[test]
fn passive_text_rejects_parent_only_stylesheet_before_allocation() -> Result<()> {
    let mut app = setup();
    let sheet = app
        .world_mut()
        .resource_mut::<Assets<bevy_flair::style::StyleSheet>>()
        .add(bevy_flair::style::StyleSheet::empty());
    let parent = app
        .world_mut()
        .spawn((
            Node::default(),
            bevy_flair::style::components::Styled::new(sheet),
        ))
        .id();
    let mut document = PrefabDocument::new("Detached text");
    document.nodes[0].components = BTreeMap::from([
        ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
        (
            "ui.caption".into(),
            ComponentData::ron(1, "(text:\"Detached\",justify:Left,wrap:false)")?,
        ),
    ]);
    let snapshot = compile(&app, document)?;
    let graph = snapshot.graph().clone();
    let before = app.world().entities().count_spawned();
    let error = UiView::mount_passive(app.world_mut(), &snapshot, Some(parent), 1.)
        .err()
        .ok_or("detached parent-only scope was accepted")?;
    assert!(
        error.to_string().contains("explicit stylesheet scope"),
        "{error}"
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    assert!(app.world().get::<Children>(parent).is_none());
    assert!(app.world().resource::<UiFailure>().0.is_none());
    assert_eq!(snapshot.graph(), &graph);
    Ok(())
}

#[test]
fn passive_text_rejects_non_layout_ancestor_only_stylesheet_before_allocation() -> Result<()> {
    use bevy::asset::io::memory::{Dir, MemoryAssetReader};
    let memory = Dir::default();
    memory.insert_asset_text(std::path::Path::new("scope.css"), ".label { width: 10px; }");
    let mut app = App::new();
    app.register_asset_source(
        "memory",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(MemoryAssetReader {
                root: memory.clone(),
            })
        }),
    );
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    app.finish();
    app.cleanup();
    let mut document = PrefabDocument::new("Non-layout stylesheet owner");
    let sheet = UiStylesheet {
        asset: app
            .world()
            .resource::<AssetServer>()
            .load("memory://scope.css"),
    };
    document.nodes[0].components.insert(
        "ui.stylesheet".into(),
        app.world().resource::<ComponentPolicies>().encode(
            "ui.stylesheet",
            &sheet,
            &app.world().resource::<AppTypeRegistry>().read(),
            &BTreeMap::new(),
        )?,
    );
    document.nodes.push(SourceNode {
        id: NodeId::new(),
        name: "Detached label".into(),
        parent: Some(document.root.into()),
        components: BTreeMap::from([
            ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
            (
                "ui.caption".into(),
                ComponentData::ron(1, "(text:\"Detached\",justify:Left,wrap:false)")?,
            ),
        ]),
    });
    let snapshot = compile(&app, document.clone())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !snapshot.asset_status(app.world().resource::<AssetServer>())? {
        assert!(
            std::time::Instant::now() < deadline,
            "Stylesheet did not load"
        );
        app.update();
        std::thread::yield_now();
    }
    let graph = snapshot.graph().clone();
    let before = app.world().entities().count_spawned();
    let error = UiView::mount_passive(app.world_mut(), &snapshot, None, 1.)
        .err()
        .ok_or("non-layout ancestor-only scope was accepted")?;
    assert!(
        error.to_string().contains("explicit stylesheet scope"),
        "{error}"
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    assert!(app.world().resource::<UiFailure>().0.is_none());
    assert_eq!(snapshot.graph(), &graph);
    // A stylesheet on the retained layout ancestor remains valid and owned there.
    document.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    let valid = compile(&app, document)?;
    let graph = valid.graph().clone();
    let passive = UiView::mount_passive(app.world_mut(), &valid, None, 1.)?;
    assert_eq!(passive.ui_roots, [passive.root]);
    assert!(app.world().get::<UiStylesheet>(passive.root).is_some());
    for child in app
        .world()
        .get::<Children>(passive.root)
        .ok_or("Label missing")?
        .iter()
    {
        assert!(app.world().get::<UiStylesheet>(child).is_none());
        assert!(app.world().get::<ChildOf>(child).is_some());
    }
    assert_eq!(valid.graph(), &graph);
    passive.retire(app.world_mut());
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}
