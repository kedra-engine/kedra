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
        (
            "ui.screen".into(),
            ComponentData::ron(1, "(\"passive-screen\")")?,
        ),
        ("ui.button".into(), ComponentData::ron(1, "()")?),
        ("ui.modal".into(), ComponentData::ron(1, "Dialog")?),
        (
            "ui.pickable".into(),
            ComponentData::ron(1, "(should_block_lower:true,is_hoverable:true)")?,
        ),
    ]);
    let snapshot = compile(&app, document)?;
    let graph = snapshot.graph().clone();
    let view = UiView::mount_passive(app.world_mut(), &snapshot, None, 1.)?;
    let root = app.world().entity(view.root);
    assert!(root.get::<Node>().is_some());
    assert!(root.get::<ElementId>().is_none());
    assert!(root.get::<kedra_ui::ScreenId>().is_none());
    assert!(root.get::<bevy::ui_widgets::Button>().is_none());
    assert!(root.get::<bevy::input_focus::AutoFocus>().is_none());
    assert!(root.get::<TabGroup>().is_none());
    let picking = root
        .get::<bevy::picking::Pickable>()
        .ok_or("picking missing")?;
    assert!(!picking.is_hoverable && !picking.should_block_lower);
    assert!(root.get::<bevy::ui_widgets::ModalDialog>().is_none());
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
fn passive_value_refresh_retains_nodes_and_styled_layout_and_rejects_replaced_owners() -> Result<()>
{
    let mut app = setup();
    let mut document = PrefabDocument::new("Retained passive values");
    document.nodes[0].components = BTreeMap::from([
        (
            "ui.layout".into(),
            ComponentData::ron(1, "(node:(width:Px(20.0)))")?,
        ),
        (
            "ui.classes".into(),
            ComponentData::ron(1, "(names:[\"before\"])")?,
        ),
        ("ui.button".into(), ComponentData::ron(1, "()")?),
    ]);
    let baseline = compile(&app, document.clone())?;
    let view = UiView::mount_passive(app.world_mut(), &baseline, None, 1.)?;
    let root = view.root;
    let foreign = app
        .world_mut()
        .spawn(Node {
            width: px(999),
            ..default()
        })
        .id();
    // Native stylesheet output is not the authored Node baseline. An unrelated
    // class/value refresh must not reset it to the source's default layout.
    app.world_mut().get_mut::<Node>(root).ok_or("node")?.width = px(75);
    document.nodes[0].components.insert(
        "ui.classes".into(),
        ComponentData::ron(1, "(names:[\"after\"])")?,
    );
    let updated = compile(&app, document.clone())?;
    let staged = view.prepare_refresh(app.world(), updated.clone(), 1.)?;
    assert_eq!(
        app.world().get::<UiClasses>(root).ok_or("classes")?.names,
        ["before"]
    );
    staged.apply(app.world_mut())?;
    assert_eq!(app.world().get::<Node>(root).ok_or("node")?.width, px(75));
    assert_eq!(
        app.world().get::<UiClasses>(root).ok_or("classes")?.names,
        ["after"]
    );
    assert!(app.world().get::<bevy::ui_widgets::Button>(root).is_none());
    document.nodes[0].components.insert(
        "ui.layout".into(),
        ComponentData::ron(1, "(node:(width:Px(42.0)))")?,
    );
    let layout = compile(&app, document.clone())?;
    let staged = view.prepare_refresh(app.world(), layout.clone(), 1.)?;
    let late = view.prepare_refresh(app.world(), updated, 1.)?;
    staged.apply(app.world_mut())?;
    assert!(late.apply(app.world_mut()).is_err());
    assert_eq!(app.world().get::<Node>(root).ok_or("node")?.width, px(42));
    document.nodes[0].components.remove("ui.classes");
    assert!(
        view.prepare_refresh(app.world(), compile(&app, document)?, 1.)
            .is_err()
    );
    assert_eq!(
        app.world().get::<UiClasses>(root).ok_or("classes")?.names,
        ["after"]
    );
    let staged = view.prepare_refresh(app.world(), layout, 1.)?;
    let saved_node = app.world().get::<Node>(root).ok_or("node")?.clone();
    app.world_mut().entity_mut(root).remove::<Node>();
    assert!(staged.apply(app.world_mut()).is_err());
    assert_eq!(
        app.world().get::<UiClasses>(root).ok_or("classes")?.names,
        ["after"]
    );
    app.world_mut().entity_mut(root).insert(saved_node);
    let staged = view.prepare_refresh(app.world(), baseline, 1.)?;
    view.retire(app.world_mut());
    assert!(staged.apply(app.world_mut()).is_err());
    assert_eq!(
        app.world().get::<Node>(foreign).ok_or("foreign")?.width,
        px(999)
    );
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
