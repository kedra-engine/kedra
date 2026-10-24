//! Scene-bound docking diagnostics using the editor's production capabilities.
use super::*;
use bevy::text::{EditableText, TextEdit};
use kedra_app::RemoteAppExt;
use kedra_docking::snapshot::Snapshot;
use kedra_script::{
    Scope,
    lua::{self, Lua},
};
use serde_json::{Value, json};

#[path = "fixture/overflow.rs"]
mod overflow;
#[path = "fixture_trace.rs"]
mod trace;
pub(crate) use overflow::{OverflowSource, validate_source as validate_overflow};

#[derive(Resource)]
struct Fixture {
    root: Entity,
    original: Snapshot,
    views: [ViewId; 4],
    fields: Vec<Entity>,
    generation: u64,
    overflow: overflow::Config,
}

/// Add diagnostic bindings without constructing an application or selecting a scene.
pub(crate) fn install(app: &mut App) {
    trace::install(app);
    for (name, source) in [
        ("dock.reset", include_str!("fixture/reset.luau")),
        ("dock.drafts", include_str!("fixture/drafts.luau")),
        ("dock.overflow", include_str!("fixture/overflow.luau")),
    ] {
        let mut program = kedra_script::Program::sequence(
            name,
            format!(
                "crates/kedra_editor/src/workspace/fixture/{}.luau",
                name.trim_start_matches("dock.")
            ),
            source,
            script_api,
        );
        // Like duplicate native-method registration, invalid embedded constants
        // are a host programming error, not a recoverable project-source failure.
        let helper = program.sources.insert(
            "crates/kedra_editor/src/workspace/fixture/helpers.luau",
            include_str!("fixture/helpers.luau"),
        );
        assert!(helper.is_ok(), "Invalid embedded dock helper: {helper:?}");
        kedra_app::scripting::register(app, program);
    }
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
}

fn script_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    kedra_app::scripting::install_app_api(lua, scope.clone(), ctx)?;
    let dock = lua.create_table()?;
    dock.set(
        "state",
        kedra_app::scripting::bind(lua, scope.clone(), "dock.fixture", false)?,
    )?;
    for (name, operation) in [
        ("reset", reset_setup as fn(&mut World) -> Result<(), String>),
        ("drafts", toggle_drafts),
        ("overflow", overflow_setup),
    ] {
        let owner = scope.clone();
        dock.set(
            name,
            lua.create_function(move |_, ()| {
                owner.with_world(|world| operation(world).map_err(lua::Error::runtime))
            })?,
        )?;
    }
    dock.set_readonly(true);
    ctx.set("dock", dock)
}
pub(crate) fn bind(
    world: &mut World,
    canvas: &UiView,
    window: Entity,
    root: Entity,
) -> Result<(), String> {
    world
        .resource_mut::<kedra_ui::authored::components::UiMotion>()
        .reduced = true;
    let layout = world
        .get::<kedra_ui::docking::scene::InitialLayout>(canvas.root)
        .ok_or("Docking scene has no initial layout")?
        .workspace()
        .map_err(|e| e.to_string())?;
    let host_id = *layout
        .snapshot()
        .hosts
        .keys()
        .next()
        .ok_or("Missing docking host")?;
    if layout.snapshot().hosts.len() != 1 || layout.snapshot().views.len() != 4 {
        return Err("Dock acceptance requires one host and four views".into());
    }
    let overflow = world
        .get::<OverflowSource>(canvas.root)
        .ok_or("Dock scene has no overflow specimen")?
        .config()?;
    overflow.validate_for(layout.snapshot())?;
    overflow.check_content(world)?;
    let descriptors = layout.snapshot().views.clone();
    let views: [ViewId; 4] = descriptors
        .keys()
        .copied()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "Dock acceptance requires four views")?;
    let frame = crate::views::mount_asset(
        world,
        root,
        "editor://prefabs/window-frame.prefab.ron",
        "fixture-frame",
    )?;
    let caption = crate::views::mount_asset(
        world,
        frame.root,
        "editor://prefabs/titlebar.prefab.ron",
        "window",
    )?;
    crate::chrome::bind_window(world, &caption, window, Some(frame.root))?;
    let host = canvas.root;
    world.entity_mut(host).insert(ChildOf(frame.root));
    bind_root(world, window, root)?;
    let mut fields = Vec::new();
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        state.skin = Some(load_skin(world)?);
        let old_host = state.host;
        let surface = state
            .surfaces
            .remove(&old_host)
            .ok_or("Fixture host missing")?;
        state.host = host_id;
        state.surfaces.insert(host_id, surface);
        state.layout = layout;
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
        for view in views.iter().copied() {
            let descriptor = &descriptors[&view];
            let name = &descriptor.kind;
            let path = descriptor
                .restore_key
                .as_deref()
                .ok_or("Missing content source path")?;
            let content =
                crate::views::mount_asset(world, state.stash, path, &format!("fixture.{name}"))?;
            let text = content.element("identity").map_err(|e| e.to_string())?;
            world
                .get_mut::<Text>(text)
                .ok_or("Identity text missing")?
                .0 = name.clone();
            fields.push(content.element("draft").map_err(|e| e.to_string())?);
            state.renderer.register(view, content.root)?;
            state.titles.insert(view, format!("Panel {name}"));
        }
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
        root,
        original,
        views,
        fields,
        generation: 1,
        overflow,
    });
    eprintln!("READY: dock fixture v1, A/B/C/D, production input/preview/popover/native hosts");
    Ok(())
}

pub(crate) fn retire_binding(world: &mut World, root: Entity) {
    if world
        .get_resource::<Fixture>()
        .is_some_and(|fixture| fixture.root == root)
    {
        world.remove_resource::<Fixture>();
    }
    if world
        .get_resource::<EditorWorkspace>()
        .is_some_and(|workspace| workspace.root == root)
    {
        world.remove_resource::<EditorWorkspace>();
    }
}

#[cfg(test)]
pub(crate) fn has_binding(world: &World) -> bool {
    world.contains_resource::<Fixture>()
}
#[cfg(test)]
pub(crate) fn binding_entity(world: &World) -> Option<Entity> {
    world.resource_entity::<Fixture>()
}
fn require_binding(world: &World) -> Result<(), String> {
    let fixture = world
        .get_resource::<Fixture>()
        .ok_or("Dock diagnostic scene is not bound")?;
    let workspace = world
        .get_resource::<EditorWorkspace>()
        .ok_or("Dock workspace is not bound")?;
    if workspace.root != fixture.root
        || !world.entities().contains(fixture.root)
        || world.get::<Window>(workspace.window).is_none()
    {
        return Err("Dock diagnostic binding was retired".into());
    }
    Ok(())
}

fn toggle_drafts(world: &mut World) -> Result<(), String> {
    require_binding(world)?;
    let fields = world.resource::<Fixture>().fields.clone();
    for field in fields {
        if let Some(parent) = world.get::<ChildOf>(field).map(ChildOf::parent) {
            let hidden = world
                .get::<bevy_flair::style::components::ClassList>(parent)
                .is_some_and(|c| c.contains("hidden"));
            kedra_ui::docking::class(world, parent, "hidden", !hidden);
        }
    }
    Ok(())
}
fn overflow_setup(world: &mut World) -> Result<(), String> {
    overflow::apply(world)
}
fn reset_setup(world: &mut World) -> Result<(), String> {
    require_binding(world)?;
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
    if let Err(error) = require_binding(world) {
        return Ok(json!({"ready":false,"error":error}));
    }
    let Some(fixture) = world.get_resource::<Fixture>() else {
        return Ok(json!({"ready":false}));
    };
    let state = world.resource::<EditorWorkspace>();
    let renderer = world
        .get_resource::<bevy::render::renderer::RenderAdapterInfo>()
        .map(|adapter| json!({"name":adapter.name,"backend":format!("{:?}",adapter.backend)}));
    Ok(
        json!({"ready":true,"feedback_ready":state.feedback.as_ref().is_none_or(|feedback|feedback.ready(world)),"schema":1,"id":"docking.fixture.v1","generation":fixture.generation,"views":fixture.views,"host":state.surfaces[&state.host].root.to_bits().to_string(),"window":state.window.to_bits(),"project_tools":false,"preferences":false,"pending":state.pending.is_some(),"dragging":matches!(state.gesture,Some(input::Gesture::Drag{..})),"fields":fixture.fields.iter().map(|field|field.to_bits().to_string()).collect::<Vec<_>>(),"renderer":renderer}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_identity_observation_crosses_luau_losslessly_without_mutating_drafts()
    -> Result<(), Box<dyn std::error::Error>> {
        // This owner stores entity references, not a second entity registry.
        // Large generations need not be live for describe; the binding root and
        // window remain live so the real authorization/producer path is exercised.
        let (mut app, _, _) = super::super::test_support::setup()?;
        app.add_plugins(kedra_script::LuauPlugin)
            .init_resource::<kedra_app::CommandCatalog>()
            .insert_resource(bevy_remote::RemoteMethods::new());
        app.register_remote("dock.fixture", "Fixture observation", json!({}), describe);
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        let (root, window, host, original) = {
            let state = world.resource::<EditorWorkspace>();
            (
                state.root,
                state.window,
                state.host,
                state.layout.snapshot().clone(),
            )
        };
        let draft = world.spawn(EditableText::new("Кедра draft")).id();
        world
            .resource_mut::<bevy::input_focus::InputFocus>()
            .set(draft, bevy::input_focus::FocusCause::Pressed);
        let high_host = Entity::from_index_and_generation(
            root.index(),
            root.generation().after_versions(1 << 22),
        );
        let high_field = Entity::from_index_and_generation(
            draft.index(),
            draft.generation().after_versions(1 << 22),
        );
        assert!(high_host.to_bits() > 9_007_199_254_740_991);
        assert!(high_field.to_bits() > 9_007_199_254_740_991);
        world
            .resource_mut::<EditorWorkspace>()
            .surfaces
            .get_mut(&host)
            .ok_or("Missing host")?
            .root = high_host;
        world.insert_resource(Fixture {
            root,
            original: original.clone(),
            views: [ViewId::new(), ViewId::new(), ViewId::new(), ViewId::new()],
            fields: vec![high_field, draft],
            generation: 1,
            overflow: overflow::test_source()?.config()?,
        });
        let source = "function run(ctx) return ctx.dock.state() end";
        let run = kedra_script::start(
            world,
            kedra_script::Program::sequence("dock observation", "observe.luau", source, script_api),
            Value::Null,
        )?;
        app.update();
        let Some(kedra_script::Status::Completed(value)) = kedra_script::status(app.world(), run)
        else {
            return Err(format!(
                "Expected lossless observation, got {:?}",
                kedra_script::status(app.world(), run)
            )
            .into());
        };
        assert_eq!(value["host"], high_host.to_bits().to_string());
        assert_eq!(
            value["fields"],
            json!([
                high_field.to_bits().to_string(),
                draft.to_bits().to_string()
            ])
        );
        assert_eq!(value["window"], window.to_bits().to_string());
        assert_eq!(value["generation"], 1);
        assert_eq!(value["schema"], 1);
        let raw = describe(In(None), app.world_mut()).map_err(|e| e.message)?;
        assert_eq!(raw["host"], high_host.to_bits().to_string());
        assert_eq!(raw["fields"][0], high_field.to_bits().to_string());
        app.world_mut().despawn(root);
        let denied = kedra_script::start(
            app.world_mut(),
            kedra_script::Program::sequence(
                "retired diagnostic",
                "retired.luau",
                "function run(ctx) assert(not ctx.dock.state().ready); ctx.dock.drafts() end",
                script_api,
            ),
            Value::Null,
        )?;
        app.update();
        assert!(matches!(
            kedra_script::status(app.world(), denied),
            Some(kedra_script::Status::Failed(error))
                if error.kind == kedra_script::ErrorKind::Runtime
                    && error.message.contains("Dock diagnostic binding was retired")
        ));
        assert_eq!(app.world().resource::<Fixture>().generation, 1);
        assert!(app.world().resource::<EditorWorkspace>().layout.snapshot() == &original);
        assert_eq!(
            app.world()
                .resource::<bevy::input_focus::InputFocus>()
                .get(),
            Some(draft)
        );
        assert_eq!(
            app.world()
                .get::<EditableText>(draft)
                .ok_or("Draft missing")?
                .value(),
            "Кедра draft"
        );
        assert!(app.world().get::<Window>(window).is_some());
        Ok(())
    }

    #[test]
    fn diagnostic_capabilities_reject_unbound_and_retired_owners_without_mutation()
    -> Result<(), String> {
        let mut world = World::new();
        assert!(reset_setup(&mut world).is_err());
        assert!(toggle_drafts(&mut world).is_err());
        assert!(overflow_setup(&mut world).is_err());
        let window = world.spawn(Window::default()).id();
        let root = world.spawn(Node::default()).id();
        bind_root(&mut world, window, root)?;
        let original = world
            .resource::<EditorWorkspace>()
            .layout
            .snapshot()
            .clone();
        world.insert_resource(Fixture {
            root,
            original: original.clone(),
            views: [ViewId::new(), ViewId::new(), ViewId::new(), ViewId::new()],
            fields: vec![],
            generation: 1,
            overflow: overflow::test_source()?.config()?,
        });
        world.despawn(root);
        assert!(reset_setup(&mut world).is_err());
        assert!(toggle_drafts(&mut world).is_err());
        assert!(overflow_setup(&mut world).is_err());
        assert_eq!(world.resource::<Fixture>().generation, 1);
        assert!(world.resource::<EditorWorkspace>().layout.snapshot() == &original);
        retire_binding(&mut world, root);
        assert!(!has_binding(&world));
        assert!(!world.contains_resource::<EditorWorkspace>());
        assert!(world.entities().contains(window));
        Ok(())
    }
}
