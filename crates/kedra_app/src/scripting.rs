//! Finite host-authorized Luau scenarios over the existing native application API.
//! Runtime execution belongs to kedra_script; no script source/eval is accepted remotely.
mod bindings;
#[cfg(test)]
mod tests;
pub use bindings::{bind, install_app_api};

use crate::{RemoteAppExt, operations::Operations};
use bevy::prelude::*;
use bevy_remote::BrpResult;
use kedra_script::{Program, Run, ScriptSystems, Status};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Resource, Default)]
pub(crate) struct Catalog(BTreeMap<String, Program>);
impl Catalog {
    pub(crate) fn names(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}
#[derive(Resource, Default)]
struct Active(Option<(Uuid, Run)>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Catalog>()
        .init_resource::<Active>()
        .add_systems(Last, poll.after(ScriptSystems::Run));
    for (name, source) in [
        (
            "app.input",
            "function run(ctx, args) return ctx.app.input(args.window, args.input) end",
        ),
        (
            "app.resize",
            "function run(ctx, args) return ctx.app.resize(args.window, args.width, args.height, args.dpr) end",
        ),
        (
            "app.capture",
            "function run(ctx, args) return ctx.app.capture(args.window) end",
        ),
        (
            "app.field",
            "function run(ctx, args) ctx.app.field(args.id, args.value); return {delivered = true} end",
        ),
        (
            "app.click",
            "function run(ctx, args) return ctx.app.click(args.id) end",
        ),
    ] {
        register(
            app,
            Program::sequence(
                name,
                format!("scripts/{name}.luau"),
                source,
                install_app_api,
            ),
        );
    }
    app.register_remote("app.scenario", "Execute a finite registered in-application Luau sequence; no eval", json!({
        "session":"app.describe.session", "id":"UUID idempotency key", "scenario":"host-registered program name", "arguments":"owned JSON data"}), request);
    app.register_remote("app.scenario_cancel", "Revoke the matching active sequence; prior native effects are not rolled back",
        json!({"session":"app.describe.session","id":"UUID idempotency key","operation":"active scenario operation UUID"}), cancel_request);
}

/// Register one host-owned program, not remote/project code. Duplicate names panic
/// as programmer errors, like native method registration. No execution occurs here.
pub fn register(app: &mut App, program: Program) {
    assert_eq!(
        program.kind,
        kedra_script::Kind::Sequence,
        "Application scenarios must be finite sequences"
    );
    let key = program.name.clone();
    assert!(
        app.world_mut()
            .resource_mut::<Catalog>()
            .0
            .insert(key, program)
            .is_none(),
        "Duplicate script scenario"
    );
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    session: String,
    id: Uuid,
    scenario: String,
    arguments: Value,
}

fn request(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    let original = value.clone().unwrap_or(Value::Null);
    let request: Request = crate::params(value)?;
    if request.session != world.resource::<crate::AppSession>().id {
        return Err(crate::invalid("Stale application session"));
    }
    if world.resource::<Operations>().get(request.id).is_some() {
        world
            .resource_mut::<Operations>()
            .admit(request.id, &original)
            .map_err(crate::invalid)?;
        return serde_json::to_value(world.resource::<Operations>().get(request.id))
            .map_err(bevy_remote::BrpError::internal);
    }
    if world.resource::<Active>().0.is_some() {
        return Err(crate::invalid("An application scenario is already running"));
    }
    let program = world
        .resource::<Catalog>()
        .0
        .get(&request.scenario)
        .cloned()
        .ok_or_else(|| crate::invalid("Unknown application scenario"))?;
    kedra_script::values::validate(&request.arguments).map_err(crate::invalid)?;
    world
        .resource_mut::<Operations>()
        .admit(request.id, &original)
        .map_err(crate::invalid)?;
    match kedra_script::start(world, program, request.arguments) {
        Ok(run) => world.resource_mut::<Active>().0 = Some((request.id, run)),
        Err(error) => {
            world
                .resource_mut::<Operations>()
                .finish(request.id, Err(error.to_string()));
        }
    }
    serde_json::to_value(world.resource::<Operations>().get(request.id))
        .map_err(bevy_remote::BrpError::internal)
}

fn poll(world: &mut World) {
    let Some((operation, run)) = world.resource::<Active>().0 else {
        return;
    };
    let result = match kedra_script::status(world, run) {
        Some(Status::Running) => {
            world.write_message(bevy::window::RequestRedraw);
            return;
        }
        Some(Status::Completed(value)) => Ok(value.clone()),
        Some(Status::Failed(error)) => Err(error.to_string()),
        Some(Status::Cancelled) => Err("Application scenario was cancelled".into()),
        None => Err("Application scenario owner was retired".into()),
    };
    world.resource_mut::<Operations>().finish(operation, result);
    let _ = kedra_script::retire(world, run);
    world.resource_mut::<Active>().0 = None;
}

fn cancel_request(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Cancel {
        session: String,
        id: Uuid,
        operation: Uuid,
    }
    let original = value.clone().unwrap_or(Value::Null);
    let request: Cancel = crate::params(value)?;
    if request.session != world.resource::<crate::AppSession>().id {
        return Err(crate::invalid("Stale application session"));
    }
    if world.resource::<Operations>().get(request.id).is_some() {
        world
            .resource_mut::<Operations>()
            .admit(request.id, &original)
            .map_err(crate::invalid)?;
    } else {
        let (operation, run) = world
            .resource::<Active>()
            .0
            .filter(|(id, _)| *id == request.operation)
            .ok_or_else(|| {
                crate::invalid("Scenario is no longer active or belongs to another operation")
            })?;
        world
            .resource_mut::<Operations>()
            .admit(request.id, &original)
            .map_err(crate::invalid)?;
        let result = kedra_script::cancel(world, run)
            .map(|()| json!({"cancelled":operation,"rollback":false}))
            .map_err(|error| error.to_string());
        world
            .resource_mut::<Operations>()
            .finish(request.id, result);
    }
    serde_json::to_value(world.resource::<Operations>().get(request.id))
        .map_err(bevy_remote::BrpError::internal)
}
