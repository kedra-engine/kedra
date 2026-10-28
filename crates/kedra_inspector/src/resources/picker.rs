//! A searchable native popup. Search/highlight never assigns; click/Enter does.
use super::*;
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
};
#[derive(Component)]
struct Popup {
    owner: Entity,
    reference: Reference,
    search: Entity,
    rows: Vec<CommandView>,
    options: Vec<Option<String>>,
    query: String,
    highlighted: usize,
}
#[derive(Component, Clone)]
struct Item {
    popup: Entity,
    owner: Entity,
    binding: Arc<ControlBinding>,
    path: Option<String>,
}
#[derive(Component)]
struct Search(Entity);
pub(super) fn install(app: &mut App) {
    app.add_observer(activate)
        .add_observer(keyboard)
        .add_observer(search_read)
        .add_observer(menu);
    app.add_systems(Update, update);
}
fn activate(event: On<bevy::ui_widgets::Activate>, mut commands: Commands) {
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        if let Some(item) = world.get::<Item>(entity).cloned() {
            assign(world, item.owner, item.binding, item.path);
            if let Ok(popup) = world.get_entity_mut(item.popup) {
                popup.despawn();
            }
            return;
        }
        let Some(reference) = world.get::<Reference>(entity).cloned() else {
            return;
        };
        if !eligible(world, entity, &reference.binding) {
            return;
        }
        let old = world
            .query::<(Entity, &Popup)>()
            .iter(world)
            .filter(|(_, popup)| popup.owner == entity)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        if !old.is_empty() {
            for popup in old {
                world.despawn(popup);
            }
            return;
        }
        let Ok(popup) = kedra_ui::popup::attach(
            world,
            entity,
            "property-resource-menu",
            &format!("{}/menu", reference.scope),
        ) else {
            return;
        };
        let result: Result<(), String> = (|| {
            let mut search = ValueView::mount(
                world,
                popup,
                &reference.templates,
                &format!("{}/search", reference.scope),
            )
            .map_err(|e| e.to_string())?;
            search
                .update(world, "", &FieldAccess::Editable)
                .map_err(|e| e.to_string())?;
            world.entity_mut(search.input()).insert(Search(popup));
            world.entity_mut(popup).insert(Popup {
                owner: entity,
                reference,
                search: search.input(),
                rows: Vec::new(),
                options: Vec::new(),
                query: "\0".into(),
                highlighted: 0,
            });
            Ok(())
        })();
        if result.is_err() {
            world.despawn(popup);
        }
    });
}
fn assign(world: &mut World, entity: Entity, binding: Arc<ControlBinding>, path: Option<String>) {
    if eligible(world, entity, &binding) {
        world.trigger(ReferenceEvent {
            entity,
            binding,
            path,
        });
    }
}
fn update(world: &mut World) {
    let popups = world
        .query_filtered::<Entity, With<Popup>>()
        .iter(world)
        .collect::<Vec<_>>();
    for entity in popups {
        let Some(mut popup) = world.entity_mut(entity).take::<Popup>() else {
            continue;
        };
        if !eligible(world, popup.owner, &popup.reference.binding) {
            world.despawn(entity);
            continue;
        }
        if world
            .get::<kedra_ui::text_input::TextSession>(popup.search)
            .is_none()
            && let Some(text) = world.get::<bevy::text::EditableText>(popup.search)
            && text.pending_edits.is_empty()
            && let Ok(session) = kedra_ui::text_input::TextSession::attach(text, 256)
        {
            world.entity_mut(popup.search).insert(session);
        }
        let query = world
            .get::<bevy::text::EditableText>(popup.search)
            .map(|text| text.value().to_string())
            .unwrap_or_default();
        if query != popup.query {
            popup.query = query;
            popup.options = options(&popup.reference, &popup.query);
            popup.highlighted = 0;
        }
        let result: Result<(), String> = (|| {
            for index in 0..popup.options.len() {
                if popup.rows.len() <= index {
                    let row = CommandView::mount(
                        world,
                        entity,
                        &popup.reference.templates,
                        &format!("{}/menu/{index}", popup.reference.scope),
                    )
                    .map_err(|e| e.to_string())?;
                    world.entity_mut(row.root()).insert(
                        bevy::input_focus::tab_navigation::TabIndex(index as i32 + 1),
                    );
                    popup.rows.push(row);
                }
                let row = &popup.rows[index];
                row.update(
                    world,
                    popup.options[index].as_deref().unwrap_or("Clear"),
                    true,
                )
                .map_err(|e| e.to_string())?;
                kedra_ui::docking::class(world, row.root(), "hidden", false);
                kedra_ui::docking::class(
                    world,
                    row.root(),
                    "highlighted",
                    popup.highlighted == index,
                );
                world.entity_mut(row.root()).insert(Item {
                    popup: entity,
                    owner: popup.owner,
                    binding: popup.reference.binding.clone(),
                    path: popup.options[index].clone(),
                });
            }
            for row in popup.rows.iter().skip(popup.options.len()) {
                kedra_ui::docking::class(world, row.root(), "hidden", true);
                world.entity_mut(row.root()).remove::<Item>();
            }
            Ok(())
        })();
        if result.is_err() {
            world.despawn(entity);
        } else {
            world.entity_mut(entity).insert(popup);
        }
    }
}
fn keyboard(
    mut event: On<FocusedInput<KeyboardInput>>,
    searches: Query<&Search>,
    mut commands: Commands,
) {
    let Ok(search) = searches.get(event.focused_entity) else {
        return;
    };
    if !matches!(
        event.input.logical_key,
        Key::ArrowUp | Key::ArrowDown | Key::Enter | Key::Escape
    ) {
        return;
    }
    let popup = search.0;
    let input = event.focused_entity;
    let key = event.input.logical_key.clone();
    let window = event.input.window;
    let pressed = event.input.state.is_pressed() && !event.input.repeat;
    event.propagate(false);
    if !pressed {
        return;
    }
    commands.queue(move |world: &mut World| {
        let composing = world.get::<bevy::text::EditableText>(input).is_some_and(|text| text.is_composing()
            || text.pending_edits.iter().any(|edit| matches!(edit, bevy::text::TextEdit::ImeSetCompose { .. } | bevy::text::TextEdit::ImeCommit { .. })))
            || world.get_resource::<Messages<bevy::window::Ime>>().is_some_and(|messages| messages.iter_current_update_messages().any(|ime|
                matches!(ime, bevy::window::Ime::Preedit { window: target, .. } | bevy::window::Ime::Commit { window:target, .. } if *target == window)));
        if composing { return; }
        if key == Key::Escape { if let Ok(popup) = world.get_entity_mut(popup) { popup.despawn(); } return; }
        let request = match key { Key::ArrowUp => 0, Key::ArrowDown => 1, _ => 2 };
        if let Ok((mut text, mut session)) = world.query::<(&mut bevy::text::EditableText, &mut kedra_ui::text_input::TextSession)>().get_mut(world, input) {
            session.request(&mut text, kedra_ui::text_input::Command::Read(request));
        }
    });
}
fn options(reference: &Reference, query: &str) -> Vec<Option<String>> {
    let query = query.to_lowercase();
    let mut options = Vec::new();
    if reference.optional && (query.is_empty() || "clear".contains(&query)) {
        options.push(None);
    }
    options.extend(
        reference
            .choices
            .iter()
            .filter(|path| path.to_lowercase().contains(&query))
            .take(128)
            .cloned()
            .map(Some),
    );
    options
}
fn search_read(
    event: On<kedra_ui::text_input::InputEvent>,
    searches: Query<&Search>,
    mut commands: Commands,
) {
    let Ok(search) = searches.get(event.entity) else {
        return;
    };
    let kedra_ui::text_input::Outcome::Read { request, text } = &event.outcome else {
        return;
    };
    let (popup, request, query) = (search.0, *request, text.clone());
    commands.queue(move |world: &mut World| {
        let Some(mut state) = world.get_mut::<Popup>(popup) else {
            return;
        };
        // Use the text at the native ordered boundary, not yesterday's rendered
        // result rows. Same-batch typing/paste followed by Enter cannot assign
        // a path excluded by the new query.
        if state.query != query {
            state.options = options(&state.reference, &query);
            state.query = query;
            state.highlighted = 0;
        }
        if request == 0 {
            state.highlighted = state.highlighted.saturating_sub(1);
        } else if request == 1 {
            state.highlighted = (state.highlighted + 1).min(state.options.len().saturating_sub(1));
        } else if let Some(path) = state.options.get(state.highlighted).cloned() {
            let (owner, binding) = (state.owner, state.reference.binding.clone());
            assign(world, owner, binding, path);
            if let Ok(popup) = world.get_entity_mut(popup) {
                popup.despawn();
            }
        }
    });
}
fn menu(event: On<bevy::ui_widgets::MenuEvent>, popups: Query<&Popup>, mut commands: Commands) {
    let Ok(popup) = popups.get(event.source) else {
        return;
    };
    if matches!(event.action, bevy::ui_widgets::MenuAction::CloseAll) {
        let (entity, owner) = (event.source, popup.owner);
        commands.queue(move |world: &mut World| {
            if let Ok(popup) = world.get_entity_mut(entity) {
                popup.despawn();
            }
            if world.entities().contains(owner)
                && let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>()
            {
                focus.set(owner, bevy::input_focus::FocusCause::Navigated);
            }
        });
    }
}
