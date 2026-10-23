//! Actual availability task plus retained production rows, not an alternate list.
#![allow(clippy::unwrap_used, reason = "Recent-project binding assertions")]
use super::*;
use crate::test_support::{app, projects};
use bevy::ecs::system::RunSystemOnce;
use std::time::{Duration, Instant};

#[test]
fn missing_selected_project_falls_back_and_missing_row_offers_locate_not_open() {
    let directory = tempfile::tempdir().unwrap();
    let present = kedra_project::create(directory.path(), "Present").unwrap();
    let missing = kedra_project::create(directory.path(), "Removed").unwrap();
    std::fs::remove_file(missing.root.join(kedra_project::MANIFEST_FILE)).unwrap();
    let mut app = app();
    {
        let mut manager = app.world_mut().resource_mut::<ProjectManager>();
        manager.history.remember(&present);
        manager.history.remember(&missing);
    }
    app.world_mut().resource_mut::<RecentView>().selected = Some(missing.root.clone());
    projects(&mut app);
    // Finish the real filesystem check, then consume it in the production poller.
    // This has a bounded watchdog, never a timing-based success assertion.
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<RecentView>()
        .task
        .as_ref()
        .is_some_and(|task| !task.is_finished())
    {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    app.world_mut().run_system_once(poll).unwrap();
    sync(app.world_mut());
    let recent = app.world().resource::<RecentView>();
    assert_eq!(recent.selected.as_ref(), Some(&present.root));
    assert!(recent.missing.contains(&missing.root));
    let row = &recent.rows[&missing.root];
    let missing_indicator = row.element("missing").unwrap();
    let open = row.element("open").unwrap();
    let locate = row.element("locate").unwrap();
    assert!(
        !app.world()
            .get::<ClassList>(missing_indicator)
            .unwrap()
            .contains("hidden")
    );
    assert!(
        app.world()
            .get::<ClassList>(open)
            .unwrap()
            .contains("hidden")
    );
    assert!(
        matches!(app.world().get::<Action>(locate), Some(Action::Locate(path)) if path == &missing.root)
    );
    let selected = recent.rows[&present.root].root;
    assert!(app.world().get::<bevy::ui::Selected>(selected).is_some());
    let before = recent.rows.len();
    app.world_mut()
        .resource_mut::<ProjectManager>()
        .model
        .begin(Intent::Open(present.root.clone()));
    app.world_mut().run_system_once(poll).unwrap();
    assert_eq!(app.world().resource::<RecentView>().rows.len(), before);
}
