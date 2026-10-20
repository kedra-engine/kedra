use super::*;
use crate::{actions, model::*, screens};
use bevy::text::EditableText;
use bevy_flair::style::components::ClassList;

#[test]
fn production_manager_forms_search_and_cancel_preserve_feature_ownership() {
    let mut app = app();
    projects(&mut app);
    actions::dispatch(app.world_mut(), Action::Search, None);
    let search_host = element(app.world_mut(), "projects.search_host");
    assert!(
        !app.world()
            .get::<ClassList>(search_host)
            .unwrap()
            .contains("hidden")
    );
    screens::set_field(app.world_mut(), Field::Search, "日本語");
    app.update();
    assert_eq!(app.world().resource::<ProjectManager>().filter, "日本語");
    for (action, screen) in [(Action::New, "entry.create"), (Action::Open, "entry.open")] {
        let focus = element(app.world_mut(), "projects.new");
        actions::dispatch(app.world_mut(), action, Some(focus));
        app.update();
        let modal = app.world().resource::<ProjectManager>().modal.unwrap();
        assert_eq!(
            app.world().get::<kedra_ui::ScreenId>(modal).unwrap().0,
            screen
        );
        let name = element(app.world_mut(), "entry.name");
        assert!(app.world().get::<EditableText>(name).is_some());
        screens::set_field(app.world_mut(), Field::Directory, "relative/invalid");
        app.update();
        actions::dispatch(app.world_mut(), Action::Submit, None);
        assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
        assert!(
            app.world()
                .resource::<EntryForm>()
                .error
                .contains("absolute")
        );
        app.update();
        let error = element(app.world_mut(), "entry.error");
        assert!(!app.world().get::<Text>(error).unwrap().0.is_empty());
        assert!(
            !app.world()
                .get::<ClassList>(error)
                .unwrap()
                .contains("hidden")
        );
        actions::dispatch(app.world_mut(), Action::Cancel, None);
        assert!(!app.world().entities().contains(modal));
        assert!(app.world().resource::<EntryForm>().mode.is_none());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
    }
}

#[test]
fn cancelling_before_hydration_retires_the_request_without_mounting_a_stale_form() {
    let mut app = app();
    projects(&mut app);
    actions::dispatch(app.world_mut(), Action::New, None);
    let request = app.world().resource::<ProjectManager>().modal.unwrap();
    assert!(app.world().get::<kedra_ui::ScreenId>(request).is_none());
    actions::dispatch(app.world_mut(), Action::Cancel, None);
    app.update();
    assert!(!app.world().entities().contains(request));
    assert!(app.world().resource::<ProjectManager>().modal.is_none());
    assert!(app.world().resource::<EntryForm>().mode.is_none());
    assert!(
        app.world_mut()
            .query::<&kedra_ui::ScreenId>()
            .iter(app.world())
            .all(|screen| !screen.0.starts_with("entry."))
    );
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_none()
    );
}

#[test]
fn production_recent_rows_filter_and_restore_without_duplicate_ids() {
    let directory = tempfile::tempdir().unwrap();
    let first = kedra_project::create(directory.path(), "日本語").unwrap();
    let second = kedra_project::create(directory.path(), "Other").unwrap();
    let mut app = app();
    {
        let mut manager = app.world_mut().resource_mut::<ProjectManager>();
        manager.history.remember(&first);
        manager.history.remember(&second);
    }
    projects(&mut app);
    let first_row = element(app.world_mut(), "projects.row.0");
    let second_row = element(app.world_mut(), "projects.row.1");
    screens::set_field(app.world_mut(), Field::Search, "日本語");
    app.update();
    assert!(!app.world().entities().contains(first_row));
    assert_eq!(element(app.world_mut(), "projects.row.0"), second_row);
    screens::set_field(app.world_mut(), Field::Search, "no match at all");
    app.update();
    let message = element(app.world_mut(), "projects.empty_message");
    assert_eq!(
        app.world().get::<Text>(message).unwrap().0,
        "No matching projects"
    );
    screens::set_field(app.world_mut(), Field::Search, "");
    app.update();
    let mut ids = std::collections::BTreeSet::new();
    for id in app
        .world_mut()
        .query::<&kedra_ui::ElementId>()
        .iter(app.world())
    {
        assert!(ids.insert(id.0.clone()), "duplicate {}", id.0);
    }
    element(app.world_mut(), "projects.row.1");
}
