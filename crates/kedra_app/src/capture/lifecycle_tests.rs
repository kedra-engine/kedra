use super::*;
use bevy::render::view::screenshot::{CapturedScreenshots, ScreenshotPlugin};
use std::sync::{Arc, Mutex, mpsc};

#[test]
fn cancelled_native_capture_accepts_deferred_completion_without_publication()
-> Result<(), Box<dyn std::error::Error>> {
    for window_lost in [false, true] {
        let (mut app, directory, window) = tests::host();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            ScreenshotPlugin,
        ));
        let (sender, receiver) = mpsc::channel();
        app.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(receiver))));
        let ids = [
            tests::admit(app.world_mut(), window),
            tests::admit(app.world_mut(), window),
        ];
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<CaptureOwned>>()
            .single(app.world())?;
        app.world_mut().entity_mut(entity).insert(Capturing);
        if window_lost {
            app.world_mut().despawn(window);
        } else {
            app.world_mut()
                .resource_mut::<bevy::diagnostic::FrameCount>()
                .0 = 181;
        }
        advance(app.world_mut());
        advance(app.world_mut());
        sender.send((entity, tests::image()))?;
        // Real native trigger queues Captured insertion before its event. An
        // observer-only stale-event test never exercises this command boundary.
        app.update();
        assert!(app.world().get::<Captured>(entity).is_some());
        for id in ids {
            assert_eq!(
                app.world()
                    .resource::<crate::operations::Operations>()
                    .get(id)
                    .map(|op| &op.state),
                Some(&crate::operations::OperationState::Failed)
            );
            assert!(!directory.path().join(format!("{id}.png")).exists());
        }
        app.update();
        assert!(!app.world().entities().contains(entity));
        assert!(app.world().resource::<PendingCaptures>().0.is_empty());
        // Terminal cancelled receipts cannot poison a later independent readback.
        let current_window = app.world_mut().spawn(Window::default()).id();
        let current_id = tests::admit(app.world_mut(), current_window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 += 6;
        advance(app.world_mut());
        let current = app
            .world_mut()
            .query_filtered::<Entity, With<CaptureOwned>>()
            .single(app.world())?;
        app.world_mut().entity_mut(current).insert(Capturing);
        sender.send((current, tests::image()))?;
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(current_id)
                .map(|op| &op.state),
            Some(&crate::operations::OperationState::Succeeded)
        );
        assert!(directory.path().join(format!("{current_id}.png")).exists());
        app.update();
        assert!(!app.world().entities().contains(current));
    }
    Ok(())
}

#[test]
fn cancelled_noncompleting_owned_requests_remain_bounded_and_reject_new_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut app, _directory, window) = tests::host();
    for index in 0..8 {
        tests::admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 += 6;
        advance(app.world_mut());
        let entity = app
            .world_mut()
            .query_filtered::<Entity, (With<CaptureOwned>, Without<Capturing>)>()
            .single(app.world())?;
        app.world_mut().entity_mut(entity).insert(Capturing);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 += 181;
        advance(app.world_mut());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<CaptureOwned>>()
                .iter(app.world())
                .count(),
            index + 1
        );
        assert!(app.world().resource::<PendingCaptures>().0.is_empty());
    }
    let error = request(
        In(Some(
            json!({"session":"test","id":Uuid::new_v4(),"window":window.to_bits()}),
        )),
        app.world_mut(),
    )
    .err()
    .ok_or("capture admission exceeded native capacity")?;
    assert!(error.message.contains("unfinished native readbacks"));
    Ok(())
}

#[test]
fn a_batch_of_admitted_operations_cannot_overfill_retained_native_capacity() {
    let (mut app, _directory, window) = tests::host();
    for _ in 0..7 {
        app.world_mut()
            .spawn((Screenshot::window(window), Capturing, CaptureOwned));
    }
    let mut ids = Vec::new();
    for _ in 0..8 {
        let window = app.world_mut().spawn(Window::default()).id();
        ids.push(tests::admit(app.world_mut(), window));
    }
    app.world_mut()
        .resource_mut::<bevy::diagnostic::FrameCount>()
        .0 = 6;
    advance(app.world_mut());
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<CaptureOwned>>()
            .iter(app.world())
            .count(),
        8
    );
    assert_eq!(app.world().resource::<PendingCaptures>().0.len(), 1);
    let operations = app.world().resource::<crate::operations::Operations>();
    assert_eq!(
        ids.iter()
            .filter(|id| operations.get(**id).is_some_and(
                |operation| operation.state == crate::operations::OperationState::Failed
            ))
            .count(),
        7
    );
}
