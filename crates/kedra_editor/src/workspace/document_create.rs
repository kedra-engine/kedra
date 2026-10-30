//! Editor-owned name entry; source history and Save remain owned by Documents.
use super::*;
use kedra_scene::document::DocumentKind;
use kedra_ui::fields::{TextField, TextFieldEvent, TextFieldOutcome};
use std::path::{Path, PathBuf};

#[derive(Clone)]
enum Purpose {
    Environment(ViewId),
    Document(DocumentKind),
    Folder(super::folders::Request),
    NodeRename(super::hierarchy_menu::Command, String),
}

#[derive(Component, Clone)]
struct Form {
    input: Entity,
    error: Entity,
    kind: Purpose,
    folder: PathBuf,
    busy: bool,
    binding_ready: bool,
}
#[derive(Component, Clone, Copy)]
struct Button {
    form: Entity,
    submit: bool,
}

pub(super) fn install(app: &mut App) {
    app.add_observer(activated)
        .add_observer(outcome)
        .add_observer(close_requested);
}
fn close_requested(
    event: On<bevy::ui_widgets::RequestClose>,
    parents: Query<&ChildOf>,
    forms: Query<&Form>,
    mut commands: Commands,
) {
    if let Some(id) = std::iter::once(event.source)
        .chain(parents.iter_ancestors(event.source))
        .find(|id| forms.get(*id).is_ok_and(|form| !form.busy))
    {
        commands.entity(id).despawn();
    }
}
pub(super) fn pending(world: &mut World) -> bool {
    world.query::<&Form>().iter(world).next().is_some()
}
pub(super) fn open(
    world: &mut World,
    state: &mut EditorWorkspace,
    kind: DocumentKind,
) -> Result<(), String> {
    if pending(world) || state.documents.creating() {
        return Err("Finish the current document creation first.".into());
    }
    let browser = state
        .documents
        .origin
        .and_then(|id| state.panels.browsers.get(&id))
        .or_else(|| state.panels.browsers.values().next());
    let folder = browser.map(|panel| panel.directory()).unwrap_or_default();
    open_form(world, Purpose::Document(kind), folder)
}
pub(super) fn open_folder(
    world: &mut World,
    _state: &mut EditorWorkspace,
    request: super::folders::Request,
) -> Result<(), String> {
    let folder = request.folder();
    open_form(world, Purpose::Folder(request), folder)
}
pub(super) fn open_environment(
    world: &mut World,
    state: &EditorWorkspace,
    view: ViewId,
) -> Result<(), String> {
    if pending(world)
        || !state.documents.update_clean()
        || super::import_pending(world)
        || super::settlement::source_edits(world, state)
    {
        return Err("Finish pending edits and save sources before creating an environment".into());
    }
    let panel = state
        .panels
        .browsers
        .get(&view)
        .ok_or("Asset Browser unavailable")?;
    if !super::inspector_visible(state, view) || !panel.listing_ready() {
        return Err("Wait for the Asset Browser listing".into());
    }
    open_form(world, Purpose::Environment(view), panel.directory())
}
pub(super) fn open_node_rename(
    world: &mut World,
    state: &EditorWorkspace,
    command: super::hierarchy_menu::Command,
) -> Result<(), String> {
    if pending(world) || super::settlement::source_edits(world, state) {
        return Err("Finish pending edits first.".into());
    }
    let name = state
        .documents
        .opened
        .get(&command.edit.target.document)
        .and_then(|opened| opened.graph.nodes.get(&command.edit.target.node))
        .ok_or("Node unavailable")?
        .name
        .clone();
    open_form(world, Purpose::NodeRename(command, name), PathBuf::new())
}
fn open_form(world: &mut World, kind: Purpose, folder: PathBuf) -> Result<(), String> {
    let parent = world
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("Editor surface unavailable")?;
    let view = crate::views::mount_asset(
        world,
        parent,
        "editor://prefabs/document-create.prefab.ron",
        "document-create",
    )?;
    let result = (|| {
        let element = |name: &str| view.element(name).map_err(|e| e.to_string());
        let title = element("document-create.title")?;
        world.get_mut::<Text>(title).ok_or("Title missing")?.0 = match &kind {
            Purpose::Environment(_) => "New environment",
            Purpose::Document(DocumentKind::Scene) => "New scene",
            Purpose::Document(DocumentKind::Prefab) => "New prefab",
            Purpose::Folder(request) => request.title(),
            Purpose::NodeRename(_, _) => "Rename node",
        }
        .into();
        let path = element("document-create.folder")?;
        world.get_mut::<Text>(path).ok_or("Folder missing")?.0 =
            Path::new("assets").join(&folder).display().to_string();
        if matches!(&kind, Purpose::NodeRename(_, _)) {
            world
                .get_mut::<Text>(path)
                .ok_or("Folder missing")?
                .0
                .clear();
        }
        let field = crate::views::mount_asset(
            world,
            element("document-create.name-slot")?,
            "editor://prefabs/field.prefab.ron",
            "document-create/field",
        )?;
        let input = world
            .query::<(Entity, &ChildOf, &bevy::text::EditableText)>()
            .iter(world)
            .find_map(|(id, p, _)| (p.parent() == field.root).then_some(id))
            .ok_or("Name input missing")?;
        world
            .entity_mut(input)
            .insert(kedra_ui::ElementId::new("document-create/name"));
        if let Purpose::Folder(request) = &kind
            && matches!(request.mode, super::folders::Mode::Rename)
            && let Some(name) = request.path.file_name().and_then(|name| name.to_str())
        {
            world
                .get_mut::<bevy::text::EditableText>(input)
                .ok_or("Name input retired")?
                .editor_mut()
                .set_text(name);
        }
        if let Purpose::NodeRename(_, name) = &kind {
            world
                .get_mut::<bevy::text::EditableText>(input)
                .ok_or("Name input retired")?
                .editor_mut()
                .set_text(name);
        }
        let session = kedra_ui::text_input::TextSession::attach(
            world
                .get::<bevy::text::EditableText>(input)
                .ok_or("Name input retired")?,
            256,
        )
        .map_err(|error| format!("Name input unavailable: {error:?}"))?;
        world.entity_mut(input).insert(session);
        let actions = element("document-create.actions")?;
        let accept = if matches!(&kind, Purpose::NodeRename(_, _))
            || matches!(&kind, Purpose::Folder(r) if matches!(r.mode, super::folders::Mode::Rename))
        {
            "Rename"
        } else {
            "Create"
        };
        for (submit, label) in [(false, "Cancel"), (true, accept)] {
            let button = crate::views::mount_asset(
                world,
                actions,
                "editor://prefabs/button.prefab.ron",
                &format!("document-create/{label}"),
            )?;
            let caption = world
                .query::<(Entity, &ChildOf, &Text)>()
                .iter(world)
                .find_map(|(id, p, _)| (p.parent() == button.root).then_some(id))
                .ok_or("Button caption missing")?;
            world.get_mut::<Text>(caption).ok_or("Caption retired")?.0 = label.into();
            world.entity_mut(button.root).insert((
                Button {
                    form: view.root,
                    submit,
                },
                kedra_ui::ElementId::new(format!("document-create/{label}")),
            ));
        }
        world.entity_mut(view.root).insert(Form {
            input,
            error: element("document-create.error")?,
            kind,
            folder,
            busy: false,
            binding_ready: false,
        });
        world
            .resource_mut::<bevy::input_focus::InputFocus>()
            .set(input, bevy::input_focus::FocusCause::Navigated);
        Ok(())
    })();
    if result.is_err() {
        world.despawn(view.root);
    }
    result
}

fn filename(folder: &Path, name: &str, kind: DocumentKind) -> Result<PathBuf, String> {
    if name.trim().is_empty()
        || name != name.trim()
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        || matches!(name, "." | "..")
    {
        return Err("Enter a filename without path separators or surrounding spaces.".into());
    }
    let suffix = match kind {
        DocumentKind::Scene => ".scene.ron",
        DocumentKind::Prefab => ".prefab.ron",
    };
    if name.ends_with(".ron") && !name.ends_with(suffix) {
        return Err(format!("Use a name ending in {suffix}."));
    }
    Ok(folder.join(if name.ends_with(suffix) {
        name.to_owned()
    } else {
        format!("{name}{suffix}")
    }))
}

fn activated(
    event: On<bevy::ui_widgets::Activate>,
    buttons: Query<&Button>,
    mut commands: Commands,
) {
    let Ok(button) = buttons.get(event.entity).copied() else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let Some(form) = world.get::<Form>(button.form).cloned() else {
            return;
        };
        if form.busy {
            return;
        }
        if button.submit {
            if let Err(error) = TextField::request(world, form.input) {
                show_error(world, &form, format!("{error:?}"));
            }
        } else {
            world.despawn(button.form);
        }
    });
}
fn show_error(world: &mut World, form: &Form, message: String) {
    if let Some(mut text) = world.get_mut::<Text>(form.error) {
        text.0 = message;
    }
}
fn outcome(event: On<TextFieldEvent>, mut commands: Commands) {
    let event = event.event().clone();
    commands.queue(move |world: &mut World| {
        let found = world
            .query::<(Entity, &Form)>()
            .iter(world)
            .find(|(_, f)| f.input == event.entity)
            .map(|(id, f)| (id, f.clone()));
        let Some((id, form)) = found else {
            return;
        };
        if form.busy {
            return;
        }
        match event.outcome {
            TextFieldOutcome::Cancelled => {
                world.despawn(id);
            }
            TextFieldOutcome::Requested(request) => {
                if !TextField::is_current(world, form.input, &request) {
                    return;
                }
                if let Purpose::Folder(folder) = &form.kind {
                    let result = world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                        super::folders::submit(world, &mut state, folder.clone(), request.value())
                    });
                    let _ = TextField::reject(world, form.input, &request);
                    match result {
                        Ok(()) => {
                            world.despawn(id);
                        }
                        Err(error) => show_error(world, &form, error),
                    }
                    return;
                }
                if let Purpose::Environment(view) = form.kind {
                    let result = world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                        super::asset_import::create_environment(
                            world,
                            &mut state,
                            view,
                            &form.folder,
                            request.value(),
                        )
                    });
                    let _ = TextField::reject(world, form.input, &request);
                    match result {
                        Ok(()) => {
                            world.despawn(id);
                        }
                        Err(error) => show_error(world, &form, error),
                    }
                    return;
                }
                if let Purpose::NodeRename(command, _) = &form.kind {
                    let mut command = command.clone();
                    command.edit.operation =
                        crate::documents::structure::NodeOperation::Rename(request.value().into());
                    let result = world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                        super::hierarchy_menu::validate(&state, &command)?;
                        super::source_history::edit_nodes(world, &mut state, &command.edit)
                    });
                    let _ = TextField::reject(world, form.input, &request);
                    match result {
                        Ok(()) => {
                            world.despawn(id);
                        }
                        Err(error) => show_error(world, &form, error),
                    }
                    return;
                }
                let Purpose::Document(kind) = form.kind else {
                    return;
                };
                let result = filename(&form.folder, request.value(), kind).and_then(|path| {
                    let root = world
                        .resource::<crate::model::ProjectManager>()
                        .project
                        .as_ref()
                        .ok_or("No project is open")?
                        .assets
                        .clone();
                    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                        if super::settlement::drafts(world, &state) {
                            return Err("Accept or cancel property drafts first.".into());
                        }
                        state.documents.begin_document(world, root, path, kind)
                    })
                });
                let _ = TextField::reject(world, form.input, &request);
                match result {
                    Ok(()) => {
                        if let Some(mut f) = world.get_mut::<Form>(id) {
                            f.busy = true;
                        }
                    }
                    Err(error) => show_error(world, &form, error),
                }
            }
            _ => {}
        }
    });
}
pub(super) fn poll(world: &mut World, state: &mut EditorWorkspace) {
    let unbound: Vec<_> = world
        .query::<(Entity, &Form)>()
        .iter(world)
        .filter(|(_, form)| !form.binding_ready)
        .map(|(id, form)| (id, form.clone()))
        .collect();
    for (id, form) in unbound {
        let settled = world
            .get::<bevy::text::EditableText>(form.input)
            .zip(world.get::<kedra_ui::text_input::TextSession>(form.input))
            .is_some_and(|(text, session)| {
                !session.pending() && text.pending_edits.is_empty() && !text.is_composing()
            });
        if settled {
            if let Err(error) = TextField::attach(world, form.input, 0) {
                show_error(world, &form, format!("Name input unavailable: {error:?}"));
            }
            if let Some(mut form) = world.get_mut::<Form>(id) {
                form.binding_ready = true;
            }
        }
    }
    let found = world
        .query::<(Entity, &Form)>()
        .iter(world)
        .find(|(_, f)| f.busy)
        .map(|(id, f)| (id, f.clone()));
    if let Some((id, form)) = found
        && !state.documents.creating()
    {
        if let Some(error) = state.documents.error.take() {
            show_error(world, &form, error);
            if let Some(mut form) = world.get_mut::<Form>(id) {
                form.busy = false;
            }
        } else {
            world.despawn(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_native_session_binds_without_retiring_the_form_and_cancel_preserves_sources()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::test_support::setup()?;
        install(&mut app);
        let text = bevy::text::EditableText::default();
        let session =
            kedra_ui::text_input::TextSession::attach(&text, 16).map_err(|e| format!("{e:?}"))?;
        let input = app
            .world_mut()
            .spawn((
                text,
                bevy::ui_widgets::TextInput,
                kedra_ui::authored::components::UiEditable {
                    autofocus: false,
                    select_all: true,
                },
            ))
            .id();
        let error = app.world_mut().spawn(Text::default()).id();
        let owner = app
            .world_mut()
            .spawn(Form {
                input,
                error,
                kind: Purpose::Document(DocumentKind::Scene),
                folder: PathBuf::new(),
                busy: false,
                binding_ready: false,
            })
            .id();
        app.world_mut()
            .resource_scope(|world, mut state: Mut<EditorWorkspace>| poll(world, &mut state));
        assert!(app.world().entities().contains(owner));
        assert!(app.world().get::<TextField>(input).is_none());
        app.world_mut().entity_mut(input).insert(session);
        app.world_mut()
            .resource_scope(|world, mut state: Mut<EditorWorkspace>| poll(world, &mut state));
        assert_eq!(
            app.world().get::<TextField>(input).map(TextField::accepted),
            Some(""),
            "{:?}",
            app.world().get::<Text>(error)
        );
        app.world_mut().trigger(TextFieldEvent {
            entity: input,
            outcome: TextFieldOutcome::Cancelled,
            edit_session: None,
        });
        app.world_mut().flush();
        assert!(!app.world().entities().contains(owner));
        assert!(
            app.world()
                .resource::<EditorWorkspace>()
                .documents
                .update_clean()
        );
        assert!(
            !app.world()
                .resource::<EditorWorkspace>()
                .documents
                .can_history(false)
        );
        Ok(())
    }
    #[test]
    fn names_stay_in_the_selected_folder_and_preserve_unicode_and_document_kind()
    -> Result<(), String> {
        let folder = Path::new("Prefabs/Игрок");
        assert_eq!(
            filename(folder, "Бомба", DocumentKind::Prefab)?,
            folder.join("Бомба.prefab.ron")
        );
        assert_eq!(
            filename(folder, "Arena.scene.ron", DocumentKind::Scene)?,
            folder.join("Arena.scene.ron")
        );
        for name in [
            "",
            " ",
            ".",
            "..",
            "../Other",
            "a/b",
            "a\\b",
            "/tmp/Arena",
            "C:Other",
            " leading",
            "trailing ",
            "line\nname",
            "Other.scene.ron",
        ] {
            assert!(
                filename(folder, name, DocumentKind::Prefab).is_err(),
                "{name:?}"
            );
        }
        Ok(())
    }
}
