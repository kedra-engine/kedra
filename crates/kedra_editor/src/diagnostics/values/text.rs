//! Independent ordinary-string consumer; never a project/source editing capability.
use bevy::{prelude::*, text::EditableText};
use kedra_ui::{
    authored::view::UiView,
    fields::{FieldTemplates, PropertyRow, TextField, TextFieldEvent, TextFieldOutcome},
    text_input::TextSession,
};
use std::collections::BTreeMap;

pub(super) struct Demo {
    owner: Entity,
    row: PropertyRow,
    surface: Entity,
    input: Entity,
    accepted: String,
    revision: u64,
}
pub(super) fn install(app: &mut App) {
    app.add_observer(outcome).add_systems(Update, ready);
}
pub(super) fn bind(
    world: &mut World,
    owner: Entity,
    parent: Entity,
    templates: &FieldTemplates,
) -> Result<Demo, String> {
    let mut row =
        PropertyRow::mount(world, parent, templates, "fixture/text").map_err(|e| e.to_string())?;
    let result = (|| {
        let template = crate::views::compiled(world, "editor://prefabs/field.prefab.ron")?;
        let view = UiView::mount(
            world,
            &template,
            Some(row.editor()),
            &BTreeMap::new(),
            "fixture/text/input",
            1.,
        )
        .map_err(|e| e.to_string())?;
        let instance = world
            .get::<kedra_scene::instance::PrefabInstance>(view.root)
            .ok_or("Missing field instance")?;
        let inputs: Vec<_> = instance
            .nodes
            .values()
            .copied()
            .filter(|entity| world.get::<EditableText>(*entity).is_some())
            .collect();
        let [input] = inputs.as_slice() else {
            return Err("Text field requires exactly one native input".into());
        };
        let input = *input;
        let accepted = "Draft value".to_string();
        let mut text = world
            .get_mut::<EditableText>(input)
            .ok_or("Missing native text")?;
        text.editor_mut().set_text(&accepted);
        let session = TextSession::attach(&text, 256)
            .map_err(|e| format!("Cannot admit diagnostic text: {e:?}"))?;
        world
            .entity_mut(input)
            .insert((session, kedra_ui::ElementId("fixture/text/value".into())))
            .remove::<bevy::ui_widgets::SelectAllOnFocus>();
        if let Some(mut a11y) = world.get_mut::<bevy::a11y::AccessibilityNode>(input) {
            a11y.set_label("Diagnostic text");
            a11y.set_description("Enter accepts; Escape cancels. Empty text is rejected. No project sources are edited.");
        }
        row.update(
            world,
            "Text draft",
            Some("Enter accepts; Escape cancels. Empty text is rejected."),
        )
        .map_err(|e| e.to_string())?;
        Ok((view.root, input, accepted))
    })();
    match result {
        Ok((surface, input, accepted)) => Ok(Demo {
            owner,
            row,
            surface,
            input,
            accepted,
            revision: 1,
        }),
        Err(error) => {
            row.retire(world);
            Err(error)
        }
    }
}
fn ready(world: &mut World) {
    let Some(fixture) = world.get_resource::<super::Fixture>() else {
        return;
    };
    let demo = &fixture.text;
    let input = demo.input;
    if !world.entities().contains(demo.owner) {
        return;
    }
    if world.get::<TextField>(input).is_some() {
        return;
    }
    let settled = world
        .get::<EditableText>(input)
        .zip(world.get::<TextSession>(input))
        .is_some_and(|(text, session)| {
            !session.pending() && text.pending_edits.is_empty() && !text.is_composing()
        });
    if settled && let Err(error) = TextField::attach(world, input, 1) {
        eprintln!("Diagnostic text binding failed: {error:?}");
        world.write_message(AppExit::error());
    }
}
fn outcome(event: On<TextFieldEvent>, mut commands: Commands) {
    let event = event.clone();
    commands.queue(move |world: &mut World| {
        let Some(fixture) = world.get_resource::<super::Fixture>() else {
            return;
        };
        let demo = &fixture.text;
        if demo.input != event.entity || !world.entities().contains(demo.owner) {
            return;
        }
        world.resource_scope(|world, mut fixture: Mut<super::Fixture>| {
            if let Err(error) = respond(world, &mut fixture.text, event.outcome) {
                eprintln!("Diagnostic text response failed: {error}");
                world.write_message(AppExit::error());
            }
        });
    });
}
fn respond(world: &mut World, demo: &mut Demo, outcome: TextFieldOutcome) -> Result<(), String> {
    let (invalid, message) = match outcome {
        TextFieldOutcome::Requested(request) => {
            if request.baseline() != demo.revision
                || !TextField::is_current(world, demo.input, &request)
            {
                return Err("Diagnostic text candidate lost its baseline".into());
            }
            if request.value().is_empty() {
                TextField::reject(world, demo.input, &request).map_err(|e| format!("{e:?}"))?;
                (
                    true,
                    "Text cannot be empty. Enter a value or press Escape.".into(),
                )
            } else {
                let revision = if request.value() == demo.accepted {
                    demo.revision
                } else {
                    demo.revision
                        .checked_add(1)
                        .ok_or("Diagnostic revision exhausted")?
                };
                TextField::accept(world, demo.input, &request, revision)
                    .map_err(|e| format!("{e:?}"))?;
                demo.accepted = request.value().into();
                demo.revision = revision;
                (false, format!("Accepted: {}", demo.accepted))
            }
        }
        TextFieldOutcome::Cancelled => (false, format!("Cancelled. Accepted: {}", demo.accepted)),
        TextFieldOutcome::HistoryRequested { .. } => return Ok(()),
        TextFieldOutcome::Refused(_) => (
            true,
            "Input was not accepted. Correct the draft or press Escape.".into(),
        ),
    };
    kedra_ui::docking::class(world, demo.surface, "invalid", invalid);
    demo.row
        .update(world, "Text draft", Some(&message))
        .map_err(|e| e.to_string())
}
