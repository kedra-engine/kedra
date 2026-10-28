//! Native bounded capabilities, restricted to this composition's retained subtree.
use super::*;
use bevy::reflect::ParsedPath;
use kedra_script::{
    Scope,
    lua::{self, Lua},
};

pub(super) fn install(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let target = *ctx
        .get::<lua::AnyUserData>("entity")?
        .borrow::<kedra_script::EntityHandle>()?;
    let fields = lua.create_table()?;
    fields.set(
        "add",
        lua.create_function(move |_, (path, label): (String, String)| {
            if path.is_empty()
                || path.len() > 256
                || label.is_empty()
                || label.len() > 128
                || label.chars().any(char::is_control)
            {
                return Err(lua::Error::external(
                    "Field path/label exceeds the drawer limits",
                ));
            }
            let path = ParsedPath::parse(&path)
                .map_err(|error| lua::Error::external(error.to_string()))?;
            scope.with_world(|world| {
                let root = scope.resolve_entity(world, &target)?;
                add(world, root, path, label).map_err(lua::Error::external)
            })
        })?,
    )?;
    fields.set_readonly(true);
    ctx.set("fields", fields)
}
fn add(world: &mut World, root: Entity, path: ParsedPath, label: String) -> Result<(), String> {
    let state = world.get::<State>(root).ok_or("Drawer was retired")?;
    if state.outcome.is_some() || state.controls.len() >= state.limit {
        return Err("Drawer composition is closed or exceeds its field budget (at most 32)".into());
    }
    if state.controls.iter().any(|field| {
        field
            .path
            .0
            .iter()
            .map(|part| &part.access)
            .eq(path.0.iter().map(|part| &part.access))
    }) {
        return Err("Duplicate native field path in drawer".into());
    }
    // Validate the native path/type before allocating any controls.
    let mut field = state.candidates.iter().find(|(candidate, _)| candidate.0.iter().map(|part| &part.access).eq(path.0.iter().map(|part| &part.access)))
        .map(|(_, field)| field.clone()).ok_or("Native field path is unavailable, unsupported or exceeds the reflected presentation limits")?;
    let namespace = format!("{}/field-{}", state.namespace, state.controls.len());
    let templates = state.templates.clone();
    let access = FieldAccess::ReadOnly {
        reason: "Luau drawer composition is pending".into(),
    };
    let mut row = PropertyRow::mount(world, root, &templates, &namespace)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        let mut value = ValueView::mount(world, row.editor(), &templates, &namespace)
            .map_err(|error| error.to_string())?;
        row.update(world, &label, None)
            .map_err(|error| error.to_string())?;
        value
            .update(world, &field.display(), &access)
            .map_err(|error| error.to_string())?;
        value
            .describe(world, &label, access.description())
            .map_err(|error| error.to_string())?;
        field.diagnostic = row.diagnostic();
        world.entity_mut(value.input()).insert(field);
        Ok(value)
    })();
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            row.retire(world);
            return Err(error);
        }
    };
    world
        .get_mut::<State>(root)
        .ok_or("Drawer was retired")?
        .controls
        .push(Control {
            path,
            label,
            row,
            value,
            choice: None,
        });
    world.trigger(DrawerChanged);
    Ok(())
}
