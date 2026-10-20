//! Isolated acceptance entry point: installs production dock systems and chrome,
//! but no EditorPlugin, startup states, project jobs or project panels.
use super::*;
use bevy::text::{EditableText, TextEdit};
use kedra_app::RemoteAppExt;
use kedra_docking::{
    model::{Edge, View},
    snapshot::Snapshot,
};
use serde_json::{Value, json};
use std::path::PathBuf;

#[path = "fixture_trace.rs"]
mod trace;

#[derive(Resource)]
struct Fixture {
    original: Snapshot,
    views: [ViewId; 4],
    fields: Vec<Entity>,
    generation: u64,
}

/// Run the native docking acceptance fixture. Requires an explicit delivered skin,
/// a NEW authenticated credential path and an artifact directory in environment
/// variables `KEDRA_FIXTURE_ASSETS`, `KEDRA_QA_CREDENTIALS`, `KEDRA_QA_ARTIFACTS`.
/// Never reads/writes personal preferences or project sources. F6 resets fixture
/// setup, F7 toggles native draft fields, F12 exits; all docking uses product input.
pub fn run() -> Result<(), String> {
    let assets = environment("KEDRA_FIXTURE_ASSETS")?;
    let credentials = environment("KEDRA_QA_CREDENTIALS")?;
    let artifacts = environment("KEDRA_QA_ARTIFACTS")?;
    let (assets, manifest) = kedra_assets::delivery::read_directory(&assets, "editor-desktop")?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "editor",
        assets
            .to_str()
            .ok_or("Fixture delivery root must be Unicode")?
            .into(),
        manifest,
    );
    app.add_plugins(
        DefaultPlugins
            .set(kedra_desktop::window_plugin())
            .set(AssetPlugin {
                use_asset_processor_override: Some(false),
                watch_for_changes_override: Some(false),
                ..default()
            }),
    )
    .add_plugins((
        kedra_app::KedraAppPlugin,
        kedra_desktop::DesktopPlugin,
        bevy::input_focus::tab_navigation::TabNavigationPlugin,
        kedra_ui::authored::AuthoredUiPlugin,
        kedra_scene::assets::PrefabPlugin {
            catalog_path: "editor://fixtures/docking.prefabs.ron".into(),
        },
    ))
    .init_resource::<crate::model::ProjectManager>()
    .add_systems(Startup, create)
    .add_systems(Update, (ready, keys).chain().before(super::tick));
    crate::chrome::install(&mut app);
    super::install(&mut app);
    trace::install(&mut app);
    app.world_mut()
        .resource_mut::<kedra_ui::authored::components::UiMotion>()
        .reduced = true;
    app.register_remote(
        "dock.state",
        "Read production docking state",
        json!({}),
        crate::remote::workspace_state,
    );
    app.register_remote(
        "dock.fixture",
        "Read fixture identity, never a docking mutation",
        json!({}),
        describe,
    );
    let endpoint = kedra_app::transport::enable(&mut app, 0, &credentials, &artifacts)?;
    eprintln!(
        "Dock fixture: {}; credentials: {}",
        endpoint.url,
        credentials.display()
    );
    eprintln!(
        "F6 reset · F7 editable contents · F12 exit. No editor startup or project tools installed."
    );
    if app.run().is_error() {
        Err("Dock fixture failed; see live diagnostics".into())
    } else {
        eprintln!("SUCCESS: dock fixture closed");
        Ok(())
    }
}
fn environment(key: &str) -> Result<PathBuf, String> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("Set {key} explicitly for the isolated fixture"))
}
fn create(world: &mut World) {
    let root = kedra_desktop::replace(world, kedra_desktop::SurfaceKind::Editor, "Docking".into());
    if let Some(window) = world.resource::<kedra_desktop::DesktopSurface>().window
        && let Some(mut window) = world.get_mut::<Window>(window)
    {
        window.resolution.set(1200., 800.);
    }
    world
        .entity_mut(root)
        .insert(kedra_ui::ScreenId("docking.fixture.v1".into()));
}
fn ready(world: &mut World) {
    if world.contains_resource::<Fixture>() {
        return;
    }
    let sources = world.resource::<kedra_scene::assets::PrefabSources>();
    if let Some(error) = sources.error.clone() {
        error!("Fixture sources: {error}");
        world.write_message(AppExit::error());
        return;
    }
    if !sources.ready() {
        return;
    }
    if let Err(error) = mount(world) {
        error!("Dock fixture mount: {error}");
        world.write_message(AppExit::error());
    }
}
fn mount(world: &mut World) -> Result<(), String> {
    let desktop = world.resource::<kedra_desktop::DesktopSurface>().clone();
    let window = desktop.window.ok_or("Fixture window missing")?;
    let root = desktop.root.ok_or("Fixture root missing")?;
    let frame = crate::views::mount(
        world,
        root,
        crate::views::document(0xc0600000000040008000000000000001),
        "fixture-frame",
    )?;
    let caption = crate::views::mount(
        world,
        frame.root,
        crate::views::document(0xc0900000000040008000000000000001),
        "window",
    )?;
    crate::chrome::bind_window(world, &caption, window, Some(frame.root))?;
    let style = world
        .resource::<AssetServer>()
        .load("editor://fixtures/docking.css");
    let host = world
        .spawn((
            Node::default(),
            bevy_flair::style::components::ClassList::new_with_classes([
                "dock-fixture",
                "dock-host",
            ]),
            bevy_flair::style::components::Styled::new(style),
            ChildOf(frame.root),
            kedra_ui::ElementId::new("fixture.host"),
        ))
        .id();
    bind_root(world, window, root)?;
    let views: [ViewId; 4] = serde_json::from_value(json!([
        "f0000000-0000-4000-8000-000000000001",
        "f0000000-0000-4000-8000-000000000002",
        "f0000000-0000-4000-8000-000000000003",
        "f0000000-0000-4000-8000-000000000004"
    ]))
    .map_err(|e| e.to_string())?;
    let mut fields = Vec::new();
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        state.skin = Some(load_skin(world)?);
        let id = state.host;
        state
            .surfaces
            .get_mut(&id)
            .ok_or("Fixture host missing")?
            .root = host;
        state.stash = world
            .spawn((
                Node::default(),
                bevy_flair::style::components::ClassList::new_with_classes(["dock-stash"]),
                ChildOf(host),
            ))
            .id();
        for (index, view) in views.iter().copied().enumerate() {
            let name = ["A", "B", "C", "D"][index];
            let placement = match index {
                0 => Destination::EmptyHost(state.host),
                1 => Destination::Edge {
                    target: state
                        .layout
                        .snapshot()
                        .location(views[0])
                        .ok_or("A missing")?
                        .0,
                    edge: Edge::Right,
                    fraction: 894. / 1194.,
                },
                2 => Destination::Edge {
                    target: state
                        .layout
                        .snapshot()
                        .location(views[1])
                        .ok_or("B missing")?
                        .0,
                    edge: Edge::Bottom,
                    fraction: 296. / 762.,
                },
                _ => Destination::Tabs {
                    group: state
                        .layout
                        .snapshot()
                        .location(views[2])
                        .ok_or("C missing")?
                        .0,
                    index: 1,
                },
            };
            state
                .layout
                .execute(Command::Open {
                    id: view,
                    view: View {
                        kind: name.into(),
                        restore_key: None,
                    },
                    destination: placement,
                })
                .map_err(|e| e.to_string())?;
            let content = crate::views::mount(
                world,
                state.stash,
                crate::views::document(0xd0900000000040008000000000000001),
                &format!("fixture.{name}"),
            )?;
            let text = content.element("identity").map_err(|e| e.to_string())?;
            world
                .get_mut::<Text>(text)
                .ok_or("Identity text missing")?
                .0 = name.into();
            fields.push(content.element("draft").map_err(|e| e.to_string())?);
            state.renderer.register(view, content.root)?;
            state.titles.insert(view, format!("Panel {name}"));
        }
        state
            .layout
            .execute(Command::Activate(views[2]))
            .map_err(|e| e.to_string())?;
        state.focused_group = state
            .layout
            .snapshot()
            .location(views[2])
            .map(|(group, _)| group);
        redraw(world, &mut state)
    })?;
    let original = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .clone();
    world.insert_resource(Fixture {
        original,
        views,
        fields,
        generation: 1,
    });
    eprintln!("READY: dock fixture v1, A/B/C/D, production input/preview/popover/native hosts");
    Ok(())
}
fn keys(world: &mut World) {
    if !world.contains_resource::<Fixture>() {
        return;
    }
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let reset = keys.just_pressed(KeyCode::F6);
    let edit = keys.just_pressed(KeyCode::F7);
    let overflow = keys.just_pressed(KeyCode::F8);
    let exit = keys.just_pressed(KeyCode::F12);
    if exit {
        world.write_message(AppExit::Success);
        return;
    }
    if reset && let Err(error) = reset_setup(world) {
        error!("Fixture reset: {error}");
        world.write_message(AppExit::error());
    }
    if edit {
        let fields = world.resource::<Fixture>().fields.clone();
        for field in fields {
            if let Some(parent) = world.get::<ChildOf>(field).map(ChildOf::parent) {
                let hidden = world
                    .get::<bevy_flair::style::components::ClassList>(parent)
                    .is_some_and(|c| c.contains("hidden"));
                kedra_ui::docking::class(world, parent, "hidden", !hidden);
            }
        }
    }
    if overflow && let Err(error) = overflow_setup(world) {
        error!("Fixture overflow: {error}");
        world.write_message(AppExit::error());
    }
}
fn overflow_setup(world: &mut World) -> Result<(), String> {
    let view: ViewId = serde_json::from_value(json!("f0000000-0000-4000-8000-000000000005"))
        .map_err(|e| e.to_string())?;
    let b = world.resource::<Fixture>().views[1];
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if state.layout.snapshot().views.contains_key(&view) {
            return Ok(());
        }
        let group = state
            .layout
            .snapshot()
            .location(b)
            .ok_or("Fixture B missing")?
            .0;
        state
            .layout
            .execute(Command::Open {
                id: view,
                view: View {
                    kind: "E".into(),
                    restore_key: None,
                },
                destination: Destination::Tabs { group, index: 1 },
            })
            .map_err(|e| e.to_string())?;
        state.titles.insert(
            view,
            "Panel with a very long title that does not fit its tab".into(),
        );
        if state.renderer.content(view).is_none() {
            let content = crate::views::mount(
                world,
                state.stash,
                crate::views::document(0xd0900000000040008000000000000001),
                "fixture.E",
            )?;
            let identity = content.element("identity").map_err(|e| e.to_string())?;
            world
                .get_mut::<Text>(identity)
                .ok_or("Fixture E identity missing")?
                .0 = "E".into();
            state.renderer.register(view, content.root)?;
        }
        state.focused_group = None;
        redraw(world, &mut state)
    })
}
fn reset_setup(world: &mut World) -> Result<(), String> {
    let original = world.resource::<Fixture>().original.clone();
    let fields = world.resource::<Fixture>().fields.clone();
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        input::cancel(world, &mut state);
        input::dismiss(world, &mut state);
        state.layout = original.into_workspace().map_err(|e| e.to_string())?;
        state.focused_group = state
            .layout
            .snapshot()
            .location(world.resource::<Fixture>().views[2])
            .map(|(group, _)| group);
        state.last_divider = None;
        redraw(world, &mut state)?;
        hosts::retire(world, &mut state);
        Ok::<_, String>(())
    })?;
    world
        .resource_mut::<bevy::input_focus::InputFocus>()
        .clear();
    for field in fields {
        if let Some(mut text) = world.get_mut::<EditableText>(field) {
            text.clear();
            text.editor_mut().set_text("");
            text.queue_edit(TextEdit::TextEnd(false));
        }
        if let Some(parent) = world.get::<ChildOf>(field).map(ChildOf::parent) {
            kedra_ui::docking::class(world, parent, "hidden", true);
        }
    }
    world.resource_mut::<Fixture>().generation += 1;
    Ok(())
}
fn describe(_: In<Option<Value>>, world: &mut World) -> bevy_remote::BrpResult {
    let Some(fixture) = world.get_resource::<Fixture>() else {
        return Ok(json!({"ready":false}));
    };
    let state = world.resource::<EditorWorkspace>();
    let renderer = world
        .get_resource::<bevy::render::renderer::RenderAdapterInfo>()
        .map(|adapter| json!({"name":adapter.name,"backend":format!("{:?}",adapter.backend)}));
    Ok(
        json!({"ready":true,"feedback_ready":state.feedback.as_ref().is_none_or(|feedback|feedback.ready(world)),"schema":1,"id":"docking.fixture.v1","generation":fixture.generation,"views":fixture.views,"host":state.surfaces[&state.host].root.to_bits(),"window":state.window.to_bits(),"project_tools":false,"preferences":false,"pending":state.pending.is_some(),"dragging":matches!(state.gesture,Some(input::Gesture::Drag{..})),"fields":fixture.fields.iter().map(|field|field.to_bits()).collect::<Vec<_>>(),"renderer":renderer}),
    )
}
