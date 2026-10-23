//! Minimal workspace host and synthetic geometry; no native/layout acceptance.
use super::super::input::CursorSample;
use super::super::*;
use bevy::{
    asset::{AssetApp, AssetPlugin},
    ui::Node as UiNode,
};
use kedra_docking::model::View;
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, PrefabDocument},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};

fn compile(app: &App, keys: &[&str]) -> Result<Arc<CompiledPrefab>, Box<dyn std::error::Error>> {
    let mut document = PrefabDocument::new("Dock admission test");
    let root = document.root;
    document.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    for key in keys {
        document.nodes.push(SourceNode {
            id: kedra_scene::document::NodeId::new(),
            name: (*key).into(),
            parent: Some(root.into()),
            components: BTreeMap::from([
                ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
                (
                    "ui.id".into(),
                    ComponentData::ron(1, &format!("(\"{key}\")"))?,
                ),
            ]),
        });
    }
    let id = document.id;
    let mut catalog = Catalog::default();
    catalog.insert(document)?;
    let registry = app.world().resource::<AppTypeRegistry>();
    Ok(Arc::new(CompiledPrefab::compile(
        catalog.resolve(id)?,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut app.world().resource::<AssetServer>(),
        &BTreeMap::new(),
    )?))
}

pub(in crate::workspace) fn setup() -> Result<(App, ViewId, ViewId), Box<dyn std::error::Error>> {
    let mut app = App::new();
    crate::views::test_support::install(&mut app);
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .add_plugins(kedra_scene::assets::SceneAssetPlugin)
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(kedra_ui::authored::AuthoredUiPlugin)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<bevy::input_focus::InputFocus>()
        .init_resource::<CursorSample>()
        .add_message::<bevy::window::CursorMoved>()
        .init_resource::<super::super::hosts::CloseRequests>();
    kedra_ui::docking::scene::register(&mut app)?;
    let skin = Arc::new(Skin {
        group: compile(&app, &["tabs", "body", "menu"])?,
        tab: compile(&app, &["label", "title", "close"])?,
        split: compile(&app, &["divider"])?,
    });
    let world = app.world_mut();
    let window = world.spawn(Window::default()).id();
    let root = world.spawn(UiNode::default()).id();
    super::super::bind_root(world, window, root)?;
    world.resource_mut::<EditorWorkspace>().panels.preset =
        Some(super::super::preset::Preset::test_source()?);
    let a = ViewId::new();
    let b = ViewId::new();
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            state.skin = Some(skin);
            let host = state.host;
            state
                .layout
                .execute(Command::Open {
                    id: a,
                    view: View {
                        kind: "a".into(),
                        restore_key: None,
                    },
                    destination: Destination::EmptyHost(host),
                })
                .map_err(|e| e.to_string())?;
            let group = state
                .layout
                .snapshot()
                .location(a)
                .ok_or("Missing group")?
                .0;
            state
                .layout
                .execute(Command::Open {
                    id: b,
                    view: View {
                        kind: "b".into(),
                        restore_key: None,
                    },
                    destination: Destination::Tabs { group, index: 1 },
                })
                .map_err(|e| e.to_string())?;
            redraw(world, &mut state)
        },
    )?;
    Ok((app, a, b))
}

pub(in crate::workspace) fn box_at(world: &mut World, entity: Entity, center: Vec2, size: Vec2) {
    world.entity_mut(entity).insert((
        ComputedNode {
            size,
            inverse_scale_factor: 1.,
            ..default()
        },
        UiGlobalTransform::from(bevy::math::Affine2::from_translation(center)),
    ));
}
