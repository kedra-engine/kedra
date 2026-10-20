use crate::{jobs, model::*, screens};
pub(crate) mod native;
use bevy::{
    input_focus::{FocusedInput, InputFocus},
    prelude::*,
    ui_widgets::{Activate, RequestClose},
};

pub(crate) fn submit_key(
    mut event: On<FocusedInput<bevy::input::keyboard::KeyboardInput>>,
    fields: Query<&Field>,
    text: Query<&bevy::text::EditableText>,
    manager: Res<ProjectManager>,
    ime: Option<Res<Messages<bevy::window::Ime>>>,
    mut commands: Commands,
) {
    let input = &event.input;
    // Inspect the ordered retained message batch without consuming it. Every
    // target in this event's bubble chain must see the same composition guard,
    // even if another native observer re-enables propagation at the field.
    let queued_composition = ime.as_ref().is_some_and(|messages| {
        messages
            .get_cursor()
            .read(messages)
            .fold(false, |composing, ime| match ime {
                bevy::window::Ime::Preedit { window, value, .. } if *window == input.window => {
                    !value.is_empty()
                }
                bevy::window::Ime::Commit { window, .. }
                | bevy::window::Ime::Disabled { window }
                | bevy::window::Ime::Enabled { window }
                    if *window == input.window =>
                {
                    false
                }
                _ => composing,
            })
    });
    if matches!(input.key_code, KeyCode::Enter | KeyCode::Escape)
        && (text
            .get(event.original_event_target())
            .is_ok_and(|text| text.is_composing())
            // IME messages are converted to TextEdits after focused keyboard
            // dispatch. Escape can clear focus before that conversion runs.
            || queued_composition)
    {
        event.propagate(false);
        return;
    }
    if input.state == bevy::input::ButtonState::Pressed
        && !input.repeat
        && input.key_code == KeyCode::Escape
        && manager.modal.is_some()
    {
        // Blank dialog clicks can leave native InputFocus empty. The window
        // still owns the modal, so Escape must cancel from that fallback target.
        event.propagate(false);
        let source = event.original_event_target();
        let owner = native::owner(&manager);
        commands.queue(move |world: &mut World| native::key(world, Action::Cancel, source, owner));
        return;
    }
    if input.state == bevy::input::ButtonState::Pressed
        && !input.repeat
        && input.key_code == KeyCode::Enter
        && fields
            .get(event.event_target())
            .is_ok_and(|field| matches!(field, Field::Name | Field::Directory))
    {
        event.propagate(false);
        let source = event.original_event_target();
        let owner = native::owner(&manager);
        commands.queue(move |world: &mut World| native::key(world, Action::Submit, source, owner));
    }
}

pub(crate) fn activated(
    event: On<Activate>,
    actions: Query<&Action>,
    manager: Res<ProjectManager>,
    mut commands: Commands,
) {
    if let Ok(action) = actions.get(event.entity) {
        let action = action.clone();
        let source = event.entity;
        let owner = native::owner(&manager);
        commands
            .queue(move |world: &mut World| native::request(world, action, Some(source), owner));
    }
}

pub(crate) fn request_close(
    mut event: On<RequestClose>,
    manager: Res<ProjectManager>,
    mut commands: Commands,
) {
    event.propagate(false);
    let owner = native::owner(&manager);
    commands.queue(move |world: &mut World| native::request(world, Action::Cancel, None, owner));
}

pub(crate) fn dispatch(world: &mut World, action: Action, source: Option<Entity>) {
    if world
        .get_resource::<crate::boot::Boot>()
        .is_some_and(|boot| !boot.complete)
    {
        return;
    }
    // Multiple native activations in one frame cannot queue multiple transitions.
    if !matches!(
        world.resource::<NextState<AppPhase>>(),
        NextState::Unchanged
    ) {
        return;
    }
    let phase = match world.resource::<ProjectManager>().phase() {
        Phase::Projects => AppPhase::Projects,
        Phase::Loading => AppPhase::Loading,
        Phase::Editor => AppPhase::Editor,
    };
    match action {
        Action::LoadingLog if phase == AppPhase::Loading => crate::loading::toggle_log(world),
        Action::Search
            if phase == AppPhase::Projects
                && world.resource::<ProjectManager>().modal.is_none() =>
        {
            for mut classes in world
                .query_filtered::<&mut bevy_flair::style::components::ClassList, With<screens::SearchHost>>()
                .iter_mut(world)
            {
                classes.toggle("hidden");
            }
        }
        Action::New | Action::Open if phase == AppPhase::Projects => {
            if world.contains_resource::<jobs::PendingJob>()
                || world.resource::<ProjectManager>().modal.is_some()
            {
                return;
            }
            let mut form = world.resource_mut::<EntryForm>();
            form.mode = Some(if matches!(action, Action::New) {
                EntryMode::Create
            } else {
                EntryMode::Open
            });
            form.error.clear();
            form.restore_focus = source;
            crate::entry::mount(world);
        }
        Action::Cancel if phase == AppPhase::Projects => {
            if world.contains_resource::<jobs::PendingJob>() {
                return;
            }
            close_entry(world);
        }
        Action::Submit if phase == AppPhase::Projects => {
            if world.contains_resource::<jobs::PendingJob>() {
                return;
            }
            if native::unsettled(world) {
                world.resource_mut::<EntryForm>().error = "Wait for text editing or composition to finish before submitting.".into();
                return;
            }
            screens::capture_fields(world);
            match entry_intent(world.resource::<EntryForm>()) {
                Ok(intent) => {
                    if let Err(error) = request_project(world, intent) {
                        world.resource_mut::<EntryForm>().error = error;
                    }
                }
                Err(error) => world.resource_mut::<EntryForm>().error = error,
            }
        }
        Action::Browse if phase == AppPhase::Projects => {
            if world.resource::<EntryForm>().mode.is_some()
                && !world.contains_resource::<jobs::PendingJob>()
            {
                screens::capture_fields(world);
                jobs::browse(world);
            }
        }
        Action::Select(root) if phase == AppPhase::Projects => {
            if world.resource::<ProjectManager>().modal.is_none()
                && !world.contains_resource::<jobs::PendingJob>()
            {
                world.resource_mut::<crate::recent::RecentView>().selected = Some(root);
                world.resource_mut::<ProjectManager>().list_dirty=true;
            }
        }
        Action::Locate(root) if phase == AppPhase::Projects => {
            if world.resource::<ProjectManager>().modal.is_none()
                && !world.contains_resource::<jobs::PendingJob>()
            {
                let mut form = world.resource_mut::<EntryForm>();
                form.directory = crate::recent::display_path(&root);
                form.mode = Some(EntryMode::Open);
                form.error.clear();
                form.restore_focus = source;
                crate::entry::mount(world);
            }
        }
        Action::OpenRecent(root) if phase == AppPhase::Projects => {
            if world.resource::<ProjectManager>().modal.is_none()
                && !world.contains_resource::<jobs::PendingJob>()
                && let Err(error) = request_project(world, Intent::Open(root))
            {
                world.resource_mut::<ProjectManager>().notice = error;
            }
        }
        Action::Back if phase == AppPhase::Loading => {
            // Back is offered only on a completed error. Do not claim that
            // cancellation can roll back a filesystem commit in progress.
            if world.resource::<ProjectManager>().loading_error().is_some() {
                world.resource_mut::<ProjectManager>().back_to_projects();
            }
        }
        Action::Retry if phase == AppPhase::Loading => {
            if !world.contains_resource::<jobs::PendingJob>()
                && world.resource_mut::<ProjectManager>().retry()
            {
                jobs::prepare(world);
            }
        }
        Action::Projects if phase == AppPhase::Editor => {
            world.resource_mut::<ProjectManager>().back_to_projects();
        }
        _ => {}
    }
}

pub(crate) fn request_project(world: &mut World, intent: Intent) -> Result<(), String> {
    if world
        .get_resource::<crate::boot::Boot>()
        .is_some_and(|boot| !boot.complete)
    {
        return Err("Wait for engine startup to complete".into());
    }
    if world.resource::<ProjectManager>().phase() != Phase::Projects
        || world.contains_resource::<jobs::PendingJob>()
    {
        return Err("Return to projects and wait for the current operation first".into());
    }
    if let Intent::Create { name, .. } = &intent {
        kedra_project::validate_name(name).map_err(|e| e.to_string())?;
    }
    world.resource_mut::<EntryForm>().error.clear();
    jobs::preflight(world, intent);
    Ok(())
}

pub(crate) fn begin(world: &mut World, intent: Intent) {
    if world
        .resource_mut::<ProjectManager>()
        .model
        .begin(intent)
        .is_none()
    {
        return;
    }
    close_entry(world);
}

fn close_entry(world: &mut World) {
    let modal = world.resource_mut::<ProjectManager>().modal.take();
    if let Some(modal) = modal {
        world.despawn(modal);
    }
    let mut form = world.resource_mut::<EntryForm>();
    form.mode = None;
    let focus = form.restore_focus.take();
    if let Some(entity) = focus.filter(|entity| world.get_entity(*entity).is_ok()) {
        world
            .resource_mut::<InputFocus>()
            .set(entity, bevy::input_focus::FocusCause::Navigated);
    }
}

#[cfg(test)]
mod tests;
