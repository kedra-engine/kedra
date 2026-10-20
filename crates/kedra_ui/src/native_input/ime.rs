use super::*;
use bevy::{
    text::{EditableText, PreeditCursor, TextEdit, TextReadWriteMode},
    ui_widgets::TextInput,
    window::Ime,
};

pub(super) fn events(world: &mut World, mut cursor: Local<MessageCursor<Ime>>) {
    // Always drain events, including those from retired windows or lost focus.
    let events: Vec<_> = cursor
        .read(world.resource::<Messages<Ime>>())
        .cloned()
        .collect();
    let Some((focused, window)) = editable_focus(world) else {
        return;
    };
    for event in events {
        let (source, edit) = match event {
            Ime::Preedit {
                window,
                value,
                cursor,
            } => (
                window,
                TextEdit::ImeSetCompose {
                    value: value.into(),
                    cursor: cursor.map(|(anchor, focus)| PreeditCursor { anchor, focus }),
                },
            ),
            Ime::Commit { window, value } => (
                window,
                TextEdit::ImeCommit {
                    value: value.into(),
                },
            ),
            Ime::Enabled { window } | Ime::Disabled { window } => {
                (window, TextEdit::clear_ime_compose())
            }
        };
        if source == window
            && let Some(mut text) = world.get_mut::<EditableText>(focused)
        {
            text.queue_edit(edit);
        }
    }
}

fn editable_focus(world: &mut World) -> Option<(Entity, Entity)> {
    let entity = world.resource::<InputFocus>().get()?;
    let window = owner(world, entity)?;
    (super::focused(world, window) == Some(entity)
        && world.get::<TextInput>(entity).is_some()
        && world.get::<TextReadWriteMode>(entity) == Some(&TextReadWriteMode::Editable))
    .then_some((entity, window))
}

pub(super) fn enable(world: &mut World) {
    let focused = world.resource::<InputFocus>().get();
    let enabled = editable_focus(world).map(|(_, window)| window);
    for (entity, mut window) in world.query::<(Entity, &mut Window)>().iter_mut(world) {
        let next = Some(entity) == enabled;
        if window.ime_enabled != next {
            window.ime_enabled = next;
        }
    }
    if enabled.is_none()
        && let Some(mut text) = focused.and_then(|e| world.get_mut::<EditableText>(e))
        && text.is_composing()
    {
        text.queue_edit(TextEdit::clear_ime_compose());
    }
}
pub(super) fn position(world: &mut World) {
    let Some((focused, window)) = editable_focus(world) else {
        return;
    };
    if !world.get::<Window>(window).is_some_and(|w| w.ime_enabled) {
        return;
    }
    let Some((text, node, transform, target)) = world
        .get::<EditableText>(focused)
        .zip(world.get::<ComputedNode>(focused))
        .zip(world.get::<UiGlobalTransform>(focused))
        .zip(world.get::<ComputedUiRenderTargetInfo>(focused))
        .map(|(((a, b), c), d)| (a, b, c, d))
    else {
        return;
    };
    let area = text.editor().ime_cursor_area();
    let local =
        Vec2::new(area.x0 as f32, area.y1 as f32) + node.content_box().min - text.viewport.offset;
    let position = transform.affine().transform_point2(local) * world.resource::<UiScale>().0
        / target.scale_factor();
    if position.is_finite()
        && let Some(mut window) = world.get_mut::<Window>(window)
    {
        window.ime_position = position;
    }
}
