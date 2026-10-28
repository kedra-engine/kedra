//! Typed single-asset drops reuse current source proof and the ordinary reference
//! event. Hover/rejection/outside drop/Escape never mutate source or consume Undo.
use super::*;
use bevy::picking::events::{PointerDragDrop, PointerDragLeave, PointerDragOver};
use kedra_inspector::resources::ReferenceTarget;
pub(super) fn install(app: &mut App) {
    app.add_observer(hover)
        .add_observer(leave)
        .add_observer(drop_asset);
}
fn hover(event: On<PointerDragOver>, targets: Query<&ReferenceTarget>, mut commands: Commands) {
    let entity = event.event_target();
    let Ok(target) = targets.get(entity).cloned() else {
        return;
    };
    let dragged = event.dragged;
    commands.queue(move |world: &mut World| {
        let payload = crate::drag::payload(world, dragged);
        let path = single_asset_path(payload.as_ref());
        let accepted = path
            .as_ref()
            .is_some_and(|path| target.accepts(world, path));
        kedra_ui::docking::class(world, entity, "resource-drop", path.is_some() && accepted);
        kedra_ui::docking::class(
            world,
            entity,
            "resource-drop-rejected",
            payload.is_some() && !accepted,
        );
    });
}
fn leave(event: On<PointerDragLeave>, targets: Query<&ReferenceTarget>, mut commands: Commands) {
    let entity = event.event_target();
    if targets.contains(entity) {
        commands.queue(move |world: &mut World| clear(world, entity));
    }
}
fn clear(world: &mut World, entity: Entity) {
    kedra_ui::docking::class(world, entity, "resource-drop", false);
    kedra_ui::docking::class(world, entity, "resource-drop-rejected", false);
}
fn drop_asset(
    mut event: On<PointerDragDrop>,
    targets: Query<&ReferenceTarget>,
    mut commands: Commands,
) {
    let entity = event.event_target();
    let Ok(target) = targets.get(entity).cloned() else {
        return;
    };
    event.propagate(false);
    let dragged = event.dropped;
    commands.queue(move |world: &mut World| {
        clear(world, entity);
        let payload = crate::drag::payload(world, dragged);
        let Some(path) = single_asset_path(payload.as_ref()) else {
            return;
        };
        if let Some(path) = target.drop_path(world, path) {
            // The source adapter rechecks the original binding, actual native type,
            // source/delivery membership and asset readiness before publication.
            crate::workspace::property_reference(
                world,
                ReferenceEvent {
                    entity: target.endpoint,
                    binding: target.binding,
                    path: Some(path),
                },
            );
        }
    });
}
fn single_asset_path(payload: Option<&crate::drag::DragPayload>) -> Option<&str> {
    let crate::drag::DragPayload::Assets(assets) = payload? else {
        return None;
    };
    let [asset] = assets.as_slice() else {
        return None;
    };
    Some(asset.path())
}
