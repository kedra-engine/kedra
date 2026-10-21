//! Explicit update exit admission, separate from document Save authority.
use bevy::prelude::*;

pub(super) fn ready(world: &mut World) -> Result<(), String> {
    let manager = world
        .get_resource::<crate::model::ProjectManager>()
        .ok_or("Project session is unavailable; restart was not authorized")?;
    if manager.modal.is_some() || world.contains_resource::<crate::jobs::PendingJob>() {
        return Err("Finish the current project operation before updating".into());
    }
    if world
        .get_resource::<Messages<AppExit>>()
        .is_some_and(|messages| !messages.is_empty())
    {
        return Err("Application exit was already requested; no update will be installed".into());
    }
    if world
        .query::<&bevy::text::EditableText>()
        .iter(world)
        .any(|text| {
            text.is_composing() || !text.pending_edits.is_empty() || text.pending_paste.is_some()
        })
    {
        return Err("Finish text input or composition before updating".into());
    }
    let workspace = world
        .get_resource::<crate::workspace::EditorWorkspace>()
        .ok_or("Editor workspace is unavailable; restart was not authorized")?;
    if !workspace.update_ready() {
        return Err("Finish source operations and save changes before updating; this editor cannot save dirty documents yet".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clean_readonly_workspace_can_exit_but_pending_input_cannot()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = crate::workspace::browser_test_app()?;
        app.init_resource::<crate::model::ProjectManager>();
        app.add_message::<AppExit>();
        ready(app.world_mut())?;
        let text = app
            .world_mut()
            .spawn(bevy::text::EditableText::default())
            .id();
        app.world_mut()
            .get_mut::<bevy::text::EditableText>(text)
            .ok_or("Text")?
            .pending_edits
            .push(bevy::text::TextEdit::Insert("unsaved input".into()));
        assert!(ready(app.world_mut()).is_err());
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        Ok(())
    }
    #[test]
    fn unknown_session_cannot_authorize_restart() {
        let mut world = World::new();
        assert!(ready(&mut world).is_err());
    }

    #[test]
    fn ordinary_close_is_not_permission_to_install_on_exit()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut app = crate::workspace::browser_test_app()?;
        app.init_resource::<crate::model::ProjectManager>()
            .add_message::<AppExit>();
        app.world_mut().write_message(AppExit::Success);
        assert!(ready(app.world_mut()).is_err());
        Ok(())
    }
}
