//! Host-owned folder commands; filesystem ownership stays in authoring_fs.
use super::*;
use bevy::tasks::IoTaskPool;
use kedra_authoring_fs::entries::Change;
use kedra_wakeup::{Task, Wakeup};
use std::path::PathBuf;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Mode {
    Create,
    Rename,
    Delete,
}
#[derive(Clone)]
pub(super) struct Request {
    pub view: ViewId,
    pub root: PathBuf,
    pub path: PathBuf,
    pub mode: Mode,
}
impl Request {
    pub fn folder(&self) -> PathBuf {
        match self.mode {
            Mode::Create => self.path.clone(),
            _ => self
                .path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_owned(),
        }
    }
    pub fn title(&self) -> &'static str {
        match self.mode {
            Mode::Create => "New folder",
            Mode::Rename => "Rename asset",
            Mode::Delete => "Delete asset",
        }
    }
}
enum Outcome {
    Confirmed(bool),
    Applied(Result<Box<kedra_authoring::source::Admission>, String>),
}
#[derive(Resource)]
struct Pending {
    request: Request,
    change: Change,
    task: Task<Outcome>,
}
pub(super) fn pending(world: &World) -> bool {
    world.contains_resource::<Pending>()
}
pub(crate) fn move_entry(
    world: &mut World,
    source_view: ViewId,
    view: ViewId,
    from: PathBuf,
    directory: PathBuf,
) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = (|| {
            if !super::browser_visible(&state, source_view)
                || !super::browser_visible(&state, view)
                || document_create::pending(world)
            {
                return Err("Asset move target was retired.".into());
            }
            let root = world
                .resource::<crate::model::ProjectManager>()
                .project
                .as_ref()
                .ok_or("No project is open")?
                .assets
                .clone();
            let to = directory.join(from.file_name().ok_or("The asset root cannot be moved")?);
            if from == to {
                return Ok(());
            }
            let request = Request {
                view,
                root,
                path: from.clone(),
                mode: Mode::Rename,
            };
            start(world, &mut state, request, Change::Rename { from, to })
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}
pub(super) fn install(app: &mut App) {
    app.add_systems(Update, poll);
}

pub(crate) fn context_menu(
    world: &mut World,
    view: ViewId,
    path: PathBuf,
    generation: u64,
    target: Option<PathBuf>,
    anchor: Entity,
    position: Vec2,
) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if !inspector_visible(&state, view)
            || !state
                .panels
                .browsers
                .get(&view)
                .is_some_and(|panel| panel.current(&path, generation))
        {
            return;
        }
        input::dismiss(world, &mut state);
        let result = (|| {
            let point = popup::attach_at(world, anchor, position)?;
            state.menu.push(point.root);
            let menu = point.menu;
            let mut items = super::document_menu::entry_items(view)?;
            if let Some(path) = target {
                let root = &world
                    .resource::<crate::model::ProjectManager>()
                    .project
                    .as_ref()
                    .ok_or("No project is open")?
                    .assets;
                if std::fs::symlink_metadata(root.join(&path))
                    .is_ok_and(|m| !m.is_symlink() && (m.is_file() || m.is_dir()))
                {
                    items.push((
                        "Rename".into(),
                        input::Action::Folder(view, Mode::Rename, Some(path.clone())),
                    ));
                    items.push((
                        "Delete".into(),
                        input::Action::Folder(view, Mode::Delete, Some(path)),
                    ));
                }
            }
            for (index, (label, action)) in items.into_iter().enumerate() {
                let branch = matches!(action, input::Action::Submenu(_));
                let button = crate::views::mount_asset(
                    world,
                    menu,
                    "editor://prefabs/button.prefab.ron",
                    &format!("folder-menu/{index}"),
                )?;
                let caption = world
                    .query::<(Entity, &ChildOf, &Text)>()
                    .iter(world)
                    .find_map(|(id, parent, _)| (parent.parent() == button.root).then_some(id))
                    .ok_or("Menu caption missing")?;
                world.get_mut::<Text>(caption).ok_or("Caption retired")?.0 = label;
                world
                    .entity_mut(button.root)
                    .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
                    .insert((
                        bevy::ui_widgets::MenuItem,
                        action,
                        bevy::input_focus::tab_navigation::TabIndex(index as i32),
                        kedra_ui::ElementId::new(format!("folder-menu/{index}")),
                    ));
                if branch {
                    kedra_ui::popup::cascade::branch(world, button.root);
                }
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}

pub(super) fn open(
    world: &mut World,
    state: &mut EditorWorkspace,
    view: ViewId,
    mode: Mode,
    target: Option<PathBuf>,
) -> Result<(), String> {
    if !state.documents.update_clean()
        || settlement::source_edits(world, state)
        || document_create::pending(world)
    {
        return Err(
            "Finish pending operations and save source changes before changing folders.".into(),
        );
    }
    if !inspector_visible(state, view) {
        return Err("Asset Browser is no longer visible.".into());
    }
    let panel = state
        .panels
        .browsers
        .get(&view)
        .ok_or("Asset Browser unavailable")?;
    let path = match mode {
        Mode::Create => panel.directory(),
        _ => target
            .or_else(|| panel.selected_path())
            .ok_or("Select a file or folder first.")?,
    };
    let root = world
        .resource::<crate::model::ProjectManager>()
        .project
        .as_ref()
        .ok_or("No project is open")?
        .assets
        .clone();
    if matches!(mode, Mode::Create) {
        kedra_import::ingest::checked_directory(&root, &path)?;
    }
    if !matches!(mode, Mode::Create) && path.as_os_str().is_empty() {
        return Err("The asset root cannot be changed.".into());
    }
    let request = Request {
        view,
        root,
        path,
        mode,
    };
    if matches!(mode, Mode::Delete) {
        let window = kedra_ui::native_input::owner(world, panel.root())
            .ok_or("Browser window unavailable")?;
        let mut dialog = rfd::AsyncMessageDialog::new()
            .set_title("Delete asset")
            .set_description(format!("Delete assets/{}?", request.path.display()))
            .set_buttons(rfd::MessageButtons::OkCancel);
        dialog =
            bevy::winit::WINIT_WINDOWS.with_borrow(|windows| match windows.get_window(window) {
                Some(w) => dialog.set_parent(&**w),
                None => dialog,
            });
        let task = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
            Outcome::Confirmed(dialog.show().await == rfd::MessageDialogResult::Ok)
        });
        world.insert_resource(Pending {
            change: Change::Delete(request.path.clone()),
            request,
            task,
        });
        Ok(())
    } else {
        document_create::open_folder(world, state, request)
    }
}
pub(super) fn submit(
    world: &mut World,
    state: &mut EditorWorkspace,
    request: Request,
    name: &str,
) -> Result<(), String> {
    kedra_authoring_fs::entries::name(name)?;
    let path = request.folder().join(name);
    let change = match request.mode {
        Mode::Create => Change::Create(path),
        Mode::Rename => Change::Rename {
            from: request.path.clone(),
            to: path,
        },
        Mode::Delete => Change::Delete(request.path.clone()),
    };
    start(world, state, request, change)
}
fn start(
    world: &mut World,
    state: &mut EditorWorkspace,
    request: Request,
    change: Change,
) -> Result<(), String> {
    if settlement::source_edits(world, state)
        || !state.documents.update_clean()
        || !world
            .resource::<crate::model::ProjectManager>()
            .project
            .as_ref()
            .is_some_and(|p| p.assets == request.root)
        || !state.panels.browsers.contains_key(&request.view)
    {
        return Err("Folder operation cancelled: project or source state changed.".into());
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let policies = world
        .resource::<kedra_scene::reflection::ComponentPolicies>()
        .clone();
    let scope = state.documents.folder_scope();
    let root = request.root.clone();
    let operation = change.clone();
    notice(world, state, "Updating assets…");
    let task = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
        Outcome::Applied(kedra_authoring_fs::entries::apply(&root, "game", &operation, |before, after| {
            let types = registry.read();
            if let Some(mut scope) = scope {
                let mut baseline = scope.source.clone();
                baseline.generation = before.generation;
                let mut current = before.clone();
                baseline.documents.sort_by(|a, b| a.path.cmp(&b.path));
                baseline.assets.sort_by(|a, b| a.path.cmp(&b.path));
                current.documents.sort_by(|a, b| a.path.cmp(&b.path));
                current.assets.sort_by(|a, b| a.path.cmp(&b.path));
                if ron::to_string(&baseline).map_err(|e| e.to_string())? != ron::to_string(&current).map_err(|e| e.to_string())? {
                    return Err("Sources changed externally; reopen the project before changing folders.".into());
                }
                let compiled = kedra_authoring::source::reconcile(&mut scope, after, &types, &policies)?;
                Ok((scope, compiled))
            } else { kedra_authoring::source::admit(after, &types, &policies) }
        }).map(Box::new))
    });
    world.insert_resource(Pending {
        request,
        change,
        task,
    });
    Ok(())
}
fn poll(world: &mut World) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    let Some(mut work) = world.remove_resource::<Pending>() else {
        return;
    };
    let Some(result) = work.task.poll() else {
        world.insert_resource(work);
        return;
    };
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = match result {
            Outcome::Confirmed(false) => Ok(()),
            Outcome::Confirmed(true) => start(world, &mut state, work.request, work.change),
            Outcome::Applied(result) => result.and_then(|admission| {
                for view in state.documents.install_folder(*admission) {
                    if state.layout.snapshot().location(view).is_some() {
                        apply(world, &mut state, Command::Hide(view))?;
                    }
                }
                for panel in state.panels.browsers.values_mut() {
                    panel.folder_changed(world, &work.change)?;
                }
                notice(world, &state, "Assets updated.");
                Ok(())
            }),
        };
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}
