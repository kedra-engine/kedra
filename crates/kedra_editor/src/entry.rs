//! Entry-form bindings. Source documents own all dialog/control geometry and styles.
use crate::{
    model::*,
    screens::{ManagedControl, set_field},
    views::{self, Kind},
};
use bevy::prelude::*;
use kedra_ui::{ScreenId, authored::view::UiView};

pub(crate) fn mount(world: &mut World) {
    let surface = world.resource::<kedra_desktop::DesktopSurface>();
    let (Some(_), Some(parent)) = (surface.frame, surface.root) else {
        world.resource_mut::<EntryForm>().error =
            "Wait for the project manager to finish loading".into();
        return;
    };
    let Some(mode) = world.resource::<EntryForm>().mode else {
        return;
    };
    let pending = match views::request(world, parent, Kind::Entry(mode)) {
        Ok(pending) => pending,
        Err(error) => {
            views::fail(world, error);
            return;
        }
    };
    world.resource_mut::<ProjectManager>().modal = Some(pending);
}

pub(crate) fn bind(
    world: &mut World,
    view: &UiView,
    mode: EntryMode,
    request: Entity,
) -> Result<(), String> {
    if world.resource::<ProjectManager>().modal != Some(request)
        || world.resource::<EntryForm>().mode != Some(mode)
    {
        return Err("entry-form ownership changed before materialization".into());
    }
    let name = view.element("entry.name").map_err(|e| e.to_string())?;
    let directory = view.element("entry.directory").map_err(|e| e.to_string())?;
    let destination = view
        .element("entry.destination")
        .map_err(|e| e.to_string())?;
    let error = view.element("entry.error").map_err(|e| e.to_string())?;
    let controls = [
        ("entry.browse", Action::Browse),
        ("entry.cancel", Action::Cancel),
        ("entry.submit", Action::Submit),
    ]
    .into_iter()
    .map(|(key, action)| view.element(key).map(|entity| (entity, action)))
    .collect::<kedra_scene::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;
    for (entity, action) in controls {
        world.entity_mut(entity).insert((action, ManagedControl));
    }
    world.entity_mut(name).insert(Field::Name);
    world.entity_mut(directory).insert(Field::Directory);
    if mode == EntryMode::Open {
        let name = view
            .element("entry.preview_name")
            .map_err(|error| error.to_string())?;
        world
            .entity_mut(view.root)
            .insert(crate::entry_preview::Preview::new(name, destination));
    } else {
        world
            .entity_mut(destination)
            .insert(StatusLabel::Destination);
    }
    world.entity_mut(error).insert(StatusLabel::EntryError);
    let form = world.resource::<EntryForm>();
    let initial_name = form.name.clone();
    let initial_directory = form.directory.clone();
    set_field(world, Field::Name, &initial_name);
    set_field(world, Field::Directory, &initial_directory);
    world.entity_mut(view.root).insert(ScreenId(
        if mode == EntryMode::Create {
            "entry.create"
        } else {
            "entry.open"
        }
        .into(),
    ));
    world.resource_mut::<ProjectManager>().modal = Some(view.root);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modal_overlay_uses_full_window_root_not_the_inset_client_frame() {
        let mut world = World::new();
        world.init_resource::<ProjectManager>();
        world.init_resource::<EntryForm>();
        world.init_resource::<views::ViewClock>();
        world.init_resource::<Time<Real>>();
        let root = world.spawn_empty().id();
        let frame = world.spawn(ChildOf(root)).id();
        world.insert_resource(kedra_desktop::DesktopSurface {
            root: Some(root),
            frame: Some(frame),
            ..default()
        });
        world.resource_mut::<EntryForm>().mode = Some(EntryMode::Create);
        mount(&mut world);
        let pending = world
            .resource::<ProjectManager>()
            .modal
            .unwrap_or_else(|| panic!("modal was not requested"));
        assert_eq!(
            world.get::<ChildOf>(pending).map(ChildOf::parent),
            Some(root)
        );
    }
}
