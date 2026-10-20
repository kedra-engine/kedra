#![allow(clippy::unwrap_used, reason = "State and filesystem assertions")]
use crate::domain::*;

#[test]
fn editor_requires_a_prepared_project_and_cannot_bypass_an_inflight_request() {
    let mut model = StartupModel::default();
    assert!(!model.enter_editor());
    assert_eq!(model.phase(), Phase::Projects);
    assert!(model.project().is_none());
    assert!(model.begin(Intent::Open("Game".into())).is_some());
    assert!(!model.enter_editor());
    assert_eq!(model.phase(), Phase::Loading);
}

#[test]
fn escape_closes_entry_even_when_a_blank_click_cleared_input_focus() {
    use bevy::{
        input::{
            ButtonState,
            keyboard::{Key, KeyboardInput},
        },
        input_focus::{FocusedInput, InputFocus},
        prelude::*,
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<crate::model::ProjectManager>()
        .init_resource::<crate::model::EntryForm>()
        .init_resource::<NextState<crate::AppPhase>>()
        .init_resource::<InputFocus>()
        .add_systems(PostUpdate, crate::actions::native::apply)
        .add_observer(crate::actions::submit_key);
    let window = app.world_mut().spawn_empty().id();
    let modal = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<crate::model::ProjectManager>()
        .modal = Some(modal);
    app.world_mut()
        .resource_mut::<crate::model::EntryForm>()
        .mode = Some(EntryMode::Create);
    app.world_mut().trigger(FocusedInput::new(
        window,
        KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        },
        window,
    ));
    app.update();
    assert!(
        app.world()
            .resource::<crate::model::ProjectManager>()
            .modal
            .is_none()
    );
    assert!(app.world().get_entity(modal).is_err());
}

#[test]
fn form_validates_without_writing_and_retains_unicode() {
    let dir = tempfile::tempdir().unwrap();
    let mut form = EntryDraft {
        mode: Some(EntryMode::Create),
        name: "日本語 Кедра".into(),
        directory: dir.path().display().to_string(),
        ..Default::default()
    };
    let Intent::Create { parent, name } = entry_intent(&form, None).unwrap() else {
        panic!("Expected create")
    };
    assert_eq!(parent, dir.path());
    assert_eq!(name, "日本語 Кедра");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    form.name = "../escape".into();
    assert!(entry_intent(&form, None).is_err());
    form.mode = Some(EntryMode::Open);
    assert!(entry_intent(&form, None).is_ok());
    form.directory = "relative".into();
    assert!(entry_intent(&form, None).is_err());
    form.directory = "~/Projects".into();
    assert!(entry_intent(&form, None).is_err());
    assert_eq!(
        entry_intent(&form, Some(dir.path())).unwrap().root(),
        dir.path().join("Projects")
    );
    form.mode = None;
    assert!(entry_intent(&form, Some(dir.path())).is_err());
}

#[test]
fn duplicate_activations_and_stale_completions_cannot_replace_request() {
    let mut model = StartupModel::default();
    let intent = Intent::Open("Game".into());
    let generation = model.begin(intent.clone()).unwrap();
    assert_eq!(model.phase(), Phase::Loading);
    assert!(model.begin(Intent::Open("Other game".into())).is_none());
    assert!(!model.complete(generation.wrapping_sub(1), Err("stale".into())));
    assert!(model.loading_error().is_none());
    assert_eq!(model.intent, Some(intent));
    assert!(!model.back_to_projects());
    assert!(!model.enter_editor());
    assert!(model.complete(generation, Err("Cannot read manifest".into())));
    assert!(model.retry());
    assert!(model.loading_error().is_none());
    let retry_generation = model.generation;
    assert_ne!(retry_generation, generation);
    assert!(!model.complete(generation, Err("late first completion".into())));
    assert!(model.complete(retry_generation, Err("Cannot read manifest".into())));
    assert!(!model.complete(generation, Err("duplicate".into())));
    assert!(model.project().is_none());
    assert!(!model.enter_editor());
    assert!(model.back_to_projects());
    assert_eq!(model.phase(), Phase::Projects);
    assert!(model.loading_error().is_none());
    let next = model.begin(Intent::Open("Next".into())).unwrap();
    assert_ne!(generation, next);
    assert!(!model.complete(generation, Err("late failure".into())));
}

#[test]
fn failed_creation_is_not_replayed_by_retry() {
    let mut model = StartupModel::default();
    let generation = model
        .begin(Intent::Create {
            parent: "/projects".into(),
            name: "Game".into(),
        })
        .unwrap();
    assert!(model.complete(generation, Err("Destination already exists".into())));
    assert!(!model.retry());
    assert_eq!(model.loading_error(), Some("Destination already exists"));
    assert!(model.back_to_projects());
}

#[test]
fn project_publishes_only_after_validation_and_native_readiness() {
    let dir = tempfile::tempdir().unwrap();
    let project = kedra_project::create(dir.path(), "Game").unwrap();
    let mut model = StartupModel::default();
    let generation = model.begin(Intent::Open(project.root.clone())).unwrap();
    assert!(model.project().is_none());
    assert!(model.complete(generation, Ok(project.clone())));
    assert_eq!(model.phase(), Phase::Loading);
    assert_eq!(model.project(), Some(&project));
    assert!(model.enter_editor());
    assert_eq!(model.phase(), Phase::Editor);
    assert!(!model.enter_editor());
    assert!(model.back_to_projects());
    assert_eq!(model.phase(), Phase::Projects);
    assert!(model.project().is_none());
    assert_eq!(kedra_project::open(&project.root).unwrap(), project);
}
