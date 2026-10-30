//! Dockable source target adaptation; generic value editors live in kedra_inspector.
pub(crate) mod discrete;
pub(crate) mod editing;
pub(crate) mod environment;
pub(crate) mod history;
mod input;
pub(crate) mod metadata;
pub(crate) mod model;
mod references;
mod view;
use bevy::prelude::*;
use kedra_docking::model::ViewId;
use kedra_inspector::{
    editors::EditorRegistry,
    view::{Disclosure, TreeView},
};
use kedra_ui::{authored::view::UiView, fields::FieldTemplates};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) enum Action {
    Search,
    CloseSearch,
    Lock,
    Raw,
    Details,
    Disclosure(Disclosure),
}
#[derive(Component, Clone)]
pub(crate) struct Binding {
    pub view: ViewId,
    pub action: Action,
}

pub(crate) struct Panel {
    view: UiView,
    pub(crate) state: model::State,
    templates: FieldTemplates,
    editors: EditorRegistry,
    components: BTreeMap<String, TreeView>,
    dirty: bool,
    filter: String,
    width: u32,
    limited: bool,
    source_revisions: BTreeMap<kedra_scene::document::DocumentId, u64>,
    edit_epoch: u64,
    inputs: std::collections::BTreeSet<Entity>,
    blocked_follow: bool,
    pending_presentation: Option<(u64, Action)>,
    pending_history: Option<history::Pending>,
}
pub(crate) fn install(app: &mut App) {
    app.register_type::<Option<Handle<kedra_scene::environment::Environment>>>();
    app.add_plugins(kedra_inspector_luau::InspectorLuauPlugin);
    app.add_plugins(kedra_inspector::controls::InspectorControlsPlugin);
    discrete::install(app);
    app.add_observer(
        |_: On<kedra_inspector_luau::DrawerChanged>, mut commands: Commands| {
            commands.queue(|world: &mut World| {
                crate::workspace::refresh_inspector_presentations(world)
            });
        },
    );
    app.add_observer(
        |_: On<kedra_inspector::collections::Changed>, mut commands: Commands| {
            commands.queue(|world: &mut World| {
                crate::workspace::refresh_inspector_presentations(world)
            });
        },
    );
    input::install(app);
    editing::install(app);
    references::install(app);
}

pub(crate) fn templates(world: &World, density: f32) -> Result<FieldTemplates, String> {
    Ok(FieldTemplates {
        row: crate::views::compiled(world, "editor://prefabs/property-row.prefab.ron")?,
        value: crate::views::compiled(world, "editor://prefabs/property-value.prefab.ron")?,
        section: crate::views::compiled(world, "editor://prefabs/property-section.prefab.ron")?,
        vector: crate::views::compiled(world, "editor://prefabs/property-vector.prefab.ron")?,
        command: crate::views::compiled(world, "editor://prefabs/button.prefab.ron")?,
        density,
    })
}

impl Panel {
    pub(crate) fn refresh_presentation(&mut self) {
        self.dirty = true;
    }
    pub fn mount(world: &mut World, parent: Entity, id: ViewId) -> Result<Self, String> {
        let density = world
            .resource::<kedra_desktop::DesktopSurface>()
            .window
            .and_then(|w| world.get::<Window>(w))
            .map_or(1., Window::scale_factor)
            * world.resource::<UiScale>().0;
        let templates = templates(world, density)?;
        let view = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/inspector.prefab.ron",
            &format!("inspector/{id:?}"),
        )?;
        for (key, action) in [
            ("search", Action::Search),
            ("close-search", Action::CloseSearch),
        ] {
            world
                .entity_mut(view.element(key).map_err(|e| e.to_string())?)
                .insert(Binding { view: id, action });
        }
        world.entity_mut(view.root).insert(Binding {
            view: id,
            action: Action::Search,
        });
        world
            .entity_mut(view.root)
            .insert(crate::drag::ContextTarget(id));
        let search = view.element("search").map_err(|e| e.to_string())?;
        if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(search) {
            text.max_characters = Some(256);
            text.allow_newlines = false;
        }
        let session = kedra_ui::text_input::TextSession::attach(
            world
                .get::<bevy::text::EditableText>(search)
                .ok_or("Inspector search input is missing")?,
            256,
        )
        .map_err(|error| format!("Cannot attach Inspector search history: {error:?}"))?;
        world.entity_mut(search).insert(session);
        let area = view.element("entries").map_err(|e| e.to_string())?;
        world
            .entity_mut(area)
            .insert(crate::asset_browser::ScrollRegion(0));
        world
            .entity_mut(view.element("track").map_err(|e| e.to_string())?)
            .insert(bevy::ui_widgets::Scrollbar::new(
                area,
                bevy::ui_widgets::ControlOrientation::Vertical,
                24.,
            ));
        world
            .entity_mut(view.element("thumb").map_err(|e| e.to_string())?)
            .remove::<Node>()
            .insert(bevy::ui_widgets::ScrollbarThumb::default());
        let mut editors = kedra_inspector::builtin::editors()?;
        for ty in [
            std::any::TypeId::of::<Handle<bevy_flair::style::StyleSheet>>(),
            std::any::TypeId::of::<Handle<kedra_scene::environment::Environment>>(),
            std::any::TypeId::of::<Option<Handle<kedra_scene::environment::Environment>>>(),
            std::any::TypeId::of::<Handle<bevy::world_serialization::WorldAsset>>(),
        ] {
            editors.register_type(
                ty,
                std::sync::Arc::new(kedra_inspector::resources::ReferenceEditor),
            )?;
        }
        editors.register_variants(
            std::any::TypeId::of::<kedra_scene::spatial::Primitive>(),
            std::sync::Arc::new(|value| {
                use kedra_scene::spatial::Primitive;
                let current = Primitive::from_reflect(value).ok_or("Native primitive required")?;
                let cuboid = match &current {
                    Primitive::Cuboid { .. } => current.clone(),
                    _ => Primitive::Cuboid { size: Vec3::ONE },
                };
                let sphere = match &current {
                    Primitive::Sphere { .. } => current.clone(),
                    _ => Primitive::Sphere { radius: 0.5 },
                };
                Ok(vec![
                    ("Cuboid".into(), std::sync::Arc::new(cuboid)),
                    ("Sphere".into(), std::sync::Arc::new(sphere)),
                ])
            }),
        )?;
        editors.register_type(
            std::any::TypeId::of::<kedra_ui::authored::components::UiCaption>(),
            std::sync::Arc::new(kedra_inspector_luau::LuauEditor::new(
                "editor.inspector.caption",
                "crates/kedra_editor/src/inspector/caption.luau",
                include_str!("caption.luau"),
            )),
        )?;
        Ok(Self {
            view,
            state: model::State::default(),
            templates,
            editors,
            components: BTreeMap::new(),
            dirty: true,
            filter: String::new(),
            width: 0,
            limited: false,
            source_revisions: BTreeMap::new(),
            edit_epoch: 0,
            inputs: default(),
            blocked_follow: false,
            pending_presentation: None,
            pending_history: None,
        })
    }
    pub fn root(&self) -> Entity {
        self.view.root
    }
    pub(crate) fn discard_binding(&mut self, world: &mut World) {
        self.pending_history = None;
        self.clear(world);
        self.state.target = None;
        self.state.locked = false;
        self.dirty = true;
    }
    fn element(&self, key: &str) -> Result<Entity, String> {
        self.view.element(key).map_err(|e| e.to_string())
    }
    fn text(&self, world: &mut World, key: &str, text: &str) -> Result<(), String> {
        let mut value = world
            .get_mut::<Text>(self.element(key)?)
            .ok_or("Inspector text binding is missing")?;
        if value.0 != text {
            value.0 = text.into();
        }
        Ok(())
    }
    fn clear(&mut self, world: &mut World) {
        kedra_preview::catalog::choices(world, self.root(), &[]);
        for tree in self.components.values() {
            tree.retire(world);
        }
        self.components.clear();
        self.inputs.clear();
        self.pending_presentation = None;
    }
    pub fn owns(&self, disclosure: &Disclosure) -> bool {
        self.components
            .values()
            .any(|tree| tree.owns_disclosure(disclosure))
    }
    pub fn action(&mut self, world: &mut World, action: Action) -> Result<(), String> {
        if self.has_drafts(world) && matches!(action, Action::Raw | Action::Disclosure(_)) {
            if self.native_work_pending(world) {
                // A click can itself queue native focus/selection edits. Wait for
                // their acknowledgment, then recheck every draft and original token.
                self.pending_presentation = Some((self.state.token, action));
                return Ok(());
            }
            self.text(
                world,
                "message",
                "Accept or cancel the property draft before changing its presentation.",
            )?;
            kedra_ui::docking::class(world, self.element("message")?, "hidden", false);
            return Ok(());
        }
        match action {
            Action::Lock => self.state.toggle_lock(),
            Action::Raw => self.state.raw = !self.state.raw,
            Action::Details => self.state.details_open = !self.state.details_open,
            Action::Search => {
                self.state.search_open = true;
                kedra_ui::docking::class(world, self.element("toolbar")?, "hidden", false);
                world.resource_mut::<bevy::input_focus::InputFocus>().set(
                    self.element("search")?,
                    bevy::input_focus::FocusCause::Navigated,
                );
            }
            Action::CloseSearch => {
                self.state.search_open = false;
                let search = self.element("search")?;
                let mut inputs = world.query::<(
                    &mut kedra_ui::text_input::TextSession,
                    &mut bevy::text::EditableText,
                )>();
                let (mut session, mut value) = inputs
                    .get_mut(world, search)
                    .map_err(|error| format!("Inspector search session is unavailable: {error}"))?;
                session
                    .replace(&mut value, "")
                    .map_err(|error| format!("Cannot reset Inspector search: {error:?}"))?;
                self.filter.clear();
                world
                    .resource_mut::<bevy::input_focus::InputFocus>()
                    .set(self.root(), bevy::input_focus::FocusCause::Navigated);
            }
            Action::Disclosure(disclosure) => {
                for tree in self.components.values_mut() {
                    tree.disclose(&disclosure);
                }
            }
        }
        self.dirty = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
