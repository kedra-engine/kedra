//! Debug-only, scoped native projection operations. Luau owns scenario choices.
use super::Lab;
use bevy::prelude::*;
use kedra_app::RemoteAppExt;
use kedra_docking::{
    command::{Command, Destination, Selection},
    model::{Edge, ViewId},
};
use kedra_script::{
    Scope,
    lua::{self, Lua},
};
use serde_json::{Value, json};

pub(super) fn install(app: &mut App) {
    app.register_remote(
        "projection.state",
        "Read native projection topology and retained identities",
        json!({}),
        |_: In<Option<Value>>, world: &mut World| -> bevy_remote::BrpResult {
            state(world).map_err(bevy_remote::BrpError::internal)
        },
    );
    for action in [
        "exercise",
        "reorder",
        "split",
        "activate",
        "maximize",
        "visibility",
        "restore",
    ] {
        let mut program = kedra_script::Program::sequence(
            format!("projection.{action}"),
            format!("scripts/Native/projection.{action}.luau"),
            format!(
                "local projection = require(\"@game/Native/projection\")\nfunction run(ctx, args) return projection.run(ctx, \"{action}\") end"
            ),
            bindings,
        );
        program
            .sources
            .insert(".luaurc", include_str!("../../.luaurc"))
            .expect("Embedded Luau configuration is valid");
        program
            .sources
            .insert(
                "scripts/Native/projection.luau",
                include_str!("../../scripts/Native/projection.luau"),
            )
            .expect("Embedded projection module is valid");
        kedra_app::scripting::register(app, program);
    }
}

fn view(lab: &Lab, name: &str) -> Result<ViewId, String> {
    let mut matches = lab
        .model
        .snapshot()
        .views
        .iter()
        .filter(|(_, descriptor)| descriptor.kind == name);
    let id = *matches.next().ok_or("Unknown projection view")?.0;
    if matches.next().is_some() {
        return Err("Ambiguous projection view".into());
    }
    Ok(id)
}

pub(super) fn state(world: &World) -> Result<Value, String> {
    if let Some(sources) = world.get_resource::<kedra_scene::assets::PrefabSources>()
        && let Some(error) = &sources.error
    {
        return Err(error.clone());
    }
    if let Some(startup) = world.get_resource::<kedra_scene::startup::StartupScene>() {
        match startup.state() {
            kedra_scene::startup::StartupState::Failed(error) => return Err(error.clone()),
            kedra_scene::startup::StartupState::Retired => {
                return Err("Startup scene retired".into());
            }
            _ => (),
        }
    }
    if let Some(failure) = world.get_resource::<kedra_ui::authored::components::UiFailure>()
        && let Some(error) = &failure.0
    {
        return Err(error.clone());
    }
    let Some(lab) = world.get_resource::<Lab>() else {
        return Ok(json!({"mounted":false,"projected":false}));
    };
    let mut views = serde_json::Map::new();
    for (id, descriptor) in &lab.model.snapshot().views {
        let content = lab
            .projection
            .content(*id)
            .ok_or("Missing retained content")?;
        let tab = lab
            .projection
            .tab(*id)
            .and_then(|entity| world.get::<kedra_ui::ElementId>(entity))
            .map(|id| id.0.clone());
        views.insert(
            descriptor.kind.clone(),
            json!({
                "id":id, "content":content.to_bits().to_string(), "tab":tab,
                "group":lab.model.snapshot().location(*id).map(|location| location.0),
                "hidden":lab.model.snapshot().hidden.contains_key(id),
            }),
        );
    }
    Ok(
        json!({"mounted":true,"snapshot":lab.model.snapshot(), "views":views,
        "projected":lab.rendered == Some(lab.model.revision())}),
    )
}

fn with_lab<T>(
    scope: &Scope,
    operation: impl FnOnce(&mut Lab) -> Result<T, String>,
) -> lua::Result<T> {
    scope.with_world(|world| {
        if !world.contains_resource::<Lab>() {
            return Err(lua::Error::runtime("Projection is not mounted"));
        }
        world
            .resource_scope(|_, mut lab: Mut<Lab>| operation(&mut lab))
            .map_err(lua::Error::runtime)
    })
}

fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    kedra_app::scripting::install_app_api(lua, scope.clone(), ctx)?;
    let api = lua.create_table()?;
    let state_scope = scope.clone();
    api.set(
        "state",
        lua.create_function(move |lua, ()| {
            let state =
                state_scope.with_world(|world| state(world).map_err(lua::Error::runtime))?;
            kedra_script::values::to_lua(lua, &state)
        })?,
    )?;
    let move_scope = scope.clone();
    api.set(
        "move",
        lua.create_function(
            move |_,
                  (name, target, edge, index, fraction): (
                String,
                String,
                Option<String>,
                usize,
                Option<f32>,
            )| {
                with_lab(&move_scope, |lab| {
                    let id = view(lab, &name)?;
                    let target = view(lab, &target)?;
                    let group = lab
                        .model
                        .snapshot()
                        .location(target)
                        .ok_or("Target view is hidden")?
                        .0;
                    let destination = match edge.as_deref() {
                        None if fraction.is_none() => Destination::Tabs { group, index },
                        Some(edge) => Destination::Edge {
                            target: group,
                            edge: match edge {
                                "left" => Edge::Left,
                                "right" => Edge::Right,
                                "top" => Edge::Top,
                                "bottom" => Edge::Bottom,
                                _ => return Err("Unsupported projection edge".into()),
                            },
                            fraction: fraction.ok_or("Edge placement requires a fraction")?,
                        },
                        None => return Err("Tab placement does not accept an edge fraction".into()),
                    };
                    lab.model
                        .execute(Command::Move {
                            selection: Selection::View(id),
                            destination,
                        })
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                })
            },
        )?,
    )?;
    let maximize_scope = scope.clone();
    api.set(
        "maximize",
        lua.create_function(move |_, name: Option<String>| {
            with_lab(&maximize_scope, |lab| {
                let group = name
                    .map(|name| {
                        let id = view(lab, &name)?;
                        lab.model
                            .snapshot()
                            .location(id)
                            .map(|location| location.0)
                            .ok_or_else(|| "Cannot maximize hidden view".to_owned())
                    })
                    .transpose()?;
                lab.model
                    .execute(Command::Maximize {
                        host: lab.host,
                        group,
                    })
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
        })?,
    )?;
    let visibility_scope = scope.clone();
    api.set(
        "visibility",
        lua.create_function(move |_, (name, target, visible): (String, String, bool)| {
            with_lab(&visibility_scope, |lab| {
                let id = view(lab, &name)?;
                let command = if visible {
                    let target = view(lab, &target)?;
                    let group = lab
                        .model
                        .snapshot()
                        .location(target)
                        .ok_or("Target view is hidden")?
                        .0;
                    Command::Show {
                        view: id,
                        destination: Destination::Tabs { group, index: 0 },
                    }
                } else {
                    Command::Hide(id)
                };
                lab.model
                    .execute(command)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
        })?,
    )?;
    api.set(
        "restore",
        lua.create_function(move |_, ()| {
            with_lab(&scope, |lab| {
                lab.model = lab
                    .original
                    .clone()
                    .into_workspace()
                    .map_err(|error| error.to_string())?;
                lab.rendered = None;
                Ok(())
            })
        })?,
    )?;
    api.set_readonly(true);
    ctx.set("projection", api)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmounted_projection_is_explicitly_unready() {
        let world = World::new();
        assert_eq!(
            state(&world).unwrap(),
            json!({"mounted":false,"projected":false})
        );
    }

    #[test]
    fn unmounted_readiness_does_not_hide_source_failure() {
        let mut world = World::new();
        let mut sources = kedra_scene::assets::PrefabSources::default();
        sources.error = Some("Rejected source revision".into());
        world.insert_resource(sources);
        assert_eq!(state(&world).unwrap_err(), "Rejected source revision");
    }

    #[test]
    fn unmounted_readiness_does_not_hide_ui_failure() {
        let mut world = World::new();
        world.insert_resource(kedra_ui::authored::components::UiFailure(Some(
            "Rejected stylesheet".into(),
        )));
        assert_eq!(state(&world).unwrap_err(), "Rejected stylesheet");
    }
}
