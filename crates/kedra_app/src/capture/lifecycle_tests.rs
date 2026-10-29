use super::*;
use bevy::render::view::screenshot::{CapturedScreenshots, ScreenshotPlugin};
use std::sync::{Arc, Mutex, mpsc};

#[test]
fn readback_deadline_uses_real_time_and_rejects_late_callbacks_before_the_next_sweep()
-> Result<(), Box<dyn std::error::Error>> {
    for expired in [false, true] {
        let (mut app, directory, window) = test_support::host();
        let id = test_support::admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        assert!(
            app.world_mut()
                .resource_mut::<Messages<bevy::window::RequestRedraw>>()
                .drain()
                .next()
                .is_some()
        );
        let screenshot = app
            .world_mut()
            .query_filtered::<Entity, With<CaptureOwned>>()
            .single(app.world())?;
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 10_000;
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(READBACK_TIMEOUT - Duration::from_nanos(1));
        advance(app.world_mut());
        assert!(
            app.world()
                .resource::<PendingCaptures>()
                .0
                .contains_key(&id)
        );
        if expired {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_nanos(1));
        }
        // Completion can run before Last's timeout sweep. It must enforce the
        // same deadline, rather than publishing a frame that already expired.
        app.world_mut().trigger(ScreenshotCaptured {
            entity: screenshot,
            image: test_support::image(),
        });
        app.world_mut().flush();
        let expected = if expired {
            crate::operations::OperationState::Failed
        } else {
            crate::operations::OperationState::Succeeded
        };
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(id)
                .map(|operation| &operation.state),
            Some(&expected)
        );
        assert_eq!(
            directory.path().join(format!("{id}.png")).exists(),
            !expired
        );
        assert!(app.world().get::<Window>(window).is_some());
        assert!(app.world().resource::<PendingCaptures>().0.is_empty());
        app.world_mut()
            .resource_mut::<Messages<bevy::window::RequestRedraw>>()
            .clear();
        advance(app.world_mut());
        assert!(
            app.world()
                .resource::<Messages<bevy::window::RequestRedraw>>()
                .is_empty()
        );
    }
    Ok(())
}

#[test]
fn capture_admission_retry_and_rejection_preserve_window_focus()
-> Result<(), Box<dyn std::error::Error>> {
    for focused in [false, true] {
        let (mut app, _directory, window) = test_support::host();
        app.world_mut()
            .get_mut::<Window>(window)
            .ok_or("target")?
            .focused = focused;
        let other = app
            .world_mut()
            .spawn(Window {
                focused: !focused,
                ..default()
            })
            .id();
        let id = test_support::admit(app.world_mut(), window);
        let params = json!({"session":"test","id":id,"window":window.to_bits()});
        request(In(Some(params)), app.world_mut()).map_err(|error| error.message)?;
        let rejected = Uuid::new_v4();
        assert!(
            request(
                In(Some(
                    json!({"session":"stale","id":rejected,"window":window.to_bits()})
                )),
                app.world_mut()
            )
            .is_err()
        );
        assert_eq!(
            app.world().get::<Window>(window).ok_or("target")?.focused,
            focused
        );
        assert_eq!(
            app.world().get::<Window>(other).ok_or("other")?.focused,
            !focused
        );
        assert!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(rejected)
                .is_none()
        );
        assert_eq!(app.world().resource::<PendingCaptures>().0.len(), 1);
    }
    Ok(())
}

#[test]
fn cancelled_native_capture_accepts_deferred_completion_without_publication()
-> Result<(), Box<dyn std::error::Error>> {
    for window_lost in [false, true] {
        let (mut app, directory, window) = test_support::host();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            ScreenshotPlugin,
        ));
        let (sender, receiver) = mpsc::channel();
        app.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(receiver))));
        let ids = [
            test_support::admit(app.world_mut(), window),
            test_support::admit(app.world_mut(), window),
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
                .resource_mut::<Time<Real>>()
                .advance_by(READBACK_TIMEOUT);
        }
        advance(app.world_mut());
        advance(app.world_mut());
        sender.send((entity, test_support::image()))?;
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
        let current_id = test_support::admit(app.world_mut(), current_window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 += 6;
        advance(app.world_mut());
        let current = app
            .world_mut()
            .query_filtered::<Entity, With<CaptureOwned>>()
            .single(app.world())?;
        app.world_mut().entity_mut(current).insert(Capturing);
        sender.send((current, test_support::image()))?;
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
    let (mut app, _directory, window) = test_support::host();
    for index in 0..8 {
        test_support::admit(app.world_mut(), window);
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
            .resource_mut::<Time<Real>>()
            .advance_by(READBACK_TIMEOUT);
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
    let (mut app, _directory, window) = test_support::host();
    for _ in 0..7 {
        app.world_mut()
            .spawn((Screenshot::window(window), Capturing, CaptureOwned));
    }
    let mut ids = Vec::new();
    for _ in 0..8 {
        let window = app.world_mut().spawn(Window::default()).id();
        ids.push(test_support::admit(app.world_mut(), window));
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
