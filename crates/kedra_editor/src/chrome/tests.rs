//! Independent authored header fixture exercises the production composition adapter.
use super::*;
use bevy::{
    asset::{AssetApp, AssetPlugin},
    camera::RenderTarget,
    window::WindowRef,
};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, NodeId, PrefabDocument},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use std::sync::Arc;
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn fixture() -> TestResult<(App, Entity, UiView)> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(kedra_ui::authored::AuthoredUiPlugin);
    let mut document = PrefabDocument::new("Independent header adapter fixture");
    document.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    for key in [
        "window.title",
        "window.minimize",
        "window.maximize",
        "window.close",
        "window.maximize_glyph",
        "window.restore_glyph",
        "window.native",
        "window.tabs",
        "window.menu",
        "window.tools",
        "editor.play",
        "editor.pause",
        "editor.stop",
        "editor.save",
        "editor.build",
        "editor.search",
        "editor.panels",
        "editor.reset",
        "editor.project",
    ] {
        document.nodes.push(SourceNode {
            id: NodeId::new(),
            name: key.into(),
            parent: Some(document.root.into()),
            components: BTreeMap::from([
                ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
                (
                    "ui.id".into(),
                    ComponentData::ron(1, &format!("(\"{key}\")"))?,
                ),
                ("ui.classes".into(), ComponentData::ron(1, "(names:[])")?),
            ]),
        });
    }
    let id = document.id;
    let mut catalog = Catalog::default();
    catalog.insert(document)?;
    let compiled = Arc::new(CompiledPrefab::compile(
        catalog.resolve(id)?,
        &app.world().resource::<AppTypeRegistry>().read(),
        app.world().resource::<ComponentPolicies>(),
        &mut app.world().resource::<AssetServer>(),
        &BTreeMap::new(),
    )?);
    let world = app.world_mut();
    let window = world.spawn(Window::default()).id();
    let camera = world
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    let parent = world.spawn((Node::default(), UiTargetCamera(camera))).id();
    let view = UiView::mount(world, &compiled, Some(parent), &BTreeMap::new(), "test", 1.)?;
    Ok((app, window, view))
}
#[test]
fn unimplemented_commands_are_disabled() -> TestResult<()> {
    let (mut app, window, view) = fixture()?;
    bind_window(app.world_mut(), &view, window, None)?;
    for key in [
        "editor.play",
        "editor.pause",
        "editor.stop",
        "editor.save",
        "editor.build",
        "editor.search",
    ] {
        assert!(
            app.world()
                .get::<bevy::ui::InteractionDisabled>(view.element(key)?)
                .is_some(),
            "{key}"
        );
    }
    Ok(())
}
#[test]
fn split_float_has_standalone_caption_and_single_group_reuses_slots() -> TestResult<()> {
    let (mut app, window, view) = fixture()?;
    bind_window(app.world_mut(), &view, window, None)?;
    assert!(floating_destination(app.world_mut(), window, false)?.is_none());
    let destination = floating_destination(app.world_mut(), window, true)?
        .ok_or("Missing integrated destination")?;
    assert_eq!(destination.row, view.element("window.tabs")?);
    assert_eq!(destination.menu, view.element("window.menu")?);
    assert!(floating_destination(app.world_mut(), window, false)?.is_none());
    Ok(())
}
