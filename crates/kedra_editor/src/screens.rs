//! Retained feature bindings. Layout, type and control skin are external assets.
use crate::{
    jobs::PendingJob,
    loading::WorkingActions,
    model::*,
    views::{self, Kind},
};
use bevy::{prelude::*, text::EditableText, ui::InteractionDisabled};
use bevy_flair::style::components::ClassList;
use kedra_desktop::{SurfaceKind, replace};
use kedra_ui::{ScreenId, authored::view::UiView};

#[derive(Component)]
pub(crate) struct ManagedControl;
#[derive(Component)]
pub(crate) struct SearchHost;

pub(crate) fn projects(world: &mut World) {
    let root = replace(
        world,
        SurfaceKind::Projects,
        "Projects — Kedra Engine".into(),
    );
    crate::recent::invalidate(world);
    let mut manager = world.resource_mut::<ProjectManager>();
    manager.modal = None;
    manager.list_dirty = true;
    manager.native_created_at = None;
    if let Err(error) = views::request(world, root, Kind::Frame(SurfaceKind::Projects)) {
        views::fail(world, error);
    }
}

pub(crate) fn bind_projects(world: &mut World, view: &UiView) -> Result<(), String> {
    let controls = [
        ("projects.open", Action::Open),
        ("projects.new", Action::New),
        ("projects.search_toggle", Action::Search),
    ]
    .into_iter()
    .map(|(key, action)| view.element(key).map(|entity| (entity, action)))
    .collect::<kedra_scene::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;
    let new = view.element("projects.new").map_err(|e| e.to_string())?;
    let search = view.element("projects.search").map_err(|e| e.to_string())?;
    let search_host = view
        .element("projects.search_host")
        .map_err(|e| e.to_string())?;
    let notice = view.element("projects.notice").map_err(|e| e.to_string())?;
    let version = view
        .element("projects.version")
        .map_err(|e| e.to_string())?;
    crate::recent::bind(world, view)?;
    let logo = view.element("projects.logo").map_err(|e| e.to_string())?;
    world
        .entity_mut(logo)
        .insert(bevy::picking::Pickable::IGNORE);
    for (entity, action) in controls {
        world.entity_mut(entity).insert((action, ManagedControl));
    }
    world.entity_mut(new).insert(bevy::input_focus::AutoFocus);
    world.entity_mut(search).insert(Field::Search);
    world.entity_mut(search_host).insert(SearchHost);
    world.entity_mut(notice).insert(StatusLabel::Notice);
    world
        .get_mut::<Text>(version)
        .ok_or("release binding is not native text")?
        .0 = concat!("Kedra Engine ", env!("CARGO_PKG_VERSION")).into();
    let filter = world.resource::<ProjectManager>().filter.clone();
    set_field(world, Field::Search, &filter);
    world
        .entity_mut(view.root)
        .insert(ScreenId("projects".into()));
    Ok(())
}

pub(crate) fn editor(world: &mut World) {
    let title = world
        .resource::<ProjectManager>()
        .project
        .as_ref()
        .map_or_else(
            || "Kedra Editor".into(),
            |project| format!("{} — Kedra Editor", project.manifest.name),
        );
    let root = replace(world, SurfaceKind::Editor, title);
    if let Err(error) = views::request(world, root, Kind::Frame(SurfaceKind::Editor)) {
        views::fail(world, error);
    }
}

pub(crate) fn sync_fields(
    fields: Query<(&Field, &EditableText), Changed<EditableText>>,
    mut form: ResMut<EntryForm>,
    mut manager: ResMut<ProjectManager>,
) {
    for (field, value) in &fields {
        let value = value.value().to_string();
        match field {
            Field::Name if form.name != value => {
                form.name = value;
                form.error.clear();
            }
            Field::Directory if form.directory != value => {
                form.directory = value;
                form.error.clear();
            }
            Field::Search if manager.filter != value => {
                manager.filter = value;
                manager.list_dirty = true;
            }
            _ => {}
        }
    }
}

pub(crate) fn capture_fields(world: &mut World) {
    let values = world
        .query::<(&Field, &EditableText)>()
        .iter(world)
        .map(|(field, value)| (*field, value.value().to_string()))
        .collect::<Vec<_>>();
    let mut form = world.resource_mut::<EntryForm>();
    for (field, value) in values {
        match field {
            Field::Name => form.name = value,
            Field::Directory => form.directory = value,
            Field::Search => {}
        }
    }
}

pub(crate) fn set_field(world: &mut World, target: Field, text: &str) {
    for (field, mut value) in world.query::<(&Field, &mut EditableText)>().iter_mut(world) {
        if *field == target {
            // Retain PlainEditor styles, viewport and generation continuity.
            value.clear();
            value.editor_mut().set_text(text);
            value.queue_edit(bevy::text::TextEdit::TextEnd(false));
        }
    }
}

type FailureNodes<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ClassList,
        Has<RetryAction>,
        Has<WorkingActions>,
    ),
    (
        Or<(
            With<FailureActions>,
            With<RetryAction>,
            With<WorkingActions>,
        )>,
        Without<StatusLabel>,
    ),
>;
type ManagedControls<'w, 's> =
    Query<'w, 's, (Entity, Has<InteractionDisabled>), Or<(With<ManagedControl>, With<Field>)>>;
type StatusLabels<'w, 's> = Query<
    'w,
    's,
    (
        &'static StatusLabel,
        &'static mut Text,
        &'static mut ClassList,
    ),
    (Without<FailureActions>, Without<WorkingActions>),
>;

pub(crate) fn sync_status(
    manager: Res<ProjectManager>,
    form: Res<EntryForm>,
    pending: Option<Res<PendingJob>>,
    mut labels: StatusLabels<'_, '_>,
    mut failures: FailureNodes<'_, '_>,
    controls: ManagedControls<'_, '_>,
    mut commands: Commands,
) {
    for (kind, mut text, mut classes) in &mut labels {
        let value = match kind {
            StatusLabel::Notice => manager.notice.clone(),
            StatusLabel::EntryError => form.error.clone(),
            StatusLabel::Destination => entry_intent(&form)
                .map(|intent| crate::recent::display_path(&intent.root()))
                .unwrap_or_else(|_| "Choose a valid name and location.".into()),
            StatusLabel::Loading => crate::loading::phase(&manager),
        };
        if text.0 != value {
            text.0 = value;
        }
        if matches!(kind, StatusLabel::Notice | StatusLabel::EntryError)
            && classes.contains("hidden") != text.0.is_empty()
        {
            classes.toggle("hidden");
        }
        if matches!(kind, StatusLabel::Loading)
            && classes.contains("error") != manager.loading_error().is_some()
        {
            classes.toggle("error");
        }
    }
    for (mut classes, retry, working) in &mut failures {
        let visible = (working && manager.loading_error().is_none())
            || (!working
                && manager.loading_error().is_some()
                && (!retry || matches!(manager.intent, Some(Intent::Open(_)))));
        if classes.contains("hidden") == visible {
            classes.toggle("hidden");
        }
    }
    for (entity, disabled) in &controls {
        if pending.is_some() && !disabled {
            commands.entity(entity).insert(InteractionDisabled);
        }
        if pending.is_none() && disabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picker_updates_existing_text_editor_without_resetting_it() {
        let mut world = World::new();
        world.init_resource::<EntryForm>();
        let mut input = EditableText::new("/original/location");
        input.visible_width = Some(42.);
        input.cursor_width = 1.5;
        let mut fonts = bevy::text::FontCx::default();
        let mut layout = bevy::text::LayoutCx::default();
        input.editor_mut().set_scale(2.);
        input.editor_mut().refresh_layout(&mut fonts, &mut layout.0);
        let generation = input.editor().generation();
        let entity = world.spawn((Field::Directory, input)).id();
        world.spawn((Field::Name, EditableText::new("Кедра")));
        let selected = "/Users/test/Проекты 日本語";
        set_field(&mut world, Field::Directory, selected);
        capture_fields(&mut world);
        let mut input = world
            .get_mut::<EditableText>(entity)
            .unwrap_or_else(|| panic!("Field was removed"));
        input.editor_mut().refresh_layout(&mut fonts, &mut layout.0);
        assert_eq!(input.value().to_string(), selected);
        assert_eq!(input.visible_width, Some(42.));
        assert_eq!(input.cursor_width, 1.5);
        assert_eq!(input.editor().get_scale(), 2.);
        assert!(
            input.editor().generation() != generation,
            "Native layout must observe a new generation, not a replacement editor's reset generation"
        );
        let draft = world.resource::<EntryForm>();
        assert_eq!(draft.directory, selected);
        assert_eq!(draft.name, "Кедра");
    }
}
