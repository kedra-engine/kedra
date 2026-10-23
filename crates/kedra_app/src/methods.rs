use crate::{AppSession, CommandCatalog, invalid, operations::Operations, params};
use bevy::prelude::*;
use bevy_remote::BrpResult;
use serde_json::{Value, json};
pub(super) fn describe(
    _: In<Option<Value>>,
    session: Res<AppSession>,
    catalog: Res<CommandCatalog>,
    scripts: Res<crate::scripting::Catalog>,
) -> BrpResult {
    Ok(
        json!({"protocol":1,"session":session.id,"methods":catalog.0,"operation_capacity":256,"coordinates":"logical window pixels","input":"injected Bevy events, not OS automation","scripting":{"language":"Luau","programs":scripts.names(),"source_eval":false}}),
    )
}
pub(super) fn operation(In(value): In<Option<Value>>, operations: Res<Operations>) -> BrpResult {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        id: uuid::Uuid,
    }
    let request: Request = params(value)?;
    serde_json::to_value(
        operations
            .get(request.id)
            .ok_or_else(|| invalid("Unknown or expired operation"))?,
    )
    .map_err(bevy_remote::BrpError::internal)
}
