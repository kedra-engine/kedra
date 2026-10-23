//! Dockable source browser: ordinary model, asynchronous filesystem effects and retained authored UI.
mod hints;
mod input;
mod model;
#[cfg(test)]
mod regression_tests;
mod tiles;
mod tree;
mod view;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use kedra_docking::model::ViewId;
use kedra_ui::authored::view::UiView;
use model::{Kind, Listing, Model, Request};
use std::{ffi::OsString, path::PathBuf};
type TreeRead = (u64, PathBuf, Result<Listing, String>);

#[derive(Clone)]
pub(crate) enum Action {
    Back,
    Forward,
    Up,
    Refresh,
    Open,
    Select(OsString),
    Enter(OsString),
    Root,
    Navigate(PathBuf),
    Toggle(PathBuf),
    Grid,
    List,
    Documents,
    Clear,
    Move(OsString, KeyCode),
    TreeNavigate(PathBuf),
    TreeKey(PathBuf, KeyCode),
    FocusSearch,
    Unavailable(&'static str),
}
#[derive(Component, Clone)]
struct Binding {
    view: ViewId,
    path: PathBuf,
    generation: u64,
    action: Action,
}
/// Presentation-only scroll targets inside a retained browser panel.
#[derive(Component)]
pub(crate) struct ScrollRegion(pub u8);

pub(crate) struct Panel {
    model: Model,
    root: PathBuf,
    view: UiView,
    task: Option<Task<(Request, Result<Listing, String>)>>,
    rows: Vec<(OsString, UiView)>,
    folders: Vec<(PathBuf, UiView)>,
    crumbs: Vec<UiView>,
    crumb_path: Option<PathBuf>,
    tree: tree::Tree,
    tree_task: Option<Task<TreeRead>>,
    pending_open: Option<PathBuf>,
    restore_scroll: bool,
    viewport: tiles::Window,
    focus_selection: bool,
    dirty: bool,
}
impl Panel {
    pub fn mount(
        world: &mut World,
        parent: Entity,
        id: ViewId,
        root: PathBuf,
    ) -> Result<Self, String> {
        let view = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/asset-browser.prefab.ron",
            &format!("browser/{id:?}"),
        )?;
        for (key, action) in [
            ("back", Action::Back),
            ("forward", Action::Forward),
            ("up", Action::Up),
            ("root", Action::Root),
            ("grid", Action::Grid),
            ("list", Action::List),
            ("filter", Action::Documents),
            ("clear", Action::Clear),
            (
                "new",
                Action::Unavailable(
                    "New is unavailable: source-writing contracts are not connected.",
                ),
            ),
            (
                "import",
                Action::Unavailable(
                    "Import is unavailable: this browser never starts asset processing.",
                ),
            ),
        ] {
            world
                .entity_mut(view.element(key).map_err(|e| e.to_string())?)
                .insert(Binding {
                    view: id,
                    path: PathBuf::new(),
                    generation: 0,
                    action,
                });
        }
        for (key, priority) in [
            ("entries", 0),
            ("folders", 0),
            ("breadcrumbs", 0),
            ("toolbar", 1),
        ] {
            world
                .entity_mut(view.element(key).map_err(|e| e.to_string())?)
                .insert(ScrollRegion(priority));
        }
        for (track, thumb, target) in [
            ("content-scrollbar", "content-thumb", "entries"),
            ("tree-scrollbar", "tree-thumb", "folders"),
        ] {
            let target = view.element(target).map_err(|e| e.to_string())?;
            world
                .entity_mut(view.element(track).map_err(|e| e.to_string())?)
                .insert(bevy::ui_widgets::Scrollbar::new(
                    target,
                    bevy::ui_widgets::ControlOrientation::Vertical,
                    24.,
                ));
            world
                .entity_mut(view.element(thumb).map_err(|e| e.to_string())?)
                .remove::<Node>()
                .insert(bevy::ui_widgets::ScrollbarThumb::default());
        }
        world.entity_mut(view.root).insert(Binding {
            view: id,
            path: PathBuf::new(),
            generation: 0,
            action: Action::FocusSearch,
        });
        let search = view.element("search").map_err(|e| e.to_string())?;
        if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(search) {
            text.max_characters = Some(256);
            text.allow_newlines = false;
        }
        world
            .entity_mut(search)
            .insert(bevy::text::EditableTextFilter::new(|c| !c.is_control()));
        world
            .entity_mut(
                view.element("search-placeholder")
                    .map_err(|e| e.to_string())?,
            )
            .insert(bevy::picking::Pickable::IGNORE);
        world
            .entity_mut(view.element("search-icon").map_err(|e| e.to_string())?)
            .insert(bevy::picking::Pickable::IGNORE);
        let mut panel = Self {
            model: Model::default(),
            root,
            view,
            task: None,
            rows: Vec::new(),
            folders: Vec::new(),
            crumbs: Vec::new(),
            crumb_path: None,
            tree: tree::Tree::default(),
            tree_task: None,
            pending_open: None,
            restore_scroll: false,
            viewport: tiles::Window::default(),
            focus_selection: false,
            dirty: true,
        };
        panel.request(PathBuf::new(), None)?;
        panel.tree.reveal(std::path::Path::new(""));
        panel.tick(world, id)?;
        Ok(panel)
    }
    pub fn root(&self) -> Entity {
        self.view.root
    }
    pub fn take_open(&mut self) -> Option<PathBuf> {
        self.pending_open.take()
    }
    pub fn message(&mut self, message: String) {
        self.model.notice = Some(message);
        self.dirty = true;
    }
    pub fn clear_message(&mut self) {
        self.model.notice = None;
        self.dirty = true;
    }
    pub fn dispatch(
        &mut self,
        world: &mut World,
        id: ViewId,
        path: PathBuf,
        generation: u64,
        action: Action,
    ) -> Result<(), String> {
        if path != self.model.path || generation != self.model.generation() {
            return Ok(());
        }
        self.action(world, id, action)
    }
    fn request(&mut self, path: PathBuf, history: Option<usize>) -> Result<(), String> {
        let request = self.model.request(path, history)?;
        let root = self.root.clone();
        self.task = Some(IoTaskPool::get().spawn(async move {
            let result = model::list(&root, &request.path);
            (request, result)
        }));
        self.dirty = true;
        Ok(())
    }
    pub fn tick(&mut self, world: &mut World, id: ViewId) -> Result<(), String> {
        let search = self.view.element("search").map_err(|e| e.to_string())?;
        if let Some(text) = world.get::<bevy::text::EditableText>(search) {
            let value = text.value().to_string();
            if value != self.model.filter {
                self.model.filter = value;
                self.model.notice = None;
                self.dirty = true;
                if let Some(mut scroll) = world.get_mut::<ScrollPosition>(self.element("entries")?)
                {
                    scroll.y = 0.;
                }
            }
        }
        if let Some((request, result)) = self.task.as_mut().and_then(|t| block_on(poll_once(t))) {
            self.task = None;
            let changed = self.model.path != request.path && result.is_ok();
            if self.model.complete(&request, result) {
                self.dirty = true;
                if changed {
                    self.restore_scroll = true;
                    self.tree.reveal(&self.model.path);
                }
            }
        }
        if let Some((clock, path, result)) =
            self.tree_task.as_mut().and_then(|t| block_on(poll_once(t)))
        {
            self.tree_task = None;
            self.tree.complete(clock, path, result);
            self.dirty = true;
        }
        if self.tree_task.is_none()
            && let Some((clock, path)) = self.tree.next()
        {
            let root = self.root.clone();
            self.tree_task = Some(IoTaskPool::get().spawn(async move {
                let result = model::list(&root, &path);
                (clock, path, result)
            }));
        }
        if world.contains_resource::<bevy::render::renderer::RenderDevice>()
            && self.viewport.columns > 0
            && world
                .get::<ComputedNode>(self.view.root)
                .is_some_and(ComputedNode::is_empty)
        {
            return Ok(());
        }
        if self.dirty {
            self.render(world, id)?;
            self.dirty = false;
        }
        if self.viewport != self.window(world)? {
            self.dirty = true;
        }
        for (thumb, target) in [("content-thumb", "entries"), ("tree-thumb", "folders")] {
            let overflowing = world
                .get::<ComputedNode>(self.element(target)?)
                .is_some_and(|node| node.content_size().y > node.size().y + 1.);
            world
                .entity_mut(self.element(thumb)?)
                .insert(if overflowing {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
        }
        Ok(())
    }
    pub fn action(&mut self, world: &mut World, id: ViewId, action: Action) -> Result<(), String> {
        let entries = self.view.element("entries").map_err(|e| e.to_string())?;
        self.model
            .remember(world.get::<ScrollPosition>(entries).map_or(0., |p| p.y));
        match action {
            Action::Refresh => {
                self.tree.refresh();
                self.tree_task = None;
                self.request(self.model.path.clone(), None)?;
            }
            Action::Back | Action::Forward => {
                if !self.model.loading
                    && let Some((path, pos)) = self.model.history(matches!(action, Action::Forward))
                {
                    self.request(path, Some(pos))?;
                }
            }
            Action::Up => {
                if !self.model.loading
                    && let Some(parent) = self.model.path.parent()
                {
                    self.request(parent.to_path_buf(), None)?;
                }
            }
            Action::Root => {
                if !self.model.loading {
                    self.request(PathBuf::new(), None)?;
                }
            }
            Action::Select(name) => {
                self.model.select(&name);
                self.dirty = true;
            }
            Action::Enter(name) => {
                if self.model.select(&name) {
                    self.open()?;
                }
            }
            Action::Open => self.open()?,
            Action::Navigate(path) | Action::TreeNavigate(path) => {
                if !self.model.loading {
                    self.request(path, None)?;
                }
            }
            Action::Toggle(path) => {
                self.tree.toggle(path);
                self.dirty = true;
            }
            Action::Grid => {
                self.model.mode = model::Mode::Grid;
                self.dirty = true;
            }
            Action::List => {
                self.model.mode = model::Mode::List;
                self.dirty = true;
            }
            Action::Documents => {
                self.model.documents_only = !self.model.documents_only;
                self.dirty = true;
                if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entries) {
                    scroll.y = 0.;
                }
            }
            Action::Clear => {
                if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(
                    self.view.element("search").map_err(|e| e.to_string())?,
                ) {
                    text.editor_mut().set_text("");
                    text.queue_edit(bevy::text::TextEdit::TextEnd(false));
                }
                self.model.filter.clear();
                self.dirty = true;
            }
            Action::FocusSearch => {
                world.resource_mut::<bevy::input_focus::InputFocus>().set(
                    self.view.element("search").map_err(|e| e.to_string())?,
                    bevy::input_focus::FocusCause::Navigated,
                );
            }
            Action::Move(name, key) => {
                if self.model.select(&name) {
                    self.move_selection(world, key)?;
                }
            }
            Action::TreeKey(path, key) => self.tree_key(world, path, key)?,
            Action::Unavailable(reason) => {
                self.message(reason.into());
            }
        }
        self.tick(world, id)
    }
    fn open(&mut self) -> Result<(), String> {
        if self.model.loading || self.model.error.is_some() {
            return Ok(());
        }
        if let Some(entry) = self.model.selected_entry().cloned() {
            if entry.kind == Kind::Folder {
                return self.request(self.model.path.join(&entry.name), None);
            }
            self.model.notice = Some(
                match entry.kind {
                    Kind::Scene | Kind::Prefab => "Opening source document…",
                    Kind::Unavailable => {
                        "Linked, special or unsupported source names cannot be opened."
                    }
                    _ => {
                        "This source type has no document editor. No external program was started."
                    }
                }
                .into(),
            );
            if matches!(entry.kind, Kind::Scene | Kind::Prefab)
                || self
                    .model
                    .path
                    .join(&entry.name)
                    .extension()
                    .is_some_and(|ext| ext == "png" || ext == "glb")
            {
                self.pending_open = Some(self.model.path.join(&entry.name));
                self.model.notice = Some("Opening read-only asset preview…".into());
            }
            self.dirty = true;
        }
        Ok(())
    }
}

pub(crate) fn install(app: &mut App) {
    if app.is_plugin_added::<bevy::ui::UiPlugin>()
        && !app.is_plugin_added::<bevy::ui_widgets::ScrollbarPlugin>()
    {
        app.add_plugins(bevy::ui_widgets::ScrollbarPlugin);
    }
    input::install(app);
}
fn dispatch(world: &mut World, binding: Binding) {
    crate::workspace::browser_action(
        world,
        binding.view,
        binding.path,
        binding.generation,
        binding.action,
    );
}
