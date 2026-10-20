//! Candidate admission regressions using the production projection and synthetic layout.
use super::*;
use bevy::asset::{AssetApp, AssetPlugin};
use kedra_docking::model::{Edge, View};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, PrefabDocument},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

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
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
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
    let skin = Arc::new(Skin {
        group: compile(&app, &["tabs", "body", "menu"])?,
        tab: compile(&app, &["label", "title", "close"])?,
        split: compile(&app, &["divider"])?,
    });
    let world = app.world_mut();
    let window = world.spawn(Window::default()).id();
    let root = world.spawn(UiNode::default()).id();
    super::super::bind_root(world, window, root)?;
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

#[test]
fn r13_missing_measurements_never_become_ready() -> TestResult {
    let (mut app, _, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let snapshot = state.layout.snapshot().clone();
            measurement(world, &mut state, &snapshot, Selection::View(b))?;
            for _ in 0..5 {
                update_measurement(world, &mut state)?;
            }
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn r13_maximized_edge_candidate_is_not_admissible() -> TestResult {
    let (mut app, a, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state
                .layout
                .snapshot()
                .location(a)
                .ok_or("Missing group")?
                .0;
            let host = state.host;
            state
                .layout
                .execute(Command::Maximize {
                    host,
                    group: Some(group),
                })
                .map_err(|e| e.to_string())?;
            let root = state.renderer.group(group).ok_or("Missing chrome")?;
            box_at(world, root, Vec2::splat(400.), Vec2::splat(800.));
            box_at(world, state.root, Vec2::splat(400.), Vec2::splat(800.));
            let before = state.layout.snapshot().clone();
            let destination = Destination::Edge {
                target: group,
                edge: Edge::Right,
                fraction: 0.5,
            };
            assert!(!super::super::feedback::admissible(
                world,
                &state,
                Selection::View(b),
                destination
            ));
            assert_eq!(state.layout.snapshot(), &before);
            state
                .layout
                .execute(Command::Maximize { host, group: None })
                .map_err(|e| e.to_string())?;
            assert!(super::super::feedback::admissible(
                world,
                &state,
                Selection::View(b),
                destination
            ));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn r13_ready_measurement_is_revoked_when_geometry_changes_or_retires() -> TestResult {
    let (mut app, _, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let snapshot = state.layout.snapshot().clone();
            measurement(world, &mut state, &snapshot, Selection::View(b))?;
            let measure = state.measurements.as_ref().ok_or("Missing measurement")?;
            let target = measure
                .projection
                .group(measure.target)
                .ok_or("Missing target")?;
            box_at(world, state.root, Vec2::splat(400.), Vec2::splat(800.));
            box_at(world, target, Vec2::splat(200.), Vec2::splat(400.));
            for _ in 0..3 {
                update_measurement(world, &mut state)?;
            }
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(3));

            // Current measurements, not an earlier settled rectangle, gate release.
            box_at(world, target, Vec2::splat(250.), Vec2::splat(400.));
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(1));
            box_at(world, target, Vec2::splat(200.), Vec2::ZERO);
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            box_at(world, target, Vec2::splat(900.), Vec2::splat(400.));
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            world.despawn(target);
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn r13_invisible_maximized_candidate_rejects_even_stale_positive_geometry() -> TestResult {
    let (mut app, a, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state
                .layout
                .snapshot()
                .location(a)
                .ok_or("Missing group")?
                .0;
            let host = state.host;
            state
                .layout
                .execute(Command::Maximize {
                    host,
                    group: Some(group),
                })
                .map_err(|e| e.to_string())?;
            let before = state.layout.snapshot().clone();
            let candidate = state
                .layout
                .prepare(Command::Move {
                    selection: Selection::View(b),
                    destination: Destination::Edge {
                        target: group,
                        edge: Edge::Right,
                        fraction: 0.5,
                    },
                })
                .map_err(|e| e.to_string())?;
            measurement(world, &mut state, candidate.snapshot(), Selection::View(b))?;
            let measure = state.measurements.as_ref().ok_or("Missing measurement")?;
            let target = measure
                .projection
                .group(measure.target)
                .ok_or("Missing target")?;
            box_at(world, state.root, Vec2::splat(400.), Vec2::splat(800.));
            box_at(world, target, Vec2::splat(200.), Vec2::splat(400.));
            for _ in 0..5 {
                update_measurement(world, &mut state)?;
            }
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            assert_eq!(state.layout.snapshot(), &before);
            Ok(())
        },
    )?;
    Ok(())
}
